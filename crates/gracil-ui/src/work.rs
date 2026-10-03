//! Background work: starting the engine and compressing, each on its own thread so the window
//! never waits on FFmpeg.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use gracil_core::compress::{CompressError, Compressed, Engine, compress};
use gracil_core::encode::JobControl;
use gracil_core::output::save_as_new;
use gracil_core::toolchain;
use iced::Task;
use iced::futures::SinkExt;
use iced::futures::channel::mpsc::{self, Sender};
use iced::futures::executor::block_on;
use tracing::{error, info, warn};

/// Why a video could not be compressed, in the terms the user sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    NoVideo,
    Unreadable,
    CouldNotCompress,
    CouldNotSave,
    NotSmaller,
}

impl Failure {
    pub fn explanation(self) -> &'static str {
        match self {
            Self::NoVideo => "This file has no video",
            Self::Unreadable => "This file can't be opened as a video",
            Self::CouldNotCompress => "This video couldn't be compressed",
            Self::CouldNotSave => "The compressed video couldn't be saved in this folder",
            Self::NotSmaller => {
                "This video couldn't be made smaller than the original, so it wasn't saved"
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Outcome {
    /// `smaller_by` is the share of the original size that was saved, from 0 to 1.
    Saved {
        path: PathBuf,
        smaller_by: f64,
    },
    Failed(Failure),
    Cancelled,
}

#[derive(Debug, Clone)]
pub enum Report {
    Progress(Option<f64>),
    Done(Outcome),
}

/// Finds or installs FFmpeg and detects the hardware.
pub fn start_engine() -> Task<Option<Arc<Engine>>> {
    on_thread(|mut sender: Sender<Option<Arc<Engine>>>| {
        let engine = toolchain::default_private_dir().and_then(|dir| {
            Engine::start(&dir)
                .inspect_err(|e| error!(error = %e, "engine unavailable"))
                .ok()
        });
        let _ = block_on(sender.send(engine.map(Arc::new)));
    })
}

/// Compresses `source` and saves the result beside it.
pub fn compress_video(engine: Arc<Engine>, source: PathBuf, control: JobControl) -> Task<Report> {
    on_thread(move |mut sender: Sender<Report>| {
        let mut progress = sender.clone();
        let result = compress(&engine, &source, &control, &mut |update| {
            // Progress is best effort: when the window is busy, a skipped update costs nothing.
            let _ = progress.try_send(Report::Progress(update.fraction));
        });

        let outcome = match result {
            Ok(compressed) => keep(&compressed, &source),
            Err(CompressError::Cancelled) => Outcome::Cancelled,
            Err(e) => {
                warn!(error = %e, "compression failed");
                Outcome::Failed(match e {
                    CompressError::NoVideo => Failure::NoVideo,
                    CompressError::Probe(_) => Failure::Unreadable,
                    CompressError::Io(_) => Failure::CouldNotSave,
                    CompressError::Failed(_) | CompressError::Cancelled => {
                        Failure::CouldNotCompress
                    }
                })
            }
        };
        let _ = block_on(sender.send(Report::Done(outcome)));
    })
}

/// Saves the compressed copy beside the source, unless it is not smaller (spec §40.1).
fn keep(compressed: &Compressed, source: &Path) -> Outcome {
    let original = fs::metadata(source).map_or(0, |metadata| metadata.len());
    let saved = if compressed.size < original {
        save_as_new(&compressed.temp, source).map_err(|e| {
            warn!(error = %e, "could not place compressed copy");
            Failure::CouldNotSave
        })
    } else {
        info!(
            original,
            compressed = compressed.size,
            "result not smaller; not saved"
        );
        Err(Failure::NotSmaller)
    };
    match saved {
        Ok(path) => {
            info!(?compressed.attempt, original, compressed = compressed.size, "saved compressed copy");
            Outcome::Saved {
                path,
                smaller_by: 1.0 - compressed.size as f64 / original as f64,
            }
        }
        Err(failure) => {
            let _ = fs::remove_file(&compressed.temp);
            Outcome::Failed(failure)
        }
    }
}

/// Runs `work` on its own thread; whatever it sends becomes the task's output.
fn on_thread<T: Send + 'static>(work: impl FnOnce(Sender<T>) + Send + 'static) -> Task<T> {
    let (sender, receiver) = mpsc::channel(1);
    thread::spawn(move || work(sender));
    Task::stream(receiver)
}
