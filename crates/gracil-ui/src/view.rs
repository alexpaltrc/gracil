//! Focus + Drawer (spec §31–§32): one video in focus, everything else one quiet click away.

use gracil_core::job::JobState;
use gracil_core::queue::Job;
use iced::widget::text::Wrapping;
use iced::widget::{button, column, container, progress_bar, row, scrollable, text, tooltip};
use iced::{Center, Element, Fill, window};

use crate::app::{EngineState, Gracil, Message};
use crate::style::{self, size, space};
use crate::work::Outcome;

/// Longest file name shown before it is shortened in the middle.
const FOCUS_NAME_CHARS: usize = 28;
const DRAWER_NAME_CHARS: usize = 30;
const TOOLTIP_WIDTH: f32 = 240.0;
const DRAWER_MAX_HEIGHT: f32 = 148.0;

pub fn view(state: &Gracil, _window: window::Id) -> Element<'_, Message> {
    let focus = container(focus(state))
        .center(Fill)
        .padding(space::L)
        .style(move |theme| style::drop_zone(theme, state.hovering_files));

    let mut content = column![focus].spacing(space::M);
    if !state.queue.is_empty() {
        content = content.push(footer(state));
    }
    if state.drawer_open && !state.queue.is_empty() {
        content = content.push(drawer(state));
    }
    container(content).padding(space::M).into()
}

fn focus(state: &Gracil) -> Element<'_, Message> {
    if state.hovering_files {
        return text("Release to add").size(size::TITLE).into();
    }
    match focused_job(state) {
        Some(job) => focused(state, job),
        None => column![
            text("Drop videos here").size(size::TITLE),
            button(text("or choose files").size(size::BODY))
                .style(style::quiet_button)
                .on_press(Message::ChooseFiles),
        ]
        .spacing(space::S)
        .align_x(Center)
        .into(),
    }
}

/// The running video; otherwise the next one waiting; otherwise the last one finished.
fn focused_job(state: &Gracil) -> Option<&Job> {
    let jobs = state.queue.jobs();
    state
        .queue
        .active()
        .or_else(|| jobs.iter().find(|job| job.state == JobState::Pending))
        .or_else(|| jobs.iter().rev().find(|job| job.state.is_finished()))
}

fn focused<'a>(state: &'a Gracil, job: &'a Job) -> Element<'a, Message> {
    let mut content = column![name(job, FOCUS_NAME_CHARS, size::TITLE)]
        .spacing(space::M)
        .align_x(Center)
        .max_width(320);

    if job.state.is_active() {
        content = content.push(
            progress_bar(0.0..=1.0, job.fraction.unwrap_or(0.0) as f32)
                .girth(style::PROGRESS_GIRTH)
                .style(style::progress),
        );
    }

    let status = status(state, job);
    let mut details = column![status_line(&status, size::BODY)]
        .spacing(space::XS)
        .align_x(Center);
    if let Some(Outcome::Saved { path, .. }) = state.outcomes.get(&job.id) {
        let saved_as = path.file_name().unwrap_or_default().to_string_lossy();
        details = details.push(
            text(format!("Saved as {}", shorten(&saved_as, FOCUS_NAME_CHARS)))
                .size(size::CAPTION)
                .style(style::muted)
                .wrapping(Wrapping::None),
        );
    }
    content.push(details).into()
}

/// A job's state in a few words, plus an explanation behind a "!" when something needs it.
struct Status {
    label: String,
    notice: Option<&'static str>,
}

impl Status {
    fn plain(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            notice: None,
        }
    }
}

fn status(state: &Gracil, job: &Job) -> Status {
    match job.state {
        JobState::Pending => match state.engine {
            EngineState::Starting => Status::plain("Getting ready"),
            EngineState::Ready(_) => Status::plain("Waiting"),
            EngineState::Unavailable => Status {
                label: "Waiting".into(),
                notice: Some(
                    "Gracil needs an internet connection once, to set up its video engine",
                ),
            },
        },
        JobState::Preparing => Status::plain("Getting ready"),
        JobState::Compressing => match job.fraction {
            Some(fraction) => Status::plain(format!("{:.0}%", fraction * 100.0)),
            None => Status::plain("Compressing"),
        },
        JobState::Paused => Status::plain("Paused"),
        JobState::Finalizing => Status::plain("Saving"),
        JobState::Completed | JobState::Failed | JobState::Cancelled => {
            match state.outcomes.get(&job.id) {
                Some(Outcome::Saved { smaller_by, .. }) => {
                    Status::plain(format!("{:.0}% smaller", smaller_by * 100.0))
                }
                Some(Outcome::Failed(failure)) => Status {
                    label: "Not compressed".into(),
                    notice: Some(failure.explanation()),
                },
                _ => Status::plain("Cancelled"),
            }
        }
    }
}

fn status_line(status: &Status, text_size: f32) -> Element<'static, Message> {
    let mut line = row![
        text(status.label.clone())
            .size(text_size)
            .style(style::muted)
            .wrapping(Wrapping::None)
    ]
    .spacing(space::S)
    .align_y(Center);
    if let Some(explanation) = status.notice {
        line = line.push(notice(explanation));
    }
    line.into()
}

fn name(job: &Job, max_chars: usize, text_size: f32) -> Element<'static, Message> {
    text(shorten(&job.file_name(), max_chars))
        .size(text_size)
        .wrapping(Wrapping::None)
        .into()
}

fn footer(state: &Gracil) -> Element<'_, Message> {
    let count = match state.queue.len() {
        1 => "1 video".to_owned(),
        n => format!("{n} videos"),
    };
    let toggle = if state.drawer_open {
        "Hide files"
    } else {
        "View files"
    };
    row![
        button(text(format!("{count}  ·  {toggle}")).size(size::CAPTION))
            .style(style::quiet_button)
            .on_press(Message::ToggleDrawer),
        button(text("Add videos").size(size::CAPTION))
            .style(style::quiet_button)
            .on_press(Message::ChooseFiles),
    ]
    .spacing(space::S)
    .align_y(Center)
    .into()
}

fn drawer(state: &Gracil) -> Element<'_, Message> {
    let rows = state.queue.jobs().iter().map(|job| {
        row![
            container(name(job, DRAWER_NAME_CHARS, size::BODY)).width(Fill),
            status_line(&status(state, job), size::CAPTION),
        ]
        .spacing(space::M)
        .align_y(Center)
        .into()
    });
    container(scrollable(column(rows).spacing(space::S).padding(space::M)))
        .max_height(DRAWER_MAX_HEIGHT)
        .width(Fill)
        .style(style::drawer)
        .into()
}

/// A small "!" that explains itself on hover (spec §2.3).
fn notice(explanation: &'static str) -> Element<'static, Message> {
    tooltip(
        text("!").size(size::BODY).style(style::notice),
        container(text(explanation).size(size::CAPTION))
            .padding(space::S)
            .max_width(TOOLTIP_WIDTH),
        tooltip::Position::Top,
    )
    .style(style::tooltip)
    .gap(space::XS)
    .into()
}

/// Shortens `name` to `max` characters by cutting the middle, so the start and the extension
/// stay readable: `A very long holiday video from 2024.mp4` → `A very long holi…om 2024.mp4`.
fn shorten(name: &str, max: usize) -> String {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= max || max < 3 {
        return name.to_owned();
    }
    let keep = max - 1;
    let head = keep * 3 / 5;
    let tail = keep - head;
    let mut short: String = chars[..head].iter().collect();
    short.push('…');
    short.extend(&chars[chars.len() - tail..]);
    short
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_names_are_untouched() {
        assert_eq!(shorten("Vacation.mp4", 40), "Vacation.mp4");
    }

    #[test]
    fn long_names_keep_start_and_extension() {
        let short = shorten("A very long holiday video from the summer of 2024.mp4", 24);
        assert_eq!(short.chars().count(), 24);
        assert!(short.starts_with("A very long"), "{short}");
        assert!(short.ends_with("2024.mp4"), "{short}");
        assert!(short.contains('…'));
    }

    #[test]
    fn shortening_respects_characters_not_bytes() {
        let short = shorten("🎂🎂🎂🎂🎂🎂🎂🎂🎂🎂🎂🎂.mp4", 10);
        assert_eq!(short.chars().count(), 10);
        assert!(short.ends_with(".mp4"));
    }
}
