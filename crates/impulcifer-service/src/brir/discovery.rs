use super::BrirError;
use impulcifer_dsp::stages::{
    headphone::resolve_headphone_file,
    room::{RoomMeasurementName, discover_room_measurements},
};
use impulcifer_types::config::ProcessingConfig;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct RoomDiscovery {
    pub recordings: Vec<(PathBuf, RoomMeasurementName)>,
    pub generic: Option<PathBuf>,
    pub target: Option<PathBuf>,
    pub calibration: Option<PathBuf>,
}
#[derive(Debug)]
pub struct EqDiscovery {
    pub common: Option<PathBuf>,
    pub left: Option<PathBuf>,
    pub right: Option<PathBuf>,
    pub deprecated_wav: bool,
}
#[derive(Debug)]
pub struct MeasurementDir {
    pub dir: PathBuf,
    pub recordings: Vec<(PathBuf, Vec<String>)>,
    pub headphones: Option<PathBuf>,
    pub room: RoomDiscovery,
    pub eq: EqDiscovery,
    pub test_signal: Option<PathBuf>,
}
pub fn recording_speakers(name: &str) -> Option<Vec<String>> {
    let names: Vec<_> = name.strip_suffix(".wav")?.split(',').collect();
    names
        .iter()
        .all(|n| (2..=3).contains(&n.len()) && n.bytes().all(|c| c.is_ascii_uppercase()))
        .then(|| names.into_iter().map(str::to_owned).collect())
}
pub fn listing(dir: &Path) -> Result<Vec<String>, BrirError> {
    let mut names = std::fs::read_dir(dir)?
        .map(|e| e.map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    names.sort();
    Ok(names)
}
fn existing(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    names.iter().map(|n| dir.join(n)).find(|p| p.is_file())
}
pub fn discover(dir: &Path, config: &ProcessingConfig) -> Result<MeasurementDir, BrirError> {
    if !dir.is_dir() {
        return Err(BrirError::Missing(dir.display().to_string()));
    }
    let dir = std::path::absolute(dir)?;
    let names = listing(&dir)?;
    let refs: Vec<_> = names.iter().map(String::as_str).collect();
    let recordings = names
        .iter()
        .filter_map(|n| recording_speakers(n).map(|s| (dir.join(n), s)))
        .collect();
    let mut hp_listing = names.clone();
    if let Some(request) = config.headphone_compensation_file.as_deref() {
        let path = Path::new(request);
        if path.is_dir() {
            let absolute = std::path::absolute(path)?;
            hp_listing.push(format!("{}/", absolute.display()));
            for name in listing(path)? {
                hp_listing.push(absolute.join(name).to_string_lossy().into_owned());
            }
        } else if dir.join(path).is_file() {
            hp_listing.push(request.into());
        }
    }
    let requested = config.headphone_compensation_file.as_deref().map(|p| {
        if Path::new(p).is_dir() {
            std::path::absolute(p)
                .unwrap_or_else(|_| PathBuf::from(p))
                .to_string_lossy()
                .into_owned()
        } else {
            p.to_owned()
        }
    });
    let hp_refs: Vec<_> = hp_listing.iter().map(String::as_str).collect();
    let headphones = resolve_headphone_file(requested.as_deref(), &hp_refs).map(|p| dir.join(p));
    let eq_file = |base: &str| existing(&dir, &[&format!("{base}.csv"), &format!("{base}.txt")]);
    Ok(MeasurementDir {
        recordings,
        headphones,
        room: RoomDiscovery {
            recordings: discover_room_measurements(&refs)
                .into_iter()
                .map(|(p, m)| (dir.join(p), m))
                .collect(),
            generic: existing(&dir, &["room.wav"]),
            target: config
                .room_target
                .as_ref()
                .map(PathBuf::from)
                .or_else(|| existing(&dir, &["room-target.csv"])),
            calibration: config
                .room_mic_calibration
                .as_ref()
                .map(PathBuf::from)
                .or_else(|| {
                    existing(
                        &dir,
                        &["room-mic-calibration.csv", "room-mic-calibration.txt"],
                    )
                }),
        },
        eq: EqDiscovery {
            common: eq_file("eq"),
            left: eq_file("eq-left"),
            right: eq_file("eq-right"),
            deprecated_wav: dir.join("eq.wav").is_file(),
        },
        test_signal: existing(&dir, &["test.wav"]),
        dir,
    })
}
