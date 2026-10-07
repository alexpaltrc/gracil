//! The dial: a marble that fills an invisible circle clockwise as the video compresses, with the
//! controls in its middle (ADR 0038). The top of the middle pauses and resumes; the bottom stops,
//! unless the dial is one that can't stop: then its whole middle pauses and resumes.
//!
//! It animates itself: every value eases towards what the app says, and the dial only asks for
//! new frames while something is still moving (spec §51).

use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::time::Instant;

use gracil_core::queue::JobId;
use iced::alignment::Vertical;
use iced::mouse;
use iced::widget::canvas::{self, Action, Event, Frame, Geometry, LineCap, Path, Stroke};
use iced::widget::text::Alignment;
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, window};

use crate::app::Message;
use crate::motion::{self, approach};
use crate::style::{self, size};

/// Width and height of the dial, in logical pixels. Drawn smaller, everything shrinks with it.
pub const SIZE: f32 = 148.0;
/// The dial in the mini-window.
pub const SMALL: f32 = 76.0;

const PIPE_WIDTH: f32 = 13.0;
/// Space between the pipe and the buttons in its middle.
const GAP: f32 = 7.0;
/// Where the two buttons meet, as a share of the middle's radius below its
/// centre. At 0.404 the bottom button is a quarter of the middle's area.
const DIVIDER: f32 = 0.404;

/// How quickly each value catches up, in seconds (the time to cover about two thirds).
const FILL_EASE: f32 = 0.28;
const SYMBOL_EASE: f32 = 0.05;
const HOVER_EASE: f32 = 0.07;
/// How long the marble takes to breathe in and out while Gracil gets ready, in seconds.
const BREATH: f32 = 1.6;
/// How faint the marble's middle gets at the bottom of a breath.
const BREATH_DEPTH: f32 = 0.45;
const BREATH_EASE: f32 = 0.3;

/// What the ring stands for. When it changes, the ring shows the new one as it is, without
/// sweeping from the old one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shows {
    /// One file.
    File(JobId),
    /// The whole list (ADR 0034, item 13), by the first file of its round of work.
    List(JobId),
}

pub struct Dial {
    pub shows: Shows,
    /// How much is done, when known.
    pub fraction: Option<f64>,
    pub paused: bool,
    /// Pause is offered for every file being worked on, except while a result is put in place.
    pub can_pause: bool,
    /// Whether the bottom of the middle stops. The mini-window's dial only pauses (spec §55).
    pub can_stop: bool,
    /// The word that takes the stop square's place under the pointer, so it says what it does.
    pub stop_word: &'static str,
    /// Gracil is getting ready and nothing has filled yet: the marble breathes softly, so the
    /// wait never looks stuck (ADR 0055).
    pub getting_ready: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    PauseResume,
    Stop,
}

/// What is on screen, which follows the app's values smoothly.
#[derive(Default)]
pub struct State {
    shows: Option<Shows>,
    fill: f32,
    /// 0 shows pause, 1 shows play.
    play: f32,
    hover_top: f32,
    hover_bottom: f32,
    pressed: Option<Part>,
    last_frame: Option<Instant>,
    /// How much the marble breathes, from 0 (steady) to 1, and where in its breath it is.
    breathing: f32,
    breath: f32,
}

impl Dial {
    fn target_fill(&self) -> f32 {
        self.fraction.unwrap_or(0.0).clamp(0.0, 1.0) as f32
    }

    fn target_play(&self) -> f32 {
        if self.paused { 1.0 } else { 0.0 }
    }

    fn enabled(&self, part: Part) -> bool {
        match part {
            Part::PauseResume => self.paused || self.can_pause,
            Part::Stop => self.can_stop,
        }
    }

    /// The enabled button under the cursor, if any.
    fn part_at(&self, bounds: Rectangle, cursor: mouse::Cursor) -> Option<Part> {
        let position = cursor.position_in(bounds)?;
        let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let radius = middle_radius(bounds.size());
        if position.distance(center) > radius {
            return None;
        }
        let part = if self.can_stop && position.y > center.y + DIVIDER * radius {
            Part::Stop
        } else {
            Part::PauseResume
        };
        self.enabled(part).then_some(part)
    }

    fn message(&self, part: Part) -> Message {
        match part {
            Part::Stop => Message::Cancel,
            Part::PauseResume if self.paused => Message::Resume,
            Part::PauseResume => Message::Pause,
        }
    }

    fn targets(&self, hovered: Option<Part>) -> [f32; 5] {
        [
            self.target_fill(),
            self.target_play(),
            f32::from(hovered == Some(Part::PauseResume)),
            f32::from(hovered == Some(Part::Stop)),
            f32::from(self.getting_ready && !self.paused && !motion::still()),
        ]
    }
}

impl canvas::Program<Message> for Dial {
    type State = State;

    fn update(
        &self,
        state: &mut State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        let hovered = self.part_at(bounds, cursor);

        // Another file in focus: show it as it is, without sweeping from the previous one.
        if state.shows != Some(self.shows) {
            state.shows = Some(self.shows);
            state.fill = self.target_fill();
            state.play = self.target_play();
            state.pressed = None;
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(part) = hovered {
                    state.pressed = Some(part);
                    return Some(Action::request_redraw().and_capture());
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let Some(part) = state.pressed.take()
                    && hovered == Some(part)
                {
                    return Some(Action::publish(self.message(part)).and_capture());
                }
            }
            Event::Window(window::Event::RedrawRequested(now)) => {
                let elapsed = motion::elapsed(state.last_frame, *now);
                let [fill, play, top, bottom, breathing] = self.targets(hovered);
                state.fill = approach(state.fill, fill, elapsed, FILL_EASE);
                state.play = approach(state.play, play, elapsed, SYMBOL_EASE);
                state.hover_top = approach(state.hover_top, top, elapsed, HOVER_EASE);
                state.hover_bottom = approach(state.hover_bottom, bottom, elapsed, HOVER_EASE);
                state.breathing = approach(state.breathing, breathing, elapsed, BREATH_EASE);
                state.breath = if state.breathing > 0.0 {
                    (state.breath + elapsed / BREATH).fract()
                } else {
                    0.0
                };
                state.last_frame = Some(*now);
            }
            _ => {}
        }

        let current = [
            state.fill,
            state.play,
            state.hover_top,
            state.hover_bottom,
            state.breathing,
        ];
        // A breath never settles, so the dial keeps drawing while it breathes.
        if current == self.targets(hovered) && state.breathing == 0.0 {
            state.last_frame = None;
            None
        } else {
            Some(Action::request_redraw())
        }
    }

    fn draw(
        &self,
        state: &State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let palette = theme.palette();
        let tokens = style::tokens(theme);
        let mut frame = Frame::new(renderer, bounds.size());
        let center = frame.center();
        let scale = scale(bounds.size());
        let pipe_width = PIPE_WIDTH * scale;
        let (pipe, trail_width, radius) = ring(bounds.size());

        // No pipe is drawn, only what the marble has filled (ADR 0038): from twelve o'clock
        // clockwise, behind the marble, in the accent, like the filled part of Windows' sliders.
        let head = -FRAC_PI_2 + TAU * state.fill;
        if state.fill > 0.001 {
            let trail = Path::new(|path| {
                path.arc(canvas::path::Arc {
                    center,
                    radius: pipe,
                    start_angle: (-FRAC_PI_2).into(),
                    end_angle: head.into(),
                });
            });
            frame.stroke(
                &trail,
                Stroke::default()
                    .with_width(trail_width)
                    .with_line_cap(LineCap::Butt)
                    .with_color(tokens.accent.scale_alpha(1.0 - 0.35 * state.play)),
            );
        }

        let marble_radius = pipe_width / 2.0 - 1.5 * scale;

        // The marble, drawn like the thumb of Windows' own sliders: a solid face with a thin edge
        // and the accent in its middle, so it stands apart from the trail in both themes.
        let marble = Point::new(center.x + pipe * head.cos(), center.y + pipe * head.sin());
        frame.fill(&Path::circle(marble, marble_radius), tokens.thumb);
        frame.stroke(
            &Path::circle(marble, marble_radius),
            Stroke::default()
                .with_width(1.0)
                .with_color(palette.text.scale_alpha(0.12)),
        );
        let breath = 0.5 - 0.5 * (TAU * state.breath).cos();
        frame.fill(
            &Path::circle(marble, marble_radius * 0.55),
            tokens
                .accent
                .scale_alpha(1.0 - BREATH_DEPTH * state.breathing * breath),
        );

        // The middle: two buttons that light up softly under the pointer.
        let divider_y = center.y + DIVIDER * radius;
        let below = (DIVIDER).asin();
        let top = if self.can_stop {
            Path::new(|path| {
                path.arc(canvas::path::Arc {
                    center,
                    radius,
                    start_angle: (PI - below).into(),
                    end_angle: (TAU + below).into(),
                });
                path.close();
            })
        } else {
            Path::circle(center, radius)
        };
        let bottom = Path::new(|path| {
            path.arc(canvas::path::Arc {
                center,
                radius,
                start_angle: below.into(),
                end_angle: (PI - below).into(),
            });
            path.close();
        });
        frame.fill(&top, palette.text.scale_alpha(0.06 * state.hover_top));
        frame.fill(&bottom, palette.text.scale_alpha(0.06 * state.hover_bottom));

        let symbol = |hover: f32, enabled: bool| {
            if enabled {
                mix(tokens.secondary, palette.text, hover)
            } else {
                tokens.secondary.scale_alpha(0.45)
            }
        };

        // Pause turns into play by moving the corners of its two bars into the triangle.
        let size = radius * 0.46;
        let icon = if self.can_stop {
            Point::new(center.x, center.y - radius * 0.2)
        } else {
            center
        };
        let color = symbol(state.hover_top, self.enabled(Part::PauseResume));
        for (pause, play) in pause_play_shapes() {
            let corners: Vec<Point> = pause
                .iter()
                .zip(play.iter())
                .map(|(from, to)| {
                    let (x, y) = (
                        lerp(from.0, to.0, state.play),
                        lerp(from.1, to.1, state.play),
                    );
                    Point::new(icon.x + x * size, icon.y + y * size)
                })
                .collect();
            frame.fill(
                &Path::new(|path| {
                    path.move_to(corners[0]);
                    for corner in &corners[1..] {
                        path.line_to(*corner);
                    }
                    path.close();
                }),
                color,
            );
        }

        if !self.can_stop {
            return vec![frame.into_geometry()];
        }

        // Stop sits in the middle of the bottom button; under the pointer the square gives way to
        // the word, so what it does is never a guess.
        let stop = radius * 0.17;
        let stop_center = Point::new(center.x, (divider_y + center.y + radius) / 2.0);
        frame.fill(
            &Path::rounded_rectangle(
                Point::new(stop_center.x - stop / 2.0, stop_center.y - stop / 2.0),
                Size::new(stop, stop),
                (stop * 0.15).into(),
            ),
            symbol(state.hover_bottom, true).scale_alpha(1.0 - state.hover_bottom),
        );
        frame.fill_text(canvas::Text {
            content: self.stop_word.to_owned(),
            position: stop_center,
            max_width: radius * 1.6,
            color: palette.text.scale_alpha(state.hover_bottom),
            size: size::CAPTION.into(),
            font: style::FONT,
            align_x: Alignment::Center,
            align_y: Vertical::Center,
            ..canvas::Text::default()
        });

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if self.part_at(bounds, cursor).is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

/// Where a dial drawn in `bounds` puts its ring: the ring's radius, the width of its filled part,
/// and the radius of its middle. The finished view draws the same shapes.
pub fn ring(bounds: Size) -> (f32, f32, f32) {
    let scale = scale(bounds);
    let pipe_width = PIPE_WIDTH * scale;
    let pipe = SIZE * scale / 2.0 - 1.0 - pipe_width / 2.0;
    (pipe, pipe_width - 5.0 * scale, middle_radius(bounds))
}

/// The radius of the buttons in the middle of the circle.
fn middle_radius(bounds: Size) -> f32 {
    let scale = scale(bounds);
    SIZE * scale / 2.0 - 1.0 - (PIPE_WIDTH + GAP) * scale
}

/// How much smaller than [SIZE] the dial is drawn, to fit its space.
fn scale(bounds: Size) -> f32 {
    SIZE.min(bounds.width).min(bounds.height) / SIZE
}

/// Four corners, as x and y.
type Quad = [(f32, f32); 4];

/// Pause's two bars and play's triangle split down its middle, each as four corners in the same
/// order (top left, top right, bottom right, bottom left), in a box one unit tall.
fn pause_play_shapes() -> [(Quad, Quad); 2] {
    const BAR: f32 = 0.27;
    const GAP: f32 = 0.12;
    // The triangle sits a little right of centre, where it looks centred.
    const NUDGE: f32 = 0.07;
    let left = -0.42 + NUDGE;
    let middle = 0.0 + NUDGE;
    let tip = 0.45 + NUDGE;
    [
        (
            [
                (-GAP - BAR, -0.5),
                (-GAP, -0.5),
                (-GAP, 0.5),
                (-GAP - BAR, 0.5),
            ],
            [(left, -0.5), (middle, -0.25), (middle, 0.25), (left, 0.5)],
        ),
        (
            [(GAP, -0.5), (GAP + BAR, -0.5), (GAP + BAR, 0.5), (GAP, 0.5)],
            [(middle, -0.25), (tip, 0.0), (tip, 0.0), (middle, 0.25)],
        ),
    ]
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

fn mix(from: Color, to: Color, t: f32) -> Color {
    Color {
        r: lerp(from.r, to.r, t),
        g: lerp(from.g, to.g, t),
        b: lerp(from.b, to.b, t),
        a: lerp(from.a, to.a, t),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stop_button_is_about_a_quarter_of_the_middle() {
        // Area of the circular segment below the divider, as a share of the whole disc.
        let half_angle = DIVIDER.acos();
        let share = (2.0 * half_angle - (2.0 * half_angle).sin()) / (2.0 * PI);
        assert!((share - 0.25).abs() < 0.01, "{share}");
    }

    fn dial(paused: bool, can_pause: bool) -> Dial {
        Dial {
            shows: Shows::File({
                let mut queue = gracil_core::queue::Queue::default();
                queue.add([std::path::PathBuf::from("a.mp4")]);
                queue.jobs()[0].id
            }),
            fraction: Some(0.5),
            paused,
            can_pause,
            can_stop: true,
            stop_word: "Stop all",
            getting_ready: false,
        }
    }

    #[test]
    fn the_top_pauses_resumes_and_the_bottom_stops() {
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(SIZE, SIZE));
        let at = |x: f32, y: f32| mouse::Cursor::Available(Point::new(x, y));
        let middle = SIZE / 2.0;
        let radius = middle_radius(bounds.size());

        let compressing = dial(false, true);
        let top = compressing.part_at(bounds, at(middle, middle - radius * 0.5));
        assert_eq!(top, Some(Part::PauseResume));
        assert!(matches!(
            compressing.message(Part::PauseResume),
            Message::Pause
        ));
        assert!(matches!(
            dial(true, false).message(Part::PauseResume),
            Message::Resume
        ));

        let bottom = compressing.part_at(bounds, at(middle, middle + radius * 0.8));
        assert_eq!(bottom, Some(Part::Stop));
        assert_eq!(compressing.part_at(bounds, at(2.0, 2.0)), None);

        // While getting ready only Stop answers.
        let preparing = dial(false, false);
        assert_eq!(
            preparing.part_at(bounds, at(middle, middle - radius * 0.5)),
            None
        );
        assert_eq!(
            preparing.part_at(bounds, at(middle, middle + radius * 0.8)),
            Some(Part::Stop)
        );
    }

    #[test]
    fn a_dial_that_cannot_stop_pauses_from_its_whole_middle() {
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(SIZE, SIZE));
        let at = |x: f32, y: f32| mouse::Cursor::Available(Point::new(x, y));
        let middle = SIZE / 2.0;
        let radius = middle_radius(bounds.size());
        let mini = Dial {
            can_stop: false,
            stop_word: "Stop all",
            ..dial(false, true)
        };
        assert_eq!(
            mini.part_at(bounds, at(middle, middle + radius * 0.8)),
            Some(Part::PauseResume)
        );
    }
}
