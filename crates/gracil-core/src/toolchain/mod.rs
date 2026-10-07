//! Finding the FFmpeg executables Gracil runs (spec §20.1, ADR 0050).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use tracing::{info, warn};

use crate::capabilities::Encoders;

/// The encoder every compression can fall back to. An FFmpeg without it cannot be relied on.
const REQUIRED_ENCODER: &str = "libx265";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// The copy that comes with Gracil, in the `ffmpeg` folder beside `gracil.exe` (ADR 0050).
    Bundled,
    /// Installed by the user and found on `PATH`, which is how Gracil runs while developing.
    System,
}

#[derive(Debug, Clone)]
pub struct Toolchain {
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
    origin: Origin,
}

impl Toolchain {
    /// A toolchain whose executables live in `dir`, if both are there.
    fn in_dir(dir: &Path, origin: Origin) -> Option<Self> {
        let ffmpeg = dir.join("ffmpeg.exe");
        let ffprobe = dir.join("ffprobe.exe");
        (ffmpeg.is_file() && ffprobe.is_file()).then_some(Self {
            ffmpeg,
            ffprobe,
            origin,
        })
    }

    pub fn ffmpeg_path(&self) -> &Path {
        &self.ffmpeg
    }

    /// A command for `ffmpeg.exe` that never shows a console window or reads stdin.
    pub fn ffmpeg(&self) -> Command {
        hidden_command(&self.ffmpeg)
    }

    /// A command for `ffprobe.exe` that never shows a console window or reads stdin.
    pub fn ffprobe(&self) -> Command {
        hidden_command(&self.ffprobe)
    }
}

/// jpegli's `cjpegli.exe` (ADR 0034): next to Gracil's own executable, where the zip puts it,
/// or, while developing, where `scripts/build-jpegli.ps1` builds it inside `target`.
pub fn find_jpegli() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let found = exe
        .ancestors()
        .skip(1)
        .take(4)
        .flat_map(|dir| {
            [
                dir.join("cjpegli.exe"),
                dir.join(r"jpegli\build\tools\cjpegli.exe"),
            ]
        })
        .find(|path| path.is_file());
    match &found {
        Some(path) => info!(path = %path.display(), "using jpegli"),
        None => warn!("jpegli not found; photos cannot be compressed"),
    }
    found
}

pub(crate) fn hidden_command(program: &Path) -> Command {
    let mut command = Command::new(program);
    command.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        // Below other programs, so they keep the processor first (ADR 0040).
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
        command.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
    }
    command
}

/// A toolchain ready to use, with the encoder list FFmpeg reported while it was checked.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub toolchain: Toolchain,
    pub encoders: Encoders,
}

/// The name of Gracil's data folder beside `gracil.exe`.
pub const DATA_FOLDER: &str = "data";

/// Gracil's data folder (ADR 0050, ADR 0054): `data` beside `gracil.exe`, always, so the folder
/// Gracil came in carries its settings, logs and temporary notes with it and deleting it leaves
/// nothing behind. Where that can't be written (a read-only drive, or a folder only an
/// administrator can change), there is none: Gracil still compresses, without remembering
/// anything.
pub fn default_private_dir() -> Option<PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        own_folder()
            .map(|folder| folder.join(DATA_FOLDER))
            .filter(|dir| writable(dir))
    })
    .clone()
}

/// The folder holding the running executable.
pub fn own_folder() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(Path::to_path_buf)
}

/// Where the FFmpeg that comes with Gracil is kept: the `ffmpeg` folder beside the running
/// executable.
pub fn bundled_dir() -> Option<PathBuf> {
    own_folder().map(|folder| folder.join("ffmpeg"))
}

/// Whether Gracil can keep files in `dir`, creating it if needed.
fn writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".write-test-{}", std::process::id()));
    let ok = std::fs::create_dir_all(dir).is_ok() && std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// A usable FFmpeg: the copy that comes with Gracil, then one installed on the computer. Gracil
/// never downloads one.
pub fn find() -> Option<Resolved> {
    let bundled = bundled_dir().and_then(|dir| Toolchain::in_dir(&dir, Origin::Bundled));
    let system = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .filter_map(|dir| Toolchain::in_dir(&dir, Origin::System));
    bundled.into_iter().chain(system).find_map(qualify)
}

/// Accepts `candidate` only if FFmpeg itself reports the encoders Gracil depends on.
fn qualify(candidate: Toolchain) -> Option<Resolved> {
    match Encoders::list(&candidate) {
        Ok(encoders) if encoders.contains(REQUIRED_ENCODER) => {
            info!(path = %candidate.ffmpeg.display(), origin = ?candidate.origin, "using FFmpeg");
            Some(Resolved {
                toolchain: candidate,
                encoders,
            })
        }
        Ok(_) => {
            info!(path = %candidate.ffmpeg.display(), "skipping FFmpeg without {REQUIRED_ENCODER}");
            None
        }
        Err(error) => {
            warn!(path = %candidate.ffmpeg.display(), %error, "skipping FFmpeg that failed to run");
            None
        }
    }
}
