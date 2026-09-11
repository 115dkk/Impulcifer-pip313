#![forbid(unsafe_code)]

//! Output recovery by channel identity only (core/brir_recovery.py).
//! Existing sources are validated before staging any new PCM_32 outputs.
use impulcifer_dsp::hrir::compact_tracks;
use impulcifer_io::{
    IoError, Wav,
    brir_layout::{append_track_names, read_track_names},
    read_wav,
};
use impulcifer_jobs::registry::JobFailure;
use impulcifer_types::{
    constants::{
        HESUVI_TRACK_ORDER, HEXADECAGONAL_TRACK_ORDER, SPEAKER_NAMES, base_channel_count,
        track_name,
    },
    ipc::{self, ErrorCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecoveryErrorCode {
    InvalidDirectory,
    NoRecoverySource,
    AmbiguousSource,
    InvalidWav,
    InvalidChannelMap,
    InvalidChannelCount,
    NonSilentLfe,
    SampleRateMismatch,
    SampleCountMismatch,
    SourceMismatch,
    AllChannelsSilent,
    OutputConflict,
    OutputWriteFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[error("{message}")]
pub struct RecoveryError {
    pub code: RecoveryErrorCode,
    pub message: String,
    pub details: Value,
}
impl RecoveryError {
    fn new(code: RecoveryErrorCode, message: impl Into<String>, details: Value) -> Self {
        Self {
            code,
            message: message.into(),
            details,
        }
    }
    pub(crate) fn failure(self) -> JobFailure {
        JobFailure {
            error: json!({"code":self.code,"message":self.message,"details":self.details,"retryable":false}),
            cancelled: false,
        }
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RecoveryOptions {
    pub include_hangloose: bool,
    pub remove_silent_channels: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryResult {
    pub source_kind: String,
    pub source_path: String,
    pub output_dir: String,
    pub sample_rate: u32,
    pub sample_count: usize,
    pub speakers: Vec<String>,
    pub created_files: Vec<String>,
    pub existing_files: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedFile {
    pub path: String,
    pub kind: String,
    pub channels: usize,
    pub speaker: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryPlan {
    pub source_kind: String,
    pub source_path: String,
    pub output_dir: String,
    pub sample_rate: u32,
    pub sample_count: usize,
    pub speakers: Vec<String>,
    pub existing_files: Vec<String>,
    pub planned_files: Vec<PlannedFile>,
    pub hangloose_dir: Option<String>,
}

pub(crate) fn validate_request(raw: &Value) -> Result<(PathBuf, RecoveryOptions), Value> {
    let invalid = |message| ipc::error(ErrorCode::InvalidRequest, message, json!({}), false);
    let object = raw
        .as_object()
        .ok_or_else(|| invalid("Output recovery request must be an object."))?;
    let mut unknown: Vec<_> = object
        .keys()
        .filter(|k| {
            !["dir_path", "include_hangloose", "remove_silent_channels"].contains(&k.as_str())
        })
        .collect();
    unknown.sort();
    if !unknown.is_empty() {
        return Err(ipc::error(
            ErrorCode::InvalidRequest,
            "Unknown output recovery fields.",
            json!({"fields":unknown}),
            false,
        ));
    }
    let path = raw["dir_path"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| invalid("dir_path must be a directory path."))?;
    if !Path::new(path).is_dir() {
        return Err(ipc::error(
            ErrorCode::FileNotFound,
            "Recovery directory does not exist.",
            json!({"path":path}),
            false,
        ));
    }
    let boolean = |key: &str| {
        object.get(key).map_or(Ok(false), |v| {
            v.as_bool().ok_or_else(|| {
                ipc::error(
                    ErrorCode::InvalidRequest,
                    format!("{key} must be a boolean."),
                    json!({}),
                    false,
                )
            })
        })
    };
    let remove_silent_channels = boolean("remove_silent_channels")?;
    let include_hangloose = boolean("include_hangloose")?;
    Ok((
        PathBuf::from(path),
        RecoveryOptions {
            include_hangloose,
            remove_silent_channels,
        },
    ))
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
fn filename(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}
fn io_message(error: IoError) -> String {
    match error {
        IoError::Format(s) | IoError::InvalidArgument(s) => s,
        other => other.to_string(),
    }
}
fn invalid_directory() -> RecoveryError {
    RecoveryError::new(
        RecoveryErrorCode::InvalidDirectory,
        "The selected recovery directory is invalid.",
        json!({}),
    )
}
// canonicalize uses verbatim Windows paths; Python Path.resolve uses ordinary
// drive/UNC paths in its public manifest. Keep verbatim paths out of the wire.
fn ordinary_path(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let text = path.to_string_lossy();
        if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{unc}"));
        }
        if let Some(drive) = text.strip_prefix(r"\\?\") {
            return PathBuf::from(drive);
        }
    }
    path
}
fn expand_user(directory: &Path, raw: &str) -> Result<PathBuf, RecoveryError> {
    if !raw.starts_with('~') {
        return Ok(directory.to_owned());
    }
    let end = raw
        .find(|c| c == '/' || (cfg!(windows) && c == '\\'))
        .unwrap_or(raw.len());
    let user = &raw[1..end];
    #[cfg(windows)]
    let home = {
        let mut home = std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOMEPATH").map(|path| {
                    let mut drive = std::env::var_os("HOMEDRIVE").unwrap_or_default();
                    drive.push(path);
                    PathBuf::from(drive)
                })
            })
            .ok_or_else(invalid_directory)?;
        if !user.is_empty() && std::env::var("USERNAME").ok().as_deref() != Some(user) {
            if std::env::var("USERNAME").ok().as_deref()
                != home.file_name().and_then(|n| n.to_str())
            {
                return Err(invalid_directory());
            }
            home = home.parent().ok_or_else(invalid_directory)?.join(user);
        }
        home
    };
    #[cfg(not(windows))]
    let home = {
        // The safe standard library has no NSS/getpwnam API. HOME handles the
        // ordinary case; /etc/passwd covers local named users without a shell.
        let environment = user.is_empty().then(|| std::env::var_os("HOME")).flatten();
        if let Some(home) = environment {
            PathBuf::from(home)
        } else {
            let requested = if user.is_empty() {
                std::env::var("USER").ok()
            } else {
                Some(user.to_owned())
            };
            let passwd = fs::read_to_string("/etc/passwd").map_err(|_| invalid_directory())?;
            let home = passwd
                .lines()
                .find_map(|line| {
                    let fields: Vec<_> = line.split(':').collect();
                    (fields.len() >= 6 && Some(fields[0]) == requested.as_deref())
                        .then(|| fields[5].to_owned())
                })
                .ok_or_else(invalid_directory)?;
            PathBuf::from(home)
        }
    };
    // Concatenate, as expanduser does: join would discard home for '//child'.
    let mut expanded = home.into_os_string();
    expanded.push(&raw[end..]);
    Ok(PathBuf::from(expanded))
}
fn resolve_directory(directory: &Path) -> Result<PathBuf, RecoveryError> {
    let raw = path_text(directory);
    if raw.contains('\0') {
        return Err(invalid_directory());
    }
    let expanded = expand_user(directory, &raw)?;
    let absolute = std::path::absolute(expanded).map_err(|_| invalid_directory())?;
    // Resolve existing components, including symlinks, before handling '..'.
    // Unlike canonicalize alone, Path.resolve also returns absent destinations.
    let mut resolved = PathBuf::new();
    for part in absolute.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            other => {
                resolved.push(other.as_os_str());
                if let Ok(real) = fs::canonicalize(&resolved) {
                    resolved = ordinary_path(real);
                }
            }
        }
    }
    if !resolved.is_dir() {
        return Err(RecoveryError::new(
            RecoveryErrorCode::InvalidDirectory,
            "The selected recovery directory does not exist.",
            json!({"directory":resolved}),
        ));
    }
    Ok(resolved)
}

fn children(dir: &Path) -> Result<Vec<PathBuf>, RecoveryError> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    // Sorted: Python's iterdir follows the filesystem (alphabetical on NTFS,
    // where the goldens were made; arbitrary on APFS/ext4), so a fixed order
    // keeps error lists and scans identical on every platform.
    fs::read_dir(dir)
        .and_then(|it| it.map(|e| e.map(|e| e.path())).collect())
        .map(|mut paths: Vec<PathBuf>| {
            paths.sort();
            paths
        })
        .map_err(|e| {
            RecoveryError::new(
                RecoveryErrorCode::InvalidDirectory,
                "The selected recovery directory is invalid.",
                json!({"directory":dir,"reason":e.to_string()}),
            )
        })
}
fn find_named(dir: &Path, name: &str, is_dir: bool) -> Result<Option<PathBuf>, RecoveryError> {
    let matches: Vec<_> = children(dir)?
        .into_iter()
        .filter(|p| {
            (if is_dir { p.is_dir() } else { p.is_file() })
                && casefold(&filename(p)) == casefold(name)
        })
        .collect();
    if matches.len() > 1 {
        let (noun, key) = if is_dir {
            ("directories", "directories")
        } else {
            ("files", "files")
        };
        return Err(RecoveryError::new(
            RecoveryErrorCode::AmbiguousSource,
            format!("Multiple {noun} match {name}."),
            json!({key:matches}),
        ));
    }
    Ok(matches.into_iter().next())
}
fn find_split(dir: &Path) -> Result<Vec<(String, PathBuf)>, RecoveryError> {
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    let mut prefix = None;
    let mut suffixes = SPEAKER_NAMES;
    suffixes.sort_by_key(|s| std::cmp::Reverse(s.len()));
    for path in children(dir)? {
        if !path.is_file()
            || !path
                .extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("wav"))
        {
            continue;
        }
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let normalized = casefold(&stem);
        let Some(speaker) = suffixes
            .iter()
            .find(|s| normalized.ends_with(&s.to_ascii_lowercase()))
        else {
            continue;
        };
        // Python slices code points, not UTF-8 bytes (e.g. a long-s suffix).
        let prefix_len = stem.chars().count().saturating_sub(speaker.len());
        let file_prefix = casefold(&stem.chars().take(prefix_len).collect::<String>());
        if prefix.as_ref().is_some_and(|p| *p != file_prefix) {
            let mut files: Vec<_> = found.iter().map(|(_, p)| p.clone()).collect();
            files.push(path);
            return Err(RecoveryError::new(
                RecoveryErrorCode::AmbiguousSource,
                "Hangloose files must use one shared filename prefix.",
                json!({"files":files}),
            ));
        }
        prefix = Some(file_prefix);
        if let Some((_, previous)) = found.iter().find(|(s, _)| s == speaker) {
            return Err(RecoveryError::new(
                RecoveryErrorCode::AmbiguousSource,
                format!("Multiple Hangloose files match speaker {speaker}."),
                json!({"files":[previous,path]}),
            ));
        }
        found.push((speaker.to_string(), path));
    }
    Ok(found)
}
fn split_dir(dir: &Path) -> Result<Option<PathBuf>, RecoveryError> {
    if let Some(nested) = find_named(dir, "Hangloose", true)?
        && !find_split(&nested)?.is_empty()
    {
        return Ok(Some(nested));
    }
    Ok(None)
}
fn locate(selected: &Path) -> Result<(PathBuf, Option<PathBuf>), RecoveryError> {
    if find_named(selected, "hrir.wav", false)?.is_some()
        || find_named(selected, "hesuvi.wav", false)?.is_some()
    {
        return Ok((selected.to_owned(), split_dir(selected)?));
    }
    let loose = find_split(selected)?;
    if casefold(&filename(selected)) == "hangloose" && !loose.is_empty() {
        return Ok((
            selected.parent().unwrap_or(selected).to_owned(),
            Some(selected.to_owned()),
        ));
    }
    if let Some(nested) = split_dir(selected)? {
        return Ok((selected.to_owned(), Some(nested)));
    }
    Ok((
        selected.to_owned(),
        (!loose.is_empty()).then(|| selected.to_owned()),
    ))
}

struct TrackSet {
    tracks: HashMap<String, Vec<f64>>,
    rate: u32,
    count: usize,
    speakers: Vec<String>,
}
fn speaker_tracks() -> impl Iterator<Item = String> {
    SPEAKER_NAMES
        .into_iter()
        .flat_map(|s| [track_name(s, "left"), track_name(s, "right")])
}
fn read_matrix(path: &Path) -> Result<Wav, RecoveryError> {
    let wav = read_wav(path).map_err(|e| {
        RecoveryError::new(
            RecoveryErrorCode::InvalidWav,
            format!("Could not read {} as a WAV file.", filename(path)),
            json!({"path":path,"reason":io_message(e)}),
        )
    })?;
    let count = wav.tracks.first().map_or(0, Vec::len);
    if count == 0 {
        return Err(RecoveryError::new(
            RecoveryErrorCode::InvalidWav,
            format!("{} contains no usable audio samples.", filename(path)),
            json!({"path":path,"shape":[wav.tracks.len(),count]}),
        ));
    }
    if wav.tracks.iter().flatten().any(|x| !x.is_finite()) {
        return Err(RecoveryError::new(
            RecoveryErrorCode::InvalidWav,
            format!("{} contains NaN or infinite samples.", filename(path)),
            json!({"path":path}),
        ));
    }
    Ok(wav)
}
fn read_combined(path: &Path, order: &[&str], hrir: bool) -> Result<TrackSet, RecoveryError> {
    let wav = read_matrix(path)?;
    let channels = wav.tracks.len();
    let count = wav.tracks[0].len();
    let map = read_track_names(path, order, channels).map_err(|e| {
        RecoveryError::new(
            RecoveryErrorCode::InvalidChannelMap,
            io_message(e),
            json!({"path":path}),
        )
    })?;
    let minimum = base_channel_count(order);
    if map.is_none() && (channels < minimum || channels > order.len() || channels % 2 != 0) {
        return Err(RecoveryError::new(
            RecoveryErrorCode::InvalidChannelCount,
            format!(
                "{} must contain {minimum}–{} channels in complete stereo pairs.",
                filename(path),
                order.len()
            ),
            json!({"path":path,"expected":(minimum..=order.len()).step_by(2).collect::<Vec<_>>(),"actual":channels}),
        ));
    }
    let names = map.unwrap_or_else(|| order.iter().map(|s| s.to_string()).collect());
    let mut tracks: HashMap<_, _> = names.into_iter().zip(wav.tracks).collect();
    for name in speaker_tracks().chain(["LFE-left".into(), "LFE-right".into()]) {
        tracks.entry(name).or_insert_with(|| vec![0.0; count]);
    }
    if hrir {
        let non_silent: Vec<_> = ["LFE-left", "LFE-right"]
            .into_iter()
            .filter(|n| tracks[*n].iter().any(|x| *x != 0.0))
            .collect();
        if !non_silent.is_empty() {
            return Err(RecoveryError::new(
                RecoveryErrorCode::NonSilentLfe,
                "hrir.wav contains non-silent LFE tracks that cannot be represented in hesuvi.wav.",
                json!({"path":path,"tracks":non_silent}),
            ));
        }
    }
    let speakers = SPEAKER_NAMES
        .into_iter()
        .filter(|s| {
            ["left", "right"]
                .into_iter()
                .any(|side| tracks[&track_name(s, side)].iter().any(|x| *x != 0.0))
        })
        .map(str::to_owned)
        .collect();
    tracks.remove("LFE-left");
    tracks.remove("LFE-right");
    Ok(TrackSet {
        tracks,
        rate: wav.sample_rate,
        count,
        speakers,
    })
}
fn read_split(dir: &Path) -> Result<(TrackSet, Vec<PathBuf>), RecoveryError> {
    let files = find_split(dir)?;
    if files.is_empty() {
        return Err(RecoveryError::new(
            RecoveryErrorCode::NoRecoverySource,
            "No Hangloose speaker WAV files were found.",
            json!({"directory":dir}),
        ));
    }
    let mut set = TrackSet {
        tracks: HashMap::new(),
        rate: 0,
        count: 0,
        speakers: Vec::new(),
    };
    let mut paths = Vec::new();
    for speaker in SPEAKER_NAMES {
        let Some((_, path)) = files.iter().find(|(s, _)| s == speaker) else {
            continue;
        };
        let wav = read_matrix(path)?;
        if wav.tracks.len() != 2 {
            return Err(RecoveryError::new(
                RecoveryErrorCode::InvalidChannelCount,
                format!("Hangloose file {} must be stereo.", filename(path)),
                json!({"path":path,"actual":wav.tracks.len()}),
            ));
        }
        let count = wav.tracks[0].len();
        if paths.is_empty() {
            set.rate = wav.sample_rate;
            set.count = count;
        } else if set.rate != wav.sample_rate {
            return Err(RecoveryError::new(
                RecoveryErrorCode::SampleRateMismatch,
                "Hangloose files must use the same sample rate.",
                json!({"path":path,"expected":set.rate,"actual":wav.sample_rate}),
            ));
        } else if set.count != count {
            return Err(RecoveryError::new(
                RecoveryErrorCode::SampleCountMismatch,
                "Hangloose files must contain the same number of samples.",
                json!({"path":path,"expected":set.count,"actual":count}),
            ));
        }
        for (side, data) in ["left", "right"].into_iter().zip(wav.tracks) {
            set.tracks.insert(track_name(speaker, side), data);
        }
        set.speakers.push(speaker.into());
        paths.push(path.clone());
    }
    for name in speaker_tracks() {
        set.tracks
            .entry(name)
            .or_insert_with(|| vec![0.0; set.count]);
    }
    Ok((set, paths))
}
fn verify_pair(
    hrir: &TrackSet,
    hesuvi: &TrackSet,
    hp: &Path,
    vp: &Path,
) -> Result<(), RecoveryError> {
    if hrir.rate != hesuvi.rate {
        return Err(RecoveryError::new(
            RecoveryErrorCode::SampleRateMismatch,
            "hrir.wav and hesuvi.wav use different sample rates.",
            json!({"hrir":hrir.rate,"hesuvi":hesuvi.rate}),
        ));
    }
    if hrir.count != hesuvi.count {
        return Err(RecoveryError::new(
            RecoveryErrorCode::SampleCountMismatch,
            "hrir.wav and hesuvi.wav contain different sample counts.",
            json!({"hrir":hrir.count,"hesuvi":hesuvi.count}),
        ));
    }
    let mismatched: Vec<_> = speaker_tracks()
        .filter(|n| hrir.tracks[n] != hesuvi.tracks[n])
        .collect();
    if !mismatched.is_empty() {
        return Err(RecoveryError::new(
            RecoveryErrorCode::SourceMismatch,
            "hrir.wav and hesuvi.wav do not contain the same impulse responses.",
            json!({"hrir_path":hp,"hesuvi_path":vp,"tracks":mismatched}),
        ));
    }
    Ok(())
}
fn verify_subset(
    combined: &TrackSet,
    split: &TrackSet,
    files: &[PathBuf],
) -> Result<(), RecoveryError> {
    if combined.rate != split.rate {
        return Err(RecoveryError::new(
            RecoveryErrorCode::SampleRateMismatch,
            "Existing Hangloose files use a different sample rate.",
            json!({"files":files}),
        ));
    }
    if combined.count != split.count {
        return Err(RecoveryError::new(
            RecoveryErrorCode::SampleCountMismatch,
            "Existing Hangloose files use a different sample count.",
            json!({"files":files}),
        ));
    }
    let mismatched: Vec<_> = split
        .speakers
        .iter()
        .flat_map(|s| [track_name(s, "left"), track_name(s, "right")])
        .filter(|n| combined.tracks[n] != split.tracks[n])
        .collect();
    if !mismatched.is_empty() {
        return Err(RecoveryError::new(
            RecoveryErrorCode::SourceMismatch,
            "Existing Hangloose files do not match the combined BRIR source.",
            json!({"files":files,"tracks":mismatched}),
        ));
    }
    Ok(())
}
struct PlannedOutput {
    target: PathBuf,
    tracks: Vec<Vec<f64>>,
    names: Vec<String>,
    kind: &'static str,
    speaker: Option<String>,
}
fn combined_output(
    dir: &Path,
    name: &str,
    set: &TrackSet,
    order: &[&str],
    compact: bool,
) -> Result<PlannedOutput, RecoveryError> {
    let mut tracks: Vec<_> = order
        .iter()
        .map(|n| {
            set.tracks
                .get(*n)
                .cloned()
                .unwrap_or_else(|| vec![0.0; set.count])
        })
        .collect();
    let minimum = base_channel_count(order);
    while tracks.len() > minimum
        && tracks[tracks.len() - 2..]
            .iter()
            .flatten()
            .all(|x| *x == 0.0)
    {
        tracks.truncate(tracks.len() - 2);
    }
    let mut names = Vec::new();
    if compact {
        (tracks, names) = compact_tracks(&tracks, &order[..tracks.len()]);
        if tracks.is_empty() {
            return Err(RecoveryError::new(
                RecoveryErrorCode::AllChannelsSilent,
                "All BRIR channels are silent; no channels remain to write.",
                json!({}),
            ));
        }
    }
    Ok(PlannedOutput {
        target: dir.join(name),
        tracks,
        names,
        kind: if name == "hrir.wav" { "hrir" } else { "hesuvi" },
        speaker: None,
    })
}

struct PreparedRecovery {
    plan: RecoveryPlan,
    outputs: Vec<PlannedOutput>,
}

fn prepare(dir: &Path, options: &RecoveryOptions) -> Result<PreparedRecovery, RecoveryError> {
    let selected = resolve_directory(dir)?;
    let (output, split) = locate(&selected)?;
    let hp = find_named(&output, "hrir.wav", false)?;
    let vp = find_named(&output, "hesuvi.wav", false)?;
    let mut existing = Vec::new();
    let (set, kind, source) = match (&hp, &vp) {
        (Some(hp), Some(vp)) => {
            let hrir = read_combined(hp, &HEXADECAGONAL_TRACK_ORDER, true)?;
            let hesuvi = read_combined(vp, &HESUVI_TRACK_ORDER, false)?;
            verify_pair(&hrir, &hesuvi, hp, vp)?;
            existing.extend([hp.clone(), vp.clone()]);
            (hrir, "hrir+hesuvi", output.clone())
        }
        (Some(path), None) => {
            existing.push(path.clone());
            (
                read_combined(path, &HEXADECAGONAL_TRACK_ORDER, true)?,
                "hrir",
                path.clone(),
            )
        }
        (None, Some(path)) => {
            existing.push(path.clone());
            (
                read_combined(path, &HESUVI_TRACK_ORDER, false)?,
                "hesuvi",
                path.clone(),
            )
        }
        (None, None) => {
            let dir = split.as_ref().ok_or_else(|| {
                RecoveryError::new(
                    RecoveryErrorCode::NoRecoverySource,
                    "No hrir.wav, hesuvi.wav, or Hangloose speaker WAV files were found.",
                    json!({"directory":selected}),
                )
            })?;
            let (set, files) = read_split(dir)?;
            existing.extend(files);
            (set, "hangloose", dir.clone())
        }
    };
    let mut outputs = Vec::new();
    if hp.is_none() {
        outputs.push(combined_output(
            &output,
            "hrir.wav",
            &set,
            &HEXADECAGONAL_TRACK_ORDER,
            options.remove_silent_channels,
        )?);
    }
    if vp.is_none() {
        outputs.push(combined_output(
            &output,
            "hesuvi.wav",
            &set,
            &HESUVI_TRACK_ORDER,
            options.remove_silent_channels,
        )?);
    }
    let hangloose_dir = if options.include_hangloose {
        Some(split.clone().unwrap_or_else(|| output.join("Hangloose")))
    } else if kind == "hangloose" {
        split.clone()
    } else {
        None
    };
    if options.include_hangloose && kind != "hangloose" {
        let dir = hangloose_dir
            .clone()
            .expect("requested Hangloose directory");
        let files = find_split(&dir)?;
        if !files.is_empty() {
            let (split, paths) = read_split(&dir)?;
            verify_subset(&set, &split, &paths)?;
            existing.extend(paths);
        }
        for speaker in &set.speakers {
            if files.iter().any(|(s, _)| s == speaker) {
                continue;
            }
            outputs.push(PlannedOutput {
                target: dir.join(format!("{speaker}.wav")),
                tracks: ["left", "right"]
                    .into_iter()
                    .map(|side| set.tracks[&track_name(speaker, side)].clone())
                    .collect(),
                names: Vec::new(),
                kind: "hangloose",
                speaker: Some(speaker.clone()),
            });
        }
    }
    let mut seen = HashSet::new();
    existing.retain(|p| {
        let resolved = fs::canonicalize(p).unwrap_or_else(|_| p.clone());
        let key = path_text(&resolved);
        seen.insert(if cfg!(windows) {
            key.to_lowercase()
        } else {
            key
        })
    });
    let planned_files = outputs
        .iter()
        .map(|item| PlannedFile {
            path: path_text(&item.target),
            kind: item.kind.into(),
            channels: item.tracks.len(),
            speaker: item.speaker.clone(),
        })
        .collect();
    Ok(PreparedRecovery {
        plan: RecoveryPlan {
            source_kind: kind.into(),
            source_path: path_text(&source),
            output_dir: path_text(&output),
            sample_rate: set.rate,
            sample_count: set.count,
            speakers: set.speakers,
            existing_files: existing.iter().map(|p| path_text(p)).collect(),
            planned_files,
            hangloose_dir: hangloose_dir.as_deref().map(path_text),
        },
        outputs,
    })
}

pub fn plan_brir_outputs(
    dir: &Path,
    options: &RecoveryOptions,
) -> Result<RecoveryPlan, RecoveryError> {
    Ok(prepare(dir, options)?.plan)
}

pub fn recover_brir_outputs(
    dir: &Path,
    options: &RecoveryOptions,
) -> Result<RecoveryResult, RecoveryError> {
    let prepared = prepare(dir, options)?;
    let created = write_all(&prepared.outputs, prepared.plan.sample_rate, |_, _, _| {
        Ok(())
    })?;
    Ok(RecoveryResult {
        source_kind: prepared.plan.source_kind,
        source_path: prepared.plan.source_path,
        output_dir: prepared.plan.output_dir,
        sample_rate: prepared.plan.sample_rate,
        sample_count: prepared.plan.sample_count,
        speakers: prepared.plan.speakers,
        created_files: created.iter().map(|p| path_text(p)).collect(),
        existing_files: prepared.plan.existing_files,
    })
}

fn conflict(target: &Path) -> RecoveryError {
    RecoveryError::new(
        RecoveryErrorCode::OutputConflict,
        format!(
            "Refusing to overwrite existing output {}.",
            filename(target)
        ),
        json!({"path":target}),
    )
}
fn write_error(target: &Path, reason: impl ToString) -> RecoveryError {
    RecoveryError::new(
        RecoveryErrorCode::OutputWriteFailed,
        format!("Could not write recovered output {}.", filename(target)),
        json!({"path":target,"reason":reason.to_string()}),
    )
}
fn check_conflict(target: &Path) -> Result<(), RecoveryError> {
    if find_named(target.parent().unwrap(), &filename(target), false)?.is_some() {
        return Err(conflict(target));
    }
    Ok(())
}
static TEMP_ID: AtomicU64 = AtomicU64::new(0);
fn temporary(target: &Path) -> std::io::Result<(PathBuf, File)> {
    loop {
        let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = target.with_file_name(format!(
            ".{}-{}-{id}.wav",
            target.file_stem().unwrap().to_string_lossy(),
            std::process::id()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
}
// A private hook makes staging/publish failures and competing output creation
// deterministic in unit tests, without process-global flags or public test APIs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WritePhase {
    BeforeStage,
    Metadata,
    BeforePublish,
    AfterConflictCheck,
}
/// Identity of a staged file: its length and a 64-bit FNV-1a digest of its
/// bytes. Rollback removes a published output only while it still carries the
/// bytes this recovery wrote, so a file another process replaced in the
/// meantime is left alone (the existing-output preservation rule).
fn fingerprint(path: &Path) -> Option<(u64, u64)> {
    let bytes = fs::read(path).ok()?;
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in &bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Some((bytes.len() as u64, hash))
}
fn write_all(
    plan: &[PlannedOutput],
    rate: u32,
    mut hook: impl FnMut(WritePhase, usize, &Path) -> std::io::Result<()>,
) -> Result<Vec<PathBuf>, RecoveryError> {
    let mut temporary_files: Vec<(PathBuf, Option<(u64, u64)>)> = Vec::new();
    let mut created: Vec<(PathBuf, Option<(u64, u64)>)> = Vec::new();
    let result = (|| {
        for (index, item) in plan.iter().enumerate() {
            hook(WritePhase::BeforeStage, index, &item.target)
                .map_err(|e| write_error(&item.target, e))?;
            fs::create_dir_all(item.target.parent().unwrap())
                .map_err(|e| write_error(&item.target, e))?;
            check_conflict(&item.target)?;
            let (path, file) = temporary(&item.target).map_err(|e| write_error(&item.target, e))?;
            temporary_files.push((path.clone(), None));
            write_pcm32(file, rate, &item.tracks).map_err(|e| write_error(&item.target, e))?;
            if !item.names.is_empty() {
                hook(WritePhase::Metadata, index, &item.target)
                    .map_err(|e| write_error(&item.target, e))?;
                append_track_names(
                    &path,
                    &item.names.iter().map(String::as_str).collect::<Vec<_>>(),
                )
                .map_err(|e| write_error(&item.target, io_message(e)))?;
            }
            temporary_files.last_mut().unwrap().1 = fingerprint(&path);
        }
        for (index, ((temp, identity), item)) in temporary_files.iter().zip(plan).enumerate() {
            hook(WritePhase::BeforePublish, index, &item.target)
                .map_err(|e| write_error(&item.target, e))?;
            check_conflict(&item.target)?;
            hook(WritePhase::AfterConflictCheck, index, &item.target)
                .map_err(|e| write_error(&item.target, e))?;
            // Python publishes with os.replace, which would overwrite a file
            // created after the check above. A hard link to the staged file
            // refuses an existing target atomically on Windows and Unix without
            // native calls; filesystems without hard links (FAT/exFAT, some
            // network shares) fall back to the rename Python uses, after the
            // same conflict check, so recovery still works there.
            match fs::hard_link(temp, &item.target) {
                Ok(()) => {}
                Err(e)
                    if e.kind() == std::io::ErrorKind::AlreadyExists && item.target.is_file() =>
                {
                    return Err(conflict(&item.target));
                }
                Err(_) => {
                    check_conflict(&item.target)?;
                    fs::rename(temp, &item.target).map_err(|e| write_error(&item.target, e))?;
                }
            }
            created.push((item.target.clone(), *identity));
        }
        Ok(())
    })();
    for (path, _) in &temporary_files {
        let _ = fs::remove_file(path);
    }
    if let Err(error) = result {
        for (path, identity) in created {
            // Only what this run published: a replacement by another process
            // has a different fingerprint and stays.
            if identity.is_none() || fingerprint(&path) == identity {
                let _ = fs::remove_file(path);
            }
        }
        return Err(error);
    }
    Ok(created.into_iter().map(|(path, _)| path).collect())
}
// Recovery uses soundfile format WAV, not the general I/O writer's WAVEX.
// The 44-byte PCM header and ties-even conversion are checked against Python.
fn write_pcm32(file: File, rate: u32, tracks: &[Vec<f64>]) -> std::io::Result<()> {
    let channels = u16::try_from(tracks.len()).map_err(std::io::Error::other)?;
    let align = channels
        .checked_mul(4)
        .ok_or_else(|| std::io::Error::other("frame too large"))?;
    let byte_rate = rate
        .checked_mul(u32::from(align))
        .ok_or_else(|| std::io::Error::other("byte rate overflow"))?;
    let size = tracks[0]
        .len()
        .checked_mul(usize::from(align))
        .and_then(|s| u32::try_from(s).ok())
        .filter(|s| *s <= u32::MAX - 36)
        .ok_or_else(|| std::io::Error::other("output exceeds RIFF size"))?;
    let mut out = BufWriter::new(file);
    out.write_all(b"RIFF")?;
    out.write_all(&(size + 36).to_le_bytes())?;
    out.write_all(b"WAVEfmt ")?;
    out.write_all(&16u32.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?;
    out.write_all(&channels.to_le_bytes())?;
    out.write_all(&rate.to_le_bytes())?;
    out.write_all(&byte_rate.to_le_bytes())?;
    out.write_all(&align.to_le_bytes())?;
    out.write_all(&32u16.to_le_bytes())?;
    out.write_all(b"data")?;
    out.write_all(&size.to_le_bytes())?;
    for i in 0..tracks[0].len() {
        for track in tracks {
            let sample = (track[i] * 2147483648.0).round_ties_even() as i32;
            out.write_all(&sample.to_le_bytes())?;
        }
    }
    out.flush()
}

// Unicode full case folding, matching CPython 3.14 (Unicode 16.0).
// Generated from str.casefold differences from str.lower; no locale dependency.
fn casefold(text: &str) -> String {
    let mut folded = String::new();
    for c in text.chars() {
        match c as u32 {
            0xb5 => folded.push('\u{3bc}'),
            0xdf => folded.push_str("\u{73}\u{73}"),
            0x149 => folded.push_str("\u{2bc}\u{6e}"),
            0x17f => folded.push('\u{73}'),
            0x1f0 => folded.push_str("\u{6a}\u{30c}"),
            0x345 => folded.push('\u{3b9}'),
            0x390 => folded.push_str("\u{3b9}\u{308}\u{301}"),
            0x3b0 => folded.push_str("\u{3c5}\u{308}\u{301}"),
            0x3c2 => folded.push('\u{3c3}'),
            0x3d0 => folded.push('\u{3b2}'),
            0x3d1 => folded.push('\u{3b8}'),
            0x3d5 => folded.push('\u{3c6}'),
            0x3d6 => folded.push('\u{3c0}'),
            0x3f0 => folded.push('\u{3ba}'),
            0x3f1 => folded.push('\u{3c1}'),
            0x3f5 => folded.push('\u{3b5}'),
            0x587 => folded.push_str("\u{565}\u{582}"),
            0x13a0 => folded.push('\u{13a0}'),
            0x13a1 => folded.push('\u{13a1}'),
            0x13a2 => folded.push('\u{13a2}'),
            0x13a3 => folded.push('\u{13a3}'),
            0x13a4 => folded.push('\u{13a4}'),
            0x13a5 => folded.push('\u{13a5}'),
            0x13a6 => folded.push('\u{13a6}'),
            0x13a7 => folded.push('\u{13a7}'),
            0x13a8 => folded.push('\u{13a8}'),
            0x13a9 => folded.push('\u{13a9}'),
            0x13aa => folded.push('\u{13aa}'),
            0x13ab => folded.push('\u{13ab}'),
            0x13ac => folded.push('\u{13ac}'),
            0x13ad => folded.push('\u{13ad}'),
            0x13ae => folded.push('\u{13ae}'),
            0x13af => folded.push('\u{13af}'),
            0x13b0 => folded.push('\u{13b0}'),
            0x13b1 => folded.push('\u{13b1}'),
            0x13b2 => folded.push('\u{13b2}'),
            0x13b3 => folded.push('\u{13b3}'),
            0x13b4 => folded.push('\u{13b4}'),
            0x13b5 => folded.push('\u{13b5}'),
            0x13b6 => folded.push('\u{13b6}'),
            0x13b7 => folded.push('\u{13b7}'),
            0x13b8 => folded.push('\u{13b8}'),
            0x13b9 => folded.push('\u{13b9}'),
            0x13ba => folded.push('\u{13ba}'),
            0x13bb => folded.push('\u{13bb}'),
            0x13bc => folded.push('\u{13bc}'),
            0x13bd => folded.push('\u{13bd}'),
            0x13be => folded.push('\u{13be}'),
            0x13bf => folded.push('\u{13bf}'),
            0x13c0 => folded.push('\u{13c0}'),
            0x13c1 => folded.push('\u{13c1}'),
            0x13c2 => folded.push('\u{13c2}'),
            0x13c3 => folded.push('\u{13c3}'),
            0x13c4 => folded.push('\u{13c4}'),
            0x13c5 => folded.push('\u{13c5}'),
            0x13c6 => folded.push('\u{13c6}'),
            0x13c7 => folded.push('\u{13c7}'),
            0x13c8 => folded.push('\u{13c8}'),
            0x13c9 => folded.push('\u{13c9}'),
            0x13ca => folded.push('\u{13ca}'),
            0x13cb => folded.push('\u{13cb}'),
            0x13cc => folded.push('\u{13cc}'),
            0x13cd => folded.push('\u{13cd}'),
            0x13ce => folded.push('\u{13ce}'),
            0x13cf => folded.push('\u{13cf}'),
            0x13d0 => folded.push('\u{13d0}'),
            0x13d1 => folded.push('\u{13d1}'),
            0x13d2 => folded.push('\u{13d2}'),
            0x13d3 => folded.push('\u{13d3}'),
            0x13d4 => folded.push('\u{13d4}'),
            0x13d5 => folded.push('\u{13d5}'),
            0x13d6 => folded.push('\u{13d6}'),
            0x13d7 => folded.push('\u{13d7}'),
            0x13d8 => folded.push('\u{13d8}'),
            0x13d9 => folded.push('\u{13d9}'),
            0x13da => folded.push('\u{13da}'),
            0x13db => folded.push('\u{13db}'),
            0x13dc => folded.push('\u{13dc}'),
            0x13dd => folded.push('\u{13dd}'),
            0x13de => folded.push('\u{13de}'),
            0x13df => folded.push('\u{13df}'),
            0x13e0 => folded.push('\u{13e0}'),
            0x13e1 => folded.push('\u{13e1}'),
            0x13e2 => folded.push('\u{13e2}'),
            0x13e3 => folded.push('\u{13e3}'),
            0x13e4 => folded.push('\u{13e4}'),
            0x13e5 => folded.push('\u{13e5}'),
            0x13e6 => folded.push('\u{13e6}'),
            0x13e7 => folded.push('\u{13e7}'),
            0x13e8 => folded.push('\u{13e8}'),
            0x13e9 => folded.push('\u{13e9}'),
            0x13ea => folded.push('\u{13ea}'),
            0x13eb => folded.push('\u{13eb}'),
            0x13ec => folded.push('\u{13ec}'),
            0x13ed => folded.push('\u{13ed}'),
            0x13ee => folded.push('\u{13ee}'),
            0x13ef => folded.push('\u{13ef}'),
            0x13f0 => folded.push('\u{13f0}'),
            0x13f1 => folded.push('\u{13f1}'),
            0x13f2 => folded.push('\u{13f2}'),
            0x13f3 => folded.push('\u{13f3}'),
            0x13f4 => folded.push('\u{13f4}'),
            0x13f5 => folded.push('\u{13f5}'),
            0x13f8 => folded.push('\u{13f0}'),
            0x13f9 => folded.push('\u{13f1}'),
            0x13fa => folded.push('\u{13f2}'),
            0x13fb => folded.push('\u{13f3}'),
            0x13fc => folded.push('\u{13f4}'),
            0x13fd => folded.push('\u{13f5}'),
            0x1c80 => folded.push('\u{432}'),
            0x1c81 => folded.push('\u{434}'),
            0x1c82 => folded.push('\u{43e}'),
            0x1c83 => folded.push('\u{441}'),
            0x1c84 => folded.push('\u{442}'),
            0x1c85 => folded.push('\u{442}'),
            0x1c86 => folded.push('\u{44a}'),
            0x1c87 => folded.push('\u{463}'),
            0x1c88 => folded.push('\u{a64b}'),
            0x1e96 => folded.push_str("\u{68}\u{331}"),
            0x1e97 => folded.push_str("\u{74}\u{308}"),
            0x1e98 => folded.push_str("\u{77}\u{30a}"),
            0x1e99 => folded.push_str("\u{79}\u{30a}"),
            0x1e9a => folded.push_str("\u{61}\u{2be}"),
            0x1e9b => folded.push('\u{1e61}'),
            0x1e9e => folded.push_str("\u{73}\u{73}"),
            0x1f50 => folded.push_str("\u{3c5}\u{313}"),
            0x1f52 => folded.push_str("\u{3c5}\u{313}\u{300}"),
            0x1f54 => folded.push_str("\u{3c5}\u{313}\u{301}"),
            0x1f56 => folded.push_str("\u{3c5}\u{313}\u{342}"),
            0x1f80 => folded.push_str("\u{1f00}\u{3b9}"),
            0x1f81 => folded.push_str("\u{1f01}\u{3b9}"),
            0x1f82 => folded.push_str("\u{1f02}\u{3b9}"),
            0x1f83 => folded.push_str("\u{1f03}\u{3b9}"),
            0x1f84 => folded.push_str("\u{1f04}\u{3b9}"),
            0x1f85 => folded.push_str("\u{1f05}\u{3b9}"),
            0x1f86 => folded.push_str("\u{1f06}\u{3b9}"),
            0x1f87 => folded.push_str("\u{1f07}\u{3b9}"),
            0x1f88 => folded.push_str("\u{1f00}\u{3b9}"),
            0x1f89 => folded.push_str("\u{1f01}\u{3b9}"),
            0x1f8a => folded.push_str("\u{1f02}\u{3b9}"),
            0x1f8b => folded.push_str("\u{1f03}\u{3b9}"),
            0x1f8c => folded.push_str("\u{1f04}\u{3b9}"),
            0x1f8d => folded.push_str("\u{1f05}\u{3b9}"),
            0x1f8e => folded.push_str("\u{1f06}\u{3b9}"),
            0x1f8f => folded.push_str("\u{1f07}\u{3b9}"),
            0x1f90 => folded.push_str("\u{1f20}\u{3b9}"),
            0x1f91 => folded.push_str("\u{1f21}\u{3b9}"),
            0x1f92 => folded.push_str("\u{1f22}\u{3b9}"),
            0x1f93 => folded.push_str("\u{1f23}\u{3b9}"),
            0x1f94 => folded.push_str("\u{1f24}\u{3b9}"),
            0x1f95 => folded.push_str("\u{1f25}\u{3b9}"),
            0x1f96 => folded.push_str("\u{1f26}\u{3b9}"),
            0x1f97 => folded.push_str("\u{1f27}\u{3b9}"),
            0x1f98 => folded.push_str("\u{1f20}\u{3b9}"),
            0x1f99 => folded.push_str("\u{1f21}\u{3b9}"),
            0x1f9a => folded.push_str("\u{1f22}\u{3b9}"),
            0x1f9b => folded.push_str("\u{1f23}\u{3b9}"),
            0x1f9c => folded.push_str("\u{1f24}\u{3b9}"),
            0x1f9d => folded.push_str("\u{1f25}\u{3b9}"),
            0x1f9e => folded.push_str("\u{1f26}\u{3b9}"),
            0x1f9f => folded.push_str("\u{1f27}\u{3b9}"),
            0x1fa0 => folded.push_str("\u{1f60}\u{3b9}"),
            0x1fa1 => folded.push_str("\u{1f61}\u{3b9}"),
            0x1fa2 => folded.push_str("\u{1f62}\u{3b9}"),
            0x1fa3 => folded.push_str("\u{1f63}\u{3b9}"),
            0x1fa4 => folded.push_str("\u{1f64}\u{3b9}"),
            0x1fa5 => folded.push_str("\u{1f65}\u{3b9}"),
            0x1fa6 => folded.push_str("\u{1f66}\u{3b9}"),
            0x1fa7 => folded.push_str("\u{1f67}\u{3b9}"),
            0x1fa8 => folded.push_str("\u{1f60}\u{3b9}"),
            0x1fa9 => folded.push_str("\u{1f61}\u{3b9}"),
            0x1faa => folded.push_str("\u{1f62}\u{3b9}"),
            0x1fab => folded.push_str("\u{1f63}\u{3b9}"),
            0x1fac => folded.push_str("\u{1f64}\u{3b9}"),
            0x1fad => folded.push_str("\u{1f65}\u{3b9}"),
            0x1fae => folded.push_str("\u{1f66}\u{3b9}"),
            0x1faf => folded.push_str("\u{1f67}\u{3b9}"),
            0x1fb2 => folded.push_str("\u{1f70}\u{3b9}"),
            0x1fb3 => folded.push_str("\u{3b1}\u{3b9}"),
            0x1fb4 => folded.push_str("\u{3ac}\u{3b9}"),
            0x1fb6 => folded.push_str("\u{3b1}\u{342}"),
            0x1fb7 => folded.push_str("\u{3b1}\u{342}\u{3b9}"),
            0x1fbc => folded.push_str("\u{3b1}\u{3b9}"),
            0x1fbe => folded.push('\u{3b9}'),
            0x1fc2 => folded.push_str("\u{1f74}\u{3b9}"),
            0x1fc3 => folded.push_str("\u{3b7}\u{3b9}"),
            0x1fc4 => folded.push_str("\u{3ae}\u{3b9}"),
            0x1fc6 => folded.push_str("\u{3b7}\u{342}"),
            0x1fc7 => folded.push_str("\u{3b7}\u{342}\u{3b9}"),
            0x1fcc => folded.push_str("\u{3b7}\u{3b9}"),
            0x1fd2 => folded.push_str("\u{3b9}\u{308}\u{300}"),
            0x1fd3 => folded.push_str("\u{3b9}\u{308}\u{301}"),
            0x1fd6 => folded.push_str("\u{3b9}\u{342}"),
            0x1fd7 => folded.push_str("\u{3b9}\u{308}\u{342}"),
            0x1fe2 => folded.push_str("\u{3c5}\u{308}\u{300}"),
            0x1fe3 => folded.push_str("\u{3c5}\u{308}\u{301}"),
            0x1fe4 => folded.push_str("\u{3c1}\u{313}"),
            0x1fe6 => folded.push_str("\u{3c5}\u{342}"),
            0x1fe7 => folded.push_str("\u{3c5}\u{308}\u{342}"),
            0x1ff2 => folded.push_str("\u{1f7c}\u{3b9}"),
            0x1ff3 => folded.push_str("\u{3c9}\u{3b9}"),
            0x1ff4 => folded.push_str("\u{3ce}\u{3b9}"),
            0x1ff6 => folded.push_str("\u{3c9}\u{342}"),
            0x1ff7 => folded.push_str("\u{3c9}\u{342}\u{3b9}"),
            0x1ffc => folded.push_str("\u{3c9}\u{3b9}"),
            0xab70 => folded.push('\u{13a0}'),
            0xab71 => folded.push('\u{13a1}'),
            0xab72 => folded.push('\u{13a2}'),
            0xab73 => folded.push('\u{13a3}'),
            0xab74 => folded.push('\u{13a4}'),
            0xab75 => folded.push('\u{13a5}'),
            0xab76 => folded.push('\u{13a6}'),
            0xab77 => folded.push('\u{13a7}'),
            0xab78 => folded.push('\u{13a8}'),
            0xab79 => folded.push('\u{13a9}'),
            0xab7a => folded.push('\u{13aa}'),
            0xab7b => folded.push('\u{13ab}'),
            0xab7c => folded.push('\u{13ac}'),
            0xab7d => folded.push('\u{13ad}'),
            0xab7e => folded.push('\u{13ae}'),
            0xab7f => folded.push('\u{13af}'),
            0xab80 => folded.push('\u{13b0}'),
            0xab81 => folded.push('\u{13b1}'),
            0xab82 => folded.push('\u{13b2}'),
            0xab83 => folded.push('\u{13b3}'),
            0xab84 => folded.push('\u{13b4}'),
            0xab85 => folded.push('\u{13b5}'),
            0xab86 => folded.push('\u{13b6}'),
            0xab87 => folded.push('\u{13b7}'),
            0xab88 => folded.push('\u{13b8}'),
            0xab89 => folded.push('\u{13b9}'),
            0xab8a => folded.push('\u{13ba}'),
            0xab8b => folded.push('\u{13bb}'),
            0xab8c => folded.push('\u{13bc}'),
            0xab8d => folded.push('\u{13bd}'),
            0xab8e => folded.push('\u{13be}'),
            0xab8f => folded.push('\u{13bf}'),
            0xab90 => folded.push('\u{13c0}'),
            0xab91 => folded.push('\u{13c1}'),
            0xab92 => folded.push('\u{13c2}'),
            0xab93 => folded.push('\u{13c3}'),
            0xab94 => folded.push('\u{13c4}'),
            0xab95 => folded.push('\u{13c5}'),
            0xab96 => folded.push('\u{13c6}'),
            0xab97 => folded.push('\u{13c7}'),
            0xab98 => folded.push('\u{13c8}'),
            0xab99 => folded.push('\u{13c9}'),
            0xab9a => folded.push('\u{13ca}'),
            0xab9b => folded.push('\u{13cb}'),
            0xab9c => folded.push('\u{13cc}'),
            0xab9d => folded.push('\u{13cd}'),
            0xab9e => folded.push('\u{13ce}'),
            0xab9f => folded.push('\u{13cf}'),
            0xaba0 => folded.push('\u{13d0}'),
            0xaba1 => folded.push('\u{13d1}'),
            0xaba2 => folded.push('\u{13d2}'),
            0xaba3 => folded.push('\u{13d3}'),
            0xaba4 => folded.push('\u{13d4}'),
            0xaba5 => folded.push('\u{13d5}'),
            0xaba6 => folded.push('\u{13d6}'),
            0xaba7 => folded.push('\u{13d7}'),
            0xaba8 => folded.push('\u{13d8}'),
            0xaba9 => folded.push('\u{13d9}'),
            0xabaa => folded.push('\u{13da}'),
            0xabab => folded.push('\u{13db}'),
            0xabac => folded.push('\u{13dc}'),
            0xabad => folded.push('\u{13dd}'),
            0xabae => folded.push('\u{13de}'),
            0xabaf => folded.push('\u{13df}'),
            0xabb0 => folded.push('\u{13e0}'),
            0xabb1 => folded.push('\u{13e1}'),
            0xabb2 => folded.push('\u{13e2}'),
            0xabb3 => folded.push('\u{13e3}'),
            0xabb4 => folded.push('\u{13e4}'),
            0xabb5 => folded.push('\u{13e5}'),
            0xabb6 => folded.push('\u{13e6}'),
            0xabb7 => folded.push('\u{13e7}'),
            0xabb8 => folded.push('\u{13e8}'),
            0xabb9 => folded.push('\u{13e9}'),
            0xabba => folded.push('\u{13ea}'),
            0xabbb => folded.push('\u{13eb}'),
            0xabbc => folded.push('\u{13ec}'),
            0xabbd => folded.push('\u{13ed}'),
            0xabbe => folded.push('\u{13ee}'),
            0xabbf => folded.push('\u{13ef}'),
            0xfb00 => folded.push_str("\u{66}\u{66}"),
            0xfb01 => folded.push_str("\u{66}\u{69}"),
            0xfb02 => folded.push_str("\u{66}\u{6c}"),
            0xfb03 => folded.push_str("\u{66}\u{66}\u{69}"),
            0xfb04 => folded.push_str("\u{66}\u{66}\u{6c}"),
            0xfb05 => folded.push_str("\u{73}\u{74}"),
            0xfb06 => folded.push_str("\u{73}\u{74}"),
            0xfb13 => folded.push_str("\u{574}\u{576}"),
            0xfb14 => folded.push_str("\u{574}\u{565}"),
            0xfb15 => folded.push_str("\u{574}\u{56b}"),
            0xfb16 => folded.push_str("\u{57e}\u{576}"),
            0xfb17 => folded.push_str("\u{574}\u{56d}"),
            _ => folded.extend(c.to_lowercase()),
        }
    }
    folded
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "impulcifer-p17-private-{}-{}",
                std::process::id(),
                TEMP_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> Value {
        serde_json::from_str(include_str!(
            "../../../tests/migration/goldens/p17_recovery.json"
        ))
        .unwrap()
    }
    fn normalize(value: Value, root: &Path) -> Value {
        match value {
            Value::String(s) => json!(
                s.replace(root.to_str().unwrap(), "$ROOT")
                    .replace(char::from(92), "/")
            ),
            Value::Array(a) => Value::Array(a.into_iter().map(|v| normalize(v, root)).collect()),
            Value::Object(o) => Value::Object(
                o.into_iter()
                    .map(|(k, v)| (k, normalize(v, root)))
                    .collect(),
            ),
            v => v,
        }
    }
    fn plan(root: &Path, compact: bool) -> Vec<PlannedOutput> {
        ["hrir.wav", "hesuvi.wav"]
            .into_iter()
            .map(|name| PlannedOutput {
                target: root.join(name),
                tracks: vec![vec![0.125; 4800]; 2],
                names: if compact {
                    vec!["FL-left".into(), "FL-right".into()]
                } else {
                    Vec::new()
                },
                kind: if name == "hrir.wav" { "hrir" } else { "hesuvi" },
                speaker: None,
            })
            .collect()
    }
    #[test]
    fn recovery_transaction_faults_match_oracle() {
        let golden = fixture();
        for name in [
            "write_metadata_failure",
            "output_conflict_before_stage",
            "output_conflict_before_publish",
            "rollback_after_first_publish",
        ] {
            let temp = Temp::new();
            let outputs = plan(&temp.0, name == "write_metadata_failure");
            let error = write_all(&outputs, 48000, |phase, index, target| {
                if name == "write_metadata_failure" && phase == WritePhase::Metadata {
                    return Err(std::io::Error::other("simulated metadata write failure"));
                }
                if name == "rollback_after_first_publish"
                    && phase == WritePhase::BeforePublish
                    && index == 1
                {
                    assert!(temp.0.join("hrir.wav").is_file());
                    return Err(std::io::Error::other("simulated second publish failure"));
                }
                if index == 0
                    && ((name == "output_conflict_before_stage"
                        && phase == WritePhase::BeforeStage)
                        || (name == "output_conflict_before_publish"
                            && phase == WritePhase::BeforePublish))
                {
                    fs::write(target, b"competing output")?;
                }
                Ok(())
            })
            .unwrap_err();
            let case = golden["scenarios"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["name"] == name)
                .unwrap();
            if name == "rollback_after_first_publish" {
                // Python propagates a raw OSError here; the fixed Rust Result
                // surface reports OUTPUT_WRITE_FAILED and keeps the OS reason.
                assert_eq!(error.code, RecoveryErrorCode::OutputWriteFailed);
                assert_eq!(error.details["reason"], case["os_error"]["message"]);
            } else {
                assert_eq!(normalize(json!(error), &temp.0), case["error"], "{name}");
            }
            assert!(!temp.0.join("hesuvi.wav").exists());
            if name.starts_with("output_conflict") {
                assert_eq!(
                    fs::read(temp.0.join("hrir.wav")).unwrap(),
                    b"competing output"
                );
            } else {
                assert!(!temp.0.join("hrir.wav").exists());
            }
            assert!(
                fs::read_dir(&temp.0).unwrap().all(|p| !p
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with('.'))
            );
        }
    }
    #[test]
    fn recovery_atomic_publish_never_overwrites_racing_output() {
        for index in [0, 1] {
            let temp = Temp::new();
            let outputs = plan(&temp.0, false);
            let error = write_all(&outputs, 48000, |phase, i, target| {
                if phase == WritePhase::AfterConflictCheck && i == index {
                    fs::write(target, b"race winner")?;
                }
                Ok(())
            })
            .unwrap_err();
            assert_eq!(error.code, RecoveryErrorCode::OutputConflict);
            assert_eq!(fs::read(&outputs[index].target).unwrap(), b"race winner");
            assert!(!outputs[1 - index].target.exists());
            assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 1);
        }
    }
    #[test]
    fn recovery_rollback_keeps_outputs_replaced_by_others() {
        // Codex on PR #188: another process replaces an already published
        // output before a later publication fails; rollback must not delete
        // the replacement.
        let temp = Temp::new();
        let outputs = plan(&temp.0, false);
        let first = outputs[0].target.clone();
        let error = write_all(&outputs, 48000, |phase, i, _target| {
            if phase == WritePhase::BeforePublish && i == 1 {
                fs::write(&first, b"replaced by another process")?;
                return Err(std::io::Error::other("simulated publish failure"));
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error.code, RecoveryErrorCode::OutputWriteFailed);
        assert_eq!(
            fs::read(&outputs[0].target).unwrap(),
            b"replaced by another process"
        );
        assert!(!outputs[1].target.exists());
        assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 1);
    }
    #[test]
    fn recovery_request_validation_matches_all_python_cases() {
        let temp = Temp::new();
        for case in fixture()["validation"].as_array().unwrap() {
            fn expand(value: Value, root: &Path) -> Value {
                match value {
                    Value::String(s) => json!(s.replace("$ROOT", root.to_str().unwrap())),
                    Value::Array(a) => {
                        Value::Array(a.into_iter().map(|v| expand(v, root)).collect())
                    }
                    Value::Object(o) => {
                        Value::Object(o.into_iter().map(|(k, v)| (k, expand(v, root))).collect())
                    }
                    v => v,
                }
            }
            let request = expand(case["request"].clone(), &temp.0);
            let actual = match validate_request(&request) {
                Ok((path, options)) => ipc::ok(
                    json!({"params":{"directory":path,"include_hangloose":options.include_hangloose,"remove_silent_channels":options.remove_silent_channels}}),
                ),
                Err(error) => error,
            };
            assert_eq!(normalize(actual, &temp.0), case["response"]);
        }
    }
    #[test]
    fn recovery_unicode_casefold_matches_python() {
        assert_eq!(casefold("Straße"), casefold("STRASSE"));
        assert_eq!(casefold("Σςσ"), "σσσ");
        assert_eq!(casefold("ﬃ"), "ffi");
        let temp = Temp::new();
        fs::write(temp.0.join("profileſL.wav"), b"suffix probe").unwrap();
        assert_eq!(find_split(&temp.0).unwrap()[0].0, "SL");
    }
    #[test]
    fn recovery_expanduser_resolves_home_without_writing() {
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap();
        assert_eq!(
            resolve_directory(Path::new("~")).unwrap(),
            ordinary_path(fs::canonicalize(home).unwrap())
        );
    }
}
