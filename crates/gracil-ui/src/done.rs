//! The finished view (spec §77.3, ADR 0039): once the whole list is done, the ring the marble
//! filled stays, its middle fills with green and a check mark draws itself, and how much room was
//! saved rises gently into place just below.
//!
//! It plays once when it appears and then asks for no more frames (spec §51).

use std::time::{Duration, Instant};

use iced::alignment::Vertical;
use iced::mouse;
use iced::widget::canvas::{self, Action, Event, Frame, Geometry, LineCap, LineJoin, Path, Stroke};
use iced::widget::text::Alignment;
use iced::{Point, Rectangle, Renderer, Size, Theme, window};

use crate::app::Message;
use crate::dial;
use crate::style::{self, size};

/// Room below the ring for the amount saved.
pub const AMOUNT_ROOM: f32 = 44.0;
/// The amount saved is a little larger than a title.
const AMOUNT_SIZE: f32 = size::TITLE + 4.0;
/// How long the whole entrance takes.
const ENTRANCE: Duration = Duration::from_millis(1100);

/// The check mark, as three points in a box one unit wide around the middle of the ring.
const CHECK: [(f32, f32); 3] = [(-0.36, 0.02), (-0.1, 0.28), (0.38, -0.24)];

pub struct Done {
    /// What the big line says: how much room was saved, or why nothing was.
    pub headline: String,
    /// Whether the list went well: the green middle and the check mark. When files failed and
    /// none came out smaller, the ring stays without them.
    pub went_well: bool,
}

#[derive(Default)]
pub struct State {
    started: Option<Instant>,
    progress: f32,
}

impl canvas::Program<Message> for Done {
    type State = State;

    fn update(
        &self,
        state: &mut State,
        event: &Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let started = *state.started.get_or_insert(*now);
            state.progress = (now.saturating_duration_since(started).as_secs_f32()
                / ENTRANCE.as_secs_f32())
            .min(1.0);
        }
        (state.progress < 1.0).then(Action::request_redraw)
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
        let ring_bounds = Size::new(dial::SIZE, dial::SIZE);
        let (ring, ring_width, middle) = dial::ring(ring_bounds);
        let center = Point::new(bounds.width / 2.0, dial::SIZE / 2.0);
        let [disc, check, amount] = stages(state.progress);
        let (disc, check) = if self.went_well {
            (disc, check)
        } else {
            (0.0, 0.0)
        };

        // The ring the marble filled, whole.
        frame.stroke(
            &Path::circle(center, ring),
            Stroke::default()
                .with_width(ring_width)
                .with_color(tokens.accent),
        );

        // The middle fills with green, growing out from its centre.
        if disc > 0.0 {
            frame.fill(
                &Path::circle(center, middle * (0.6 + 0.4 * disc)),
                palette.success.scale_alpha(disc),
            );
        }

        // The check mark draws itself from its short stroke to its long one.
        if check > 0.0 {
            let points =
                CHECK.map(|(x, y)| Point::new(center.x + x * middle, center.y + y * middle));
            let first = points[0].distance(points[1]);
            let second = points[1].distance(points[2]);
            let drawn = check * (first + second);
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
                    .with_width(middle * 0.13)
                    .with_line_cap(LineCap::Round)
                    .with_line_join(LineJoin::Round)
                    .with_color(palette.background),
            );
        }

        // How much was saved rises a little as it appears.
        frame.fill_text(canvas::Text {
            content: self.headline.clone(),
            position: Point::new(
                center.x,
                dial::SIZE + AMOUNT_ROOM / 2.0 + 8.0 * (1.0 - amount),
            ),
            max_width: bounds.width,
            color: palette.text.scale_alpha(amount),
            size: AMOUNT_SIZE.into(),
            font: style::FONT,
            align_x: Alignment::Center,
            align_y: Vertical::Center,
            ..canvas::Text::default()
        });

        vec![frame.into_geometry()]
    }
}

/// How far along the green middle, the check mark and the amount are, each from 0 to 1, at
/// `progress` through the entrance. They overlap so it reads as one gesture.
fn stages(progress: f32) -> [f32; 3] {
    let part = |from: f32, to: f32| ease_out(((progress - from) / (to - from)).clamp(0.0, 1.0));
    [part(0.0, 0.4), part(0.25, 0.7), part(0.4, 1.0)]
}

/// Fast at first, settling softly.
fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

fn towards(from: Point, to: Point, t: f32) -> Point {
    Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entrance_starts_empty_and_ends_complete() {
        assert_eq!(stages(0.0), [0.0, 0.0, 0.0]);
        assert_eq!(stages(1.0), [1.0, 1.0, 1.0]);
        let [disc, check, amount] = stages(0.3);
        assert!(disc > check && check > amount, "one after another");
    }
}
