//! Data files carried inside the library and written out where the service
//! can open them by path. An Android package holds its files as archive
//! entries, not as files on disk, so the desktop's bundled `data` folder has no
//! counterpart there (ADR 0020).

use std::fs;
use std::io;
use std::path::Path;

/// Writes every file that is missing or differs into `directory`, creating
/// it first. A file that already holds the same bytes is left alone, so a
/// normal start writes nothing. A new file appears under its final name only
/// once it is complete.
pub fn install(directory: &Path, files: &[(&str, &[u8])]) -> io::Result<()> {
    fs::create_dir_all(directory)?;
    for (name, bytes) in files {
        let path = directory.join(name);
        if fs::read(&path).is_ok_and(|current| current == *bytes) {
            continue;
        }
        let partial = directory.join(format!("{name}.partial"));
        fs::write(&partial, bytes)?;
        fs::rename(&partial, &path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::install;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Folder(PathBuf);

    impl Folder {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            Self(std::env::temp_dir().join(format!(
                "impulcifer-bundled-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn bundled_files_are_written_once_and_repaired_when_they_differ() {
        let folder = Folder::new();
        let data = folder.0.join("nested").join("data");
        let files: [(&str, &[u8]); 2] = [("sweep.wav", b"RIFF sweep"), ("notes.csv", b"1,2\n")];
        install(&data, &files).unwrap();
        assert_eq!(fs::read(data.join("sweep.wav")).unwrap(), b"RIFF sweep");
        assert_eq!(fs::read(data.join("notes.csv")).unwrap(), b"1,2\n");

        // Unchanged files are not rewritten: the modification time stays.
        let before = fs::metadata(data.join("sweep.wav"))
            .unwrap()
            .modified()
            .unwrap();
        install(&data, &files).unwrap();
        let after = fs::metadata(data.join("sweep.wav"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(before, after);

        // A damaged or outdated file is replaced, and nothing partial stays.
        fs::write(data.join("sweep.wav"), b"RIFF").unwrap();
        fs::remove_file(data.join("notes.csv")).unwrap();
        install(&data, &files).unwrap();
        assert_eq!(fs::read(data.join("sweep.wav")).unwrap(), b"RIFF sweep");
        assert_eq!(fs::read(data.join("notes.csv")).unwrap(), b"1,2\n");
        let names: Vec<_> = fs::read_dir(&data)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        assert!(
            names.iter().all(|name| !name.ends_with(".partial")),
            "{names:?}"
        );
    }
}
