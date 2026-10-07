//! The lifecycle of a single compression job.
//!
//! Every job is in exactly one [`JobState`]. State only changes through [`JobState::apply`], so
//! the set of legal transitions lives in one place and the UI can derive everything it shows from
//! the current state instead of from scattered flags.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobState {
    /// Waiting in the queue.
    Pending,
    /// Getting ready: inspecting the source and choosing how to compress it.
    Preparing,
    /// Ready, waiting its turn to compress (ADR 0040). Holds no running work.
    Prepared,
    /// FFmpeg is running.
    Compressing,
    /// FFmpeg is suspended and will continue from where it stopped.
    Paused,
    /// Validating the output and putting it in its final place.
    Finalizing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobEvent {
    Start,
    /// Getting ready is done.
    Ready,
    BeginCompressing,
    Pause,
    Resume,
    BeginFinalizing,
    Complete,
    Fail,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTransition {
    pub from: JobState,
    pub event: JobEvent,
}

impl fmt::Display for InvalidTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "cannot apply {:?} to a job that is {:?}",
            self.event, self.from
        )
    }
}

impl std::error::Error for InvalidTransition {}

impl JobState {
    pub fn apply(self, event: JobEvent) -> Result<JobState, InvalidTransition> {
        use JobEvent as E;
        use JobState as S;

        let next = match (self, event) {
            (S::Pending, E::Start) => S::Preparing,
            (S::Preparing, E::Ready) => S::Prepared,
            (S::Preparing | S::Prepared, E::BeginCompressing) => S::Compressing,
            (S::Compressing, E::Pause) => S::Paused,
            (S::Paused, E::Resume) => S::Compressing,
            (S::Compressing, E::BeginFinalizing) => S::Finalizing,
            (S::Finalizing, E::Complete) => S::Completed,
            (S::Preparing | S::Compressing | S::Paused | S::Finalizing, E::Fail) => S::Failed,
            // Finalizing cannot be cancelled: it may be in the middle of replacing the original,
            // and stopping halfway is exactly what safe replacement exists to prevent. It is short.
            (S::Pending | S::Preparing | S::Prepared | S::Compressing | S::Paused, E::Cancel) => {
                S::Cancelled
            }
            (from, event) => return Err(InvalidTransition { from, event }),
        };
        Ok(next)
    }

    /// The job owns running work (an FFmpeg process or a file replacement in progress).
    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Preparing | Self::Compressing | Self::Paused | Self::Finalizing
        )
    }

    /// The job hasn't started compressing yet, whether or not it was got ready.
    pub fn is_waiting(self) -> bool {
        matches!(self, Self::Pending | Self::Prepared)
    }

    /// The job is being got ready, or was and waits for its turn to compress.
    pub fn is_getting_ready(self) -> bool {
        matches!(self, Self::Preparing | Self::Prepared)
    }

    /// The job is compressing, including paused and putting its result in place.
    pub fn is_compressing(self) -> bool {
        matches!(self, Self::Compressing | Self::Paused | Self::Finalizing)
    }

    /// The job will not change state again, unless a failed one is retried.
    pub fn is_finished(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// The job can leave the list without cancelling anything. Removing an active job is a
    /// cancellation and must go through [`JobEvent::Cancel`] explicitly.
    pub fn is_removable(self) -> bool {
        !self.is_active()
    }
}

#[cfg(test)]
mod tests {
    use super::JobEvent as E;
    use super::JobState as S;
    use super::*;

    const ALL_STATES: [S; 9] = [
        S::Pending,
        S::Preparing,
        S::Prepared,
        S::Compressing,
        S::Paused,
        S::Finalizing,
        S::Completed,
        S::Failed,
        S::Cancelled,
    ];
    const ALL_EVENTS: [E; 9] = [
        E::Start,
        E::Ready,
        E::BeginCompressing,
        E::Pause,
        E::Resume,
        E::BeginFinalizing,
        E::Complete,
        E::Fail,
        E::Cancel,
    ];

    fn run(events: &[E]) -> Result<S, InvalidTransition> {
        events
            .iter()
            .try_fold(S::Pending, |state, &e| state.apply(e))
    }

    #[test]
    fn successful_job_with_pause() {
        let state = run(&[
            E::Start,
            E::BeginCompressing,
            E::Pause,
            E::Resume,
            E::BeginFinalizing,
            E::Complete,
        ]);
        assert_eq!(state, Ok(S::Completed));
    }

    #[test]
    fn a_file_can_wait_its_turn_once_ready() {
        let state = run(&[E::Start, E::Ready, E::BeginCompressing, E::BeginFinalizing]);
        assert_eq!(state, Ok(S::Finalizing));
        assert!(
            S::Prepared.is_removable(),
            "a ready file holds no running work"
        );
        assert_eq!(S::Prepared.apply(E::Cancel), Ok(S::Cancelled));
    }

    #[test]
    fn pending_job_can_be_cancelled() {
        assert_eq!(S::Pending.apply(E::Cancel), Ok(S::Cancelled));
    }

    #[test]
    fn paused_job_can_be_cancelled() {
        assert_eq!(
            run(&[E::Start, E::BeginCompressing, E::Pause, E::Cancel]),
            Ok(S::Cancelled)
        );
    }

    #[test]
    fn finalizing_cannot_be_cancelled_or_paused() {
        for event in [E::Cancel, E::Pause] {
            assert!(S::Finalizing.apply(event).is_err());
        }
    }

    #[test]
    fn finished_states_accept_no_events() {
        for state in [S::Completed, S::Failed, S::Cancelled] {
            for event in ALL_EVENTS {
                assert_eq!(
                    state.apply(event),
                    Err(InvalidTransition { from: state, event })
                );
            }
        }
    }

    #[test]
    fn pending_cannot_fail_before_starting() {
        assert!(S::Pending.apply(E::Fail).is_err());
    }

    #[test]
    fn active_and_removable_are_complementary() {
        for state in ALL_STATES {
            assert_ne!(state.is_active(), state.is_removable(), "{state:?}");
        }
        assert!(S::Pending.is_removable());
        assert!(S::Paused.is_active());
    }

    #[test]
    fn finished_states_are_not_active() {
        for state in ALL_STATES.into_iter().filter(|s| s.is_finished()) {
            assert!(!state.is_active(), "{state:?}");
        }
    }

    #[test]
    fn every_unfinished_state_is_waiting_getting_ready_or_compressing() {
        for state in ALL_STATES.into_iter().filter(|s| !s.is_finished()) {
            let kinds = [
                state.is_waiting(),
                state.is_getting_ready(),
                state.is_compressing(),
            ];
            assert!(kinds.contains(&true), "{state:?}");
            assert!(!(state.is_waiting() && state.is_compressing()), "{state:?}");
        }
        assert!(S::Prepared.is_waiting() && S::Prepared.is_getting_ready());
    }
}
