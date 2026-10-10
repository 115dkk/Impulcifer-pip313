//! Speaker names, delays and track orders. Port of `core/constants.py` (2.x).
//! Values must be verified against a JSON dump of the Python module
//! (see tests in impulcifer-dsp once the golden exporter exists).

/// The 2.x speaker list (core/constants.py SPEAKER_NAMES), compared with the
/// 2.x golden; 3.x code uses [`SPEAKER_NAMES`].
pub const SPEAKER_NAMES_2X: [&str; 15] = [
    "FL", "FR", "FC", "BL", "BR", "SL", "SR", "WL", "WR", "TFL", "TFR", "TSL", "TSR", "TBL", "TBR",
];

/// Every speaker slot 3.x measures: the 2.x names, then the slots added for the
/// immersive formats (ADR 0007), in [`crate::layouts::SLOTS`] order.
pub const SPEAKER_NAMES: [&str; 40] = {
    let mut out = [""; 40];
    let mut i = 0;
    while i < out.len() {
        out[i] = crate::layouts::SLOTS[i].code;
        i += 1;
    }
    out
};

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
            serde_json::json!(SPEAKER_NAMES_2X),
            oracle["outputs"]["SPEAKER_NAMES"]
        );
    }
}

/// core.constants::HESUVI_TRACK_ORDER; p08_constants.json.
pub const HESUVI_TRACK_ORDER_2X: [&str; 30] = [
    "FL-left",
    "FL-right",
    "SL-left",
    "SL-right",
    "BL-left",
    "BL-right",
    "FC-left",
    "FR-right",
    "FR-left",
    "SR-right",
    "SR-left",
    "BR-right",
    "BR-left",
    "FC-right",
    "WL-left",
    "WL-right",
    "WR-left",
    "WR-right",
    "TFL-left",
    "TFL-right",
    "TFR-left",
    "TFR-right",
    "TSL-left",
    "TSL-right",
    "TSR-left",
    "TSR-right",
    "TBL-left",
    "TBL-right",
    "TBR-left",
    "TBR-right",
];
/// [`HESUVI_TRACK_ORDER_2X`], then both ears of every slot added in 3.x (ADR 0007).
pub const HESUVI_TRACK_ORDER: [&str; 80] = [
    "FL-left",
    "FL-right",
    "SL-left",
    "SL-right",
    "BL-left",
    "BL-right",
    "FC-left",
    "FR-right",
    "FR-left",
    "SR-right",
    "SR-left",
    "BR-right",
    "BR-left",
    "FC-right",
    "WL-left",
    "WL-right",
    "WR-left",
    "WR-right",
    "TFL-left",
    "TFL-right",
    "TFR-left",
    "TFR-right",
    "TSL-left",
    "TSL-right",
    "TSR-left",
    "TSR-right",
    "TBL-left",
    "TBL-right",
    "TBR-left",
    "TBR-right",
    "FCL-left",
    "FCL-right",
    "FCR-left",
    "FCR-right",
    "SCL-left",
    "SCL-right",
    "SCR-left",
    "SCR-right",
    "SL1-left",
    "SL1-right",
    "SR1-left",
    "SR1-right",
    "SL2-left",
    "SL2-right",
    "SR2-left",
    "SR2-right",
    "BL1-left",
    "BL1-right",
    "BR1-left",
    "BR1-right",
    "BL2-left",
    "BL2-right",
    "BR2-left",
    "BR2-right",
    "BCL-left",
    "BCL-right",
    "BCR-left",
    "BCR-right",
    "BC-left",
    "BC-right",
    "HFL-left",
    "HFL-right",
    "HFR-left",
    "HFR-right",
    "HFC-left",
    "HFC-right",
    "HBL-left",
    "HBL-right",
    "HBR-left",
    "HBR-right",
    "HBC-left",
    "HBC-right",
    "TC-left",
    "TC-right",
    "DFL-left",
    "DFL-right",
    "DFR-left",
    "DFR-right",
    "DFC-left",
    "DFC-right",
];
/// core.constants::HEXADECAGONAL_TRACK_ORDER; p08_constants.json.
pub const HEXADECAGONAL_TRACK_ORDER_2X: [&str; 32] = [
    "FL-left",
    "FL-right",
    "FR-left",
    "FR-right",
    "FC-left",
    "FC-right",
    "LFE-left",
    "LFE-right",
    "BL-left",
    "BL-right",
    "BR-left",
    "BR-right",
    "SL-left",
    "SL-right",
    "SR-left",
    "SR-right",
    "WL-left",
    "WL-right",
    "WR-left",
    "WR-right",
    "TFL-left",
    "TFL-right",
    "TFR-left",
    "TFR-right",
    "TSL-left",
    "TSL-right",
    "TSR-left",
    "TSR-right",
    "TBL-left",
    "TBL-right",
    "TBR-left",
    "TBR-right",
];
/// [`HEXADECAGONAL_TRACK_ORDER_2X`], then both ears of every slot added in 3.x (ADR 0007).
pub const HEXADECAGONAL_TRACK_ORDER: [&str; 82] = [
    "FL-left",
    "FL-right",
    "FR-left",
    "FR-right",
    "FC-left",
    "FC-right",
    "LFE-left",
    "LFE-right",
    "BL-left",
    "BL-right",
    "BR-left",
    "BR-right",
    "SL-left",
    "SL-right",
    "SR-left",
    "SR-right",
    "WL-left",
    "WL-right",
    "WR-left",
    "WR-right",
    "TFL-left",
    "TFL-right",
    "TFR-left",
    "TFR-right",
    "TSL-left",
    "TSL-right",
    "TSR-left",
    "TSR-right",
    "TBL-left",
    "TBL-right",
    "TBR-left",
    "TBR-right",
    "FCL-left",
    "FCL-right",
    "FCR-left",
    "FCR-right",
    "SCL-left",
    "SCL-right",
    "SCR-left",
    "SCR-right",
    "SL1-left",
    "SL1-right",
    "SR1-left",
    "SR1-right",
    "SL2-left",
    "SL2-right",
    "SR2-left",
    "SR2-right",
    "BL1-left",
    "BL1-right",
    "BR1-left",
    "BR1-right",
    "BL2-left",
    "BL2-right",
    "BR2-left",
    "BR2-right",
    "BCL-left",
    "BCL-right",
    "BCR-left",
    "BCR-right",
    "BC-left",
    "BC-right",
    "HFL-left",
    "HFL-right",
    "HFR-left",
    "HFR-right",
    "HFC-left",
    "HFC-right",
    "HBL-left",
    "HBL-right",
    "HBR-left",
    "HBR-right",
    "HBC-left",
    "HBC-right",
    "TC-left",
    "TC-right",
    "DFL-left",
    "DFL-right",
    "DFR-left",
    "DFR-right",
    "DFC-left",
    "DFC-right",
];
/// core.constants::LEFT_SIDE_SPEAKERS; p08_constants.json.
pub const LEFT_SIDE_SPEAKERS_2X: [&str; 7] = ["BL", "FL", "SL", "TBL", "TFL", "TSL", "WL"];
/// [`LEFT_SIDE_SPEAKERS_2X`] and the slots added in 3.x.
pub const LEFT_SIDE_SPEAKERS: [&str; 17] = [
    "BL", "FL", "SL", "TBL", "TFL", "TSL", "WL", "FCL", "SCL", "SL1", "SL2", "BL1", "BL2", "BCL",
    "HFL", "HBL", "DFL",
];
/// core.constants::RIGHT_SIDE_SPEAKERS; p08_constants.json.
pub const RIGHT_SIDE_SPEAKERS_2X: [&str; 7] = ["BR", "FR", "SR", "TBR", "TFR", "TSR", "WR"];
/// [`RIGHT_SIDE_SPEAKERS_2X`] and the slots added in 3.x.
pub const RIGHT_SIDE_SPEAKERS: [&str; 17] = [
    "BR", "FR", "SR", "TBR", "TFR", "TSR", "WR", "FCR", "SCR", "SR1", "SR2", "BR1", "BR2", "BCR",
    "HFR", "HBR", "DFR",
];
/// core.constants::CENTER_SPEAKERS; p08_constants.json.
pub const CENTER_SPEAKERS_2X: [&str; 2] = ["FC", "LFE"];
/// [`CENTER_SPEAKERS_2X`] and the slots added in 3.x.
pub const CENTER_SPEAKERS: [&str; 8] = ["FC", "LFE", "BC", "HFC", "HBC", "TC", "DFC", "LFE2"];
/// core.constants:66-75; p08_constants.json.
pub const IPSILATERAL_PAIRS_2X: [(&str, &str); 8] = [
    ("FL", "FR"),
    ("SL", "SR"),
    ("BL", "BR"),
    ("TFL", "TFR"),
    ("TSL", "TSR"),
    ("TBL", "TBR"),
    ("FC", "FC"),
    ("WL", "WR"),
];
/// [`IPSILATERAL_PAIRS_2X`] and one pair per slot added in 3.x: (left, right),
/// or (centre, centre) for a centre speaker.
pub const IPSILATERAL_PAIRS: [(&str, &str); 23] = [
    ("FL", "FR"),
    ("SL", "SR"),
    ("BL", "BR"),
    ("TFL", "TFR"),
    ("TSL", "TSR"),
    ("TBL", "TBR"),
    ("FC", "FC"),
    ("WL", "WR"),
    ("FCL", "FCR"),
    ("SCL", "SCR"),
    ("SL1", "SR1"),
    ("SL2", "SR2"),
    ("BL1", "BR1"),
    ("BL2", "BR2"),
    ("BCL", "BCR"),
    ("BC", "BC"),
    ("HFL", "HFR"),
    ("HFC", "HFC"),
    ("HBL", "HBR"),
    ("HBC", "HBC"),
    ("TC", "TC"),
    ("DFL", "DFR"),
    ("DFC", "DFC"),
];
/// core.constants:55-57, in SPEAKER_NAMES order; p08_constants.json.
pub const SPEAKER_DELAYS: [f64; 40] = [0.0; 40];
/// core.impulse_response_estimator:18-23; p08_constants.json.
pub const SEQUENCE_TRACK_ORDERS: [(&str, &[&str]); 4] = [
    ("5.1", &["FL", "FR", "FC", "LFE", "BL", "BR"]),
    ("7.1", &["FL", "FR", "FC", "LFE", "BL", "BR", "SL", "SR"]),
    (
        "7.1.4",
        &[
            "FL", "FR", "FC", "LFE", "BL", "BR", "SL", "SR", "TFL", "TFR", "TBL", "TBR",
        ],
    ),
    (
        "7.1.6",
        &[
            "FL", "FR", "FC", "LFE", "BL", "BR", "SL", "SR", "TFL", "TFR", "TSL", "TSR", "TBL",
            "TBR",
        ],
    ),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Center,
}
/// Python speaker_side, core/constants.py:19-26; p08_constants.json.
pub fn speaker_side(name: &str) -> Side {
    let name = name.to_uppercase();
    if LEFT_SIDE_SPEAKERS.contains(&name.as_str()) {
        Side::Left
    } else if RIGHT_SIDE_SPEAKERS.contains(&name.as_str()) {
        Side::Right
    } else {
        Side::Center
    }
}
/// Python track_name, core/constants.py:116-118; p08_constants.json.
pub fn track_name(speaker: &str, side: &str) -> String {
    format!("{speaker}-{side}")
}
/// Python base_channel_count, core/brir_layout.py:20-27; p08_constants.json.
pub fn base_channel_count(order: &[&str]) -> usize {
    if order == HESUVI_TRACK_ORDER || order == HESUVI_TRACK_ORDER_2X {
        14
    } else if order == HEXADECAGONAL_TRACK_ORDER || order == HEXADECAGONAL_TRACK_ORDER_2X {
        16
    } else {
        order.len()
    }
}
