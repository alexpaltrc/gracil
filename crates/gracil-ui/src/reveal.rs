//! Unfolds something in place and folds it away again: the list under the ring (ADR 0056). It
//! grows from nothing to its full height while its contents come out from under the line above
//! and fade in, and does the same backwards when closed, so nothing jumps.
//!
//! Like the dial and the pill, it animates itself and only asks for frames while it moves
//! (spec §51).

use std::time::Instant;

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget, overlay, renderer};
use iced::{
    Background, Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector, mouse, window,
};

/// How quickly it unfolds, in seconds (the time to cover about two thirds): a little slower
/// than the pill's stone, since it moves more.
const EASE: f32 = 0.06;

/// How far the contents travel as they come out, in logical pixels.
const SLIDE: f32 = 10.0;

pub struct Reveal<'a, Message> {
    content: Element<'a, Message>,
    open: bool,
    on_folded: Option<Message>,
}

/// `content`, unfolding while `open` and folding away when not. Once fully folded it sends
/// `on_folded`, so whoever shows it can leave it out from then on.
pub fn reveal<'a, Message>(
    content: impl Into<Element<'a, Message>>,
    open: bool,
    on_folded: Message,
) -> Reveal<'a, Message> {
    Reveal {
        content: content.into(),
        open,
        on_folded: Some(on_folded),
    }
}

#[derive(Default)]
struct State {
    /// How far unfolded, from 0 to 1. A new one starts folded, so it unfolds when it appears.
    shown: f32,
    last_frame: Option<Instant>,
    /// Whether `on_folded` was sent for the current fold.
    told: bool,
}

impl<Message> Reveal<'_, Message> {
    fn target(&self) -> f32 {
        if self.open { 1.0 } else { 0.0 }
    }
}

/// The part of `shown` to use for the slide and the fade: settles quickly near the end, so the
/// contents are still once the space is.
fn eased(shown: f32) -> f32 {
    1.0 - (1.0 - shown.clamp(0.0, 1.0)).powi(2)
}

/// The cursor, but only where the visible part is: the folded-away rest must not be hovered or
/// clicked through what lies below it.
fn inside(cursor: mouse::Cursor, bounds: Rectangle) -> mouse::Cursor {
    if cursor.is_over(bounds) {
        cursor
    } else {
        mouse::Cursor::Unavailable
    }
}

impl<Message: Clone> Widget<Message, Theme, Renderer> for Reveal<'_, Message> {
    fn size(&self) -> Size<Length> {
        Size::new(self.content.as_widget().size().width, Length::Shrink)
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let shown = tree.state.downcast_ref::<State>().shown;
        let content = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        let full = content.size();
        let content = content.move_to((0.0, -SLIDE * (1.0 - eased(shown))));
        layout::Node::with_children(Size::new(full.width, full.height * shown), vec![content])
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let target = self.target();
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let state = tree.state.downcast_mut::<State>();
            let before = state.shown;
            let elapsed = state
                .last_frame
                .map_or(0.0, |last| now.duration_since(last).as_secs_f32().min(0.1));
            let next = state.shown + (target - state.shown) * (1.0 - (-elapsed / EASE).exp());
            if (target - next).abs() < 0.002 {
                state.shown = target;
                state.last_frame = None;
            } else {
                state.shown = next;
                state.last_frame = Some(*now);
                shell.request_redraw();
            }
            if state.shown != before {
                shell.invalidate_layout();
            }
        }

        let state = tree.state.downcast_mut::<State>();
        if state.shown != target && state.last_frame.is_none() {
            // Opened or closed since the last frame: start moving.
            shell.request_redraw();
        }
        if state.shown == 0.0 && !self.open {
            if !state.told {
                state.told = true;
                if let Some(message) = self.on_folded.clone() {
                    shell.publish(message);
                }
            }
            return;
        }
        state.told = false;

        let Some(content) = layout.children().next() else {
            return;
        };
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            content,
            inside(cursor, layout.bounds()),
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        use iced::advanced::Renderer as _;

        let shown = tree.state.downcast_ref::<State>().shown;
        let bounds = layout.bounds();
        let Some(content) = layout.children().next() else {
            return;
        };
        if shown <= 0.0 || bounds.height <= 0.0 {
            return;
        }
        renderer.with_layer(bounds, |renderer| {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                content,
                inside(cursor, bounds),
                viewport,
            );
            // Fading in: the window's own background over the contents, thinning to nothing.
            let veil = 1.0 - eased(shown);
            if veil > 0.0 {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds,
                        ..renderer::Quad::default()
                    },
                    Background::Color(theme.palette().background.scale_alpha(veil)),
                );
            }
        });
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let Some(content) = layout.children().next() else {
            return mouse::Interaction::None;
        };
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            content,
            inside(cursor, layout.bounds()),
            viewport,
            renderer,
        )
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        if let Some(content) = layout.children().next() {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                content,
                renderer,
                operation,
            );
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        // Tooltips and the like only once it is fully out.
        if tree.state.downcast_ref::<State>().shown < 1.0 {
            return None;
        }
        let content = layout.children().next()?;
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            content,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message: Clone + 'a> From<Reveal<'a, Message>> for Element<'a, Message> {
    fn from(reveal: Reveal<'a, Message>) -> Self {
        Element::new(reveal)
    }
}

#[cfg(test)]
mod tests {
    use super::eased;

    #[test]
    fn the_slide_and_fade_settle_before_the_space_does() {
        assert_eq!(eased(0.0), 0.0);
        assert_eq!(eased(1.0), 1.0);
        assert!(eased(0.5) > 0.5);
        assert!(eased(0.9) > 0.98);
    }
}
