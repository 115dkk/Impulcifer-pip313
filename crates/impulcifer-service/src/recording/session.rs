//! Where a recording lands when its folder already holds earlier takes. An app
//! run (one service) never replaces a recording it did not make itself.
//!
//! - A headphone recording takes the first free name in the run's folder:
//!   headphones.wav, then headphones_n.wav, headphones_nn.wav and so on.
//! - The run's folder is the requested folder until a speaker recording would
//!   replace a file the run did not write. From then on the run records into a
//!   new subfolder `n` (or `nn`, `nnn`… when `n` is taken), headphones
//!   included, so each measurement stays one set BRIR processing can read.
//! - Within the run a speaker recording replaces its own earlier take (a
//!   guided step recorded again), and appending adds to the file in the run's
//!   folder.
use super::{naming, request::Mode};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct Sessions {
    folders: HashMap<String, Session>,
}

#[derive(Clone, Default)]
struct Session {
    /// Subfolder of the requested folder; empty for the folder itself.
    sub: String,
    /// Speaker recordings this run made in `sub` (folded names).
    written: HashSet<String>,
}

pub struct Placement {
    pub record_path: String,
    /// The subfolder this recording moves the run to, if it does.
    pub new_folder: Option<String>,
    key: String,
    /// The run's state once the recording has started; None leaves it as is.
    session: Option<Session>,
}

impl Sessions {
    /// Where a recording of `name` requested in `dir` goes; nothing changes
    /// until [`Sessions::commit`]. `dir` stays as the caller wrote it.
    pub fn place(&self, mode: Mode, append: bool, dir: &str, name: &str) -> Placement {
        let key = folded(&absolute_text(dir));
        let mut session = self.folders.get(&key).cloned().unwrap_or_default();
        let root = Path::new(dir);
        let folder = |sub: &str| -> PathBuf {
            if sub.is_empty() {
                root.to_path_buf()
            } else {
                root.join(sub)
            }
        };
        let mut new_folder = None;
        let (path, session) = match mode {
            Mode::Headphones => (
                naming::first_free_path(&folder(&session.sub), "headphones.wav"),
                None,
            ),
            Mode::Speakers if append => (folder(&session.sub).join(name), None),
            Mode::Speakers => {
                let target = folder(&session.sub).join(name);
                if taken(&target) && !session.written.contains(&folded(name)) {
                    let sub = first_free_folder(root);
                    session = Session {
                        sub: sub.clone(),
                        written: HashSet::new(),
                    };
                    new_folder = Some(sub);
                }
                session.written.insert(folded(name));
                (folder(&session.sub).join(name), Some(session))
            }
        };
        Placement {
            record_path: path.to_string_lossy().into_owned(),
            new_folder,
            key,
            session,
        }
    }

    /// Record that the placed recording started.
    pub fn commit(&mut self, placement: Placement) {
        if let Some(session) = placement.session {
            self.folders.insert(placement.key, session);
        }
    }
}

/// `n`, `nn`, `nnn`…: the first that is not in `root` yet.
fn first_free_folder(root: &Path) -> String {
    let mut sub = String::from("n");
    while taken(&root.join(&sub)) {
        sub.push('n');
    }
    sub
}

/// symlink_metadata, so a dangling link also counts as taken.
fn taken(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

fn absolute_text(dir: &str) -> String {
    let path = std::path::absolute(dir).unwrap_or_else(|_| PathBuf::from(dir));
    let text = path.to_string_lossy();
    let trimmed = text.trim_end_matches(std::path::is_separator);
    if trimmed.is_empty() {
        text.into_owned()
    } else {
        trimmed.to_owned()
    }
}

/// Windows and macOS file names ignore case by default.
fn folded(text: &str) -> String {
    if cfg!(any(windows, target_os = "macos")) {
        text.to_lowercase()
    } else {
        text.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Temp(PathBuf);
    impl Temp {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("impulcifer-session-{}-{tag}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Place, commit and write the file, as a finished recording does.
    fn record(sessions: &mut Sessions, mode: Mode, dir: &Path, name: &str) -> PathBuf {
        let placement = sessions.place(mode, false, &dir.to_string_lossy(), name);
        let path = PathBuf::from(&placement.record_path);
        sessions.commit(placement);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"take").unwrap();
        path
    }

    #[test]
    fn a_run_moves_to_a_subfolder_instead_of_replacing_an_earlier_run() {
        let temp = Temp::new("earlier-run");
        let root = &temp.0;
        std::fs::write(root.join("FL,FR.wav"), b"earlier").unwrap();
        std::fs::write(root.join("headphones.wav"), b"earlier").unwrap();
        let mut sessions = Sessions::default();
        // The preview does not move the run; only a started recording does.
        let preview = sessions.place(Mode::Speakers, false, &root.to_string_lossy(), "FL,FR.wav");
        assert_eq!(preview.new_folder.as_deref(), Some("n"));
        assert_eq!(
            PathBuf::from(&preview.record_path),
            root.join("n").join("FL,FR.wav")
        );
        assert_eq!(
            record(&mut sessions, Mode::Speakers, root, "FL,FR.wav"),
            root.join("n").join("FL,FR.wav")
        );
        // The rest of the run follows, headphones included, and a speaker
        // recorded again replaces the run's own take.
        assert_eq!(
            record(&mut sessions, Mode::Speakers, root, "FC.wav"),
            root.join("n").join("FC.wav")
        );
        let again = sessions.place(Mode::Speakers, false, &root.to_string_lossy(), "FL,FR.wav");
        assert_eq!(again.new_folder, None);
        assert_eq!(
            PathBuf::from(&again.record_path),
            root.join("n").join("FL,FR.wav")
        );
        assert_eq!(
            record(&mut sessions, Mode::Headphones, root, "headphones.wav"),
            root.join("n").join("headphones.wav")
        );
        assert_eq!(
            record(&mut sessions, Mode::Headphones, root, "headphones.wav"),
            root.join("n").join("headphones_n.wav")
        );
        assert_eq!(std::fs::read(root.join("FL,FR.wav")).unwrap(), b"earlier");
        assert_eq!(
            std::fs::read(root.join("headphones.wav")).unwrap(),
            b"earlier"
        );
        // The next run skips the subfolder the last one made.
        let mut next = Sessions::default();
        assert_eq!(
            record(&mut next, Mode::Speakers, root, "FL,FR.wav"),
            root.join("nn").join("FL,FR.wav")
        );
    }

    #[test]
    fn a_run_stays_in_the_folder_until_it_would_replace_an_earlier_run() {
        let temp = Temp::new("continue");
        let root = &temp.0;
        std::fs::write(root.join("FL,FR.wav"), b"earlier").unwrap();
        let mut sessions = Sessions::default();
        // A speaker the folder does not hold yet joins the earlier set, and
        // the run replaces its own take of it.
        assert_eq!(
            record(&mut sessions, Mode::Speakers, root, "FC.wav"),
            root.join("FC.wav")
        );
        assert_eq!(
            record(&mut sessions, Mode::Speakers, root, "FC.wav"),
            root.join("FC.wav")
        );
        // Recording FL,FR again would replace the earlier run's take.
        assert_eq!(
            record(&mut sessions, Mode::Speakers, root, "FL,FR.wav"),
            root.join("n").join("FL,FR.wav")
        );
        assert_eq!(std::fs::read(root.join("FL,FR.wav")).unwrap(), b"earlier");
        // Appending adds to the file in the run's folder and moves nothing.
        let append = sessions.place(Mode::Speakers, true, &root.to_string_lossy(), "FL,FR.wav");
        assert_eq!(append.new_folder, None);
        assert_eq!(
            PathBuf::from(&append.record_path),
            root.join("n").join("FL,FR.wav")
        );
    }

    #[test]
    fn appending_to_an_earlier_take_keeps_its_file() {
        let temp = Temp::new("append");
        let root = &temp.0;
        std::fs::write(root.join("FL,FR.wav"), b"earlier").unwrap();
        let sessions = Sessions::default();
        let placement = sessions.place(Mode::Speakers, true, &root.to_string_lossy(), "FL,FR.wav");
        assert_eq!(placement.new_folder, None);
        assert_eq!(
            PathBuf::from(&placement.record_path),
            root.join("FL,FR.wav")
        );
    }
}
