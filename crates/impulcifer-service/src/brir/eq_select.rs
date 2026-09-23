//! Custom EQ slots of a BRIR request and the read-only `inspect_eq` preview.
//!
//! There are three slots, the file for both ears (`eq`), the left-ear file
//! (`eq-left`) and the right-ear file (`eq-right`). Each slot follows the
//! measurement folder (`<base>.csv`, else `<base>.txt`), names a file anywhere,
//! or is off. A named file is read where it is; nothing is copied into the
//! measurement folder.
use super::discovery::EqDiscovery;
use super::inputs::{EqFileLoader, decode_text};
use crate::args::invalid;
use impulcifer_dsp::fr::{FrequencyResponse, generate_frequencies};
use impulcifer_dsp::stages::eq_files::{
    EqApoLogReport, finalize_eq, looks_like_eqapo_config, read_eq_settings, select_eq_pair,
};
use impulcifer_types::ipc::{self, ErrorCode};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

/// Request field, folder base name and wire name of each slot, in UI order.
pub(crate) const SLOTS: [(&str, &str, &str); 3] = [
    ("eq_file", "eq", "both"),
    ("eq_left_file", "eq-left", "left"),
    ("eq_right_file", "eq-right", "right"),
];

#[derive(Clone, Debug, Default, PartialEq)]
pub enum EqChoice {
    /// `<base>.csv`, else `<base>.txt`, in the measurement folder.
    #[default]
    Folder,
    File(PathBuf),
    Off,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EqChoices {
    pub both: EqChoice,
    pub left: EqChoice,
    pub right: EqChoice,
}

impl EqChoices {
    /// `eq_file`, `eq_left_file` and `eq_right_file` of a request. Absent, null
    /// or an empty string follows the folder, `false` turns the slot off, and a
    /// path (absolute, or relative to the measurement folder) must name a file.
    pub(crate) fn from_request(object: &Map<String, Value>, dir: &Path) -> Result<Self, Value> {
        let mut choices = [EqChoice::Folder, EqChoice::Folder, EqChoice::Folder];
        for ((field, _, _), choice) in SLOTS.iter().zip(&mut choices) {
            *choice = match object.get(*field) {
                None | Some(Value::Null) => EqChoice::Folder,
                Some(Value::Bool(false)) => EqChoice::Off,
                Some(Value::String(text)) if text.trim().is_empty() => EqChoice::Folder,
                Some(Value::String(text)) => {
                    let path = dir.join(text.trim());
                    if !path.is_file() {
                        return Err(ipc::error(
                            ErrorCode::FileNotFound,
                            "Custom EQ file does not exist.",
                            json!({"field":field,"path":text.trim()}),
                            false,
                        ));
                    }
                    EqChoice::File(std::path::absolute(&path).unwrap_or(path))
                }
                Some(_) => {
                    return Err(invalid(format!(
                        "{field} must be a file path, null (use the folder) or false (off)."
                    )));
                }
            };
        }
        let [both, left, right] = choices;
        Ok(Self { both, left, right })
    }

    fn slots(&self) -> [&EqChoice; 3] {
        [&self.both, &self.left, &self.right]
    }

    /// Replace the folder discovery of every slot that is not `Folder`.
    pub fn apply(&self, found: &mut EqDiscovery) {
        for (choice, slot) in
            self.slots()
                .into_iter()
                .zip([&mut found.common, &mut found.left, &mut found.right])
        {
            match choice {
                EqChoice::Folder => {}
                EqChoice::File(path) => *slot = Some(path.clone()),
                EqChoice::Off => *slot = None,
            }
        }
    }
}

/// Folder candidates in priority order; the pipeline takes the first that exists.
fn folder_candidates(dir: &Path, base: &str) -> Vec<PathBuf> {
    [format!("{base}.csv"), format!("{base}.txt")]
        .into_iter()
        .map(|name| dir.join(name))
        .filter(|path| path.is_file())
        .collect()
}

struct Parsed {
    left: FrequencyResponse,
    right: Option<FrequencyResponse>,
    report: Option<EqApoLogReport>,
    format: &'static str,
}

fn parse(path: &Path, fs: u32) -> Result<Parsed, String> {
    let text = decode_text(&std::fs::read(path).map_err(|e| e.to_string())?);
    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let eqapo = looks_like_eqapo_config(&text);
    let frequency = generate_frequencies(10.0, f64::from(fs) / 2.0, 1.01);
    let (left, right, report) = read_eq_settings(
        name,
        &text,
        fs,
        &frequency,
        path.parent(),
        &mut EqFileLoader,
    )
    .map_err(|e| e.to_string())?;
    let format = if eqapo {
        "eqapo"
    } else if left.error.is_empty() && left.raw.is_empty() {
        return Err("no gain or error column".into());
    } else {
        "csv"
    };
    Ok(Parsed {
        left,
        right,
        report,
        format,
    })
}

/// Gain the equalize stage applies before smoothing: the negated error.
fn preview(fr: &FrequencyResponse, grid: &[f64]) -> Option<Vec<f64>> {
    if fr.error.is_empty() || fr.error.len() != fr.frequency.len() {
        return None;
    }
    let (f, e) = (&fr.frequency, &fr.error);
    let points = grid
        .iter()
        .map(|&x| {
            let i = f.partition_point(|&v| v < x);
            let gain = if i == 0 {
                -e[0]
            } else if i >= f.len() {
                -e[f.len() - 1]
            } else {
                let t = (x.ln() - f[i - 1].ln()) / (f[i].ln() - f[i - 1].ln());
                -(e[i - 1] + t * (e[i] - e[i - 1]))
            };
            (gain * 100.0).round() / 100.0
        })
        .collect();
    Some(points)
}

/// `inspect_eq`: what each slot resolves to, which file feeds which ear, and
/// the resulting per-ear curves on a 1/12-octave grid (20 Hz to 20 kHz) for a
/// 48 kHz project. Unreadable files are reported per slot, never as a failure;
/// `blocked` is then true and the per-ear result is left empty.
pub(crate) fn inspect(request: &Value) -> Result<Value, Value> {
    let object = request
        .as_object()
        .ok_or_else(|| invalid("EQ request must be an object."))?;
    let dir = object
        .get("dir_path")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let dir = Path::new(dir);
    if !dir.is_dir() {
        return Err(ipc::error(
            ErrorCode::FileNotFound,
            "Measurement directory does not exist.",
            json!({"path":dir}),
            false,
        ));
    }
    let choices = EqChoices::from_request(object, dir)?;
    let fs = 48_000;
    let mut slots = Vec::new();
    let mut parsed: [Option<Parsed>; 3] = [None, None, None];
    for (((_, base, wire), choice), slot_parsed) in
        SLOTS.iter().zip(choices.slots()).zip(&mut parsed)
    {
        let candidates = folder_candidates(dir, base);
        let (source, path) = match choice {
            EqChoice::Folder => ("folder", candidates.first().cloned()),
            EqChoice::File(path) => ("file", Some(path.clone())),
            EqChoice::Off => ("off", None),
        };
        let ignored: Vec<_> = match choice {
            EqChoice::Folder => candidates
                .iter()
                .skip(1)
                .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                .collect(),
            _ => Vec::new(),
        };
        let mut entry = json!({
            "slot": wire,
            "source": source,
            "path": path.as_ref().map(|p| crate::plain_path(p).to_string_lossy().into_owned()),
            "name": path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()),
            "folder_default": format!("{base}.csv"),
            "ignored": ignored,
            "format": Value::Null,
            "channels": Value::Null,
            "error": Value::Null,
        });
        if let Some(path) = &path {
            match parse(path, fs) {
                Ok(p) => {
                    entry["format"] = json!(p.format);
                    entry["channels"] = json!(if p.right.is_some() { "split" } else { "both" });
                    if let Some(report) = &p.report {
                        entry["eqapo"] = json!({
                            "preamp_db": [report.preamp_left, report.preamp_right],
                            "applied": report.applied.len(),
                            "bypassed": report.bypassed.len(),
                            "skipped": report.skipped_count,
                        });
                    }
                    *slot_parsed = Some(p);
                }
                Err(message) => entry["error"] = json!(message),
            }
        }
        slots.push(entry);
    }
    // A slot whose file cannot be read stops the BRIR job, so there is no
    // honest per-ear result to show until it is fixed or switched off.
    let blocked = slots.iter().any(|slot| !slot["error"].is_null());
    let [both, left, right] = parsed;
    // Which slot (and which channel of a split file) feeds each ear; mirrors
    // inputs.rs: eq-left takes a file's left curve, eq-right a split file's
    // right curve or its only one, and eq covers whatever is left.
    let left_source = if left.is_some() {
        Some(json!({"slot":"left","channel":"left"}))
    } else {
        both.as_ref()
            .map(|p| json!({"slot":"both","channel":if p.right.is_some() {"left"} else {"both"}}))
    };
    let right_source = if let Some(p) = &right {
        Some(json!({"slot":"right","channel":if p.right.is_some() {"right"} else {"both"}}))
    } else {
        both.as_ref()
            .map(|p| json!({"slot":"both","channel":if p.right.is_some() {"right"} else {"both"}}))
    };
    let (both_left, both_right) = match both {
        Some(p) => (Some(p.left), p.right),
        None => (None, None),
    };
    let (eq_left, eq_right) = select_eq_pair(
        both_left,
        both_right,
        left.map(|p| p.left),
        right.map(|p| p.right.unwrap_or(p.left)),
    );
    let grid: Vec<f64> = (0..=120)
        .map(|i| 20.0 * 2f64.powf(f64::from(i) / 12.0))
        .take_while(|f| *f <= 20_000.0 * 1.0001)
        .collect();
    let (curve_left, curve_right) = match finalize_eq(eq_left, eq_right, fs) {
        Ok((l, r)) => (
            l.as_ref().and_then(|fr| preview(fr, &grid)),
            r.as_ref().and_then(|fr| preview(fr, &grid)),
        ),
        Err(_) => (None, None),
    };
    let (left_source, right_source, curve_left, curve_right) = if blocked {
        (None, None, None, None)
    } else {
        (left_source, right_source, curve_left, curve_right)
    };
    Ok(json!({
        "slots": slots,
        "blocked": blocked,
        "ears": {"left": left_source, "right": right_source},
        "curves": {"frequency": grid, "left": curve_left, "right": curve_right},
    }))
}
