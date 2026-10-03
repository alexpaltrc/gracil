//! End-to-end compression with a real FFmpeg on synthetic videos.
//!
//! Skipped (with a note) when no FFmpeg is installed; these tests never download one.

// Test-only crate: a failed setup step should panic, helpers included.
#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use gracil_core::compress::{CompressError, Engine, compress};
use gracil_core::encode::JobControl;
use gracil_core::probe::{self, StreamKind};
use gracil_core::toolchain;

/// A fresh temporary folder, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "gracil-it-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn engine() -> Option<Engine> {
    let unused_private_dir = std::env::temp_dir().join("gracil-it-no-private-copy");
    let found = toolchain::find(&unused_private_dir);
    if found.is_none() {
        eprintln!("skipped: no FFmpeg installed");
    }
    found.map(Engine::with)
}

/// Makes a three-second test video with sound. `args` are the output options.
fn make(engine: &Engine, output: &Path, args: &[&str]) {
    let status = Command::new(engine.toolchain.ffmpeg_path())
        .args(["-hide_banner", "-v", "error", "-y"])
        .args(["-f", "lavfi", "-i", "testsrc2=s=320x240:r=30:d=3"])
        .args(["-f", "lavfi", "-i", "sine=f=440:d=3"])
        .args(args)
        .arg(output)
        .status()
        .unwrap();
    assert!(status.success(), "could not create {}", output.display());
}

fn leftovers(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".part"))
        .collect()
}

#[test]
fn compresses_and_keeps_metadata() {
    let Some(engine) = engine() else { return };
    let dir = TempDir::new();
    let plain = dir.0.join("plain.mp4");
    make(
        &engine,
        &plain,
        &[
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-metadata",
            "title=Cumpleaños",
            "-metadata",
            "creation_time=2024-05-01T10:00:00Z",
            "-metadata:s:a:0",
            "language=spa",
        ],
    );
    // Rotation, as phones record it, can only be added to an existing file.
    let source = dir.0.join("Mi cumpleaños 🎂.mp4");
    let status = Command::new(engine.toolchain.ffmpeg_path())
        .args([
            "-hide_banner",
            "-v",
            "error",
            "-display_rotation",
            "90",
            "-i",
        ])
        .arg(&plain)
        .args(["-map", "0", "-map_metadata", "0", "-c", "copy"])
        .arg(&source)
        .status()
        .unwrap();
    assert!(status.success());
    let original = fs::read(&source).unwrap();

    let mut updates = Vec::new();
    let result = compress(&engine, &source, &JobControl::default(), &mut |u| {
        updates.push(u)
    })
    .unwrap();

    assert_eq!(
        fs::read(&source).unwrap(),
        original,
        "source must not change"
    );
    assert!(updates.iter().any(|u| u.fraction.is_some_and(|f| f > 0.5)));

    let output = probe::probe(&engine.toolchain, &result.temp).unwrap();
    let video = output
        .streams
        .iter()
        .find(|s| s.kind == StreamKind::Video)
        .unwrap();
    assert_eq!(video.codec, "hevc");
    assert!(
        output
            .streams
            .iter()
            .any(|s| s.kind == StreamKind::Audio && s.codec == "aac")
    );
    let duration = output.duration.unwrap();
    assert!(duration > Duration::from_millis(2_500), "{duration:?}");

    let tags = ffprobe_entries(&engine, &result.temp);
    assert!(tags.contains("title=Cumpleaños"), "{tags}");
    assert!(tags.contains("creation_time=2024-05-01T10:00:00"), "{tags}");
    assert!(tags.contains("language=spa"), "{tags}");
    assert!(
        tags.contains("rotation=-90") || tags.contains("rotation=90"),
        "{tags}"
    );
}

#[test]
fn other_containers_become_mp4_with_compatible_audio() {
    let Some(engine) = engine() else { return };
    let dir = TempDir::new();
    let source = dir.0.join("old.avi");
    make(&engine, &source, &["-c:v", "mpeg4", "-c:a", "pcm_s16le"]);

    let result = compress(&engine, &source, &JobControl::default(), &mut |_| {}).unwrap();
    let output = probe::probe(&engine.toolchain, &result.temp).unwrap();
    assert!(output.streams.iter().any(|s| s.codec == "hevc"));
    assert!(output.streams.iter().any(|s| s.codec == "aac"));
}

#[test]
fn matroska_becomes_mp4_keeping_subtitles() {
    let Some(engine) = engine() else { return };
    let dir = TempDir::new();
    let subtitles = dir.0.join("subs.srt");
    fs::write(&subtitles, "1\n00:00:00,000 --> 00:00:02,000\nHola\n").unwrap();
    let source = dir.0.join("movie.mkv");
    make(
        &engine,
        &source,
        &[
            "-i",
            subtitles.to_str().unwrap(),
            "-map",
            "0",
            "-map",
            "1",
            "-map",
            "2",
            "-c:v",
            "libx264",
            "-c:a",
            "libopus",
        ],
    );

    let result = compress(&engine, &source, &JobControl::default(), &mut |_| {}).unwrap();
    let output = probe::probe(&engine.toolchain, &result.temp).unwrap();
    let codecs: Vec<_> = output
        .streams
        .iter()
        .map(|s| (s.kind, s.codec.as_str()))
        .collect();
    assert_eq!(
        codecs,
        [
            (StreamKind::Video, "hevc"),
            (StreamKind::Audio, "opus"),
            (StreamKind::Subtitle, "mov_text")
        ]
    );
    assert!(ffprobe_entries(&engine, &result.temp).contains("format_name=mov,mp4"));
}

#[test]
fn cancelling_leaves_nothing_behind() {
    let Some(engine) = engine() else { return };
    let dir = TempDir::new();
    let source = dir.0.join("cancel.mp4");
    make(&engine, &source, &["-c:v", "libx264", "-c:a", "aac"]);

    let control = JobControl::default();
    let canceller = control.clone();
    let result = compress(&engine, &source, &control, &mut |_| canceller.cancel());

    assert!(
        matches!(result, Err(CompressError::Cancelled)),
        "{result:?}"
    );
    assert!(leftovers(&dir.0).is_empty(), "{:?}", leftovers(&dir.0));
    assert!(source.is_file());
}

#[test]
fn files_without_video_are_reported() {
    let Some(engine) = engine() else { return };
    let dir = TempDir::new();
    let source = dir.0.join("song.m4a");
    let status = Command::new(engine.toolchain.ffmpeg_path())
        .args([
            "-hide_banner",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "sine=d=1",
        ])
        .arg(&source)
        .status()
        .unwrap();
    assert!(status.success());

    let result = compress(&engine, &source, &JobControl::default(), &mut |_| {});
    assert!(matches!(result, Err(CompressError::NoVideo)), "{result:?}");
    assert!(leftovers(&dir.0).is_empty());
}

/// Every tag and side-data field FFprobe reports, as `key=value` lines.
fn ffprobe_entries(engine: &Engine, path: &Path) -> String {
    let ffprobe = engine.toolchain.ffmpeg_path().with_file_name("ffprobe.exe");
    let output = Command::new(ffprobe)
        .args(["-v", "error", "-show_format", "-show_streams"])
        .args(["-of", "default=noprint_wrappers=1"])
        .arg(path)
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).replace("TAG:", "")
}
