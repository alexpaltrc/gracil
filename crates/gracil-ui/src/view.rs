//! Focus + Drawer (spec §31–§32): one video in focus, everything else one quiet click away.

use gracil_core::job::JobState;
use gracil_core::queue::Job;
use iced::widget::text::Wrapping;
use iced::widget::{
    Space, button, column, container, progress_bar, row, scrollable, text, tooltip,
};
use iced::{Center, Element, Fill, window};

use crate::app::{EngineState, Gracil, Message};
use crate::style::{self, size, space};
use crate::work::{Choice, Failure, Outcome};

/// Longest file name shown before it is shortened in the middle.
const FOCUS_NAME_CHARS: usize = 28;
const DRAWER_NAME_CHARS: usize = 26;
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
        return text(state.text.release_to_add).size(size::TITLE).into();
    }
    match state.focused() {
        Some(job) => focused(state, job),
        None => column![
            text(state.text.drop_videos_here).size(size::TITLE),
            button(text(state.text.or_choose_files).size(size::BODY))
                .style(style::quiet_button)
                .on_press(Message::ChooseFiles),
        ]
        .spacing(space::S)
        .align_x(Center)
        .into(),
    }
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

    let mut details = column![status_line(&status(state, job), size::BODY)]
        .spacing(space::S)
        .align_x(Center);
    if job.state.is_active() {
        details = details.push(
            button(text(state.text.cancel).size(size::CAPTION))
                .style(style::quiet_button)
                .on_press(Message::Cancel),
        );
    }
    match state.outcomes.get(&job.id) {
        Some(Outcome::Ready { placing, .. }) => {
            details = details.push(choices(state, job, !placing));
        }
        Some(Outcome::Failed(Failure::NotSmaller)) => {
            // Shown turned off, so it is clear there was nothing worth keeping (spec §40.1).
            details = details.push(choices(state, job, false));
        }
        Some(Outcome::Placed { choice, path, .. }) => {
            details = details.push(caption(placed_caption(state, job, *choice, path)));
        }
        _ => {}
    }
    content.push(details).into()
}

/// Replace and Save copy, the two things to do with a finished result (ADR 0009).
fn choices(state: &Gracil, job: &Job, enabled: bool) -> Element<'static, Message> {
    let action = |label: &'static str, choice: Choice| {
        button(text(label).size(size::BODY))
            .padding([space::XS, space::M])
            .style(style::action_button)
            .on_press_maybe(enabled.then_some(Message::Choose(job.id, choice)))
    };
    row![
        action(state.text.replace, Choice::Replace),
        action(state.text.save_copy, Choice::SaveCopy)
    ]
    .spacing(space::S)
    .into()
}

fn placed_caption(state: &Gracil, job: &Job, choice: Choice, path: &std::path::Path) -> String {
    let placed_name = path.file_name().unwrap_or_default().to_string_lossy();
    match choice {
        Choice::SaveCopy => state
            .text
            .saved_as(&shorten(&placed_name, FOCUS_NAME_CHARS)),
        Choice::Replace if placed_name == job.file_name() => {
            state.text.replaced_the_original.to_owned()
        }
        Choice::Replace => state
            .text
            .replaced_by(&shorten(&placed_name, FOCUS_NAME_CHARS)),
    }
}

fn caption(label: String) -> Element<'static, Message> {
    text(label)
        .size(size::CAPTION)
        .style(style::muted)
        .wrapping(Wrapping::None)
        .into()
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
            EngineState::Starting => Status::plain(state.text.getting_ready),
            EngineState::Ready(_) => Status::plain(state.text.waiting),
            EngineState::Unavailable => Status {
                label: state.text.waiting.into(),
                notice: Some(state.text.needs_internet_once),
            },
        },
        JobState::Preparing => Status::plain(state.text.getting_ready),
        JobState::Compressing => match job.fraction {
            Some(fraction) => Status::plain(format!("{:.0}%", fraction * 100.0)),
            None => Status::plain(state.text.compressing),
        },
        JobState::Paused => Status::plain(state.text.paused),
        JobState::Finalizing => Status::plain(state.text.saving),
        JobState::Completed | JobState::Failed | JobState::Cancelled => {
            match state.outcomes.get(&job.id) {
                Some(Outcome::Ready { placing: true, .. }) => Status::plain(state.text.saving),
                Some(Outcome::Ready {
                    smaller_by,
                    problem,
                    ..
                }) => Status {
                    label: state.text.smaller(*smaller_by),
                    notice: problem.map(|failure| state.text.failure(failure)),
                },
                Some(Outcome::Placed { smaller_by, .. }) => {
                    Status::plain(state.text.smaller(*smaller_by))
                }
                Some(Outcome::Failed(Failure::NotSmaller)) => Status {
                    label: state.text.not_smaller.into(),
                    notice: Some(state.text.failure(Failure::NotSmaller)),
                },
                Some(Outcome::Failed(failure)) => Status {
                    label: state.text.not_compressed.into(),
                    notice: Some(state.text.failure(*failure)),
                },
                _ => Status::plain(state.text.cancelled),
            }
        }
    }
}

/// The drawer says what happened to a result in one word, since the focus shows the details.
fn drawer_status(state: &Gracil, job: &Job) -> Status {
    match state.outcomes.get(&job.id) {
        Some(Outcome::Placed {
            choice: Choice::Replace,
            ..
        }) => Status::plain(state.text.replaced),
        Some(Outcome::Placed {
            choice: Choice::SaveCopy,
            ..
        }) => Status::plain(state.text.copy_saved),
        _ => status(state, job),
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
    let mut summary = state.text.videos(state.queue.len());
    match state.waiting_count() {
        0 => {}
        n => summary.push_str(&format!("  ·  {}", state.text.ready(n))),
    }
    let toggle = if state.drawer_open {
        state.text.hide_files
    } else {
        state.text.view_files
    };
    row![
        button(text(format!("{summary}  ·  {toggle}")).size(size::CAPTION))
            .style(style::quiet_button)
            .on_press(Message::ToggleDrawer),
        Space::new().width(Fill),
        button(text(state.text.add_videos).size(size::CAPTION))
            .style(style::quiet_button)
            .on_press(Message::ChooseFiles),
    ]
    .spacing(space::S)
    .align_y(Center)
    .into()
}

fn drawer(state: &Gracil) -> Element<'_, Message> {
    let quiet = |label: &'static str, message: Option<Message>| {
        button(text(label).size(size::CAPTION))
            .style(style::quiet_button)
            .on_press_maybe(message)
    };

    let mut header = row![].spacing(space::XS).align_y(Center);
    if state.waiting_count() >= 2 {
        header = header
            .push(quiet(
                state.text.replace_all,
                Some(Message::ChooseAll(Choice::Replace)),
            ))
            .push(quiet(
                state.text.save_all_copies,
                Some(Message::ChooseAll(Choice::SaveCopy)),
            ));
    }
    let any_removable = state.queue.jobs().iter().any(|job| state.can_remove(job));
    header = header.push(Space::new().width(Fill)).push(quiet(
        state.text.clear,
        any_removable.then_some(Message::Clear),
    ));

    let rows = state.queue.jobs().iter().map(|job| {
        let line = row![
            container(name(job, DRAWER_NAME_CHARS, size::BODY)).width(Fill),
            status_line(&drawer_status(state, job), size::CAPTION),
        ]
        .spacing(space::M)
        .align_y(Center);
        let remove = quiet(
            "×",
            state.can_remove(job).then_some(Message::Remove(job.id)),
        );
        row![
            button(line)
                .padding([space::XS, space::S])
                .width(Fill)
                .style(style::quiet_button)
                .on_press(Message::Select(job.id)),
            remove,
        ]
        .align_y(Center)
        .into()
    });

    container(
        column![
            header,
            scrollable(column(rows).spacing(space::XS))
                .spacing(space::XS)
                .height(iced::Shrink),
        ]
        .spacing(space::XS)
        .padding(space::S),
    )
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
