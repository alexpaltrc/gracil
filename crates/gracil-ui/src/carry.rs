//! Taking a result out of the window (ADR 0083): a finished file, or the whole finished list, is
//! pressed and moved like a file in File Explorer, and the file itself goes wherever it is let go.
//!
//! `carry` wraps what stands for the result. A press followed by a move past Windows' drag
//! distance starts the carrying, which Windows itself then leads; a press without a move is the
//! click it always was.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget, overlay, renderer};
use iced::{Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector, mouse};

/// How far the pointer moves, pressed, before a press becomes carrying: Windows' own distance.
const DRAG_DISTANCE: f32 = 4.0;

/// `content`, which sends `message` once pressed and moved; with no message, just `content`.
pub fn carry<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    message: Option<Message>,
) -> Element<'a, Message> {
    let content = content.into();
    match message {
        Some(message) => Element::new(Carry { content, message }),
        None => content,
    }
}

struct Carry<'a, Message> {
    content: Element<'a, Message>,
    message: Message,
}

/// Where the press began, while the button is held.
#[derive(Default)]
struct Pressed(Option<Point>);

impl<Message: Clone> Widget<Message, Theme, Renderer> for Carry<'_, Message> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Pressed>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Pressed::default())
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
        let pressed = tree.state.downcast_mut::<Pressed>();
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                pressed.0 = cursor.position_over(layout.bounds());
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => pressed.0 = None,
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if let Some(from) = pressed.0
                    && from.distance(*position) > DRAG_DISTANCE
                {
                    pressed.0 = None;
                    // What it wraps lets go of the press without taking it as a click: Windows
                    // keeps the release for itself while it carries the file.
                    let mut ignored = Vec::new();
                    let mut aside = Shell::new(&mut ignored);
                    self.content.as_widget_mut().update(
                        &mut tree.children[0],
                        &Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                        layout,
                        mouse::Cursor::Unavailable,
                        renderer,
                        clipboard,
                        &mut aside,
                        viewport,
                    );
                    shell.publish(self.message.clone());
                    shell.capture_event();
                    shell.request_redraw();
                    return;
                }
            }
            _ => {}
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
