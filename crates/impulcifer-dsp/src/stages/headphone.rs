//! Headphone DSP and snapshot-based discovery; core/pipeline_stages.py:353-482.
use crate::{
    DspError,
    fr::{CenterAt, CompensateOptions, FrequencyResponse, magnitude_to_frequency_response},
    hrir::Hrir,
};
use impulcifer_types::constants::HEXADECAGONAL_TRACK_ORDER;
#[derive(Clone, Debug)]
pub struct HeadphoneCompensation {
    pub left: FrequencyResponse,
    pub right: FrequencyResponse,
    pub responses_tracks: Vec<Vec<f64>>,
}
/// Python headphone_compensation, core/pipeline_stages.py:420-438; p10_headphone.
pub fn headphone_compensation(hp: &Hrir) -> Result<HeadphoneCompensation, DspError> {
    let l = hp
        .get("FL")
        .and_then(|s| s.left.as_ref())
        .ok_or_else(|| DspError::InvalidArgument("FL left headphone measurement missing".into()))?;
    let r = hp.get("FR").and_then(|s| s.right.as_ref()).ok_or_else(|| {
        DspError::InvalidArgument("FR right headphone measurement missing".into())
    })?;
    let mut left = magnitude_to_frequency_response("Frequency response", hp.fs, &l.data)?;
    let mut right = magnitude_to_frequency_response("Frequency response", hp.fs, &r.data)?;
    let gain = left.center(CenterAt::Band(100.0, 10000.0))?;
    for v in &mut right.raw {
        *v += gain;
    }
    let zero = FrequencyResponse::constant("zero", Some(left.frequency.clone()), 0.0, 0.0)?;
    left.compensate(&zero, &CompensateOptions::default())?;
    right.compensate(&zero, &CompensateOptions::default())?;
    Ok(HeadphoneCompensation {
        left,
        right,
        responses_tracks: hp.stack_tracks(&HEXADECAGONAL_TRACK_ORDER, false)?,
    })
}
/// Python normpath in headphone_compensation, core/pipeline_stages.py:370; p10_headphone_resolution.
fn normalize(path: &str) -> String {
    let path = path.replace('\\', "/");
    let prefix = if path.starts_with("//") {
        "//"
    } else if path.starts_with('/') {
        "/"
    } else {
        ""
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => (),
            ".." if parts
                .last()
                .is_some_and(|s| *s != ".." && !s.ends_with(':')) =>
            {
                parts.pop();
            }
            ".." if !prefix.is_empty() => (),
            p => parts.push(p),
        }
    }
    let result = format!("{prefix}{}", parts.join("/"));
    if result.is_empty() {
        ".".into()
    } else {
        result
    }
}
/// Python headphone_compensation resolution, core/pipeline_stages.py:369-418;
/// p10_headphone_resolution. The ordered snapshot contains EXISTING entries:
/// files relative to the measurement root or absolute external files; directories
/// explicitly end in '/' and their entries carry that directory prefix. Only
/// directories resolved by the caller's original Python cwd context are marked:
/// Python checks isdir(requested) BEFORE joining the measurement root. The service
/// normalizes this context. No stat/race detection is possible from a snapshot.
/// None/empty selects headphones.wav; directories try four named fallbacks then
/// first case-insensitive .wav in enumeration order. Missing/empty requests fall
/// back to root headphones.wav. Relative results stay relative; external stay absolute.
pub fn resolve_headphone_file(requested: Option<&str>, dir_listing: &[&str]) -> Option<String> {
    let entries: Vec<_> = dir_listing
        .iter()
        .map(|s| (normalize(s), s.ends_with('/') || s.ends_with('\\')))
        .collect();
    let exists = |s: &str| entries.iter().any(|(p, _)| p == s);
    let default = || exists("headphones.wav").then(|| "headphones.wav".into());
    let Some(request) = requested.filter(|s| !s.is_empty()) else {
        return default();
    };
    let request = normalize(request);
    let candidate = if entries.iter().any(|(p, d)| p == &request && *d) {
        let prefix = if request == "." {
            String::new()
        } else {
            format!("{}/", request.trim_end_matches('/'))
        };
        [
            "headphones.wav",
            "headphone.wav",
            "hp.wav",
            "compensation.wav",
        ]
        .iter()
        .map(|n| format!("{prefix}{n}"))
        .find(|p| entries.iter().any(|(e, d)| e == p && !*d))
        .or_else(|| {
            entries
                .iter()
                .find(|(p, _)| {
                    p.strip_prefix(&prefix)
                        .is_some_and(|s| !s.contains('/') && s.to_lowercase().ends_with(".wav"))
                })
                .map(|(p, _)| p.clone())
        })
    } else {
        Some(request)
    };
    candidate.filter(|p| exists(p)).or_else(default)
}
