//! Focus + Drawer (spec §31–§32): one video in focus, everything else one quiet click away.

use gracil_core::job::JobState;
use gracil_core::plan::output_extension;
use gracil_core::queue::Job;
use iced::widget::text::Wrapping;
use iced::widget::{Canvas, Space, button, column, container, row, scrollable, text, tooltip};
use iced::{Center, Element, Fill, window};

use crate::app::{EngineState, Gracil, Message};
use crate::dial::{self, Dial};
use crate::style::{self, size, space};
use crate::work::{Choice, Failure, Outcome};

/// Longest file name shown before it is shortened in the middle.
const FOCUS_NAME_CHARS: usize = 28;
const DRAWER_NAME_CHARS: usize = 26;
const TOOLTIP_WIDTH: f32 = 240.0;
const DRAWER_MAX_HEIGHT: f32 = 148.0;
const MINI_NAME_CHARS: usize = 22;

pub fn view(state: &Gracil, window: window::Id) -> Element<'_, Message> {
    if state.is_mini(window) {
        return mini(state, window);
    }
    let hovering = state.hovering == Some(window);
    if state.confirming_exit {
        return container(exit_question(state))
            .center(Fill)
            .padding(space::L)
            .into();
    }
    let focus = container(focus(state, hovering))
        .center(Fill)
        .padding(space::L)
        .style(move |theme| style::drop_zone(theme, hovering));

    let mut content = column![focus].spacing(space::M);
    if !state.queue.is_empty() {
        content = content.push(footer(state));
    }
    if state.drawer_open && !state.queue.is_empty() {
        content = content.push(drawer(state));
    }
    container(content).padding(space::M).into()
}

fn focus(state: &Gracil, hovering: bool) -> Element<'_, Message> {
    if hovering {
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
        let dial = Dial {
            job: job.id,
            fraction: job.fraction,
            paused: job.state == JobState::Paused,
            // Pause only once FFmpeg is compressing; before that there is nothing to hold.
            can_pause: job.state == JobState::Compressing,
        };
        content = content.push(Canvas::new(dial).width(dial::SIZE).height(dial::SIZE));
    }

    let mut details = column![status_line(&status(state, job), size::BODY)]
        .spacing(space::S)
        .align_x(Center);
    match state.outcomes.get(&job.id) {
        Some(Outcome::Ready { placing, .. }) => {
            details = details.push(choices(state, job, !placing));
            if let Some(change) = format_change(job) {
                details = details.push(caption(change));
            }
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

/// The mini-window next to the tray icon (spec §55): the video being worked on, with its dial
/// to pause it, and a way into Gracil. Videos can be dropped on it too.
fn mini(state: &Gracil, window: window::Id) -> Element<'_, Message> {
    let hovering = state.hovering == Some(window);
    let body: Element<'_, Message> = if hovering {
        text(state.text.release_to_add).size(size::BODY).into()
    } else {
        match state.queue.active().or_else(|| state.focused()) {
            Some(job) => mini_job(state, job),
            None => text(state.text.drop_videos_here).size(size::BODY).into(),
        }
    };
    let side = column![
        button(text("×").size(size::BODY))
            .style(style::quiet_button)
            .on_press(Message::CloseMini),
        Space::new().height(Fill),
        button(text(state.text.open_gracil).size(size::CAPTION))
            .style(style::quiet_button)
            .on_press(Message::OpenMain),
    ]
    .align_x(iced::Right);
    container(
        row![container(body).width(Fill).center_y(Fill), side]
            .spacing(space::S)
            .height(Fill),
    )
    .padding(space::S)
    .style(move |theme| style::drop_zone(theme, hovering))
    .into()
}

fn mini_job<'a>(state: &'a Gracil, job: &'a Job) -> Element<'a, Message> {
    let mut details = column![
        name(job, MINI_NAME_CHARS, size::BODY),
        status_line(&status(state, job), size::CAPTION),
    ]
    .spacing(space::XS);
    if let Some(Outcome::Placed { choice, path, .. }) = state.outcomes.get(&job.id) {
        details = details.push(caption(placed_caption(state, job, *choice, path)));
    }
    match state.waiting_to_compress() {
        0 => {}
        n => details = details.push(caption(state.text.in_line(n))),
    }
    let mut line = row![].spacing(space::M).align_y(Center);
    if job.state.is_active() {
        let dial = Dial {
            job: job.id,
            fraction: job.fraction,
            paused: job.state == JobState::Paused,
            can_pause: job.state == JobState::Compressing,
        };
        line = line.push(Canvas::new(dial).width(dial::SMALL).height(dial::SMALL));
    }
    line.push(details).into()
}

/// Exit asked while work is in progress: what would stop, in so many words, and the two ways out
/// (spec §54.1). Shown in the window itself, never as a dialog (spec §2.3).
fn exit_question(state: &Gracil) -> Element<'_, Message> {
    let explanation = state
        .text
        .exit_explanation(state.compressing(), state.waiting_to_compress());
    column![
        text(state.text.exit_question).size(size::TITLE),
        text(explanation)
            .size(size::BODY)
            .style(style::muted)
            .align_x(Center),
        row![
            button(text(state.text.exit).size(size::BODY))
                .padding([space::XS, space::M])
                .style(style::action_button)
                .on_press(Message::ConfirmExit),
            button(text(state.text.keep_working).size(size::BODY))
                .padding([space::XS, space::M])
                .style(style::quiet_button)
                .on_press(Message::KeepWorking),
        ]
        .spacing(space::S),
    ]
    .spacing(space::M)
    .align_x(Center)
    .max_width(320)
    .into()
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

/// Every result is an MP4 (ADR 0012). When that changes the name, say so before the user
/// chooses, in the plainest words there are: `IMG_0042.MOV → IMG_0042.mp4`.
fn format_change(job: &Job) -> Option<String> {
    let source = job.path.extension();
    let output = output_extension(&job.path);
    if source == Some(output.as_os_str()) {
        return None;
    }
    let renamed = job.path.with_extension(output);
    let renamed = renamed.file_name()?.to_string_lossy();
    // Both names have to fit on one line, so each gets half the room.
    let room = FOCUS_NAME_CHARS / 2 + 2;
    Some(format!(
        "{} → {}",
        shorten(&job.file_name(), room),
        shorten(&renamed, room)
    ))
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
