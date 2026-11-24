//! Focus + Drawer (spec §31–§32): one file in focus, with the ring that follows it from its first
//! moment to its last (ADR 0076), and everything else one quiet click away.

use std::ffi::OsStr;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;

use gracil_core::job::JobState;
use gracil_core::queue::{Job, JobId};
use iced::advanced::graphics::text::Paragraph;
use iced::advanced::text::{self as plain, Paragraph as _};
use iced::alignment::Vertical;
use iced::widget::operation::{self, AbsoluteOffset};
use iced::widget::text::Wrapping;
use iced::widget::{Id, Space, button, column, container, row, scrollable, text, tooltip};
use iced::{Center, Color, Element, Fill, Padding, Size, Task, Theme, window};

use crate::app::{EngineState, Finished, Gracil, Message};
use crate::appear::appear;
use crate::card::card;
use crate::carry::carry;
use crate::dial::{self, Controls, Dial, Face, Shows};
use crate::focus::{Scope, keyed, keyed_if};
use crate::pill::pill;
use crate::reveal::reveal;
use crate::style::{self, Surface, size, space};
use crate::work::{Choice, Failure, Outcome};

/// Longest file name shown before it is shortened in the middle.
const LARGE_NAME_CHARS: usize = 28;
const DRAWER_NAME_CHARS: usize = 26;
/// The mini-window is narrower than the main window's small focus.
const MINI_NAME_CHARS: usize = 22;
const TOOLTIP_WIDTH: f32 = 240.0;
/// The height of every row in the list. All rows are the same height, so only the rows in sight
/// are built, however long the list is (spec §77.3).
const ROW_HEIGHT: f32 = 30.0;
/// Rows built beyond each edge of what is in sight, so scrolling never shows a gap.
const ROWS_BEYOND: usize = 4;
/// The list, so it can be scrolled to a file.
const DRAWER_LIST: Id = Id::new("drawer-list");

/// The heights of the window's lines, in logical pixels: the view shares out the window's room
/// from them, so each is held to its own.
const FOOTER: f32 = 28.0;
const ALWAYS: f32 = 26.0;
const NOTE: f32 = 26.0;
const DRAWER_HEADER: f32 = 26.0;
const DRAWER_SUM: f32 = 16.0;

/// The large focus (ADR 0076): the ring in the middle, a line above it for what it stands for,
/// and room below for how it goes and what to do next. Both are held to their heights, so the
/// ring stays where it is while the words around it change.
const ABOVE_RING: f32 = 30.0;
const BELOW_RING: f32 = 88.0;
const LARGE: f32 = ABOVE_RING + space::M + dial::SIZE + space::M + BELOW_RING;
/// What the focus keeps clear above and below what it shows.
const FOCUS_EDGE: f32 = space::S;
/// The small focus: the ring beside the words, as in the mini-window. It is what shows while the
/// window has no room for the large one, as it hasn't while the list is open.
const SMALL: f32 = dial::SMALL + 2.0 * space::M;
/// The least of the list that shows, however small the window: its header and two files.
const DRAWER_LEAST: f32 = 2.0 * space::S + DRAWER_HEADER + space::XS + 2.0 * ROW_HEIGHT;

/// The main window's size when it opens, in logical pixels: room for the large focus, with its
/// ring in the middle.
pub const WINDOW: Size = Size::new(440.0, 480.0);

/// How far down the list is scrolled and how tall its visible part is, in logical pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DrawerScroll {
    pub offset: f32,
    pub height: f32,
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

    /// The rows of a list of `len` that are in sight, or nearly, in a list `height` tall: the
    /// list tells its height only once it is scrolled, and may have grown with the window since.
    fn rows(self, len: usize, height: f32) -> Range<usize> {
        let height = self.height.max(height).max(0.0);
        let first = (self.offset.max(0.0) / ROW_HEIGHT) as usize;
        let last = ((self.offset.max(0.0) + height) / ROW_HEIGHT).ceil() as usize;
        let start = first.saturating_sub(ROWS_BEYOND).min(len);
        start..(last + ROWS_BEYOND).clamp(start, len)
    }
}

/// How the main window's room is shared out (ADR 0076).
struct Room {
    /// How tall the focus is, once the list has finished opening or closing.
    focus: f32,
    /// How tall the list is while open.
    drawer: f32,
}

impl Room {
    fn of(state: &Gracil) -> Self {
        let listed = !state.queue.is_empty();
        let lines = [
            (true, ALWAYS),
            (listed, FOOTER),
            (!state.offered.is_empty(), NOTE),
            (state.cleared.is_some(), NOTE),
        ];
        let lines: f32 = lines
            .iter()
            .filter(|(shown, _)| *shown)
            .map(|(_, height)| height + space::M)
            .sum();
        let inside = state.main_size.height - 2.0 * space::M - lines;
        // An open list takes all the room but the small focus's, however many files it holds, so
        // the window is laid out the same way every time it opens.
        let drawer = (inside - SMALL - space::M).max(DRAWER_LEAST);
        let open = listed && state.drawer_open;
        let focus = inside - if open { drawer + space::M } else { 0.0 };
        Self { focus, drawer }
    }

    /// Whether the focus has room for the ring in its middle.
    fn large(&self) -> bool {
        self.focus >= LARGE + 2.0 * FOCUS_EDGE
    }

    /// How far down the large focus starts, so the ring is in the very middle of the focus while
    /// there is room: the words below it take more room than the line above.
    fn lift(&self) -> f32 {
        (self.focus - 2.0 * FOCUS_EDGE - LARGE).clamp(0.0, BELOW_RING - ABOVE_RING)
    }
}

pub fn view(state: &Gracil, window: window::Id) -> Element<'_, Message> {
    if state.is_mini(window) {
        // Kept hidden between openings, it draws nothing, so nothing in it moves.
        if !state.mini_is_open() {
            return column![].into();
        }
        return mini(state, window);
    }
    let room = Room::of(state);
    // The question before leaving takes the focus, and files dragged over the window light it.
    let asking = state.confirming_exit;
    let lit = state.hovering == Some(window) && !asking;
    let large = room.large();
    let shows = if asking {
        exit_question(state)
    } else {
        focus(state, lit, &room)
    };
    let shows = appear(shown(state, lit, large), face(style::card, lit), shows);
    let focus = card(
        style::card,
        lit,
        if large || asking || lit || state.focused().is_none() {
            container(shows)
                .center(Fill)
                .padding([FOCUS_EDGE, space::M])
        } else {
            container(shows)
                .center_y(Fill)
                .width(Fill)
                .padding([FOCUS_EDGE, space::M])
        },
    );

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
                drawer(state, room.drawer),
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

/// The color of `surface`, lit or not: what a view coming in on it fades in from.
fn face(surface: Surface, lit: bool) -> impl Fn(&Theme) -> Color + Copy {
    move |theme| surface(theme, f32::from(lit)).0
}

/// A number that changes when what it is made `of` does.
fn key(of: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    of.hash(&mut hasher);
    hasher.finish()
}

/// Files left unfinished the last time Gracil closed, and a way to carry on with them or let them
/// go (ADR 0042). A quiet line, never a dialog (spec §2.3).
fn offer(state: &Gracil) -> Element<'_, Message> {
    note(row![
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
    ])
}

/// For a few seconds after Clear: a quiet way to put the list back, never a question first.
fn undo(state: &Gracil) -> Element<'_, Message> {
    note(row![
        text(state.text.list_cleared)
            .size(size::CAPTION)
            .style(style::muted),
        keyed(
            Scope::Main,
            button(text(state.text.undo).size(size::CAPTION))
                .style(style::quiet_button)
                .on_press(Message::Undo)
        ),
    ])
}

/// A quiet line of its own between the focus and the foot of the window.
fn note<'a>(line: iced::widget::Row<'a, Message>) -> Element<'a, Message> {
    container(line.spacing(space::S).align_y(Center))
        .center_x(Fill)
        .center_y(NOTE)
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
    .center_y(ALWAYS)
    .into()
}

/// Which view the focus shows, as a number that changes when the view does, so the new one can
/// come in softly: the question before leaving, files dragged over, the empty window, a file
/// without a ring, or the ring, which stays through everything it shows (ADR 0076): then only
/// the words around it change, each coming in softly by itself.
fn shown(state: &Gracil, lit: bool, large: bool) -> u64 {
    if state.confirming_exit {
        key(0)
    } else if lit {
        key(1)
    } else if state.selected.is_none() && state.finished().is_some() {
        key((2, large))
    } else if let Some(job) = state.focused() {
        match ring(state, job, true) {
            Some(_) => key((2, large)),
            None => key((3, job.id, large)),
        }
    } else {
        key((4, state.gathering > 0))
    }
}

fn focus<'a>(state: &'a Gracil, lit: bool, room: &Room) -> Element<'a, Message> {
    if lit {
        return text(state.text.release_to_add).size(size::TITLE).into();
    }
    let surface = face(style::card, false);
    // Once everything is done, a clear "done" instead of the last file (ADR 0039).
    if state.selected.is_none()
        && let Some(finished) = state.finished()
    {
        return if room.large() {
            large(done_scene(state, &finished), surface, room.lift())
        } else {
            small_done(state, &finished, Scope::Main, surface)
        };
    }
    match state.focused() {
        Some(job) if !room.large() => small_file(state, job, Scope::Main, surface),
        Some(job) => match ring(state, job, true) {
            Some(ring) => large(file_scene(state, job, ring), surface, room.lift()),
            None => ringless(state, job),
        },
        None if state.gathering > 0 => text(state.text.looking).size(size::TITLE).into(),
        None => {
            let mut empty = column![
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
            .max_width(320);
            if let Some(notice) = cannot_work(state) {
                empty = empty.push(
                    text(notice)
                        .size(size::CAPTION)
                        .style(style::muted)
                        .align_x(Center),
                );
            }
            empty.into()
        }
    }
}

/// Why nothing could be compressed now, said from the start in the empty window rather than only
/// once files fail (ADR 0066): Gracil was opened inside its zip, or part of it is missing.
fn cannot_work(state: &Gracil) -> Option<&'static str> {
    if crate::place::inside_zip() {
        Some(state.text.inside_zip)
    } else if matches!(state.engine, EngineState::Unavailable) {
        Some(state.text.ffmpeg_missing)
    } else {
        None
    }
}

/// The ring for `job`, when it has one: at work, or done and smaller, with how much in its
/// middle. While Gracil works through more than one file it stands for the whole list (ADR 0034,
/// item 13), unless the user picked another file in the list to look at.
fn ring(state: &Gracil, job: &Job, can_stop: bool) -> Option<Dial> {
    if !state.shows_dial(job) {
        let sizes = state.outcomes.get(&job.id)?.sizes()?;
        return Some(Dial {
            shows: Shows::File(job.id),
            fraction: None,
            face: Face::Saved(state.text.shrunk(sizes.smaller_by())),
        });
    }
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
    Some(Dial {
        shows: list.map_or(Shows::File(job.id), |list| Shows::List(list.round)),
        fraction,
        face: Face::Working(Controls {
            paused: state.paused,
            // Pause and resume act on every file being worked on (ADR 0040); only putting a
            // result in place can't be held.
            can_pause: job.state != JobState::Finalizing,
            can_stop,
            stop_word: state.text.stop_all,
            // Before the ring starts filling, its marble breathes, so a long first moment never
            // looks stuck (ADR 0055).
            getting_ready: preparing && fraction.is_none_or(|fraction| fraction <= 0.0),
            resting: state.is_resting(),
        }),
    })
}

/// The ring once the whole list is done.
fn done_ring(state: &Gracil, finished: &Finished) -> Dial {
    Dial {
        shows: Shows::Finished,
        fraction: None,
        // Files Stop set aside mean the list isn't all done: no check.
        face: Face::Done {
            went_well: state.went_well(finished),
        },
    }
}

/// What the ring stands for, and how it goes, for the file in focus: a ring for the whole list
/// is named after how many files it holds, not after one of them, and its percentage is the
/// whole list's. With a number that changes when the name does.
fn heading<'a>(
    state: &'a Gracil,
    job: &'a Job,
    ring: Option<&Dial>,
    name_chars: usize,
    name_size: f32,
) -> (Element<'a, Message>, u64, Status) {
    let list = ring
        .filter(|ring| matches!(ring.shows, Shows::List(_)))
        .and_then(|_| state.list_progress());
    match list {
        Some(list) => (
            text(state.text.compressing_files(list.files))
                .size(name_size)
                .wrapping(Wrapping::None)
                .into(),
            key((0, list.round)),
            match job.state {
                JobState::Compressing | JobState::Preparing
                    if !state.waiting_for_battery(job.state) =>
                {
                    Status::plain(crate::text::progress(list.fraction))
                }
                _ => status(state, job),
            },
        ),
        // The whole path on hover, for a name that is shortened or one of several alike.
        None => (
            hint_owned(
                name(job, name_chars, name_size),
                job.path.display().to_string(),
            ),
            key((1, job.id)),
            detail(state, job),
        ),
    }
}

/// What the large focus shows around its ring.
struct Scene<'a> {
    above: Element<'a, Message>,
    ring: Dial,
    /// What pressing the ring and moving it carries out of the window, once it is done.
    carries: Option<Message>,
    below: Element<'a, Message>,
    /// Numbers that change when what is above and below the ring does.
    keys: (u64, u64),
}

/// The large focus: `scene` laid out around its ring, `lift` pixels down. The ring is the same
/// drawing whatever the scene, so it turns from one into the next instead of starting again,
/// and only the words around it come in anew.
fn large<'a>(
    scene: Scene<'a>,
    surface: impl Fn(&Theme) -> Color + Copy + 'a,
    lift: f32,
) -> Element<'a, Message> {
    column![
        Space::new().height(lift),
        container(appear(scene.keys.0, surface, scene.above))
            .center_x(Fill)
            .align_bottom(ABOVE_RING),
        Space::new().height(space::M),
        carry(scene.ring.keyed(Scope::Main, dial::SIZE), scene.carries),
        Space::new().height(space::M),
        container(appear(scene.keys.1, surface, scene.below))
            .center_x(Fill)
            .align_top(BELOW_RING),
    ]
    .align_x(Center)
    .max_width(360)
    .into()
}

/// A file with its ring: its name above, and below how it goes or what to do with it.
fn file_scene<'a>(state: &'a Gracil, job: &'a Job, ring: Dial) -> Scene<'a> {
    let (above, above_key, shown) = heading(state, job, Some(&ring), LARGE_NAME_CHARS, size::TITLE);
    let mut below = column![status_line(&shown, size::BODY, Explain::Below)]
        .spacing(space::S)
        .align_x(Center);
    let outcome = state.outcomes.get(&job.id);
    match outcome {
        Some(Outcome::Ready {
            placing,
            extension,
            problem,
            ..
        }) => {
            below = below.push(choices(state, job, Scope::Main, !placing, *problem));
            if let Some(change) = format_change(job, extension) {
                below = below.push(caption(change));
            }
        }
        Some(Outcome::Placed { choice, path, .. }) => {
            below = below.push(caption(placed_caption(state, job, *choice, path)));
        }
        _ => {}
    }
    if let Some(back) = way_back(state) {
        below = below.push(back);
    }
    let stage = match outcome {
        Some(Outcome::Ready { .. }) => 1,
        Some(Outcome::Placed { .. }) => 2,
        _ => 0,
    };
    Scene {
        above,
        below: below.into(),
        keys: (above_key, key((stage, ring.shows))),
        ring,
        carries: carries(state, Scope::Main, job),
    }
}

/// The whole list is done: how much room was saved above the ring with its green check, and
/// below how the list went and the way to a new start (ADR 0039).
fn done_scene<'a>(state: &'a Gracil, finished: &Finished) -> Scene<'a> {
    let headline = state.text.headline(finished);
    let width = (state.main_size.width - 4.0 * space::M).min(360.0);
    let above = text(headline.clone())
        .size(fitting(&headline, width))
        .wrapping(Wrapping::None);
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
            .padding(0)
            .style(style::quiet_button)
            .on_press(Message::ShowProblem),
        ));
    }
    let mut below = column![how].spacing(space::XS).align_x(Center);
    if let Some((failure, shared_by_all)) = state.common_fault() {
        below = below.push(
            text(state.text.fault_reason(failure, shared_by_all))
                .size(size::CAPTION)
                .style(style::muted)
                .align_x(Center),
        );
    }
    // The way to a new start: back to the empty window, ready for the next files.
    below = below.push(
        container(keyed(
            Scope::Main,
            button(text(state.text.clear_list).size(size::BODY))
                .padding([space::XS, space::M])
                .style(style::action_button)
                .on_press(Message::Clear),
        ))
        .padding(Padding::default().top(space::S)),
    );
    Scene {
        above: above.into(),
        ring: done_ring(state, finished),
        carries: carries_all(state, Scope::Main),
        below: below.into(),
        keys: (key((2, &headline)), key(2)),
    }
}

/// The headline's size: a title's, or a little smaller when a longer line, in a language with
/// longer words, would otherwise not fit on one line in `width`.
fn fitting(headline: &str, width: f32) -> f32 {
    let wide = measure(headline, size::TITLE);
    if wide <= width {
        size::TITLE
    } else {
        (size::TITLE * width / wide).max(size::BODY)
    }
}

/// How wide `words` are on one line at `text_size`, in logical pixels.
fn measure(words: &str, text_size: f32) -> f32 {
    Paragraph::with_text(plain::Text {
        content: words,
        bounds: Size::INFINITE,
        size: text_size.into(),
        line_height: plain::LineHeight::default(),
        font: style::FONT,
        align_x: plain::Alignment::Default,
        align_y: Vertical::Top,
        shaping: plain::Shaping::default(),
        wrapping: plain::Wrapping::None,
    })
    .min_width()
}

/// Carrying a file that is done out of the window, where it was put (ADR 0083).
fn carries(state: &Gracil, scope: Scope, job: &Job) -> Option<Message> {
    match state.outcomes.get(&job.id) {
        Some(Outcome::Placed { .. }) => Some(Message::Carry(scope, Some(job.id))),
        _ => None,
    }
}

/// Carrying every file of the finished list that was put in place.
fn carries_all(state: &Gracil, scope: Scope) -> Option<Message> {
    let any = state
        .outcomes
        .values()
        .any(|outcome| matches!(outcome, Outcome::Placed { .. }));
    any.then_some(Message::Carry(scope, None))
}

/// A file without a ring, shown large: one that waits, was stopped or couldn't be compressed.
fn ringless<'a>(state: &'a Gracil, job: &'a Job) -> Element<'a, Message> {
    let (above, _, shown) = heading(state, job, None, LARGE_NAME_CHARS, size::TITLE);
    let mut content = column![above, status_line(&shown, size::BODY, Explain::Below)]
        .spacing(space::M)
        .align_x(Center)
        .max_width(320);
    if let Some(back) = way_back(state) {
        content = content.push(back);
    }
    content.into()
}

/// Looking at another file never hides the work: one quiet line leads back to it (spec §32.1).
fn way_back(state: &Gracil) -> Option<Element<'_, Message>> {
    let current = state.way_back()?;
    let label = format!(
        "‹  {}  ·  {}",
        shorten(&current.file_name(), DRAWER_NAME_CHARS),
        status(state, current).label
    );
    Some(keyed(
        Scope::Main,
        // No room of its own at its sides, so it lines up with the words above it.
        button(text(label).size(size::CAPTION).wrapping(Wrapping::None))
            .padding([2.0, 0.0])
            .style(style::quiet_button)
            .on_press(Message::ShowCurrent),
    ))
}

/// The small focus on a file: its ring, when it has one, beside its name, how it goes, and what
/// to do with it. The mini-window shows the same, with what waits after it.
fn small_file<'a>(
    state: &'a Gracil,
    job: &'a Job,
    scope: Scope,
    surface: impl Fn(&Theme) -> Color + 'a,
) -> Element<'a, Message> {
    let mini = scope == Scope::Mini;
    // Stopping is for the main window, where it is clear what stops (spec §55).
    let ring = ring(state, job, !mini);
    let name_chars = if mini {
        MINI_NAME_CHARS
    } else {
        LARGE_NAME_CHARS
    };
    let (name, name_key, shown) = heading(state, job, ring.as_ref(), name_chars, size::BODY);
    let explain = if mini {
        Explain::Link(job.id)
    } else {
        Explain::Hover
    };
    let mut details = column![name, status_line(&shown, size::CAPTION, explain)].spacing(space::XS);
    let outcome = state.outcomes.get(&job.id);
    match outcome {
        // The choice is made right here.
        Some(Outcome::Ready {
            placing, problem, ..
        }) => {
            details = details.push(choices(state, job, scope, !placing, *problem));
        }
        Some(Outcome::Placed { choice, path, .. }) => {
            details = details.push(caption(placed_caption(state, job, *choice, path)));
        }
        _ => {}
    }
    if mini {
        // Results that finish while another file compresses come up here, one by one, after it.
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
    } else if let Some(back) = way_back(state) {
        details = details.push(back);
    }
    let stage = match outcome {
        Some(Outcome::Ready { .. }) => 1,
        Some(Outcome::Placed { .. }) => 2,
        _ => 0,
    };
    let carries = carries(state, scope, job);
    small(
        ring,
        carries,
        scope,
        key((name_key, stage)),
        surface,
        details.into(),
    )
}

/// The small focus once the whole list is done (ADR 0066): the ring with its check, how much room
/// was saved, how the list went, and what failed, whose "!" leads to the first of those files.
fn small_done<'a>(
    state: &'a Gracil,
    finished: &Finished,
    scope: Scope,
    surface: impl Fn(&Theme) -> Color + 'a,
) -> Element<'a, Message> {
    let mut details =
        column![text(state.text.headline(finished)).size(size::BODY)].spacing(space::XS);
    let summary = state.text.summary(finished);
    if !summary.is_empty() {
        details = details.push(text(summary).size(size::CAPTION).style(style::muted));
    }
    if finished.failed > 0 {
        details = details.push(
            row![
                text(state.text.failed(finished.failed))
                    .size(size::CAPTION)
                    .style(style::muted),
                keyed(
                    scope,
                    button(text("!").size(size::BODY).style(style::notice))
                        .padding([0.0, space::XS])
                        .style(style::quiet_button)
                        .on_press(if scope == Scope::Mini {
                            Message::OpenProblem
                        } else {
                            Message::ShowProblem
                        }),
                ),
            ]
            .spacing(space::XS)
            .align_y(Center),
        );
    }
    small(
        Some(done_ring(state, finished)),
        carries_all(state, scope),
        scope,
        key(2),
        surface,
        details.into(),
    )
}

/// The small focus: the ring, when there is one, with `details` beside it, which come in softly
/// when `changes` does while the ring stays.
fn small<'a>(
    ring: Option<Dial>,
    carries: Option<Message>,
    scope: Scope,
    changes: u64,
    surface: impl Fn(&Theme) -> Color + 'a,
    details: Element<'a, Message>,
) -> Element<'a, Message> {
    let mut line = row![].spacing(space::M).align_y(Center);
    if let Some(ring) = ring {
        line = line.push(carry(ring.keyed(scope, dial::SMALL), carries));
    }
    line.push(appear(changes, surface, details)).into()
}

/// The mini-window next to the tray icon (spec §55): what is being worked on, with its ring to
/// pause it, what to do with a result, and a way into Gracil. Files can be dropped on it too.
fn mini(state: &Gracil, window: window::Id) -> Element<'_, Message> {
    let lit = state.hovering == Some(window);
    let surface = face(style::bare, false);
    let finished = state.finished().filter(|_| state.mini_shows_done());
    let (shows, body): (u64, Element<'_, Message>) = if lit {
        (
            key(1),
            text(state.text.release_to_add).size(size::BODY).into(),
        )
    } else if let Some(finished) = finished {
        (key(2), small_done(state, &finished, Scope::Mini, surface))
    } else {
        match state.mini_job() {
            Some(job) => (
                key((2, ring(state, job, false).is_none().then_some(job.id))),
                small_file(state, job, Scope::Mini, surface),
            ),
            None if state.gathering > 0 => {
                (key(3), text(state.text.looking).size(size::BODY).into())
            }
            None => {
                let mut empty =
                    column![text(state.text.drop_files_here).size(size::BODY)].spacing(space::XS);
                if let Some(notice) = cannot_work(state) {
                    empty = empty.push(text(notice).size(size::CAPTION).style(style::muted));
                }
                (key(4), empty.into())
            }
        }
    };
    let side = column![
        keyed(
            Scope::Mini,
            button(text("×").size(size::BODY))
                .padding([space::XS, space::S])
                .style(style::quiet_button)
                .on_press(Message::CloseMini)
        ),
        Space::new().height(Fill),
        keyed(
            Scope::Mini,
            button(text(state.text.open_gracil).size(size::CAPTION))
                .padding([space::XS, space::S])
                .style(style::quiet_button)
                .on_press(Message::OpenMain)
        ),
    ]
    .align_x(iced::Right);
    card(
        style::bare,
        lit,
        container(
            row![
                container(appear(shows, face(style::bare, lit), body))
                    .width(Fill)
                    .center_y(Fill),
                side
            ]
            .spacing(space::S)
            .height(Fill),
        )
        // The ring is as far from the window's left edge as from its top and bottom.
        .padding(Padding::new(space::S).left(space::M)),
    )
    .into()
}

/// Exit asked while work is in progress: what would stop, in so many words, and the two ways out
/// (spec §54.1). Shown in the window itself, never as a dialog (spec §2.3).
fn exit_question(state: &Gracil) -> Element<'_, Message> {
    let explanation = state.text.exit_explanation(
        state.being_worked_on(),
        state.waiting_to_compress(),
        state.waiting_count(),
        state.gathering > 0,
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
/// again once another file turned out to have the new name, or the original turned out to be
/// read-only, since it would fail the same way.
fn can_choose(enabled: bool, problem: Option<Failure>, choice: Choice) -> bool {
    enabled
        && !(choice == Choice::Replace
            && matches!(problem, Some(Failure::NameTaken | Failure::ReadOnly)))
}

/// Replace and Save copy, the two things to do with a finished result (ADR 0009): full size in
/// the main window, smaller in the mini-window.
fn choices(
    state: &Gracil,
    job: &Job,
    scope: Scope,
    enabled: bool,
    problem: Option<Failure>,
) -> Element<'static, Message> {
    let (text_size, padding, spacing) = match scope {
        Scope::Main => (size::BODY, [space::XS, space::M], space::S),
        // Five pixels above and below make it at least 24 pixels tall, easy to hit.
        Scope::Mini => (size::CAPTION, [5.0, space::S], space::XS),
    };
    let action = |label: &'static str, choice: Choice| {
        let can = can_choose(enabled, problem, choice);
        keyed_if(
            scope,
            can,
            button(text(label).size(text_size))
                .padding(padding)
                .style(style::action_button)
                .on_press_maybe(can.then_some(Message::Choose(job.id, choice))),
        )
    };
    row![
        action(state.text.replace, Choice::Replace),
        action(state.text.save_copy, Choice::SaveCopy)
    ]
    .spacing(spacing)
    .into()
}

fn placed_caption(state: &Gracil, job: &Job, choice: Choice, path: &std::path::Path) -> String {
    let placed_name = path.file_name().unwrap_or_default().to_string_lossy();
    match choice {
        Choice::SaveCopy => state
            .text
            .saved_as(&shorten(&placed_name, LARGE_NAME_CHARS)),
        Choice::Replace if placed_name == job.file_name() => {
            state.text.replaced_the_original.to_owned()
        }
        Choice::Replace => state
            .text
            .replaced_by(&shorten(&placed_name, LARGE_NAME_CHARS)),
    }
}

/// Every video becomes an MP4 (ADR 0012), and every image other than a PNG a JPEG, or a PNG when
/// it has transparency (ADR 0034). When that changes the name, say so before the user chooses,
/// in the plainest way there is: `IMG_0042.MOV → .mp4`.
fn format_change(job: &Job, output: &OsStr) -> Option<String> {
    if job.path.extension() == Some(output) {
        return None;
    }
    Some(format!(
        "{} → .{}",
        shorten(&job.file_name(), LARGE_NAME_CHARS),
        output.to_string_lossy()
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
        JobState::Pending | JobState::Preparing | JobState::Prepared if state.paused => {
            Status::plain(state.text.paused)
        }
        // A laptop's battery running low holds the work quietly, saying why (ADR 0063).
        JobState::Pending | JobState::Preparing | JobState::Prepared | JobState::Compressing
            if state.waiting_for_battery(job.state) =>
        {
            Status::plain(state.text.low_battery)
        }
        JobState::Pending | JobState::Prepared => match state.engine {
            EngineState::Starting => Status::plain(state.text.getting_ready),
            EngineState::Ready(_) => Status::plain(state.text.waiting),
            EngineState::Unavailable => Status {
                label: state.text.cannot_start.into(),
                notice: Some(if crate::place::inside_zip() {
                    state.text.inside_zip
                } else {
                    state.text.ffmpeg_missing
                }),
            },
        },
        // A file kept only online comes down first, at the connection's pace (ADR 0066).
        JobState::Preparing if state.is_downloading(job.id) => {
            Status::plain(state.text.downloading)
        }
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

/// The same in the focus, where the ring already says how much smaller a finished file is: its
/// size before and after instead (`120 MB → 78 MB`).
fn detail(state: &Gracil, job: &Job) -> Status {
    match state.outcomes.get(&job.id) {
        Some(Outcome::Ready {
            placing: false,
            sizes,
            problem,
            ..
        }) => Status {
            label: state.text.change(*sizes),
            notice: problem.map(|failure| state.text.failure(failure)),
        },
        Some(Outcome::Placed { sizes, .. }) => Status::plain(state.text.change(*sizes)),
        _ => status(state, job),
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
    /// Behind the "!", which opens the main window on the file: the mini-window, too small for it.
    Link(JobId),
    /// Behind the "!", on hover: the list, and the small focus above it.
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

/// The line under the focus: how the list goes on its left, and on its right the way into the
/// list and the way to add files. Always one line: what doesn't fit gives way, the least needed
/// first (ADR 0076).
fn footer(state: &Gracil) -> Element<'_, Message> {
    let toggle = if state.drawer_open {
        state.text.hide_files
    } else {
        state.text.view_files
    };
    let quiet = |label: &'static str, text_size: f32, message: Message| {
        button(text(label).size(text_size).line_height(1.0))
            .padding([space::XS, space::S])
            .style(style::quiet_button)
            .on_press(message)
    };
    let list = keyed(
        Scope::Main,
        quiet(toggle, size::CAPTION, Message::ToggleDrawer),
    );
    let add = keyed(
        Scope::Main,
        hint(
            // Two pixels up, where the sign sits level with the middle of the words beside it.
            quiet("+", size::TITLE, Message::ChooseFiles).padding(
                Padding::new(space::S)
                    .top(space::XS - 2.0)
                    .bottom(space::XS + 2.0),
            ),
            state.text.add_files,
        ),
    );
    let taken = measure(toggle, size::CAPTION) + measure("+", size::TITLE) + 6.0 * space::S;
    let room = state.main_size.width - 2.0 * space::M - space::S - taken;
    row![
        container(
            text(summary(state, room))
                .size(size::CAPTION)
                .style(style::muted)
                .wrapping(Wrapping::None)
        )
        .width(Fill)
        .padding(Padding::default().left(space::S)),
        list,
        add
    ]
    .align_y(Center)
    .height(FOOTER)
    .into()
}

/// How the list goes, in as many of its parts as fit on one line `room` wide. Once all is done,
/// the focus already says how it went, so the line says nothing.
fn summary(state: &Gracil, room: f32) -> String {
    if state.selected.is_none() && state.finished().is_some() {
        return String::new();
    }
    // Each part with how much it is needed, the lower the more.
    let mut parts = Vec::new();
    // How many are left, once there are any: with none left, the rest says it (ADR 0066). With
    // about how long they take, once that is known well enough (ADR 0069).
    match (state.left(), state.time_left()) {
        (0, _) => {}
        (left, Some(time)) => parts.push((0, state.text.files_left_about(left, time))),
        (left, None) => parts.push((0, state.text.files_left(left))),
    }
    // "More" to decide: besides the result already shown large, as in the mini-window.
    let shown_waiting = state.focused().is_some_and(|job| state.is_waiting(job.id));
    match state.waiting_count() - usize::from(shown_waiting) {
        0 => {}
        n => parts.push((1, state.text.ready(n))),
    }
    // The room freed so far, once there is more than one result to add up (spec §32.2).
    let (total, results) = state.total();
    if results >= 2 {
        parts.push((3, state.text.less(total)));
    }
    if state.gathering > 0 {
        parts.push((2, state.text.looking_for_more.to_owned()));
    }
    if parts.is_empty() {
        parts.push((0, state.text.files_left(0)));
    }
    let line = |parts: &[(u8, String)]| {
        parts
            .iter()
            .map(|(_, part)| part.as_str())
            .collect::<Vec<_>>()
            .join("  ·  ")
    };
    while parts.len() > 1 && measure(&line(&parts), size::CAPTION) > room {
        let least = parts
            .iter()
            .enumerate()
            .max_by_key(|(_, (need, _))| *need)
            .map_or(0, |(index, _)| index);
        parts.remove(least);
    }
    // Still too long: the count alone, without how long it takes.
    if measure(&line(&parts), size::CAPTION) > room && state.left() > 0 {
        return state.text.files_left(state.left());
    }
    line(&parts)
}

/// The list: a card of its own, `height` tall.
fn drawer(state: &Gracil, height: f32) -> Element<'_, Message> {
    let quiet = |label: &'static str, message: Option<Message>| {
        keyed_if(
            Scope::Main,
            message.is_some(),
            button(text(label).size(size::CAPTION))
                .padding([5.0, space::S])
                .style(style::quiet_button)
                .on_press_maybe(message),
        )
    };

    let mut header = row![].align_y(Center).height(DRAWER_HEADER);
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
    let any_removable = state.any_removable();
    header = header.push(Space::new().width(Fill)).push(quiet(
        state.text.clear,
        any_removable.then_some(Message::Clear),
    ));

    let (total, done) = state.total();
    let in_all = (done >= 2).then(|| caption(state.text.in_all(total)));

    let jobs = state.queue.jobs();
    let shown = state.drawer_scroll.rows(jobs.len(), height);
    let above = shown.start as f32 * ROW_HEIGHT;
    let below = (jobs.len() - shown.end) as f32 * ROW_HEIGHT;
    let rows = jobs[shown].iter().map(|job| {
        let line = row![
            // The whole path on hover, or with the ring (ADR 0066).
            hint_owned(
                container(name(job, DRAWER_NAME_CHARS, size::BODY)).width(Fill),
                job.path.display().to_string(),
            ),
            status_line(&drawer_status(state, job), size::CAPTION, Explain::Hover),
        ]
        .spacing(space::M)
        .align_y(Center);
        let remove = quiet(
            "×",
            state.can_remove(job).then_some(Message::Remove(job.id)),
        );
        row![
            carry(
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
                carries(state, Scope::Main, job),
            ),
            remove,
        ]
        .align_y(Center)
        .height(ROW_HEIGHT)
        .into()
    });

    let mut content = column![header].spacing(space::XS).padding(space::S);
    if let Some(line) = in_all {
        content = content.push(
            container(line)
                .padding([0.0, space::S])
                .center_y(DRAWER_SUM),
        );
    }
    content = content.push(
        scrollable(
            column![Space::new().height(above)]
                .extend(rows)
                .push(Space::new().height(below)),
        )
        .id(DRAWER_LIST)
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new()
                .width(style::SCROLL_BAR)
                .scroller_width(style::SCROLL_BAR)
                .spacing(space::XS),
        ))
        .style(style::scroll_bar)
        .height(Fill)
        .on_scroll(|viewport| {
            Message::DrawerScrolled(DrawerScroll {
                offset: viewport.absolute_offset().y,
                height: viewport.bounds().height,
            })
        }),
    );
    container(content)
        .height(height)
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

/// `content`, with words worked out as it is built, such as a file's whole path, on hover. A
/// path can be one long word, so it breaks anywhere rather than running out of the tip.
fn hint_owned<'a>(content: impl Into<Element<'a, Message>>, words: String) -> Element<'a, Message> {
    tooltip(
        content,
        container(
            text(words)
                .size(size::CAPTION)
                .wrapping(Wrapping::WordOrGlyph),
        )
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
        assert_eq!(top.rows(0, 148.0), 0..0);
        assert_eq!(top.rows(3, 148.0), 0..3);
        // 148 pixels show five rows of 30, plus four beyond the bottom edge.
        assert_eq!(top.rows(15_031, 148.0), 0..9);
        // A list that grew with the window since it last said its height.
        assert_eq!(top.rows(15_031, 300.0), 0..14);
        let further = DrawerScroll {
            offset: 300.0 * ROW_HEIGHT + 10.0,
            height: 148.0,
        };
        assert_eq!(further.rows(15_031, 148.0), 296..310);
        let end = DrawerScroll {
            offset: 15_026.0 * ROW_HEIGHT,
            height: 148.0,
        };
        assert_eq!(end.rows(15_031, 148.0), 15_022..15_031);
    }

    #[test]
    fn the_large_focus_fits_the_window_as_it_opens() {
        // The window as it opens, less its edges, the line under the focus and Always.
        let focus = WINDOW.height - 2.0 * space::M - (FOOTER + space::M) - (ALWAYS + space::M);
        let room = Room { focus, drawer: 0.0 };
        assert!(room.large());
        // And its ring is within a few pixels of the focus's very middle.
        let ring_middle = FOCUS_EDGE
            + room.lift()
            + ABOVE_RING
            + space::M
            + dial::SIZE / 2.0
            + (focus - 2.0 * FOCUS_EDGE - room.lift() - LARGE) / 2.0;
        assert!((ring_middle - focus / 2.0).abs() <= 6.0, "{ring_middle}");
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
