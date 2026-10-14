//! Focus + Drawer (spec §31–§32): one video in focus, everything else one quiet click away.

use std::ffi::OsStr;
use std::ops::Range;

use gracil_core::job::JobState;
use gracil_core::queue::{Job, JobId};
use iced::widget::operation::{self, AbsoluteOffset};
use iced::widget::text::Wrapping;
use iced::widget::{Canvas, Id, Space, button, column, container, row, scrollable, text, tooltip};
use iced::{Center, Element, Fill, Padding, Task, Theme, window};

use crate::app::{EngineState, Finished, Gracil, Message};
use crate::appear::appear;
use crate::dial::{self, Dial, Shows};
use crate::done::{self, Done};
use crate::focus::{Scope, keyed, keyed_if};
use crate::pill::pill;
use crate::reveal::reveal;
use crate::style::{self, size, space};
use crate::work::{Choice, Failure, Outcome};

/// Longest file name shown before it is shortened in the middle.
const FOCUS_NAME_CHARS: usize = 28;
const DRAWER_NAME_CHARS: usize = 26;
const TOOLTIP_WIDTH: f32 = 240.0;
const DRAWER_MAX_HEIGHT: f32 = 148.0;
/// The height of every row in the list. All rows are the same height, so only the rows in sight
/// are built, however long the list is (spec §77.3).
const ROW_HEIGHT: f32 = 30.0;
/// Rows built beyond each edge of what is in sight, so scrolling never shows a gap.
const ROWS_BEYOND: usize = 4;
/// The list, so it can be scrolled to a file.
const DRAWER_LIST: Id = Id::new("drawer-list");

/// How far down the list is scrolled and how tall its visible part is, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawerScroll {
    pub offset: f32,
    pub height: f32,
}

impl Default for DrawerScroll {
    fn default() -> Self {
        Self {
            offset: 0.0,
            height: DRAWER_MAX_HEIGHT,
        }
    }
}

impl DrawerScroll {
    /// Scrolled so the row at `index` is in sight, a row below the top when it can be.
    pub fn showing(self, index: usize) -> Self {
        let top = index as f32 * ROW_HEIGHT;
        if top >= self.offset && top + ROW_HEIGHT <= self.offset + self.height {
            return self;
        }
        Self {
            offset: (top - ROW_HEIGHT).max(0.0),
            ..self
        }
    }

    /// Puts the list back where this says, once it is built: a list folded away forgets its place.
    pub fn restore<T>(self) -> Task<T> {
        operation::scroll_to(
            DRAWER_LIST,
            AbsoluteOffset {
                x: 0.0,
                y: self.offset,
            },
        )
    }

    /// The rows of a list of `len` that are in sight, or nearly.
    fn rows(self, len: usize) -> Range<usize> {
        let first = (self.offset.max(0.0) / ROW_HEIGHT) as usize;
        let last = ((self.offset.max(0.0) + self.height.max(0.0)) / ROW_HEIGHT).ceil() as usize;
        let start = first.saturating_sub(ROWS_BEYOND).min(len);
        start..(last + ROWS_BEYOND).clamp(start, len)
    }
}
const MINI_NAME_CHARS: usize = 22;

pub fn view(state: &Gracil, window: window::Id) -> Element<'_, Message> {
    if state.is_mini(window) {
        // Kept hidden between openings, it draws nothing, so nothing in it moves.
        if !state.mini_is_open() {
            return column![].into();
        }
        return mini(state, window);
    }
    let hovering = state.hovering == Some(window);
    // The question before leaving takes the drop area's place, so it comes in softly too.
    let asking = state.confirming_exit;
    let shows = if asking {
        exit_question(state)
    } else {
        focus(state, hovering)
    };
    let focus = container(appear(
        shown(state, hovering),
        move |theme: &Theme| style::zone_surface(theme, hovering && !asking),
        shows,
    ))
    .center(Fill)
    .padding(space::L)
    .style(move |theme| {
        if asking {
            container::Style::default()
        } else {
            style::drop_zone(theme, hovering)
        }
    });

    let mut content = column![focus].spacing(space::M);
    if asking {
        return container(content).padding(space::M).into();
    }
    if !state.offered.is_empty() {
        content = content.push(offer(state));
    }
    if state.cleared.is_some() {
        content = content.push(undo(state));
    }
    if !state.queue.is_empty() {
        content = content.push(footer(state));
        if state.drawer_open || state.drawer_folding {
            content = content.push(reveal(
                drawer(state),
                state.drawer_open,
                Message::DrawerFolded,
            ));
        }
    }
    // Always in sight, the empty window included: it is also what File Explorer's menu follows,
    // so it can be set before the files come (ADR 0057).
    content = content.push(always(state));
    container(content).padding(space::M).into()
}

/// Files left unfinished the last time Gracil closed, and a way to carry on with them or let them
/// go (ADR 0042). A quiet line, never a dialog (spec §2.3).
fn offer(state: &Gracil) -> Element<'_, Message> {
    container(
        row![
            text(state.text.unfinished(state.offered.len()))
                .size(size::CAPTION)
                .style(style::muted),
            keyed(
                Scope::Main,
                button(text(state.text.carry_on).size(size::CAPTION))
                    .style(style::quiet_button)
                    .on_press(Message::ContinueUnfinished)
            ),
            keyed(
                Scope::Main,
                hint(
                    button(text("×").size(size::CAPTION))
                        .style(style::quiet_button)
                        .on_press(Message::LetGoUnfinished),
                    state.text.forget_unfinished,
                ),
            ),
        ]
        .spacing(space::S)
        .align_y(Center),
    )
    .center_x(Fill)
    .into()
}

/// For a few seconds after Clear: a quiet way to put the list back, never a question first.
fn undo(state: &Gracil) -> Element<'_, Message> {
    container(
        row![
            text(state.text.list_cleared)
                .size(size::CAPTION)
                .style(style::muted),
            keyed(
                Scope::Main,
                button(text(state.text.undo).size(size::CAPTION))
                    .style(style::quiet_button)
                    .on_press(Message::Undo)
            ),
        ]
        .spacing(space::S)
        .align_y(Center),
    )
    .center_x(Fill)
    .into()
}

/// "Always (Replace / Ask / Save copy)": what to do with every finished file, or, in the middle,
/// ask each time (ADR 0025, ADR 0060).
fn always(state: &Gracil) -> Element<'_, Message> {
    let selected = match state.always() {
        Some(Choice::Replace) => 0,
        None => 1,
        Some(Choice::SaveCopy) => 2,
    };
    let pill = pill(Scope::Main, state.text.always_choices, selected, |index| {
        Message::Always(match index {
            0 => Some(Choice::Replace),
            2 => Some(Choice::SaveCopy),
            _ => None,
        })
    });
    container(
        row![
            hint(
                text(state.text.always)
                    .size(size::CAPTION)
                    .style(style::muted),
                state.text.always_tip,
            ),
            pill,
        ]
        .spacing(space::S)
        .align_y(Center),
    )
    .center_x(Fill)
    .into()
}

/// Which view the window shows, as a number that changes when the view does, so the new one can
/// come in softly: the question before leaving, files dragged over, the finished view, a file, or
/// the empty window.
fn shown(state: &Gracil, hovering: bool) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    if state.confirming_exit {
        0.hash(&mut hasher);
    } else if hovering {
        1.hash(&mut hasher);
    } else if state.selected.is_none() && state.finished().is_some() {
        2.hash(&mut hasher);
    } else if let Some(job) = state.focused() {
        (3, job.id).hash(&mut hasher);
    } else {
        (4, state.gathering > 0).hash(&mut hasher);
    }
    hasher.finish()
}

fn focus(state: &Gracil, hovering: bool) -> Element<'_, Message> {
    if hovering {
        return text(state.text.release_to_add).size(size::TITLE).into();
    }
    // Once everything is done, a clear "done" instead of the last file (ADR 0039).
    if state.selected.is_none()
        && let Some(finished) = state.finished()
    {
        return finished_view(state, &finished);
    }
    match state.focused() {
        Some(job) => focused(state, job),
        None if state.gathering > 0 => text(state.text.looking).size(size::TITLE).into(),
        None => column![
            text(state.text.drop_files_here).size(size::TITLE),
            keyed(
                Scope::Main,
                button(text(state.text.or_choose_files).size(size::BODY))
                    .style(style::quiet_button)
                    .on_press(Message::ChooseFiles)
            ),
        ]
        .spacing(space::S)
        .align_x(Center)
        .into(),
    }
}

/// The whole list is done: the filled ring with a green check, how much room was saved, and how
/// the list went (ADR 0039).
fn finished_view<'a>(state: &'a Gracil, finished: &Finished) -> Element<'a, Message> {
    let done = Done {
        headline: state.text.headline(finished),
        // Files left unfinished, by Stop or last time, mean the list isn't all done: no check.
        went_well: finished.compressed > 0 && state.offered.is_empty(),
    };
    let mut how = row![].spacing(space::S).align_y(Center);
    let summary = state.text.summary(finished);
    if !summary.is_empty() {
        how = how.push(text(summary).size(size::BODY).style(style::muted));
    }
    // What failed leads to those files, with the reason under each.
    if finished.failed > 0 {
        how = how.push(keyed(
            Scope::Main,
            button(
                row![
                    text(state.text.failed(finished.failed)).size(size::BODY),
                    text("!").size(size::BODY).style(style::notice),
                ]
                .spacing(space::XS)
                .align_y(Center),
            )
            .style(style::quiet_button)
            .on_press(Message::ShowProblem),
        ));
    }
    column![
        Canvas::new(done)
            .width(Fill)
            .height(dial::SIZE + done::AMOUNT_ROOM),
        how,
        // The way to a new start: back to the empty window, ready for the next files.
        container(keyed(
            Scope::Main,
            button(text(state.text.clear_list).size(size::BODY))
                .padding([space::XS, space::M])
                .style(style::action_button)
                .on_press(Message::Clear)
        ),)
        .padding(Padding::default().top(space::M)),
    ]
    .spacing(space::XS)
    .align_x(Center)
    .max_width(360)
    .into()
}

/// The ring for `job`. While Gracil works through more than one file it stands for the whole list
/// (ADR 0034, item 13), unless the user picked another file in the list to look at.
fn dial(state: &Gracil, job: &Job) -> Dial {
    let picked_another = state
        .selected
        .is_some_and(|picked| state.current().is_some_and(|current| current.id != picked));
    let list = state.list_progress().filter(|_| !picked_another);
    let fraction = list.map_or(job.fraction, |list| Some(list.fraction));
    let preparing = match job.state {
        JobState::Preparing => true,
        JobState::Pending | JobState::Prepared => matches!(state.engine, EngineState::Starting),
        _ => false,
    };
    Dial {
        shows: list.map_or(Shows::File(job.id), |list| Shows::List(list.round)),
        fraction,
        paused: state.paused,
        // Pause and resume act on every file being worked on (ADR 0040); only putting a result
        // in place can't be held.
        can_pause: job.state != JobState::Finalizing,
        can_stop: true,
        stop_word: state.text.stop_all,
        // Before the ring starts filling, its middle breathes, so a long first moment never
        // looks stuck (ADR 0055).
        getting_ready: preparing && fraction.is_none_or(|fraction| fraction <= 0.0),
    }
}

fn focused<'a>(state: &'a Gracil, job: &'a Job) -> Element<'a, Message> {
    let ring = state.shows_dial(job).then(|| dial(state, job));
    // A ring for the whole list is titled with how many files it holds, not with one of them,
    // and its percentage is the whole list's.
    let list = ring
        .as_ref()
        .filter(|ring| matches!(ring.shows, Shows::List(_)))
        .and_then(|_| state.list_progress());
    let (title, shown): (Element<'a, Message>, Status) = match list {
        Some(list) => (
            text(state.text.compressing_files(list.files))
                .size(size::TITLE)
                .into(),
            match job.state {
                JobState::Compressing | JobState::Preparing => {
                    Status::plain(crate::text::progress(list.fraction))
                }
                _ => status(state, job),
            },
        ),
        None => (name(job, FOCUS_NAME_CHARS, size::TITLE), status(state, job)),
    };
    let mut content = column![title]
        .spacing(space::M)
        .align_x(Center)
        .max_width(320);

    if let Some(ring) = ring {
        content = content.push(ring.keyed(Scope::Main, dial::SIZE));
    }

    let mut details = column![status_line(&shown, size::BODY, Explain::Below)]
        .spacing(space::S)
        .align_x(Center);
    match state.outcomes.get(&job.id) {
        Some(Outcome::Ready {
            placing,
            extension,
            sizes,
            problem,
            ..
        }) => {
            details = details.push(choices(state, job, !placing, *problem));
            details = details.push(caption(state.text.change(*sizes)));
            if let Some(change) = format_change(job, extension) {
                details = details.push(caption(change));
            }
        }
        Some(Outcome::Placed {
            choice,
            path,
            sizes,
        }) => {
            details = details.push(caption(state.text.change(*sizes)));
            details = details.push(caption(placed_caption(state, job, *choice, path)));
        }
        _ => {}
    }
    content = content.push(details);
    // Looking at another file never hides the work: one quiet line leads back to it.
    if let Some(current) = state.way_back() {
        let label = format!(
            "‹  {}  ·  {}",
            shorten(&current.file_name(), DRAWER_NAME_CHARS),
            status(state, current).label
        );
        content = content.push(keyed(
            Scope::Main,
            button(text(label).size(size::CAPTION).wrapping(Wrapping::None))
                .style(style::quiet_button)
                .on_press(Message::ShowCurrent),
        ));
    }
    content.into()
}

/// The mini-window next to the tray icon (spec §55): the video being worked on, with its dial
/// to pause it, and a way into Gracil. Videos can be dropped on it too.
fn mini(state: &Gracil, window: window::Id) -> Element<'_, Message> {
    let hovering = state.hovering == Some(window);
    let body: Element<'_, Message> = if hovering {
        text(state.text.release_to_add).size(size::BODY).into()
    } else {
        match state.mini_job() {
            Some(job) => mini_job(state, job),
            None if state.gathering > 0 => text(state.text.looking).size(size::BODY).into(),
            None => text(state.text.drop_files_here).size(size::BODY).into(),
        }
    };
    let side = column![
        keyed(
            Scope::Mini,
            button(text("×").size(size::BODY))
                .style(style::quiet_button)
                .on_press(Message::CloseMini)
        ),
        Space::new().height(Fill),
        keyed(
            Scope::Mini,
            button(text(state.text.open_gracil).size(size::CAPTION))
                .style(style::quiet_button)
                .on_press(Message::OpenMain)
        ),
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
        status_line(&status(state, job), size::CAPTION, Explain::Link(job.id)),
    ]
    .spacing(space::XS);
    match state.outcomes.get(&job.id) {
        // The choice is made right here, without opening the big window.
        Some(Outcome::Ready {
            placing, problem, ..
        }) => {
            details = details.push(mini_choices(state, job, !placing, *problem));
        }
        Some(Outcome::Placed { choice, path, .. }) => {
            details = details.push(caption(placed_caption(state, job, *choice, path)));
        }
        _ => {}
    }
    // Results that finish while another video compresses come up here, one by one, after it.
    let shown_waiting = state.is_waiting(job.id);
    let mut later = Vec::new();
    match state.waiting_count() - usize::from(shown_waiting) {
        0 => {}
        n => later.push(state.text.ready(n)),
    }
    match state.waiting_to_compress() {
        0 => {}
        n => later.push(state.text.in_line(n)),
    }
    if !later.is_empty() {
        details = details.push(caption(later.join("  ·  ")));
    }
    let mut line = row![].spacing(space::M).align_y(Center);
    if state.shows_dial(job) {
        // Stopping is for the big window, where it is clear what stops (spec §55).
        let ring = Dial {
            can_stop: false,
            ..dial(state, job)
        };
        line = line.push(ring.keyed(Scope::Mini, dial::SMALL));
    }
    line.push(details).into()
}

/// Exit asked while work is in progress: what would stop, in so many words, and the two ways out
/// (spec §54.1). Shown in the window itself, never as a dialog (spec §2.3).
fn exit_question(state: &Gracil) -> Element<'_, Message> {
    let explanation = state.text.exit_explanation(
        state.being_worked_on(),
        state.waiting_to_compress(),
        state.waiting_count(),
    );
    let question = if state.preparing_to_delete {
        state.text.prepare_question
    } else {
        state.text.exit_question
    };
    column![
        text(question).size(size::TITLE),
        text(explanation)
            .size(size::BODY)
            .style(style::muted)
            .align_x(Center),
        // Carrying on is the safe answer, so it is the one in the accent, first, as in Windows'
        // own dialogs; leaving is still one click away.
        row![
            keyed(
                Scope::Main,
                button(text(state.text.keep_working).size(size::BODY))
                    .padding([space::XS, space::M])
                    .style(style::action_button)
                    .on_press(Message::KeepWorking)
            ),
            keyed(
                Scope::Main,
                button(text(state.text.exit).size(size::BODY))
                    .padding([space::XS, space::M])
                    .style(style::quiet_button)
                    .on_press(Message::ConfirmExit)
            ),
        ]
        .spacing(space::S),
    ]
    .spacing(space::M)
    .align_x(Center)
    .max_width(320)
    .into()
}

/// Whether `choice` can be made now: not while a result is being put in place, and not Replace
/// again once another file turned out to have the new name, since it would fail the same way.
fn can_choose(enabled: bool, problem: Option<Failure>, choice: Choice) -> bool {
    enabled && !(choice == Choice::Replace && problem == Some(Failure::NameTaken))
}

/// Replace and Save copy, the two things to do with a finished result (ADR 0009).
fn choices(
    state: &Gracil,
    job: &Job,
    enabled: bool,
    problem: Option<Failure>,
) -> Element<'static, Message> {
    let action = |label: &'static str, choice: Choice| {
        let can = can_choose(enabled, problem, choice);
        keyed_if(
            Scope::Main,
            can,
            button(text(label).size(size::BODY))
                .padding([space::XS, space::M])
                .style(style::action_button)
                .on_press_maybe(can.then_some(Message::Choose(job.id, choice))),
        )
    };
    row![
        action(state.text.replace, Choice::Replace),
        action(state.text.save_copy, Choice::SaveCopy)
    ]
    .spacing(space::S)
    .into()
}

/// The same two choices, smaller, for the mini-window.
fn mini_choices(
    state: &Gracil,
    job: &Job,
    enabled: bool,
    problem: Option<Failure>,
) -> Element<'static, Message> {
    let action = |label: &'static str, choice: Choice| {
        let can = can_choose(enabled, problem, choice);
        keyed_if(
            Scope::Mini,
            can,
            button(text(label).size(size::CAPTION))
                // Five pixels above and below make it at least 24 pixels tall, easy to hit.
                .padding([5.0, space::S])
                .style(style::action_button)
                .on_press_maybe(can.then_some(Message::Choose(job.id, choice))),
        )
    };
    row![
        action(state.text.replace, Choice::Replace),
        action(state.text.save_copy, Choice::SaveCopy)
    ]
    .spacing(space::XS)
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

/// Every video becomes an MP4 (ADR 0012), and every image other than a PNG a JPEG, or a PNG when
/// it has transparency (ADR 0034). When that changes the name, say so before the user
/// chooses, in the plainest words there are: `IMG_0042.MOV → IMG_0042.mp4`.
fn format_change(job: &Job, output: &OsStr) -> Option<String> {
    let source = job.path.extension();
    if source == Some(output) {
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

/// How far along, in whole percent, rounded down: 100% only once it is truly done.
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
        JobState::Pending | JobState::Preparing | JobState::Prepared if state.paused => {
            Status::plain(state.text.paused)
        }
        JobState::Pending | JobState::Prepared => match state.engine {
            EngineState::Starting => Status::plain(state.text.getting_ready),
            EngineState::Ready(_) => Status::plain(state.text.waiting),
            EngineState::Unavailable => Status {
                label: state.text.cannot_start.into(),
                notice: Some(state.text.ffmpeg_missing),
            },
        },
        JobState::Preparing => Status::plain(state.text.getting_ready),
        JobState::Compressing => match job.fraction {
            Some(fraction) => Status::plain(crate::text::progress(fraction)),
            None => Status::plain(state.text.compressing),
        },
        JobState::Paused => Status::plain(state.text.paused),
        JobState::Finalizing => Status::plain(state.text.saving),
        JobState::Completed | JobState::Failed | JobState::Cancelled => {
            match state.outcomes.get(&job.id) {
                Some(Outcome::Ready { placing: true, .. }) => Status::plain(state.text.saving),
                Some(Outcome::Ready { sizes, problem, .. }) => Status {
                    label: state.text.smaller(sizes.smaller_by()),
                    notice: problem.map(|failure| state.text.failure(failure)),
                },
                Some(Outcome::Placed { sizes, .. }) => {
                    Status::plain(state.text.smaller(sizes.smaller_by()))
                }
                // Either way the file is left as it was, in the same words (ADR 0055).
                Some(Outcome::Failed(
                    failure @ (Failure::NotSmaller | Failure::AsSmallAsItGets),
                )) => Status {
                    label: state.text.already_compressed.into(),
                    notice: Some(state.text.failure(*failure)),
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

/// The drawer says what happened to a result in a few words, since the focus shows the details:
/// how much smaller, and where it went (`−35%  ·  Replaced`).
fn drawer_status(state: &Gracil, job: &Job) -> Status {
    match state.outcomes.get(&job.id) {
        Some(Outcome::Placed { choice, sizes, .. }) => {
            let placed = match choice {
                Choice::Replace => state.text.replaced,
                Choice::SaveCopy => state.text.copy_saved,
            };
            Status::plain(format!(
                "{}  ·  {placed}",
                state.text.shrunk(sizes.smaller_by())
            ))
        }
        _ => status(state, job),
    }
}

/// Where a status's explanation goes.
enum Explain {
    /// In words under the status: the file shown large.
    Below,
    /// Behind the "!", which opens the big window on the file: the mini-window, too small for it.
    Link(JobId),
    /// Behind the "!", on hover: the list, where picking the row shows it large.
    Hover,
}

fn status_line(status: &Status, text_size: f32, explain: Explain) -> Element<'static, Message> {
    let label = text(status.label.clone())
        .size(text_size)
        .style(style::muted)
        .wrapping(Wrapping::None);
    let Some(explanation) = status.notice else {
        return label.into();
    };
    let mark = || text("!").size(size::BODY).style(style::notice);
    match explain {
        Explain::Below => column![
            row![label, mark()].spacing(space::S).align_y(Center),
            text(explanation)
                .size(size::CAPTION)
                .style(style::muted)
                .align_x(Center),
        ]
        .spacing(space::XS)
        .align_x(Center)
        .into(),
        Explain::Link(id) => row![
            label,
            keyed(
                Scope::Mini,
                hint(
                    button(mark())
                        .padding([0.0, space::XS])
                        .style(style::quiet_button)
                        .on_press(Message::Explain(id)),
                    explanation,
                ),
            ),
        ]
        .spacing(space::XS)
        .align_y(Center)
        .into(),
        Explain::Hover => row![label, hint(mark(), explanation)]
            .spacing(space::S)
            .align_y(Center)
            .into(),
    }
}

fn name(job: &Job, max_chars: usize, text_size: f32) -> Element<'static, Message> {
    text(shorten(&job.file_name(), max_chars))
        .size(text_size)
        .wrapping(Wrapping::None)
        .into()
}

fn footer(state: &Gracil) -> Element<'_, Message> {
    let mut summary = state.text.files_left(state.left());
    // "More" to decide: besides the result already shown large, as in the mini-window.
    let shown_waiting = state.focused().is_some_and(|job| state.is_waiting(job.id));
    match state.waiting_count() - usize::from(shown_waiting) {
        0 => {}
        n => summary.push_str(&format!("  ·  {}", state.text.ready(n))),
    }
    // The room freed so far, once there is more than one result to add up (spec §32.2).
    let (total, results) = state.total();
    if results >= 2 {
        summary.push_str(&format!("  ·  {}", state.text.less(total)));
    }
    if state.gathering > 0 {
        summary.push_str(&format!("  ·  {}", state.text.looking_for_more));
    }
    let toggle = if state.drawer_open {
        state.text.hide_files
    } else {
        state.text.view_files
    };
    // The summary takes what room is left after "+", and wraps onto a second line rather than
    // pushing "+" out of the window.
    let add = keyed(
        Scope::Main,
        hint(
            button(text("+").size(size::TITLE).line_height(1.0))
                .style(style::quiet_button)
                .on_press(Message::ChooseFiles),
            state.text.add_files,
        ),
    );
    // Once all is done, the big window already says how it went; the line below only opens the
    // list, and not even that while files failed: what failed already leads there.
    let line = match state.finished() {
        Some(finished) if state.selected.is_none() => {
            (finished.failed == 0 || state.drawer_open).then(|| toggle.to_owned())
        }
        _ => Some(format!("{summary}  ·  {toggle}")),
    };
    let line: Element<'_, Message> = match line {
        Some(line) => keyed(
            Scope::Main,
            button(text(line).size(size::CAPTION))
                .style(style::quiet_button)
                .width(Fill)
                .on_press(Message::ToggleDrawer),
        ),
        None => Space::new().width(Fill).into(),
    };
    row![line, add].spacing(space::S).align_y(Center).into()
}

fn drawer(state: &Gracil) -> Element<'_, Message> {
    let quiet = |label: &'static str, message: Option<Message>| {
        keyed_if(
            Scope::Main,
            message.is_some(),
            button(text(label).size(size::CAPTION))
                .style(style::quiet_button)
                .on_press_maybe(message),
        )
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

    let (total, done) = state.total();
    let in_all = (done >= 2).then(|| caption(state.text.in_all(total)));

    let jobs = state.queue.jobs();
    let shown = state.drawer_scroll.rows(jobs.len());
    let above = shown.start as f32 * ROW_HEIGHT;
    let below = (jobs.len() - shown.end) as f32 * ROW_HEIGHT;
    let rows = jobs[shown].iter().map(|job| {
        let line = row![
            container(name(job, DRAWER_NAME_CHARS, size::BODY)).width(Fill),
            status_line(&drawer_status(state, job), size::CAPTION, Explain::Hover),
        ]
        .spacing(space::M)
        .align_y(Center);
        let remove = quiet(
            "×",
            state.can_remove(job).then_some(Message::Remove(job.id)),
        );
        row![
            keyed(
                Scope::Main,
                button(line)
                    .padding([space::XS, space::S])
                    .width(Fill)
                    .style(if state.selected == Some(job.id) {
                        style::picked_row
                    } else {
                        style::quiet_button
                    })
                    .on_press(Message::Select(job.id))
            ),
            remove,
        ]
        .align_y(Center)
        .height(ROW_HEIGHT)
        .into()
    });

    let mut content = column![header].spacing(space::XS).padding(space::S);
    if let Some(line) = in_all {
        content = content.push(container(line).padding([0.0, space::S]));
    }
    content = content.push(
        scrollable(
            column![Space::new().height(above)]
                .extend(rows)
                .push(Space::new().height(below)),
        )
        .id(DRAWER_LIST)
        .spacing(space::XS)
        .height(iced::Shrink)
        .on_scroll(|viewport| {
            Message::DrawerScrolled(DrawerScroll {
                offset: viewport.absolute_offset().y,
                height: viewport.bounds().height,
            })
        }),
    );
    container(content)
        .max_height(DRAWER_MAX_HEIGHT)
        .width(Fill)
        .style(style::drawer)
        .into()
}

/// `content`, saying a few words about itself on hover.
fn hint<'a>(content: impl Into<Element<'a, Message>>, words: &'static str) -> Element<'a, Message> {
    tooltip(
        content,
        container(text(words).size(size::CAPTION))
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
    fn only_rows_in_sight_are_built() {
        let top = DrawerScroll::default();
        assert_eq!(top.rows(0), 0..0);
        assert_eq!(top.rows(3), 0..3);
        // 148 pixels show five rows of 30, plus four beyond the bottom edge.
        assert_eq!(top.rows(15_031), 0..9);
        let further = DrawerScroll {
            offset: 300.0 * ROW_HEIGHT + 10.0,
            height: 148.0,
        };
        assert_eq!(further.rows(15_031), 296..310);
        let end = DrawerScroll {
            offset: 15_026.0 * ROW_HEIGHT,
            height: 148.0,
        };
        assert_eq!(end.rows(15_031), 15_022..15_031);
    }

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
