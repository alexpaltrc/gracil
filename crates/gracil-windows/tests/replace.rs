//! Safe replacement end to end: the core's rules with the real Windows swap.

use std::fs::{self, File};
use std::time::{Duration, SystemTime};

use gracil_core::output::replace_original;
use gracil_windows::replace_file;

#[test]
fn replaced_video_keeps_its_identity() {
    let dir = std::env::temp_dir().join(format!("gracil-replace-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("日本語 動画.mov");
    let temp = dir.join("日本語 動画.mov.gracil-7.part");
    fs::write(&source, b"original").unwrap();
    let recorded = SystemTime::now() - Duration::from_secs(86_400);
    File::options()
        .write(true)
        .open(&source)
        .unwrap()
        .set_modified(recorded)
        .unwrap();
    let created = fs::metadata(&source).unwrap().created().unwrap();
    std::thread::sleep(Duration::from_millis(50));
    fs::write(&temp, b"compressed").unwrap();

    let placed = replace_original(&temp, &source, replace_file).unwrap();
    let metadata = fs::metadata(&placed).unwrap();
    let content = fs::read(&placed).unwrap();
    let entries = fs::read_dir(&dir).unwrap().count();
    fs::remove_dir_all(&dir).unwrap();

    assert_eq!(placed, source);
    assert_eq!(content, b"compressed");
    assert_eq!(metadata.created().unwrap(), created);
    assert_eq!(metadata.modified().unwrap(), recorded);
    assert_eq!(entries, 1, "no temporary or backup file is left");
}
