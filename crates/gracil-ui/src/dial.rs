//! The dial: a marble that fills an invisible circle clockwise as the video compresses, with the
//! controls in its middle (ADR 0038). The top of the middle pauses and resumes; the bottom stops.
//!
//! It animates itself: every value eases towards what the app says, and the dial only asks for
//! new frames while something is still moving (spec §51).

use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::time::Instant;

use gracil_core::queue::JobId;
use iced::mouse;
use iced::widget::canvas::{self, Action, Event, Frame, Geometry, LineCap, Path, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, window};

use crate::app::Message;
use crate::style;

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
    /// With the whole list: how many equal parts the circle has, one per file, and how many are
    /// done, each marked by a hollow marble where its part ends.
    pub parts: Option<(usize, usize)>,
    /// Pause is offered only once FFmpeg is compressing.
    pub can_pause: bool,
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
            Part::Stop => true,
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
        let part = if position.y > center.y + DIVIDER * radius {
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

    fn targets(&self, hovered: Option<Part>) -> [f32; 4] {
        [
            self.target_fill(),
            self.target_play(),
            f32::from(hovered == Some(Part::PauseResume)),
            f32::from(hovered == Some(Part::Stop)),
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
                let elapsed = state
                    .last_frame
                    .map_or(0.0, |last| now.duration_since(last).as_secs_f32().min(0.1));
                let [fill, play, top, bottom] = self.targets(hovered);
                state.fill = approach(state.fill, fill, elapsed, FILL_EASE);
                state.play = approach(state.play, play, elapsed, SYMBOL_EASE);
                state.hover_top = approach(state.hover_top, top, elapsed, HOVER_EASE);
                state.hover_bottom = approach(state.hover_bottom, bottom, elapsed, HOVER_EASE);
                state.last_frame = Some(*now);
            }
            _ => {}
        }

        let current = [state.fill, state.play, state.hover_top, state.hover_bottom];
        if current == self.targets(hovered) {
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

        // Each file done leaves a hollow marble where its part ends, once the marble has passed
        // it, so how many are done and how many are left shows at a glance. With parts too short
        // to tell the marks apart, only the circle shows.
        let marble_radius = pipe_width / 2.0 - 1.5 * scale;
        if let Some((parts, done)) = self.parts
            && parts >= 2
            && TAU * pipe / parts as f32 >= 2.0 * marble_radius + 6.0 * scale
        {
            let part = 1.0 / parts as f32;
            for at in (1..=done.min(parts - 1)).map(|index| index as f32 * part) {
                if at > state.fill + 0.001 {
                    break;
                }
                let angle = -FRAC_PI_2 + TAU * at;
                let mark = Point::new(center.x + pipe * angle.cos(), center.y + pipe * angle.sin());
                frame.stroke(
                    &Path::circle(mark, marble_radius - 1.0 * scale),
                    Stroke::default()
                        .with_width(1.5 * scale)
                        .with_color(tokens.thumb),
                );
            }
        }

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
        frame.fill(&Path::circle(marble, marble_radius * 0.55), tokens.accent);

        // The middle: two buttons that light up softly under the pointer.
        let divider_y = center.y + DIVIDER * radius;
        let below = (DIVIDER).asin();
        let top = Path::new(|path| {
            path.arc(canvas::path::Arc {
                center,
                radius,
                start_angle: (PI - below).into(),
                end_angle: (TAU + below).into(),
            });
            path.close();
        });
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
        let icon = Point::new(center.x, center.y - radius * 0.2);
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

        // Stop sits in the middle of the bottom button.
        let stop = radius * 0.17;
        let stop_center = Point::new(center.x, (divider_y + center.y + radius) / 2.0);
        frame.fill(
            &Path::rounded_rectangle(
                Point::new(stop_center.x - stop / 2.0, stop_center.y - stop / 2.0),
                Size::new(stop, stop),
                (stop * 0.15).into(),
            ),
            symbol(state.hover_bottom, true),
        );

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

/// Moves `value` towards `target`, snapping once the difference can't be seen.
fn approach(value: f32, target: f32, elapsed: f32, ease: f32) -> f32 {
    let next = value + (target - value) * (1.0 - (-elapsed / ease).exp());
    if (target - next).abs() < 0.002 {
        target
    } else {
        next
    }
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
    fn values_ease_and_then_settle_exactly() {
        let mut value = 0.0;
        for _ in 0..200 {
            value = approach(value, 1.0, 1.0 / 60.0, FILL_EASE);
        }
        assert_eq!(value, 1.0);
        assert!(approach(0.0, 1.0, 1.0 / 60.0, FILL_EASE) < 0.1);
    }

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
            parts: None,
            fraction: Some(0.5),
            paused,
            can_pause,
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
}
