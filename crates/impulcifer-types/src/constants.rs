//! Speaker names, delays and track orders. Port of `core/constants.py` (2.x).
//! Values must be verified against a JSON dump of the Python module
//! (see tests in impulcifer-dsp once the golden exporter exists).

pub const SPEAKER_NAMES: [&str; 15] = [
    "FL", "FR", "FC", "BL", "BR", "SL", "SR", "WL", "WR", "TFL", "TFR", "TSL", "TSR", "TBL", "TBR",
];

/// Named sweep track layouts accepted by the recorder (2.x `SWEEP_TRACK_LAYOUTS`).
pub const SWEEP_TRACK_LAYOUTS: [&str; 6] = ["mono", "stereo", "5.1", "7.1", "7.1.4", "7.1.6"];

/// Exact copies of core.constants.CHANNEL_LAYOUT_MAP (7.0.4 and 7.0.6).
pub const TRUEHD_11CH_ORDER: [&str; 11] = [
    "FL", "FR", "FC", "BL", "BR", "SL", "SR", "TFL", "TFR", "TBL", "TBR",
];
pub const TRUEHD_13CH_ORDER: [&str; 13] = [
    "FL", "FR", "FC", "BL", "BR", "SL", "SR", "TFL", "TFR", "TSL", "TSR", "TBL", "TBR",
];

#[cfg(test)]
mod p07_tests {
    use super::*;

    #[test]
    fn truehd_orders_match_python_golden() {
        let oracle: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/migration/goldens/p07_constants.json"
        ))
        .unwrap();
        assert_eq!(
            serde_json::json!(TRUEHD_11CH_ORDER),
            oracle["outputs"]["TRUEHD_11CH_ORDER"]
        );
        assert_eq!(
            serde_json::json!(TRUEHD_13CH_ORDER),
            oracle["outputs"]["TRUEHD_13CH_ORDER"]
        );
        assert_eq!(
            serde_json::json!(SPEAKER_NAMES),
            oracle["outputs"]["SPEAKER_NAMES"]
        );
    }
}
