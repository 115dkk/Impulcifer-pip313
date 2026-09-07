//! Plain EQ files; core/pipeline_stages.py:183-350.
use crate::{DspError, fr::FrequencyResponse};
/// Python _find_eq_settings_file, core/pipeline_stages.py:183-192; p10_eq_files.
pub fn eq_settings_file_names(base: &str) -> [String; 2] {
    [format!("{base}.csv"), format!("{base}.txt")]
}
/// Python looks_like_eqapo_config, core/eqapo.py:168-189; p10_eq_files.
pub fn looks_like_eqapo_config(text: &str) -> bool {
    let known = [
        "Preamp",
        "GraphicEQ",
        "Channel",
        "Include",
        "Convolution",
        "Delay",
        "Copy",
        "MultiConvolution",
        "Eval",
        "VSTPlugin",
        "LoudnessCorrection",
        "Device",
        "Stage",
        "If",
        "ElseIf",
        "Else",
        "EndIf",
    ];
    let filter = regex::Regex::new(r"^Filter(?:\s*\d+)?$").expect("constant Python regex");
    text.lines().any(|line| {
        let Some((key, _)) = line.split_once(':') else {
            return false;
        };
        let key = key.trim();
        !key.starts_with('#') && (known.contains(&key) || filter.is_match(key))
    })
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
