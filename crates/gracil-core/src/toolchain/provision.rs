//! Installing Gracil's private FFmpeg copy.
//!
//! Uses the `curl.exe` and `tar.exe` that ship with Windows, so Gracil carries no HTTP or ZIP code
//! of its own. The archive is verified against a pinned SHA-256 before anything is extracted, and
//! the finished folder appears in a single rename, so a crash never leaves a half-installed copy
//! where Gracil would use it.

use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tracing::{info, warn};

use super::hidden_command;

/// A specific FFmpeg build Gracil knows how to install. See `docs/development/ffmpeg.md`.
#[derive(Debug)]
pub struct PinnedBuild {
    /// Folder name of the installed copy; changing the build changes the folder.
    pub id: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

pub const PINNED_BUILD: PinnedBuild = PinnedBuild {
    id: "ffmpeg-9.0.2-essentials",
    url: "https://github.com/GyanD/codexffmpeg/releases/download/9.0.2/ffmpeg-9.0.2-essentials_build.zip",
    size: 114_768_076,
    sha256: "60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba",
};

const EXECUTABLES: [&str; 2] = ["ffmpeg.exe", "ffprobe.exe"];

#[derive(Debug, thiserror::Error)]
pub enum ProvisionError {
    #[error("file error while installing FFmpeg")]
    Io(#[from] io::Error),
    #[error("downloading FFmpeg failed: {0}")]
    Download(String),
    #[error("the downloaded FFmpeg archive is not the expected file")]
    Mismatch,
    #[error("extracting FFmpeg failed: {0}")]
    Extract(String),
}

fn ffmpeg_root(private_dir: &Path) -> PathBuf {
    private_dir.join("ffmpeg")
}

pub(super) fn install_dir(private_dir: &Path, build: &PinnedBuild) -> PathBuf {
    ffmpeg_root(private_dir).join(build.id)
}

/// Downloads, verifies and installs `build`. Returns the folder holding the executables.
pub(super) fn install(private_dir: &Path, build: &PinnedBuild) -> Result<PathBuf, ProvisionError> {
    let root = ffmpeg_root(private_dir);
    // The process id keeps two Gracil processes from sharing a staging folder. A leftover folder
    // with our id can only come from an earlier process that crashed, so it is safe to reuse.
    let staging = root.join(format!(".staging-{}", std::process::id()));
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)?;

    let result = install_from_staging(&root, &staging, build);
    if let Err(error) = fs::remove_dir_all(&staging) {
        warn!(%error, path = %staging.display(), "could not remove FFmpeg staging folder");
    }
    let target = result?;

    remove_other_builds(&root, build);
    info!(path = %target.display(), "installed private FFmpeg");
    Ok(target)
}

fn install_from_staging(
    root: &Path,
    staging: &Path,
    build: &PinnedBuild,
) -> Result<PathBuf, ProvisionError> {
    let archive = staging.join("download.zip");
    run_system_tool(
        "curl.exe",
        [
            OsStr::new("--fail"),
            OsStr::new("--location"),
            OsStr::new("--silent"),
            OsStr::new("--show-error"),
            OsStr::new("--proto"),
            OsStr::new("=https"),
            OsStr::new("--retry"),
            OsStr::new("3"),
            OsStr::new("--output"),
            archive.as_os_str(),
            OsStr::new(build.url),
        ],
    )
    .map_err(ProvisionError::Download)?;

    if fs::metadata(&archive)?.len() != build.size || sha256_hex(&archive)? != build.sha256 {
        return Err(ProvisionError::Mismatch);
    }

    // Archives keep the executables under `<top folder>/bin/`. Extract only those two files.
    run_system_tool(
        "tar.exe",
        [
            OsStr::new("-x"),
            OsStr::new("-f"),
            archive.as_os_str(),
            OsStr::new("-C"),
            staging.as_os_str(),
            OsStr::new("*/bin/ffmpeg.exe"),
            OsStr::new("*/bin/ffprobe.exe"),
        ],
    )
    .map_err(ProvisionError::Extract)?;

    let extracted_bin = fs::read_dir(staging)?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("bin"))
        .find(|bin| EXECUTABLES.iter().all(|exe| bin.join(exe).is_file()))
        .ok_or_else(|| ProvisionError::Extract("executables not found in archive".into()))?;

    let ready = staging.join("ready");
    fs::create_dir(&ready)?;
    for exe in EXECUTABLES {
        fs::rename(extracted_bin.join(exe), ready.join(exe))?;
    }

    // Only reached when the private copy is missing or failed to run, so whatever is at the
    // target is a broken copy of this same build, inside Gracil's own folder.
    let target = root.join(build.id);
    if target.exists() {
        fs::remove_dir_all(&target)?;
    }
    fs::rename(&ready, &target)?;
    Ok(target)
}

/// Removes private copies of builds Gracil no longer pins. Failures only cost disk space.
fn remove_other_builds(root: &Path, current: &PinnedBuild) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let is_old_build = name.to_str().is_some_and(|name| {
            name.starts_with("ffmpeg-") && name != current.id && !name.starts_with('.')
        });
        if is_old_build && let Err(error) = fs::remove_dir_all(entry.path()) {
            warn!(%error, path = %entry.path().display(), "could not remove old FFmpeg copy");
        }
    }
}

/// Runs a tool from `System32` (never from `PATH`), returning its error output on failure.
fn run_system_tool<'a>(
    name: &str,
    args: impl IntoIterator<Item = &'a OsStr>,
) -> Result<(), String> {
    let system_root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    let program = PathBuf::from(system_root).join("System32").join(name);
    let output = hidden_command(&program)
        .args(args)
        .output()
        .map_err(|error| format!("{name}: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{name} exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn sha256_hex(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 1 << 16];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_of_known_content() {
        let dir = std::env::temp_dir().join(format!("gracil-sha-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("abc.txt");
        fs::write(&file, b"abc").unwrap();
        let hash = sha256_hex(&file).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            hash,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn pinned_build_is_well_formed() {
        assert_eq!(PINNED_BUILD.sha256.len(), 64);
        assert!(PINNED_BUILD.url.starts_with("https://"));
        assert!(PINNED_BUILD.id.starts_with("ffmpeg-"));
    }

    #[test]
    fn old_builds_are_removed_but_current_and_staging_are_kept() {
        let root = std::env::temp_dir().join(format!("gracil-builds-{}", std::process::id()));
        for name in [
            "ffmpeg-8.0-essentials",
            PINNED_BUILD.id,
            ".staging-1",
            "other",
        ] {
            fs::create_dir_all(root.join(name)).unwrap();
        }
        remove_other_builds(&root, &PINNED_BUILD);
        let mut left: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(left, [".staging-1", PINNED_BUILD.id, "other"]);
    }
}
