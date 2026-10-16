//! The levels the last videos needed (ADR 0065), kept from one start to the next (ADR 0070) so
//! the first videos after a start get ready as quickly as the later ones. They are kept in
//! `hints.txt` in Gracil's data folder, with what they depend on (Gracil's version, FFmpeg, the
//! graphics card encoder and its settings); hints written for anything else are not read. Only
//! numbers by kind of video are kept, nothing about any file. Written away from the window's
//! thread, every few minutes when something changed and on leaving.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::thread;
use std::time::Duration;

use gracil_core::compress::Engine;
use gracil_core::search::{self, Hints};
use tracing::warn;

use crate::data;

const FILE: &str = "hints.txt";
/// How often new hints are written down while Gracil runs.
const KEEP_EVERY: Duration = Duration::from_secs(3 * 60);

/// The engine's hints, with what they depend on, once [`restore`] has read them.
struct Kept {
    hints: Hints,
    identity: String,
}

static KEPT: OnceLock<Kept> = OnceLock::new();
/// The engine's [`Hints::choices`] when they were last written or read.
static WRITTEN: Mutex<u64> = Mutex::new(0);

/// Gives `engine` the hints kept last time, when they were kept for the same Gracil, FFmpeg and
/// encoder, and from then on keeps its hints every few minutes. Without a data folder, nothing is
/// read or kept. Called once the engine has started, away from the window's thread.
pub fn restore(engine: &mut Engine) {
    let Some(path) = file() else {
        return;
    };
    let Some(identity) = search::hints_identity(&engine.toolchain, &engine.capabilities) else {
        return;
    };
    let hints = read(&path, &identity);
    let mut written = WRITTEN.lock().unwrap_or_else(PoisonError::into_inner);
    let kept = Kept {
        hints: hints.clone(),
        identity,
    };
    // An engine started again after a first one failed has no hints of its own yet.
    if KEPT.set(kept).is_err() {
        return;
    }
    *written = hints.choices();
    drop(written);
    engine.hints = hints;
    let keeping = thread::Builder::new()
        .name("gracil-hints".into())
        .spawn(|| {
            loop {
                thread::sleep(KEEP_EVERY);
                keep();
            }
        });
    if let Err(error) = keeping {
        warn!(%error, "the levels of the last videos will be kept only on leaving");
    }
}

/// Writes the hints down now if they changed since last time, for when Gracil is about to end.
pub fn keep() {
    let Some(kept) = KEPT.get() else {
        return;
    };
    let mut written = WRITTEN.lock().unwrap_or_else(PoisonError::into_inner);
    let choices = kept.hints.choices();
    if choices == *written {
        return;
    }
    // Tried once: a drive that stopped taking files isn't asked again until something changes.
    *written = choices;
    let Some(path) = file() else {
        return;
    };
    if let Err(error) = write(&path, &kept.hints, &kept.identity) {
        warn!(%error, "the levels of the last videos couldn't be kept");
    }
}

/// Like [`keep`], on a thread of its own, so the caller never waits for the drive.
pub fn keep_soon() {
    if KEPT.get().is_some() {
        thread::spawn(keep);
    }
}

/// The hints kept at `path` for `identity`. A missing, damaged or foreign file means there are
/// none, or only those it still holds whole.
fn read(path: &Path, identity: &str) -> Hints {
    fs::read(path).map_or_else(
        |_| Hints::default(),
        |bytes| Hints::read_back(&String::from_utf8_lossy(&bytes), identity),
    )
}

fn write(path: &Path, hints: &Hints, identity: &str) -> io::Result<()> {
    data::write_whole(path, &hints.write_down(identity))
}

fn file() -> Option<PathBuf> {
    data::file(FILE)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use gracil_core::testing::fresh_dir;

    use super::*;

    const HERE: &str = "gracil 1.0.0 ffmpeg 1 2 hevc_nvenc Full";

    fn some_hints() -> Hints {
        Hints::read_back(
            &format!(
                "gracil hints 1\t{HERE}\nhevc_nvenc\t2\th264\tsame\t33\nlibx265\t4\thevc\tagain\t30\n"
            ),
            HERE,
        )
    }

    #[test]
    fn hints_come_back_from_the_data_folder() {
        let dir = fresh_dir("hints");
        let path = dir.join("nested").join(FILE);
        let hints = some_hints();
        assert_eq!(hints.choices(), 2);
        write(&path, &hints, HERE).unwrap();
        let back = read(&path, HERE);
        assert_eq!(back.choices(), 2);
        assert_eq!(back.write_down(HERE), hints.write_down(HERE));
        assert!(!dir.join("nested").join("hints.txt.new").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_missing_damaged_or_foreign_file_holds_no_hints() {
        let dir = fresh_dir("hints-damaged");
        let path = dir.join(FILE);
        assert_eq!(read(&path, HERE).choices(), 0);
        fs::write(&path, b"").unwrap();
        assert_eq!(read(&path, HERE).choices(), 0);
        fs::write(&path, [0xff, 0xfe, 0x00, b'\n', b'\t']).unwrap();
        assert_eq!(read(&path, HERE).choices(), 0);
        write(&path, &some_hints(), HERE).unwrap();
        assert_eq!(read(&path, "another FFmpeg").choices(), 0);
        // Cut off halfway: what is whole is still read.
        let text = fs::read_to_string(&path).unwrap();
        fs::write(&path, &text[..text.len() - 8]).unwrap();
        assert_eq!(read(&path, HERE).choices(), 1);
        fs::remove_dir_all(dir).unwrap();
    }
}
