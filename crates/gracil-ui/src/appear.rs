//! Brings a new view in softly: when what the window shows changes (the work, a result, the
//! question before leaving, files dragged over it), the new view rises a little into place as it
//! fades in from the surface behind it, instead of replacing the old one in a single frame.
//!
//! Like the dial and the list, it animates itself and only asks for frames while it moves
//! (spec §51), and shows the new view at once while Windows' animation effects are off.

use std::time::Instant;

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget, overlay, renderer};
use iced::{
    Background, Color, Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector, mouse,
    window,
};

use crate::motion::{self, Spring};

/// How far a new view rises into place, in logical pixels.
const RISE: f32 = 8.0;

pub struct Appear<'a, Message> {
    content: Element<'a, Message>,
    key: u64,
    surface: Box<dyn Fn(&Theme) -> Color + 'a>,
}

/// `content`, fading in from `surface` whenever `key` changes.
pub fn appear<'a, Message>(
    key: u64,
    surface: impl Fn(&Theme) -> Color + 'a,
    content: impl Into<Element<'a, Message>>,
) -> Appear<'a, Message> {
    Appear {
        content: content.into(),
        key,
        surface: Box::new(surface),
    }
}

struct State {
    /// What was shown last; the first view of all is shown as it is.
    key: Option<u64>,
    /// How far in the new view is, from 0 to 1.
    shown: Spring,
    last_frame: Option<Instant>,
}

impl<Message> Widget<Message, Theme, Renderer> for Appear<'_, Message> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State {
            key: None,
            shown: Spring::at(1.0),
            last_frame: None,
        })
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
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
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
        let state = tree.state.downcast_mut::<State>();
        match state.key {
            None => state.key = Some(self.key),
            Some(key) if key != self.key => {
                state.key = Some(self.key);
                if !motion::still() {
                    state.shown = Spring::at(0.0);
                    state.last_frame = None;
                    shell.request_redraw();
                }
            }
            Some(_) => {}
        }
        if let Event::Window(window::Event::RedrawRequested(now)) = event
            && !state.shown.rests_at(1.0)
        {
            let elapsed = motion::elapsed(state.last_frame, *now);
            state.shown.step(1.0, elapsed, motion::SMOOTH);
            if !state.shown.rests_at(1.0) {
                state.last_frame = Some(*now);
                shell.request_redraw();
            } else {
                state.last_frame = None;
            }
        }
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
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

        let shown = tree.state.downcast_ref::<State>().shown.value;
        if shown >= 1.0 {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                layout,
                cursor,
                viewport,
            );
            return;
        }
        // Coming in: a little below its place and rising into it, under the surface behind, which
        // thins to nothing. In a layer of its own, so the surface covers the view's text and
        // drawings too, and nothing shows beyond its place.
        let bounds = layout.bounds();
        let rise = Vector::new(0.0, RISE * (1.0 - shown));
        renderer.with_layer(bounds, |renderer| {
            renderer.with_translation(rise, |renderer| {
                self.content.as_widget().draw(
                    &tree.children[0],
                    renderer,
                    theme,
                    style,
                    layout,
                    cursor,
                    viewport,
                );
            });
        });
        renderer.with_layer(bounds, |renderer| {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    ..renderer::Quad::default()
                },
                Background::Color((self.surface)(theme).scale_alpha(1.0 - shown.max(0.0))),
            );
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
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
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
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message: 'a> From<Appear<'a, Message>> for Element<'a, Message> {
    fn from(appear: Appear<'a, Message>) -> Self {
        Element::new(appear)
    }
}
