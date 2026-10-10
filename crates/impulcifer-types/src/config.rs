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
    pub room_range: String,
    pub room_volume: Option<f64>,
    pub schroeder_freq: Option<f64>,
    pub room_max_boost: f64,
    pub room_mode: String,
    #[serde(deserialize_with = "string_or_number")]
    pub room_tuning_delay: String,
    #[serde(deserialize_with = "string_or_number")]
    pub room_tuning_phase_limit: String,
    pub room_tuning_max_boost: f64,
    pub room_tuning_curtain: f64,
    pub room_tuning_level_match: bool,
    pub vbass_mode: String,
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
            room_range: "schroeder".into(),
            room_volume: None,
            schroeder_freq: None,
            room_max_boost: 12.0,
            room_mode: "eq".into(),
            room_tuning_delay: "10".into(),
            room_tuning_phase_limit: "full".into(),
            room_tuning_max_boost: 6.0,
            room_tuning_curtain: 300.0,
            room_tuning_level_match: true,
            vbass_mode: "auto".into(),
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

/// 3.x-only options, separate from the frozen 2.x dataclass surface.
pub const EXTENSION_FIELD_NAMES: [&str; 11] = [
    "room_range",
    "room_volume",
    "schroeder_freq",
    "room_max_boost",
    "room_mode",
    "room_tuning_delay",
    "room_tuning_phase_limit",
    "room_tuning_max_boost",
    "room_tuning_curtain",
    "room_tuning_level_match",
    "vbass_mode",
];

fn string_or_number<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        _ => Err(serde::de::Error::custom("expected a string or number")),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TuningDelay {
    Auto,
    Ms(f64),
}
impl TuningDelay {
    pub fn parse(s: &str) -> Option<Self> {
        if s == "auto" {
            return Some(Self::Auto);
        }
        s.parse::<f64>()
            .ok()
            .filter(|v| v.is_finite() && (2.0..=20.0).contains(v))
            .map(Self::Ms)
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PhaseLimit {
    Off,
    Schroeder,
    Full,
    Hz(f64),
}
impl PhaseLimit {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "off" => Some(Self::Off),
            "schroeder" => Some(Self::Schroeder),
            "full" => Some(Self::Full),
            _ => s
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite() && (300.0..=20000.0).contains(v))
                .map(Self::Hz),
        }
    }
    pub fn frequency(self, fs: u32, schroeder: f64) -> f64 {
        match self {
            Self::Off => 0.0,
            Self::Schroeder => schroeder.min(300.0),
            Self::Full => fs as f64 / 2.0,
            Self::Hz(f) if f >= 0.45 * fs as f64 => fs as f64 / 2.0,
            Self::Hz(f) => f,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("invalid value for {field}: {reason}")]
    Invalid { field: String, reason: String },
}

impl ProcessingConfig {
    /// The 2.x dataclass defaults; use in tests that compare against 2.x oracle output.
    pub fn oracle_defaults() -> Self {
        Self {
            room_range: "legacy".into(),
            vbass_mode: "legacy".into(),
            ..Self::default()
        }
    }

    /// Validate the 3.x extension options without changing legacy validation semantics.
    pub fn validate_room_options(&self) -> Result<(), ConfigError> {
        let invalid = |field: &str, reason: &str| ConfigError::Invalid {
            field: field.into(),
            reason: reason.into(),
        };
        if !["legacy", "modes", "schroeder", "extreme"].contains(&self.room_range.as_str()) {
            return Err(invalid(
                "room_range",
                "must be legacy, modes, schroeder or extreme",
            ));
        }
        if !["eq", "tuning"].contains(&self.room_mode.as_str()) {
            return Err(invalid("room_mode", "must be eq or tuning"));
        }
        if !["auto", "manual", "legacy"].contains(&self.vbass_mode.as_str()) {
            return Err(invalid("vbass_mode", "must be auto, manual or legacy"));
        }
        if TuningDelay::parse(&self.room_tuning_delay).is_none() {
            return Err(invalid("room_tuning_delay", "must be auto or 2–20 ms"));
        }
        if PhaseLimit::parse(&self.room_tuning_phase_limit).is_none() {
            return Err(invalid(
                "room_tuning_phase_limit",
                "must be off, schroeder, full or 300–20000 Hz",
            ));
        }
        for (field, value, lo, hi, exclusive) in [
            (
                "room_tuning_max_boost",
                Some(self.room_tuning_max_boost),
                0.0,
                12.0,
                false,
            ),
            (
                "room_tuning_curtain",
                Some(self.room_tuning_curtain),
                100.0,
                5000.0,
                false,
            ),
            ("room_volume", self.room_volume, 0.0, 10000.0, true),
            ("schroeder_freq", self.schroeder_freq, 50.0, 1000.0, false),
            (
                "room_max_boost",
                Some(self.room_max_boost),
                0.0,
                24.0,
                false,
            ),
        ] {
            if let Some(v) = value
                && (!v.is_finite() || v > hi || v < lo || (exclusive && v == lo))
            {
                return Err(invalid(field, "outside supported range"));
            }
        }
        Ok(())
    }

    /// Build a config from a loose JSON object, ignoring unknown keys exactly
    /// like 2.x `ProcessingConfig.from_kwargs`. Known keys with the wrong type
    /// are an error.
    pub fn from_kwargs(
        kwargs: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Self, ConfigError> {
        let mut filtered = serde_json::Map::new();
        for (key, value) in kwargs {
            if FIELD_NAMES.contains(&key.as_str()) || EXTENSION_FIELD_NAMES.contains(&key.as_str())
            {
                filtered.insert(key.clone(), value.clone());
            }
        }
        let explicit_crossover =
            kwargs.contains_key("vbass_freq") && !kwargs.contains_key("vbass_mode");
        let mut config: Self = serde_json::from_value(serde_json::Value::Object(filtered))
            .map_err(|e| ConfigError::Invalid {
                field: "<kwargs>".to_string(),
                reason: e.to_string(),
            })?;
        // An explicit crossover opts into manual mode unless the caller chose a mode.
        if explicit_crossover {
            config.vbass_mode = "manual".into();
        }
        Ok(config)
    }
}
