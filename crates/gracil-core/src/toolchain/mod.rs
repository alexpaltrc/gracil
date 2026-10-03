//! Finding the FFmpeg executables Gracil runs, or installing a private copy (spec §20.1, ADR 0008).

mod provision;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tracing::{info, warn};

pub use provision::{PINNED_BUILD, PinnedBuild, ProvisionError};

use crate::capabilities::Encoders;

/// The encoder every compression can fall back to. An FFmpeg without it cannot be relied on.
const REQUIRED_ENCODER: &str = "libx265";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Installed by the user (winget, Scoop, a manual download...) and found on `PATH`.
    System,
    /// Gracil's own copy in its private folder.
    Private,
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

    pub fn origin(&self) -> Origin {
        self.origin
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

pub(crate) fn hidden_command(program: &Path) -> Command {
    let mut command = Command::new(program);
    command.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

#[derive(Debug, thiserror::Error)]
pub enum ToolchainError {
    #[error("no usable FFmpeg was found and installing one failed")]
    Provision(#[from] ProvisionError),
    #[error("the installed FFmpeg copy does not work")]
    PrivateCopyUnusable,
}

/// A toolchain ready to use, with the encoder list FFmpeg reported while it was checked.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub toolchain: Toolchain,
    pub encoders: Encoders,
}

/// Gracil's private data folder, `%LOCALAPPDATA%\Gracil`.
pub fn default_private_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join("Gracil"))
}

/// Finds a usable FFmpeg: one already installed on the computer, then Gracil's private copy, and
/// as a last resort downloads and installs the pinned build into `private_dir`.
///
/// This can take minutes when it has to download, so call it off the UI thread.
pub fn resolve(private_dir: &Path) -> Result<Resolved, ToolchainError> {
    if let Some(resolved) = find(private_dir) {
        return Ok(resolved);
    }
    info!(
        build = PINNED_BUILD.id,
        "no usable FFmpeg found, installing private copy"
    );
    let dir = provision::install(private_dir, &PINNED_BUILD)?;
    Toolchain::in_dir(&dir, Origin::Private)
        .and_then(qualify)
        .ok_or(ToolchainError::PrivateCopyUnusable)
}

/// A usable FFmpeg already on this computer, never downloading anything.
pub fn find(private_dir: &Path) -> Option<Resolved> {
    let system = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .filter_map(|dir| Toolchain::in_dir(&dir, Origin::System));
    let private_bin = provision::install_dir(private_dir, &PINNED_BUILD);
    system
        .chain(Toolchain::in_dir(&private_bin, Origin::Private))
        .find_map(qualify)
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
