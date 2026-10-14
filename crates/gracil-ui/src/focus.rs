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

/// After a press that changes what the window shows, a ring still held moves to the new view's
/// first control. Rings are kept by position, so it would otherwise land on whatever control
/// took the pressed one's place. With no ring, nothing changes.
pub fn restart<Message: Send + 'static>(scope: Scope) -> Task<Message> {
    iced::advanced::widget::operate(Restart { scope, held: false }).discard()
}

struct Restart {
    scope: Scope,
    held: bool,
}

impl Operation for Restart {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }

    fn custom(
        &mut self,
        _id: Option<&iced::advanced::widget::Id>,
        _: Rectangle,
        state: &mut dyn Any,
    ) {
        if let Some(ring) = state.downcast_mut::<Ring>() {
            self.held |= ring.scope == self.scope && ring.held;
        }
    }

    fn finish(&self) -> operation::Outcome<()> {
        if !self.held {
            return operation::Outcome::None;
        }
        operation::Outcome::Chain(Box::new(Give {
            scope: self.scope,
            to: 0,
            at: 0,
        }))
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
    keyed_if(scope, true, button)
}

/// A button that the keyboard can reach while it can be pressed: Tab passes over it otherwise,
/// as Windows does with a control that can't be used now.
pub fn keyed_if<'a, Message: 'a>(
    scope: Scope,
    enabled: bool,
    button: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    Element::new(Keyed {
        scope,
        enabled,
        content: button.into(),
    })
}

struct Keyed<'a, Message> {
    scope: Scope,
    enabled: bool,
    content: Element<'a, Message>,
}

/// Whether `event` is one that a button or a hint reads where the pointer is from.
fn reads_the_pointer(event: &Event) -> bool {
    matches!(
        event,
        Event::Mouse(mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft)
            | Event::Window(iced::window::Event::RedrawRequested(_))
    )
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
        if self.enabled {
            ring.offer(layout.bounds(), operation);
        } else {
            ring.held = false;
        }
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
            // Each half gets a shell of its own: a button captures the press, and would then
            // ignore the release in the same shell.
            for click in [mouse::Event::ButtonPressed, mouse::Event::ButtonReleased] {
                let mut messages = Vec::new();
                let mut half = Shell::new(&mut messages);
                self.content.as_widget_mut().update(
                    &mut tree.children[0],
                    &Event::Mouse(click(mouse::Button::Left)),
                    layout,
                    middle,
                    renderer,
                    clipboard,
                    &mut half,
                    viewport,
                );
                shell.merge(half, std::convert::identity);
            }
            shell.capture_event();
            return;
        }
        // Holding the ring, the button lights up as under the pointer, and a hint around it
        // says its few words, so the keyboard learns what the pointer would.
        let cursor = if ring.held && reads_the_pointer(event) {
            mouse::Cursor::Available(layout.bounds().center())
        } else {
            cursor
        };
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
        let ring = tree.state.downcast_ref::<Ring>();
        let cursor = if ring.held {
            mouse::Cursor::Available(layout.bounds().center())
        } else {
            cursor
        };
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
        ring.draw(renderer, theme, layout.bounds(), RADIUS);
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

/// One of the places inside a drawn control that the ring can stop at, like the dial's Pause and
/// Stop: where it is within the control, how round its ring is, and what pressing it sends.
pub struct Part<Message> {
    pub place: fn(Size) -> Rectangle,
    pub radius: fn(Size) -> f32,
    /// None while it can't be pressed: Tab passes over it then.
    pub message: Option<Message>,
}

/// A drawn control whose parts the keyboard can reach one by one. Holding the ring, a part lights
/// up as under the pointer.
pub fn keyed_parts<'a, Message: Clone + 'a>(
    scope: Scope,
    content: impl Into<Element<'a, Message>>,
    parts: Vec<Part<Message>>,
) -> Element<'a, Message> {
    Element::new(KeyedParts {
        scope,
        content: content.into(),
        parts,
    })
}

struct KeyedParts<'a, Message> {
    scope: Scope,
    content: Element<'a, Message>,
    parts: Vec<Part<Message>>,
}

impl<Message> KeyedParts<'_, Message> {
    /// The part holding the ring, if any, and where it is.
    fn held(&self, rings: &[Ring], bounds: Rectangle) -> Option<(usize, Rectangle)> {
        let at = rings.iter().position(|ring| ring.held)?;
        let part = self.parts.get(at)?;
        let place = (part.place)(bounds.size());
        Some((at, place + Vector::new(bounds.x, bounds.y)))
    }
}

impl<Message: Clone> Widget<Message, Theme, Renderer> for KeyedParts<'_, Message> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Vec<Ring>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(vec![Ring::new(self.scope); self.parts.len()])
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
        let bounds = layout.bounds();
        let rings = tree.state.downcast_mut::<Vec<Ring>>();
        rings.resize(self.parts.len(), Ring::new(self.scope));
        for (ring, part) in rings.iter_mut().zip(&self.parts) {
            ring.scope = self.scope;
            if part.message.is_some() {
                let place = (part.place)(bounds.size()) + Vector::new(bounds.x, bounds.y);
                ring.offer(place, operation);
            } else {
                ring.held = false;
            }
        }
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
        let rings = tree.state.downcast_mut::<Vec<Ring>>();
        let mut dropped = false;
        for ring in rings.iter_mut() {
            dropped |= ring.drop_on_click(event);
        }
        if dropped {
            shell.request_redraw();
        }
        let held = self.held(rings, layout.bounds());
        if let Some((at, _)) = held
            && is_press(event)
        {
            if let Some(message) = self.parts[at].message.clone() {
                shell.publish(message);
            }
            shell.capture_event();
            return;
        }
        let cursor = match held {
            Some((_, place)) if reads_the_pointer(event) => {
                mouse::Cursor::Available(place.center())
            }
            _ => cursor,
        };
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
        let rings = tree.state.downcast_ref::<Vec<Ring>>();
        if let Some((at, place)) = self.held(rings, layout.bounds()) {
            let radius = (self.parts[at].radius)(layout.bounds().size());
            rings[at].draw(renderer, theme, place, radius);
        }
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
}
