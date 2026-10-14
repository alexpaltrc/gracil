//! The few keys Gracil answers (ADR 0055).

use super::*;

/// The few keys Gracil answers (ADR 0055): what the pointer can do, a little faster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Esc: steps back, from a question, the mini-window, a picked file or the open list.
    Back,
    /// Space: pauses or resumes the work.
    PauseResume,
    /// Ctrl+O: chooses files to add.
    ChooseFiles,
    /// Ctrl+Z: puts back what Clear just took off the list, while Undo shows.
    Undo,
    /// Tab: the keyboard's ring moves on to the next control (ADR 0059).
    NextControl,
    /// Shift+Tab: the ring moves back to the previous control.
    PreviousControl,
}

/// The key Gracil answers to `key` with `modifiers`, if any.
pub(super) fn key_for(key: &keyboard::Key, modifiers: keyboard::Modifiers) -> Option<Key> {
    use keyboard::key::Named;
    match key.as_ref() {
        keyboard::Key::Named(Named::Escape) => Some(Key::Back),
        keyboard::Key::Named(Named::Space) if modifiers.is_empty() => Some(Key::PauseResume),
        keyboard::Key::Character("o" | "O") if modifiers.command() => Some(Key::ChooseFiles),
        keyboard::Key::Character("z" | "Z") if modifiers.command() => Some(Key::Undo),
        keyboard::Key::Named(Named::Tab) if modifiers.shift() => Some(Key::PreviousControl),
        keyboard::Key::Named(Named::Tab) => Some(Key::NextControl),
        _ => None,
    }
}

impl Gracil {
    /// Answers a key (ADR 0055).
    pub(super) fn on_key(&mut self, window: window::Id, key: Key) -> Task<Message> {
        let in_mini = self.is_mini(window);
        match key {
            Key::Back if self.confirming_exit && !in_mini => {
                self.confirming_exit = false;
                self.preparing_to_delete = false;
                Task::none()
            }
            Key::Back if in_mini => self.close_mini(),
            Key::Back => {
                if self.selected.is_some() {
                    self.selected = None;
                } else {
                    self.close_drawer();
                }
                Task::none()
            }
            Key::PauseResume if self.confirming_exit => Task::none(),
            Key::PauseResume if self.paused => {
                self.pause(false);
                self.schedule()
            }
            Key::PauseResume => {
                if !self.running.is_empty() {
                    self.pause(true);
                }
                Task::none()
            }
            Key::ChooseFiles if self.confirming_exit => Task::none(),
            Key::ChooseFiles => Task::perform(choose_files(self.text), Message::FilesChosen),
            Key::Undo if self.confirming_exit || in_mini => Task::none(),
            Key::Undo => self.undo_clear(),
            Key::NextControl | Key::PreviousControl => {
                let scope = if in_mini { Scope::Mini } else { Scope::Main };
                focus::step(scope, key == Key::NextControl)
            }
        }
    }

    /// Closes the list, which folds away before it leaves the window (ADR 0056).
    pub(super) fn close_drawer(&mut self) {
        if self.drawer_open {
            self.drawer_open = false;
            self.drawer_folding = true;
        }
    }
}
