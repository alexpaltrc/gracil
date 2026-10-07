//! Keyboard focus (ADR 0059). Tab and Shift+Tab move a ring through a window's controls in
//! reading order; Enter or Space presses the one ringed, as a click would.
//!
//! Iced's buttons can't take focus, so a button is wrapped in `keyed`: it remembers whether it
//! holds the ring, draws it, and answers Enter and Space by clicking the button it wraps. The
//! pill holds a ring the same way. A click anywhere puts the ring away: it is there for the
//! keyboard only, as in Windows.

use std::any::Any;

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{Operation, Tree, operation, tree};
use iced::advanced::{Clipboard, Shell, Widget, overlay, renderer};
use iced::keyboard::{self, key::Named};
use iced::{
    Background, Border, Color, Element, Event, Length, Rectangle, Renderer, Size, Task, Theme,
    Vector, mouse,
};

use crate::style::RADIUS;

/// Which window a control is in. Tab moves only through the window it was pressed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Main,
    Mini,
}

/// What a control that can hold the ring keeps between frames.
#[derive(Debug, Clone, Copy)]
pub struct Ring {
    pub scope: Scope,
    pub held: bool,
}

impl Ring {
    pub fn new(scope: Scope) -> Self {
        Self { scope, held: false }
    }

    /// Lets Tab find this control.
    pub fn offer(&mut self, bounds: Rectangle, operation: &mut dyn Operation) {
        operation.custom(None, bounds, self as &mut dyn Any);
    }

    /// Puts the ring away on a click; returns whether it was held.
    pub fn drop_on_click(&mut self, event: &Event) -> bool {
        let clicked = matches!(event, Event::Mouse(mouse::Event::ButtonPressed(_)));
        let was = self.held;
        if clicked {
            self.held = false;
        }
        clicked && was
    }

    /// Draws the ring around `bounds`, rounded by `radius`: Windows' own two-pixel line in the
    /// text's color.
    pub fn draw(&self, renderer: &mut Renderer, theme: &Theme, bounds: Rectangle, radius: f32) {
        use iced::advanced::Renderer as _;
        if !self.held {
            return;
        }
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border::default()
                    .rounded(radius)
                    .width(2.0)
                    .color(theme.palette().text),
                ..renderer::Quad::default()
            },
            Background::Color(Color::TRANSPARENT),
        );
    }
}

/// Moves the ring to the next control in `scope`'s window, or the previous one, wrapping round.
/// With no ring yet, it goes to the first control, or the last.
pub fn step<Message: Send + 'static>(scope: Scope, forward: bool) -> Task<Message> {
    iced::advanced::widget::operate(Count {
        scope,
        forward,
        total: 0,
        held: None,
    })
    .discard()
}

/// First the controls are counted, and which one holds the ring found ...
struct Count {
    scope: Scope,
    forward: bool,
    total: usize,
    held: Option<usize>,
}

impl Operation for Count {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }

    fn custom(
        &mut self,
        _id: Option<&iced::advanced::widget::Id>,
        _: Rectangle,
        state: &mut dyn Any,
    ) {
        if let Some(ring) = state
            .downcast_mut::<Ring>()
            .filter(|ring| ring.scope == self.scope)
        {
            if ring.held {
                self.held = Some(self.total);
            }
            self.total += 1;
        }
    }

    fn finish(&self) -> operation::Outcome<()> {
        if self.total == 0 {
            return operation::Outcome::None;
        }
        let last = self.total - 1;
        let to = match (self.held, self.forward) {
            (None, true) => 0,
            (None, false) => last,
            (Some(held), true) => {
                if held == last {
                    0
                } else {
                    held + 1
                }
            }
            (Some(held), false) => {
                if held == 0 {
                    last
                } else {
                    held - 1
                }
            }
        };
        operation::Outcome::Chain(Box::new(Give {
            scope: self.scope,
            to,
            at: 0,
        }))
    }
}

/// ... then the ring is given to the control after it.
struct Give {
    scope: Scope,
    to: usize,
    at: usize,
}

impl Operation for Give {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }

    fn custom(
        &mut self,
        _id: Option<&iced::advanced::widget::Id>,
        _: Rectangle,
        state: &mut dyn Any,
    ) {
        if let Some(ring) = state
            .downcast_mut::<Ring>()
            .filter(|ring| ring.scope == self.scope)
        {
            ring.held = self.at == self.to;
            self.at += 1;
        }
    }
}

/// Whether `event` is a key that presses a control holding the ring.
pub fn is_press(event: &Event) -> bool {
    matches!(
        event,
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(Named::Enter | Named::Space),
            modifiers,
            ..
        }) if modifiers.is_empty()
    )
}

/// A button that the keyboard can reach.
pub fn keyed<'a, Message: 'a>(
    scope: Scope,
    button: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    Element::new(Keyed {
        scope,
        content: button.into(),
    })
}

struct Keyed<'a, Message> {
    scope: Scope,
    content: Element<'a, Message>,
}

impl<Message> Widget<Message, Theme, Renderer> for Keyed<'_, Message> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Ring>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Ring::new(self.scope))
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

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let ring = tree.state.downcast_mut::<Ring>();
        ring.scope = self.scope;
        ring.offer(layout.bounds(), operation);
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
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
        let ring = tree.state.downcast_mut::<Ring>();
        if ring.drop_on_click(event) {
            shell.request_redraw();
        }
        if ring.held && is_press(event) {
            // The button is clicked in its middle, so it does just what a click does, and
            // nothing if it can't be pressed now.
            let middle = mouse::Cursor::Available(layout.bounds().center());
            for click in [mouse::Event::ButtonPressed, mouse::Event::ButtonReleased] {
                self.content.as_widget_mut().update(
                    &mut tree.children[0],
                    &Event::Mouse(click(mouse::Button::Left)),
                    layout,
                    middle,
                    renderer,
                    clipboard,
                    shell,
                    viewport,
                );
            }
            shell.capture_event();
            return;
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
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
        tree.state
            .downcast_ref::<Ring>()
            .draw(renderer, theme, layout.bounds(), RADIUS);
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
