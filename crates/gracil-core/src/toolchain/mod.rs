//! Finding the FFmpeg executables Gracil runs, or installing a private copy (spec §20.1, ADR 0008).

mod provision;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use tracing::{info, warn};

pub use provision::{PINNED_BUILD, PinnedBuild, ProvisionError};

use crate::capabilities::Encoders;

/// The encoder every compression can fall back to. An FFmpeg without it cannot be relied on.
const REQUIRED_ENCODER: &str = "libx265";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// The copy that comes with Gracil, in the `ffmpeg` folder beside `gracil.exe` (ADR 0050).
    Bundled,
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

/// The name of Gracil's data folder beside `gracil.exe`.
pub const DATA_FOLDER: &str = "data";

/// Gracil's data folder (ADR 0050): `data` beside `gracil.exe`, so the folder Gracil came in
/// carries its settings, logs and temporary notes with it. Where that can't be written (a
/// read-only drive, or a folder only an administrator can change), `%LOCALAPPDATA%\Gracil`.
pub fn default_private_dir() -> Option<PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        own_folder()
            .map(|folder| folder.join(DATA_FOLDER))
            .filter(|dir| writable(dir))
            .or_else(local_app_data)
    })
    .clone()
}

/// The data folder of the Gracil in `folder`, for code that doesn't run as `gracil.exe` (File
/// Explorer's menu): its `data` folder when Gracil made one, `%LOCALAPPDATA%\Gracil` otherwise.
pub fn private_dir_of(folder: &Path) -> Option<PathBuf> {
    Some(folder.join(DATA_FOLDER))
        .filter(|dir| dir.is_dir())
        .or_else(local_app_data)
}

/// The folder holding the running executable.
pub fn own_folder() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(Path::to_path_buf)
}

fn local_app_data() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join("Gracil"))
}

/// Whether Gracil can keep files in `dir`, creating it if needed.
fn writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".write-test-{}", std::process::id()));
    let ok = std::fs::create_dir_all(dir).is_ok() && std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// Finds a usable FFmpeg: the copy that comes with Gracil, one installed on the computer, then
/// Gracil's private copy, and as a last resort downloads and installs the pinned build into
/// `private_dir`.
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
    let bundled =
        own_folder().and_then(|folder| Toolchain::in_dir(&folder.join("ffmpeg"), Origin::Bundled));
    let system = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .filter_map(|dir| Toolchain::in_dir(&dir, Origin::System));
    let private_bin = provision::install_dir(private_dir, &PINNED_BUILD);
    bundled
        .into_iter()
        .chain(system)
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
