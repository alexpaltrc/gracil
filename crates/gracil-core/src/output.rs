//! Putting a finished compression in its final place (spec §40).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::naming::save_as_new_candidate;
use crate::plan::Container;

/// Moves the validated `temp` file next to `source` as `Gracil-<name>`, never overwriting
/// anything. Returns the final path.
pub fn save_as_new(temp: &Path, source: &Path) -> io::Result<PathBuf> {
    let named_like = source.with_extension(Container::output_extension(source));
    for attempt in 1..=u32::MAX {
        let candidate = save_as_new_candidate(&named_like, attempt);
        if candidate.try_exists()? {
            continue;
        }
        // A hard link fails if the name was taken in the meantime, so nothing is ever replaced.
        match fs::hard_link(temp, &candidate) {
            Ok(()) => {
                fs::remove_file(temp)?;
                return Ok(candidate);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            // Drives without hard links (FAT32, exFAT): rename, having just checked the name.
            Err(_) => {
                fs::rename(temp, &candidate)?;
                return Ok(candidate);
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "no free name for the compressed copy",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_copy_beside_source_without_overwriting() {
        let dir = std::env::temp_dir().join(format!("gracil-output-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("Clip.AVI");
        fs::write(&source, b"original").unwrap();
        fs::write(dir.join("Gracil-Clip.mp4"), b"someone else's").unwrap();
        let temp = dir.join("Clip.AVI.gracil-1.part");
        fs::write(&temp, b"compressed").unwrap();

        let placed = save_as_new(&temp, &source).unwrap();
        let placed_content = fs::read(&placed).unwrap();
        let other = fs::read(dir.join("Gracil-Clip.mp4")).unwrap();
        let temp_left = temp.exists();
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(placed, dir.join("Gracil-Clip (2).mp4"));
        assert_eq!(placed_content, b"compressed");
        assert_eq!(other, b"someone else's");
        assert!(!temp_left);
    }
}
