//! core/recording_naming.py; preserve the caller's unnormalised directory.
use impulcifer_types::constants::SPEAKER_NAMES;
use std::path::Path;

pub fn normalize_speakers(speakers: &[String]) -> Result<Vec<String>, String> {
    let names: Vec<_> = speakers
        .iter()
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .collect();
    if names.is_empty() {
        return Err("At least one speaker name is required.".into());
    }
    for name in &names {
        if !SPEAKER_NAMES.contains(&name.as_str()) {
            return Err(format!("\"{name}\" is not a recognised speaker name."));
        }
    }
    let mut seen = std::collections::HashSet::new();
    if names.iter().any(|s| !seen.insert(s)) {
        return Err("Speaker names must be unique.".into());
    }
    Ok(names)
}
pub fn record_filename_for_speakers(speakers: &[String]) -> Result<String, String> {
    Ok(format!("{}.wav", normalize_speakers(speakers)?.join(",")))
}
pub fn derive_record_filename(play: &str) -> String {
    if play.is_empty() {
        return "headphones.wav".into();
    }
    let basename = play.rsplit(std::path::is_separator).next().unwrap_or("");
    let upper = basename.to_uppercase();
    for (index, _) in upper.match_indices("SWEEP-SEG-") {
        let rest = &upper[index + 10..];
        if let Some((names, _)) = rest.split_once('-')
            && names.split(',').all(|s| SPEAKER_NAMES.contains(&s))
        {
            return format!("{names}.wav");
        }
    }
    let stem = match basename.rfind('.') {
        Some(i) if basename[..i].chars().any(|c| c != '.') => &basename[..i],
        _ => basename,
    };
    format!("{stem}.wav")
}
pub fn resolve_record_path(dir: &str, play: &str) -> String {
    Path::new(dir)
        .join(derive_record_filename(play))
        .to_string_lossy()
        .into_owned()
}
pub fn resolve_record_path_for_speakers(dir: &str, speakers: &[String]) -> Result<String, String> {
    Ok(Path::new(dir)
        .join(record_filename_for_speakers(speakers)?)
        .to_string_lossy()
        .into_owned())
}
pub fn resolve_headphones_record_path(dir: &str) -> String {
    Path::new(dir)
        .join("headphones.wav")
        .to_string_lossy()
        .into_owned()
}
