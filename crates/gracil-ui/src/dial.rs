//! The ring: one drawing that stays through everything Gracil does to a file (ADR 0076). A marble
//! fills an invisible circle clockwise as the file compresses, with pause and stop in its middle
//! (ADR 0038); once the file is done the full ring holds how much smaller it came out; and once
//! the whole list is done its middle turns green and a check mark draws itself (ADR 0039).
//!
//! It animates itself: every value moves towards what the app says, one face gives way to the
//! next in turn, and the ring only asks for new frames while something is still moving
//! (spec §51).

use std::cell::RefCell;
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::time::{Duration, Instant};

use gracil_core::queue::JobId;
use iced::alignment::Vertical;
use iced::mouse;
use iced::theme::Palette;
use iced::widget::Canvas;
use iced::widget::canvas::{self, Action, Event, Frame, Geometry, LineCap, LineJoin, Path, Stroke};
use iced::widget::text::Alignment;
use iced::{Color, Element, Point, Rectangle, Renderer, Size, Theme, window};

use crate::app::Message;
use crate::focus::{Part, Scope, keyed_parts};
use crate::motion::{self, Spring, approach};
use crate::style::{self, size};

/// Width and height of the ring, in logical pixels. Drawn smaller, everything shrinks with it.
pub const SIZE: f32 = 148.0;
/// The ring beside the words: in the mini-window, and in the window while the list is open.
pub const SMALL: f32 = 76.0;

const PIPE_WIDTH: f32 = 13.0;
/// Space between the pipe and the buttons in its middle.
const GAP: f32 = 7.0;
/// Where the two buttons meet, as a share of the middle's radius below its
/// centre. At 0.404 the bottom button is a quarter of the middle's area.
const DIVIDER: f32 = 0.404;

/// How long the marble takes to breathe in and out while Gracil gets ready, in seconds.
const BREATH: f32 = 1.6;
/// How faint the marble's middle gets at the bottom of a breath.
const BREATH_DEPTH: f32 = 0.45;

/// How quickly the ring closes once the work is done, in seconds (see [`motion::approach`]).
const CLOSING: f32 = 0.11;
/// How full a ring that is done is when it closes at once.
const ALMOST_CLOSED: f32 = 0.98;
/// How long the check mark takes to draw itself, in seconds.
const CHECK_DRAWN_IN: f32 = 0.32;
/// How faint one face must be before the next comes in, so they never show through each other,
/// and how quickly a face leaves, in seconds (see [`motion::approach`]): quicker than it comes.
const NEARLY_GONE: f32 = 0.03;
const LEAVING: f32 = 0.035;
/// How much wider the ring swells for a moment as the check mark lands.
const SWELL: f32 = 0.28;
/// The amount saved, in the middle of a full-size ring.
const SAVED_SIZE: f32 = 27.0;

/// The check mark, as three points in a box one unit wide around the middle of the ring.
const CHECK: [(f32, f32); 3] = [(-0.36, 0.02), (-0.1, 0.28), (0.38, -0.24)];

/// What the ring stands for. A ring at work that turns to another file or list shows it as it
/// is, without sweeping from the old one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Shows {
    /// One file.
    File(JobId),
    /// The whole list (ADR 0034, item 13), by the first file of its round of work.
    List(JobId),
    /// The whole list, done.
    Finished,
}

/// What is in the ring's middle.
#[derive(Debug, Clone, PartialEq)]
pub enum Face {
    /// Work in progress: pause and stop.
    Working(Controls),
    /// A file is done: how much smaller it came out, in a few characters (`−35%`).
    Saved(String),
    /// The whole list is done: green and a check mark when it went well. When files failed and
    /// none came out smaller, or files were left unfinished, the ring stays quiet without them,
    /// so it never looks like a success it wasn't.
    Done { went_well: bool },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Controls {
    pub paused: bool,
    /// Pause is offered for every file being worked on, except while a result is put in place.
    pub can_pause: bool,
    /// Whether the bottom of the middle stops. The mini-window's ring only pauses (spec §55).
    pub can_stop: bool,
    /// The word that takes the stop square's place under the pointer, so it says what it does.
    pub stop_word: &'static str,
    /// Gracil is getting ready and nothing has filled yet: the marble breathes softly, so the
    /// wait never looks stuck (ADR 0055).
    pub getting_ready: bool,
    /// Gracil rests while the computer cools down, wakes up or runs low on battery: the marble
    /// doesn't breathe, since nothing is happening (ADR 0066).
    pub resting: bool,
}

pub struct Dial {
    pub shows: Shows,
    /// How much is done, when known. A ring that isn't at work is full.
    pub fraction: Option<f64>,
    pub face: Face,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Button {
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
    pressed: Option<Button>,
    last_frame: Option<Instant>,
    /// How much the marble breathes, from 0 (steady) to 1, and where in its breath it is.
    breathing: f32,
    breath: f32,
    /// How much of the marble and the buttons shows, from 0 to 1.
    controls: f32,
    /// How much of the amount saved shows, and the amount itself, kept while it fades away.
    saved: f32,
    saved_words: String,
    /// The green middle, which lands a little past its size and settles.
    disc: Spring,
    /// How much of the check mark is drawn.
    check: f32,
    /// How far the ring has gone quiet: a list that didn't go well.
    quiet: f32,
    /// The ring swelling for a moment as the check mark lands.
    swell: Spring,
    /// Nothing is moving: the ring is drawn once into `still`, and drawn from it again while the
    /// window draws for other reasons, until something changes (ADR 0066).
    settled: bool,
    still: canvas::Cache,
    /// What `still` was drawn for.
    still_for: RefCell<Option<Still>>,
}

/// Everything a settled ring's picture depends on.
#[derive(Debug, Clone, PartialEq)]
struct Still {
    size: Size,
    palette: Palette,
    face: Face,
    values: [f32; 4],
}

/// Where everything in the ring is heading.
struct Targets {
    fill: f32,
    play: f32,
    hover_top: f32,
    hover_bottom: f32,
    breathing: f32,
    controls: f32,
    saved: f32,
    disc: f32,
    quiet: f32,
}

impl Dial {
    fn controls(&self) -> Option<&Controls> {
        match &self.face {
            Face::Working(controls) => Some(controls),
            _ => None,
        }
    }

    fn enabled(&self, part: Button) -> bool {
        self.controls().is_some_and(|controls| match part {
            Button::PauseResume => controls.paused || controls.can_pause,
            Button::Stop => controls.can_stop,
        })
    }

    /// The enabled button under the cursor, if any.
    fn part_at(&self, bounds: Rectangle, cursor: mouse::Cursor) -> Option<Button> {
        let controls = self.controls()?;
        let position = cursor.position_in(bounds)?;
        let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let radius = middle_radius(bounds.size());
        if position.distance(center) > radius {
            return None;
        }
        let part = if controls.can_stop && position.y > center.y + DIVIDER * radius {
            Button::Stop
        } else {
            Button::PauseResume
        };
        self.enabled(part).then_some(part)
    }

    fn message(&self, part: Button) -> Message {
        match part {
            Button::Stop => Message::Cancel,
            Button::PauseResume if self.controls().is_some_and(|controls| controls.paused) => {
                Message::Resume
            }
            Button::PauseResume => Message::Pause,
        }
    }

    fn targets(&self, hovered: Option<Button>) -> Targets {
        let working = self.controls();
        Targets {
            fill: match working {
                Some(_) => self.fraction.unwrap_or(0.0).clamp(0.0, 1.0) as f32,
                None => 1.0,
            },
            play: f32::from(working.is_some_and(|controls| controls.paused)),
            hover_top: f32::from(hovered == Some(Button::PauseResume)),
            hover_bottom: f32::from(hovered == Some(Button::Stop)),
            breathing: f32::from(working.is_some_and(|controls| {
                controls.getting_ready && !controls.paused && !controls.resting && !motion::still()
            })),
            controls: f32::from(working.is_some()),
            saved: f32::from(matches!(self.face, Face::Saved(_))),
            disc: f32::from(self.face == Face::Done { went_well: true }),
            quiet: f32::from(self.face == Face::Done { went_well: false }),
        }
    }
}

impl State {
    /// A ring seen for the first time is shown as it is: nothing plays for what happened while
    /// it was out of sight.
    fn arrive(&mut self, dial: &Dial, targets: &Targets) {
        self.fill = targets.fill;
        self.play = targets.play;
        self.controls = targets.controls;
        self.saved = targets.saved;
        self.disc = Spring::at(targets.disc);
        self.check = targets.disc;
        self.quiet = targets.quiet;
        if let Face::Saved(words) = &dial.face {
            self.saved_words.clone_from(words);
        }
    }

    /// Moves everything towards `targets` for `elapsed` seconds.
    fn advance(&mut self, dial: &Dial, targets: &Targets, elapsed: f32) {
        let working = dial.controls().is_some();
        let fill_ease = if working { motion::STEADY } else { CLOSING };
        self.fill = approach(self.fill, targets.fill, elapsed, fill_ease);
        // A ring that is done closes cleanly, rather than creeping over its last hair's breadth.
        if !working && self.fill >= ALMOST_CLOSED {
            self.fill = 1.0;
        }
        self.play = approach(self.play, targets.play, elapsed, motion::QUICK);
        self.hover_top = approach(self.hover_top, targets.hover_top, elapsed, motion::QUICK);
        self.hover_bottom = approach(
            self.hover_bottom,
            targets.hover_bottom,
            elapsed,
            motion::QUICK,
        );
        self.breathing = approach(self.breathing, targets.breathing, elapsed, motion::STEADY);
        self.breath = if self.breathing > 0.0 {
            (self.breath + elapsed / BREATH).fract()
        } else {
            0.0
        };
        self.quiet = approach(self.quiet, targets.quiet, elapsed, motion::STEADY);

        // One face leaves before the next comes in.
        let pace = |target: f32| {
            if target == 0.0 {
                LEAVING
            } else {
                motion::QUICK
            }
        };
        let others_gone = |others: &[f32]| others.iter().all(|&other| other <= NEARLY_GONE);
        let controls = if others_gone(&[self.saved, self.disc.value]) {
            targets.controls
        } else {
            0.0
        };
        self.controls = approach(self.controls, controls, elapsed, pace(controls));
        let saved = if others_gone(&[self.controls, self.disc.value]) {
            targets.saved
        } else {
            0.0
        };
        self.saved = approach(self.saved, saved, elapsed, pace(saved));
        if let Face::Saved(words) = &dial.face
            && (self.saved <= NEARLY_GONE || targets.saved == 1.0)
        {
            self.saved_words.clone_from(words);
        }

        // The green lands once the ring has closed, the check mark draws itself as it does, and
        // the ring swells for a moment under it.
        let closed = self.fill == 1.0;
        if targets.disc == 1.0 {
            if closed && others_gone(&[self.controls, self.saved]) {
                let before = self.disc.value;
                self.disc.step(1.0, elapsed, motion::LIVELY);
                if before < 0.5 && self.disc.value >= 0.5 {
                    self.swell.kick(9.0);
                }
            }
            if self.disc.value >= 0.5 && !motion::still() {
                self.check = (self.check + elapsed / CHECK_DRAWN_IN).min(1.0);
            } else if motion::still() {
                self.check = 1.0;
            }
        } else {
            self.disc = Spring::at(approach(self.disc.value, 0.0, elapsed, motion::QUICK));
            self.check = approach(self.check, 0.0, elapsed, motion::QUICK);
        }
        self.swell.step(0.0, elapsed, motion::LIVELY);
    }

    fn rests_at(&self, targets: &Targets) -> bool {
        self.fill == targets.fill
            && self.play == targets.play
            && self.hover_top == targets.hover_top
            && self.hover_bottom == targets.hover_bottom
            && self.breathing == 0.0
            && targets.breathing == 0.0
            && self.controls == targets.controls
            && self.saved == targets.saved
            && self.disc.rests_at(targets.disc)
            && self.check == targets.disc
            && self.quiet == targets.quiet
            && self.swell.rests_at(0.0)
    }

    /// Whether only what follows the work is moving: the ring filling, the marble breathing.
    fn only_follows_the_work(&self, targets: &Targets) -> bool {
        targets.controls == 1.0
            && self.controls == 1.0
            && self.play == targets.play
            && self.hover_top == targets.hover_top
            && self.hover_bottom == targets.hover_bottom
            && self.pressed.is_none()
    }
}

/// How often the ring draws while only what follows the work moves (the ring filling, the
/// marble breathing): about thirty times a second, plenty for movements that slow, and half the
/// drawing of every frame (spec §51, ADR 0066). Everything else moves every frame.
const STEADY_FRAME: Duration = Duration::from_millis(33);

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
        let targets = self.targets(hovered);

        match state.shows {
            None => state.arrive(self, &targets),
            // Another file at work: shown as it is, without sweeping from the previous one.
            Some(shows) if shows != self.shows && self.controls().is_some() => {
                state.fill = targets.fill;
                state.play = targets.play;
                state.pressed = None;
            }
            Some(_) => {}
        }
        state.shows = Some(self.shows);

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
                state.advance(self, &targets, elapsed);
                state.last_frame = Some(*now);
            }
            _ => {}
        }

        let resting = state.rests_at(&targets);
        state.settled = resting && state.pressed.is_none();
        if resting {
            state.last_frame = None;
            return None;
        }
        if state.only_follows_the_work(&targets) {
            let last = state.last_frame.unwrap_or_else(Instant::now);
            Some(Action::request_redraw_at(last + STEADY_FRAME))
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
        if !state.settled {
            let mut frame = Frame::new(renderer, bounds.size());
            self.paint(state, &mut frame, theme);
            return vec![frame.into_geometry()];
        }
        let still = Still {
            size: bounds.size(),
            palette: theme.palette(),
            face: self.face.clone(),
            values: [state.fill, state.play, state.hover_top, state.hover_bottom],
        };
        let mut still_for = state.still_for.borrow_mut();
        if still_for.as_ref() != Some(&still) {
            state.still.clear();
            *still_for = Some(still);
        }
        vec![state.still.draw(renderer, bounds.size(), |frame| {
            self.paint(state, frame, theme)
        })]
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

impl Dial {
    /// Draws the ring as `state` has it into `frame`.
    fn paint(&self, state: &State, frame: &mut Frame, theme: &Theme) {
        let palette = theme.palette();
        let tokens = style::tokens(theme);
        let bounds = Rectangle::with_size(frame.size());
        let center = frame.center();
        let scale = scale(bounds.size());
        let pipe_width = PIPE_WIDTH * scale;
        let (pipe, trail_width, radius) = ring(bounds.size());

        // No pipe is drawn, only what the marble has filled (ADR 0038): from twelve o'clock
        // clockwise, behind the marble, in the accent, like the filled part of Windows' sliders.
        let head = -FRAC_PI_2 + TAU * state.fill;
        let trail = Stroke::default()
            .with_width(trail_width * (1.0 + SWELL * state.swell.value))
            .with_line_cap(LineCap::Butt)
            .with_color(mix(
                tokens.accent.scale_alpha(1.0 - 0.35 * state.play),
                tokens.secondary.scale_alpha(0.5),
                state.quiet,
            ));
        if state.fill >= 0.999 {
            frame.stroke(&Path::circle(center, pipe), trail);
        } else if state.fill > 0.001 {
            let filled = Path::new(|path| {
                path.arc(canvas::path::Arc {
                    center,
                    radius: pipe,
                    start_angle: (-FRAC_PI_2).into(),
                    end_angle: head.into(),
                });
            });
            frame.stroke(&filled, trail);
        }

        // The marble, drawn like the thumb of Windows' own sliders: a solid face with a thin edge
        // and the accent in its middle, so it stands apart from the trail in both themes. It
        // leaves with the buttons: a ring that is done is whole.
        let shown = state.controls;
        if shown > 0.0 {
            let marble_radius = pipe_width / 2.0 - 1.5 * scale;
            let marble = Point::new(center.x + pipe * head.cos(), center.y + pipe * head.sin());
            frame.fill(
                &Path::circle(marble, marble_radius),
                tokens.thumb.scale_alpha(shown),
            );
            frame.stroke(
                &Path::circle(marble, marble_radius),
                Stroke::default()
                    .with_width(1.0)
                    .with_color(palette.text.scale_alpha(0.12 * shown)),
            );
            let breath = 0.5 - 0.5 * (TAU * state.breath).cos();
            frame.fill(
                &Path::circle(marble, marble_radius * 0.55),
                tokens
                    .accent
                    .scale_alpha(shown * (1.0 - BREATH_DEPTH * state.breathing * breath)),
            );
            self.paint_buttons(state, frame, theme, center, radius);
        }

        // How much smaller the file came out, rising a little as it appears.
        if state.saved > 0.0 {
            frame.fill_text(canvas::Text {
                content: state.saved_words.clone(),
                position: Point::new(center.x, center.y + 6.0 * scale * (1.0 - state.saved)),
                max_width: radius * 2.0,
                color: palette.text.scale_alpha(state.saved),
                size: (SAVED_SIZE * scale).max(size::BODY).into(),
                font: style::STRONG,
                align_x: Alignment::Center,
                align_y: Vertical::Center,
                ..canvas::Text::default()
            });
        }

        // The middle fills with green, growing out from its centre, and the check mark draws
        // itself from its short stroke to its long one.
        let disc = state.disc.value;
        if disc > 0.0 {
            frame.fill(
                &Path::circle(center, radius * (0.6 + 0.4 * disc)),
                palette.success.scale_alpha(disc.min(1.0)),
            );
        }
        if state.check > 0.0 {
            let points =
                CHECK.map(|(x, y)| Point::new(center.x + x * radius, center.y + y * radius));
            let first = points[0].distance(points[1]);
            let second = points[1].distance(points[2]);
            let drawn = ease_out(state.check) * (first + second);
            let path = Path::new(|path| {
                path.move_to(points[0]);
                if drawn <= first {
                    path.line_to(towards(points[0], points[1], drawn / first));
                } else {
                    path.line_to(points[1]);
                    path.line_to(towards(points[1], points[2], (drawn - first) / second));
                }
            });
            frame.stroke(
                &path,
                Stroke::default()
                    .with_width(radius * 0.13)
                    .with_line_cap(LineCap::Round)
                    .with_line_join(LineJoin::Round)
                    .with_color(palette.background),
            );
        }
    }

    /// The middle while at work: two buttons that light up softly under the pointer.
    fn paint_buttons(
        &self,
        state: &State,
        frame: &mut Frame,
        theme: &Theme,
        center: Point,
        radius: f32,
    ) {
        let Some(controls) = self.controls() else {
            return;
        };
        let palette = theme.palette();
        let tokens = style::tokens(theme);
        let shown = state.controls;
        let divider_y = center.y + DIVIDER * radius;
        let below = (DIVIDER).asin();
        let top = if controls.can_stop {
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
        // A pressed button is lit a little more, so the press is felt before it does anything.
        let lit = |hover: f32, part: Button| {
            let pressed = f32::from(state.pressed == Some(part));
            palette
                .text
                .scale_alpha((0.06 + 0.04 * pressed) * hover * shown)
        };
        frame.fill(&top, lit(state.hover_top, Button::PauseResume));
        frame.fill(&bottom, lit(state.hover_bottom, Button::Stop));

        let symbol = |hover: f32, enabled: bool| {
            if enabled {
                mix(tokens.secondary, palette.text, hover).scale_alpha(shown)
            } else {
                tokens.secondary.scale_alpha(0.45 * shown)
            }
        };

        // Pause turns into play by moving the corners of its two bars into the triangle.
        let size = radius * 0.46;
        let icon = if controls.can_stop {
            Point::new(center.x, center.y - radius * 0.2)
        } else {
            center
        };
        let color = symbol(state.hover_top, self.enabled(Button::PauseResume));
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

        if !controls.can_stop {
            return;
        }

        // Stop sits in the middle of the bottom button; under the pointer the square gives way to
        // the word, so what it does is never a guess. A small ring has no room for the word, and
        // keeps the square.
        let worded = if radius * 1.6 >= WORD_ROOM {
            state.hover_bottom
        } else {
            0.0
        };
        let stop = radius * 0.17;
        let stop_center = Point::new(center.x, (divider_y + center.y + radius) / 2.0);
        frame.fill(
            &Path::rounded_rectangle(
                Point::new(stop_center.x - stop / 2.0, stop_center.y - stop / 2.0),
                Size::new(stop, stop),
                (stop * 0.15).into(),
            ),
            symbol(state.hover_bottom, true).scale_alpha(shown * (1.0 - worded)),
        );
        frame.fill_text(canvas::Text {
            content: controls.stop_word.to_owned(),
            position: stop_center,
            max_width: radius * 1.6,
            color: palette.text.scale_alpha(shown * worded),
            size: size::CAPTION.into(),
            font: style::FONT,
            align_x: Alignment::Center,
            align_y: Vertical::Center,
            ..canvas::Text::default()
        });
    }

    /// The ring, its Pause and Stop each reachable with Tab and pressed with Enter or Space
    /// (ADR 0059), drawn `side` pixels wide.
    pub fn keyed(self, scope: Scope, side: f32) -> Element<'static, Message> {
        let mut parts = Vec::new();
        if let Some(controls) = self.controls() {
            parts.push(Part {
                place: if controls.can_stop {
                    pause_place
                } else {
                    middle_place
                },
                radius: if controls.can_stop {
                    pause_radius
                } else {
                    middle_radius
                },
                message: self
                    .enabled(Button::PauseResume)
                    .then(|| self.message(Button::PauseResume)),
            });
            if controls.can_stop {
                parts.push(Part {
                    place: stop_place,
                    radius: stop_radius,
                    message: Some(Message::Cancel),
                });
            }
        }
        keyed_parts(scope, Canvas::new(self).width(side).height(side), parts)
    }
}

/// The least room, in logical pixels, in which the stop word reads well in place of its square.
const WORD_ROOM: f32 = 64.0;

/// The middle of the ring, around its centre.
fn middle_place(bounds: Size) -> Rectangle {
    let radius = middle_radius(bounds);
    Rectangle::new(
        Point::new(bounds.width / 2.0 - radius, bounds.height / 2.0 - radius),
        Size::new(2.0 * radius, 2.0 * radius),
    )
}

/// The top of the middle, which pauses and resumes, down to where Stop begins.
fn pause_place(bounds: Size) -> Rectangle {
    let middle = middle_place(bounds);
    let radius = middle_radius(bounds);
    Rectangle {
        height: radius * (1.0 + DIVIDER),
        ..middle
    }
}

fn pause_radius(bounds: Size) -> f32 {
    middle_radius(bounds) * 0.6
}

/// The bottom of the middle, which stops.
fn stop_place(bounds: Size) -> Rectangle {
    let radius = middle_radius(bounds);
    let top = bounds.height / 2.0 + DIVIDER * radius;
    let height = radius * (1.0 - DIVIDER);
    Rectangle::new(
        Point::new(bounds.width / 2.0 - radius * 0.85, top),
        Size::new(radius * 1.7, height),
    )
}

fn stop_radius(bounds: Size) -> f32 {
    stop_place(bounds).height / 2.0
}

/// Where a ring drawn in `bounds` is: its radius, the width of its filled part, and the radius of
/// its middle.
fn ring(bounds: Size) -> (f32, f32, f32) {
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

/// How much smaller than [SIZE] the ring is drawn, to fit its space.
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

/// Fast at first, settling softly.
fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

fn towards(from: Point, to: Point, t: f32) -> Point {
    Point::new(lerp(from.x, to.x, t), lerp(from.y, to.y, t))
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

    fn some_file() -> Shows {
        let mut queue = gracil_core::queue::Queue::default();
        queue.add([std::path::PathBuf::from("a.mp4")]);
        Shows::File(queue.jobs()[0].id)
    }

    fn dial(paused: bool, can_pause: bool) -> Dial {
        Dial {
            shows: some_file(),
            fraction: Some(0.5),
            face: Face::Working(Controls {
                paused,
                can_pause,
                can_stop: true,
                stop_word: "Stop all",
                getting_ready: false,
                resting: false,
            }),
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
        assert_eq!(top, Some(Button::PauseResume));
        assert!(matches!(
            compressing.message(Button::PauseResume),
            Message::Pause
        ));
        assert!(matches!(
            dial(true, false).message(Button::PauseResume),
            Message::Resume
        ));

        let bottom = compressing.part_at(bounds, at(middle, middle + radius * 0.8));
        assert_eq!(bottom, Some(Button::Stop));
        assert_eq!(compressing.part_at(bounds, at(2.0, 2.0)), None);

        // While getting ready only Stop answers.
        let preparing = dial(false, false);
        assert_eq!(
            preparing.part_at(bounds, at(middle, middle - radius * 0.5)),
            None
        );
        assert_eq!(
            preparing.part_at(bounds, at(middle, middle + radius * 0.8)),
            Some(Button::Stop)
        );
    }

    #[test]
    fn a_dial_that_cannot_stop_pauses_from_its_whole_middle() {
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(SIZE, SIZE));
        let at = |x: f32, y: f32| mouse::Cursor::Available(Point::new(x, y));
        let middle = SIZE / 2.0;
        let radius = middle_radius(bounds.size());
        let mut mini = dial(false, true);
        if let Face::Working(controls) = &mut mini.face {
            controls.can_stop = false;
        }
        assert_eq!(
            mini.part_at(bounds, at(middle, middle + radius * 0.8)),
            Some(Button::PauseResume)
        );
    }

    #[test]
    fn a_ring_that_is_done_has_nothing_to_press() {
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(SIZE, SIZE));
        let middle = mouse::Cursor::Available(Point::new(SIZE / 2.0, SIZE / 2.0));
        for face in [Face::Saved("−35%".into()), Face::Done { went_well: true }] {
            let done = Dial {
                shows: some_file(),
                fraction: None,
                face,
            };
            assert_eq!(done.part_at(bounds, middle), None);
            assert_eq!(done.targets(None).fill, 1.0);
        }
    }

    /// Plays `dial` from `state` for `seconds`, frame by frame, and gives every frame's state to
    /// `watch`.
    fn play(dial: &Dial, state: &mut State, seconds: f32, mut watch: impl FnMut(&State)) {
        let targets = dial.targets(None);
        for _ in 0..(seconds * 60.0) as usize {
            state.advance(dial, &targets, 1.0 / 60.0);
            watch(state);
        }
    }

    #[test]
    fn the_faces_take_turns_and_everything_comes_to_rest() {
        let working = dial(false, true);
        let mut state = State::default();
        state.arrive(&working, &working.targets(None));
        assert_eq!((state.controls, state.fill), (1.0, 0.5));

        // The file is done: the buttons leave before the amount comes in, and the ring closes.
        let saved = Dial {
            shows: working.shows,
            fraction: None,
            face: Face::Saved("−35%".into()),
        };
        play(&saved, &mut state, 2.0, |state| {
            assert!(state.controls <= NEARLY_GONE || state.saved == 0.0);
        });
        assert!(state.rests_at(&saved.targets(None)));
        assert_eq!((state.saved, state.fill), (1.0, 1.0));

        // The list is done: the amount leaves, the green lands a little past its size and the
        // check mark draws itself.
        let done = Dial {
            shows: Shows::Finished,
            fraction: None,
            face: Face::Done { went_well: true },
        };
        let mut furthest: f32 = 0.0;
        play(&done, &mut state, 3.0, |state| {
            assert!(state.saved <= NEARLY_GONE || state.disc.value == 0.0);
            furthest = furthest.max(state.disc.value);
        });
        assert!(furthest > 1.0, "{furthest}");
        assert!(state.rests_at(&done.targets(None)));
        assert_eq!(state.check, 1.0);
    }

    #[test]
    fn a_ring_first_seen_done_shows_it_without_playing() {
        let done = Dial {
            shows: Shows::Finished,
            fraction: None,
            face: Face::Done { went_well: true },
        };
        let mut state = State::default();
        let targets = done.targets(None);
        state.arrive(&done, &targets);
        assert!(state.rests_at(&targets));
    }
}
