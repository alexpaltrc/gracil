//! The pill: one capsule holding three choices, with a soft stone that slides to the one picked.
//! Its middle choice means "off", so there the stone fades almost away; it fills in as it slides
//! towards either end.
//!
//! Like the dial, it animates itself and only asks for frames while the stone is moving.

use std::time::Instant;

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget, renderer};
use iced::widget::{container, row, text};
use iced::{
    Background, Border, Element, Event, Length, Rectangle, Renderer, Size, Theme, mouse, window,
};

use crate::style::{size, space};

/// How quickly the stone catches up, in seconds (the time to cover about two thirds).
const EASE: f32 = 0.07;

pub struct Pill<'a, Message> {
    content: Element<'a, Message>,
    selected: usize,
    on_select: Box<dyn Fn(usize) -> Message + 'a>,
}

/// Three choices; `selected` is the one picked, and the middle one means "off".
pub fn pill<'a, Message: 'a>(
    labels: [&'a str; 3],
    selected: usize,
    on_select: impl Fn(usize) -> Message + 'a,
) -> Pill<'a, Message> {
    let segments = labels.into_iter().enumerate().map(|(index, label)| {
        let picked = index == selected;
        let off = index == 1;
        container(text(label).size(size::CAPTION).style(move |theme: &Theme| {
            let color = theme.palette().text;
            text::Style {
                color: Some(match (picked, off) {
                    (true, false) => color,
                    (true, true) => color.scale_alpha(0.6),
                    (false, _) => color.scale_alpha(0.5),
                }),
            }
        }))
        .padding([2.0, space::S + 2.0])
        .into()
    });
    Pill {
        content: row(segments).into(),
        selected,
        on_select: Box::new(on_select),
    }
}

#[derive(Default)]
struct State {
    /// Where the stone is, from 0 (first choice) to 2 (last); `None` until first drawn.
    at: Option<f32>,
    last_frame: Option<Instant>,
    /// The choice under the cursor, so it lights up.
    hovered: Option<usize>,
}

impl<Message> Widget<Message, Theme, Renderer> for Pill<'_, Message> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
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
        let content = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        layout::Node::with_children(content.size(), vec![content])
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let target = self.selected as f32;
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let hit = segments(layout).position(|bounds| cursor.is_over(bounds));
                if let Some(index) = hit {
                    if index != self.selected {
                        shell.publish((self.on_select)(index));
                        shell.request_redraw();
                    }
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft) => {
                let hovered = segments(layout).position(|bounds| cursor.is_over(bounds));
                let state = tree.state.downcast_mut::<State>();
                if state.hovered != hovered {
                    state.hovered = hovered;
                    shell.request_redraw();
                }
            }
            Event::Window(window::Event::RedrawRequested(now)) => {
                let state = tree.state.downcast_mut::<State>();
                let at = state.at.unwrap_or(target);
                let elapsed = state
                    .last_frame
                    .map_or(0.0, |last| now.duration_since(last).as_secs_f32().min(0.1));
                let next = at + (target - at) * (1.0 - (-elapsed / EASE).exp());
                if (target - next).abs() < 0.002 {
                    state.at = Some(target);
                    state.last_frame = None;
                } else {
                    state.at = Some(next);
                    state.last_frame = Some(*now);
                    shell.request_redraw();
                }
            }
            _ => {}
        }
        // A new choice starts the stone moving.
        let state = tree.state.downcast_ref::<State>();
        if state.at.is_some_and(|at| at != target) && state.last_frame.is_none() {
            shell.request_redraw();
        }
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

        let palette = theme.palette();
        let Some(content) = layout.children().next() else {
            return;
        };
        let bounds = content.bounds();
        let radius = bounds.height / 2.0;
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border::default()
                    .rounded(radius)
                    .width(1.0)
                    .color(palette.text.scale_alpha(0.12)),
                ..renderer::Quad::default()
            },
            Background::Color(palette.text.scale_alpha(0.03)),
        );

        let parts: Vec<Rectangle> = segments(layout).collect();
        if let Some(hovered) = parts
            .iter()
            .enumerate()
            .find(|(index, part)| *index != self.selected && cursor.is_over(**part))
        {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: inset(*hovered.1),
                    border: Border::default().rounded(radius),
                    ..renderer::Quad::default()
                },
                Background::Color(palette.text.scale_alpha(0.05)),
            );
        }

        let at = tree
            .state
            .downcast_ref::<State>()
            .at
            .unwrap_or(self.selected as f32)
            .clamp(0.0, 2.0);
        if parts.len() == 3 {
            let from = at.floor() as usize;
            let to = (from + 1).min(2);
            let t = at - from as f32;
            let (a, b) = (inset(parts[from]), inset(parts[to]));
            let stone = Rectangle {
                x: a.x + (b.x - a.x) * t,
                y: a.y,
                width: a.width + (b.width - a.width) * t,
                height: a.height,
            };
            // Off in the middle, filled at either end.
            let lit = (at - 1.0).abs();
            let fill = palette
                .text
                .scale_alpha(0.06)
                .mix(palette.primary.scale_alpha(0.55), lit);
            renderer.fill_quad(
                renderer::Quad {
                    bounds: stone,
                    border: Border::default()
                        .rounded(stone.height / 2.0)
                        .width(1.0)
                        .color(palette.text.scale_alpha(0.10 * (1.0 - lit))),
                    ..renderer::Quad::default()
                },
                Background::Color(fill),
            );
        }

        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            content,
            cursor,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if segments(layout)
            .enumerate()
            .any(|(index, part)| index != self.selected && cursor.is_over(part))
        {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }
}

impl<'a, Message: 'a> From<Pill<'a, Message>> for Element<'a, Message> {
    fn from(pill: Pill<'a, Message>) -> Self {
        Element::new(pill)
    }
}

/// The three choices' areas.
fn segments(layout: Layout<'_>) -> impl Iterator<Item = Rectangle> + '_ {
    layout
        .children()
        .next()
        .into_iter()
        .flat_map(|content| content.children())
        .map(|segment| segment.bounds())
}

/// The stone sits just inside the capsule's edge.
fn inset(bounds: Rectangle) -> Rectangle {
    Rectangle {
        x: bounds.x + 2.0,
        y: bounds.y + 2.0,
        width: bounds.width - 4.0,
        height: bounds.height - 4.0,
    }
}

trait Mix {
    fn mix(self, other: Self, t: f32) -> Self;
}

impl Mix for iced::Color {
    fn mix(self, other: Self, t: f32) -> Self {
        Self {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }
}
