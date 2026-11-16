//! A surface that content lies on and that lights up softly: the focus, which turns to the accent
//! while files are dragged over the window (ADR 0076).
//!
//! Like the ring, it animates itself and only asks for frames while it moves (spec §51).

use std::time::Instant;

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget, overlay, renderer};
use iced::{
    Background, Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector, mouse, window,
};

use crate::motion;
use crate::style::Surface;

pub struct Card<'a, Message> {
    content: Element<'a, Message>,
    surface: Surface,
    lit: bool,
}

/// `content` on `surface`, which lights up while `lit`.
pub fn card<'a, Message>(
    surface: Surface,
    lit: bool,
    content: impl Into<Element<'a, Message>>,
) -> Card<'a, Message> {
    Card {
        content: content.into(),
        surface,
        lit,
    }
}

#[derive(Default)]
struct State {
    /// How far lit up, from 0 to 1.
    lit: f32,
    last_frame: Option<Instant>,
}

impl<Message> Widget<Message, Theme, Renderer> for Card<'_, Message> {
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
        let target = f32::from(self.lit);
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let elapsed = motion::elapsed(state.last_frame, *now);
            state.lit = motion::approach(state.lit, target, elapsed, motion::QUICK);
            state.last_frame = (state.lit != target).then_some(*now);
        }
        if state.lit != target {
            shell.request_redraw();
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

        let (face, border) = (self.surface)(theme, tree.state.downcast_ref::<State>().lit);
        renderer.fill_quad(
            renderer::Quad {
                bounds: layout.bounds(),
                border,
                ..renderer::Quad::default()
            },
            Background::Color(face),
        );
        // Nothing shows beyond the card while it changes size around a new view.
        renderer.with_layer(layout.bounds(), |renderer| {
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

impl<'a, Message: 'a> From<Card<'a, Message>> for Element<'a, Message> {
    fn from(card: Card<'a, Message>) -> Self {
        Element::new(card)
    }
}
