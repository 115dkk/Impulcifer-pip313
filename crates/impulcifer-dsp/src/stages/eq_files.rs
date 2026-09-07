//! Plain EQ files; core/pipeline_stages.py:183-350.
use crate::{DspError, fr::FrequencyResponse};
/// Python _find_eq_settings_file, core/pipeline_stages.py:183-192; p10_eq_files.
pub fn eq_settings_file_names(base: &str) -> [String; 2] {
    [format!("{base}.csv"), format!("{base}.txt")]
}
pub use super::eqapo::looks_like_eqapo_config;
use super::eqapo::{EqApoCommandReport, EqApoLoader, parse_eqapo_config};
use std::path::Path;

/// Python _read_eq_settings logging, core/pipeline_stages.py:245-284; p12_scopes.
#[derive(Clone, Debug, PartialEq)]
pub struct EqApoLogReport {
    pub applied_left: usize,
    pub applied_right: usize,
    pub preamp_left: f64,
    pub preamp_right: f64,
    pub applied: Vec<String>,
    pub bypassed: Vec<EqApoCommandReport>,
    pub skipped_count: usize,
    pub channel_split: bool,
}
/// Python _read_eq_settings result, core/pipeline_stages.py:211-214; p12_scopes.
pub type EqSettings = (
    FrequencyResponse,
    Option<FrequencyResponse>,
    Option<EqApoLogReport>,
);

/// Python _read_eq_settings, core/pipeline_stages.py:199-291; p12_*.
/// Byte decoding belongs to the caller, as does loading includes and WAV files.
pub fn read_eq_settings(
    name: &str,
    text: &str,
    fs: u32,
    frequency: &[f64],
    base_dir: Option<&Path>,
    loader: &mut dyn EqApoLoader,
) -> Result<EqSettings, DspError> {
    if !looks_like_eqapo_config(text) {
        return Ok((read_eq_settings_csv(name, text)?, None, None));
    }
    let result = parse_eqapo_config(text, fs, frequency, base_dir, loader)?;
    let split = result.channel_split();
    let make = |name: &str, raw: Vec<f64>| {
        let mut fr = FrequencyResponse::new(name, Some(frequency.to_vec()), Some(raw))?;
        fr.error = fr.raw.iter().map(|v| -v).collect();
        Ok::<_, DspError>(fr)
    };
    let left = make(name, result.left_db)?;
    let right = if split {
        Some(make(&format!("{name} (right)"), result.right_db)?)
    } else {
        None
    };
    let report = EqApoLogReport {
        applied_left: result.applied_left,
        applied_right: result.applied_right,
        preamp_left: result.preamp_left,
        preamp_right: result.preamp_right,
        applied: result.applied,
        bypassed: result.bypassed,
        skipped_count: result.skipped.len(),
        channel_split: split,
    };
    Ok((left, right, Some(report)))
}
/// Python _read_eq_settings, core/pipeline_stages.py:199-232; p10_eq_files.
pub fn read_eq_settings_csv(name: &str, text: &str) -> Result<FrequencyResponse, DspError> {
    if looks_like_eqapo_config(text) {
        return Err(DspError::InvalidArgument(
            "EqualizerAPO configs are not supported yet (P12)".into(),
        ));
    }
    let mut fr = FrequencyResponse::parse_csv(name, text)?;
    if fr.error.is_empty() && !fr.raw.is_empty() {
        fr.error = fr.raw.iter().map(|v| -v).collect();
    }
    Ok(fr)
}
/// Python equalization precedence, core/pipeline_stages.py:294-334; p10_eq_files.
pub fn select_eq_pair(
    eq: Option<FrequencyResponse>,
    eq_right: Option<FrequencyResponse>,
    left: Option<FrequencyResponse>,
    right: Option<FrequencyResponse>,
) -> (Option<FrequencyResponse>, Option<FrequencyResponse>) {
    (left.or_else(|| eq.clone()), right.or(eq_right).or(eq))
}
/// Python equalization interpolation, core/pipeline_stages.py:322-334; p10_eq_files.
pub fn finalize_eq(
    mut left: Option<FrequencyResponse>,
    mut right: Option<FrequencyResponse>,
    fs: u32,
) -> Result<(Option<FrequencyResponse>, Option<FrequencyResponse>), DspError> {
    let same = left.is_some() && left == right;
    if let Some(fr) = &mut left {
        fr.interpolate(None, 1.01, 1, 10.0, fs as f64 / 2.0)?;
    }
    if same {
        right = left.clone();
    } else if let Some(fr) = &mut right {
        fr.interpolate(None, 1.01, 1, 10.0, fs as f64 / 2.0)?;
    }
    Ok((left, right))
}
