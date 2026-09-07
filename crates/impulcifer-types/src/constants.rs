//! Speaker names, delays and track orders. Port of `core/constants.py` (2.x).
//! Values must be verified against a JSON dump of the Python module
//! (see tests in impulcifer-dsp once the golden exporter exists).

pub const SPEAKER_NAMES: [&str; 15] = [
    "FL", "FR", "FC", "BL", "BR", "SL", "SR", "WL", "WR", "TFL", "TFR", "TSL", "TSR", "TBL", "TBR",
];

/// Named sweep track layouts accepted by the recorder (2.x `SWEEP_TRACK_LAYOUTS`).
pub const SWEEP_TRACK_LAYOUTS: [&str; 6] = ["mono", "stereo", "5.1", "7.1", "7.1.4", "7.1.6"];
