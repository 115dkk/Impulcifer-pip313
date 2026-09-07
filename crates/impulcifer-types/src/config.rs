//! `ProcessingConfig` mirrors `core/pipeline.py::ProcessingConfig` (2.x) field
//! for field. Defaults here are the single source of truth for the CLI, the
//! service's `brir_defaults` and the Python wheel.

use serde::{Deserialize, Serialize};

/// Decay specification: a single value applied to every channel, or a
/// per-channel map (2.x accepts a number or a dict keyed by speaker name).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DecaySpec {
    Uniform(f64),
    PerChannel(std::collections::BTreeMap<String, f64>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProcessingConfig {
    pub dir_path: Option<String>,
    pub test_signal: Option<String>,
    pub room_target: Option<String>,
    pub room_mic_calibration: Option<String>,
    pub headphone_compensation_file: Option<String>,
    pub fs: Option<u32>,
    pub plot: bool,
    pub interactive_plots: bool,
    pub channel_balance: Option<String>,
    pub decay: Option<DecaySpec>,
    pub target_level: Option<f64>,
    pub fr_combination_method: String,
    pub specific_limit: f64,
    pub generic_limit: f64,
    pub bass_boost_gain: f64,
    pub bass_boost_fc: f64,
    pub bass_boost_q: f64,
    pub tilt: f64,
    pub do_room_correction: bool,
    pub do_headphone_compensation: bool,
    pub do_equalization: bool,
    pub remove_silent_channels: bool,
    pub head_ms: f64,
    pub jamesdsp: bool,
    pub hangloose: bool,
    pub microphone_deviation_correction: bool,
    pub mic_deviation_strength: f64,
    pub mic_deviation_debug_plots: bool,
    pub output_truehd_layouts: bool,
    pub vbass: bool,
    pub vbass_freq: u32,
    pub vbass_hp: f64,
    pub vbass_polarity: String,
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        Self {
            dir_path: None,
            test_signal: None,
            room_target: None,
            room_mic_calibration: None,
            headphone_compensation_file: None,
            fs: None,
            plot: false,
            interactive_plots: false,
            channel_balance: None,
            decay: None,
            target_level: None,
            fr_combination_method: "average".to_string(),
            specific_limit: 400.0,
            generic_limit: 300.0,
            bass_boost_gain: 0.0,
            bass_boost_fc: 105.0,
            bass_boost_q: 0.76,
            tilt: 0.0,
            do_room_correction: true,
            do_headphone_compensation: true,
            do_equalization: true,
            remove_silent_channels: false,
            head_ms: 1.0,
            jamesdsp: false,
            hangloose: false,
            microphone_deviation_correction: false,
            mic_deviation_strength: 0.7,
            mic_deviation_debug_plots: false,
            output_truehd_layouts: false,
            vbass: false,
            vbass_freq: 250,
            vbass_hp: 15.0,
            vbass_polarity: "auto".to_string(),
        }
    }
}

/// Field names in canonical (2.x dataclass) order. The CLI, the service
/// defaults and the feature registry are checked against this list.
pub const FIELD_NAMES: [&str; 33] = [
    "dir_path",
    "test_signal",
    "room_target",
    "room_mic_calibration",
    "headphone_compensation_file",
    "fs",
    "plot",
    "interactive_plots",
    "channel_balance",
    "decay",
    "target_level",
    "fr_combination_method",
    "specific_limit",
    "generic_limit",
    "bass_boost_gain",
    "bass_boost_fc",
    "bass_boost_q",
    "tilt",
    "do_room_correction",
    "do_headphone_compensation",
    "do_equalization",
    "remove_silent_channels",
    "head_ms",
    "jamesdsp",
    "hangloose",
    "microphone_deviation_correction",
    "mic_deviation_strength",
    "mic_deviation_debug_plots",
    "output_truehd_layouts",
    "vbass",
    "vbass_freq",
    "vbass_hp",
    "vbass_polarity",
];

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("invalid value for {field}: {reason}")]
    Invalid { field: String, reason: String },
}

impl ProcessingConfig {
    /// Build a config from a loose JSON object, ignoring unknown keys exactly
    /// like 2.x `ProcessingConfig.from_kwargs`. Known keys with the wrong type
    /// are an error.
    pub fn from_kwargs(
        kwargs: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Self, ConfigError> {
        let mut filtered = serde_json::Map::new();
        for (key, value) in kwargs {
            if FIELD_NAMES.contains(&key.as_str()) {
                filtered.insert(key.clone(), value.clone());
            }
        }
        serde_json::from_value(serde_json::Value::Object(filtered)).map_err(|e| {
            ConfigError::Invalid {
                field: "<kwargs>".to_string(),
                reason: e.to_string(),
            }
        })
    }
}
