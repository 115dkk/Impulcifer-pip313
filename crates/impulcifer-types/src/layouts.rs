//! Immersive loudspeaker formats and the position-keyed speaker slots their
//! channels map onto (ADR 0007).
//!
//! Directions use the ITU-R BS.2051 convention: azimuth in degrees, positive
//! to the LEFT of the front; elevation positive upwards. Sources quote other
//! conventions (Auro and DTS write left as negative); the tables below are
//! already converted.

/// The role a speaker plays, which decides the layer it is matched within.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// Listener (ear) level.
    Ear,
    /// Wall-mounted height ring: 22.2 and Auro height layers, DTS High,
    /// Dolby front/rear height (Lfh/Lrh).
    Height,
    /// Ceiling: Dolby top front/middle/rear, DTS Top, and the zenith.
    Top,
    /// Below ear level.
    Down,
    /// Low-frequency effects: never measured, silent in every output.
    Lfe,
}

/// A speaker position Impulcifer measures: its code is the name used in
/// recording file names (`FL,FR.wav`) and in `<code>-left`/`<code>-right`
/// track names.
#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub code: &'static str,
    /// Nominal azimuth and elevation (ITU convention).
    pub azimuth: f64,
    pub elevation: f64,
    pub layer: Layer,
    /// English description, e.g. for chart titles.
    pub description: &'static str,
}

const fn slot(
    code: &'static str,
    azimuth: f64,
    elevation: f64,
    layer: Layer,
    description: &'static str,
) -> Slot {
    Slot {
        code,
        azimuth,
        elevation,
        layer,
        description,
    }
}

/// Every slot, in `SPEAKER_NAMES` order: the 15 of 2.x, then the 25 added for
/// the immersive formats (ADR 0007 table 2). This order is also the order of
/// the extension tracks in hrir.wav and hesuvi.wav and must never change.
pub const SLOTS: [Slot; 40] = [
    slot("FL", 30.0, 0.0, Layer::Ear, "Front left speaker"),
    slot("FR", -30.0, 0.0, Layer::Ear, "Front right speaker"),
    slot("FC", 0.0, 0.0, Layer::Ear, "Center speaker"),
    slot("BL", 135.0, 0.0, Layer::Ear, "Back left speaker"),
    slot("BR", -135.0, 0.0, Layer::Ear, "Back right speaker"),
    slot("SL", 90.0, 0.0, Layer::Ear, "Side left speaker"),
    slot("SR", -90.0, 0.0, Layer::Ear, "Side right speaker"),
    slot("WL", 60.0, 0.0, Layer::Ear, "Wide left speaker"),
    slot("WR", -60.0, 0.0, Layer::Ear, "Wide right speaker"),
    slot("TFL", 45.0, 45.0, Layer::Top, "Top front left speaker"),
    slot("TFR", -45.0, 45.0, Layer::Top, "Top front right speaker"),
    slot("TSL", 90.0, 65.0, Layer::Top, "Top side left speaker"),
    slot("TSR", -90.0, 65.0, Layer::Top, "Top side right speaker"),
    slot("TBL", 135.0, 45.0, Layer::Top, "Top back left speaker"),
    slot("TBR", -135.0, 45.0, Layer::Top, "Top back right speaker"),
    slot("FCL", 15.0, 0.0, Layer::Ear, "Front left-of-center speaker"),
    slot(
        "FCR",
        -15.0,
        0.0,
        Layer::Ear,
        "Front right-of-center speaker",
    ),
    slot("SCL", 10.0, 0.0, Layer::Ear, "Screen left speaker"),
    slot("SCR", -10.0, 0.0, Layer::Ear, "Screen right speaker"),
    slot("SL1", 75.0, 0.0, Layer::Ear, "Side left 1 speaker"),
    slot("SR1", -75.0, 0.0, Layer::Ear, "Side right 1 speaker"),
    slot("SL2", 105.0, 0.0, Layer::Ear, "Side left 2 speaker"),
    slot("SR2", -105.0, 0.0, Layer::Ear, "Side right 2 speaker"),
    slot("BL1", 120.0, 0.0, Layer::Ear, "Back left 1 speaker"),
    slot("BR1", -120.0, 0.0, Layer::Ear, "Back right 1 speaker"),
    slot("BL2", 150.0, 0.0, Layer::Ear, "Back left 2 speaker"),
    slot("BR2", -150.0, 0.0, Layer::Ear, "Back right 2 speaker"),
    slot("BCL", 165.0, 0.0, Layer::Ear, "Back left-of-center speaker"),
    slot(
        "BCR",
        -165.0,
        0.0,
        Layer::Ear,
        "Back right-of-center speaker",
    ),
    slot("BC", 180.0, 0.0, Layer::Ear, "Back center speaker"),
    slot(
        "HFL",
        30.0,
        30.0,
        Layer::Height,
        "Height front left speaker",
    ),
    slot(
        "HFR",
        -30.0,
        30.0,
        Layer::Height,
        "Height front right speaker",
    ),
    slot(
        "HFC",
        0.0,
        30.0,
        Layer::Height,
        "Height front center speaker",
    ),
    slot(
        "HBL",
        135.0,
        30.0,
        Layer::Height,
        "Height back left speaker",
    ),
    slot(
        "HBR",
        -135.0,
        30.0,
        Layer::Height,
        "Height back right speaker",
    ),
    slot(
        "HBC",
        180.0,
        30.0,
        Layer::Height,
        "Height back center speaker",
    ),
    slot("TC", 0.0, 90.0, Layer::Top, "Top center speaker"),
    slot("DFL", 45.0, -30.0, Layer::Down, "Bottom front left speaker"),
    slot(
        "DFR",
        -45.0,
        -30.0,
        Layer::Down,
        "Bottom front right speaker",
    ),
    slot(
        "DFC",
        0.0,
        -30.0,
        Layer::Down,
        "Bottom front center speaker",
    ),
];

/// A channel that does not read from the nearest slot of its layer, with the
/// documents that equate it with the slot it does read from (ADR 0007 rule 1).
#[derive(Clone, Copy, Debug)]
pub struct MappingException {
    pub layout: &'static str,
    pub label: &'static str,
    pub slot: &'static str,
    pub sources: &'static [&'static str],
}

const TOP_MIDDLE: &[&str] = &[
    "Dolby Atmos Home Entertainment Studio Certification Guide (2019) Table 1: Ltm = SMPTE ST 2036-2 TpSiL",
    "ETSI TS 103 491 V1.2.1 Table 7-27 (NHK 22.2 layout) and Table 7-28: Lhs doubles as Left Top in middle",
    "Trinnov speaker positioning guide (2020): Dolby Ltm = DTS Lhs/Ltm",
];
const SIDE_SURROUND: &[&str] = &[
    "Trinnov speaker positioning guide (2020): Dolby Ls = Auro Ls = DTS Lss",
    "Auro-3D Home Theater Setup Guidelines Rev 12 section 3.3.1.2: the 7.1-based side pair is Lss/Rss",
];
const REAR_SURROUND: &[&str] =
    &["Trinnov speaker positioning guide (2020): Dolby Lrs = Auro Lb = DTS Lsr"];

/// Every departure from the nearest-slot rule.
pub const MAPPING_EXCEPTIONS: [MappingException; 10] = [
    MappingException {
        layout: "22.2",
        label: "TpSiL",
        slot: "TSL",
        sources: TOP_MIDDLE,
    },
    MappingException {
        layout: "22.2",
        label: "TpSiR",
        slot: "TSR",
        sources: TOP_MIDDLE,
    },
    MappingException {
        layout: "30.2",
        label: "Lhs",
        slot: "TSL",
        sources: TOP_MIDDLE,
    },
    MappingException {
        layout: "30.2",
        label: "Rhs",
        slot: "TSR",
        sources: TOP_MIDDLE,
    },
    MappingException {
        layout: "13.1",
        label: "Ls",
        slot: "SL",
        sources: SIDE_SURROUND,
    },
    MappingException {
        layout: "13.1",
        label: "Rs",
        slot: "SR",
        sources: SIDE_SURROUND,
    },
    MappingException {
        layout: "13.1",
        label: "Lb",
        slot: "BL",
        sources: REAR_SURROUND,
    },
    MappingException {
        layout: "13.1",
        label: "Rb",
        slot: "BR",
        sources: REAR_SURROUND,
    },
    MappingException {
        layout: "30.2",
        label: "Lsr",
        slot: "BL",
        sources: REAR_SURROUND,
    },
    MappingException {
        layout: "30.2",
        label: "Rsr",
        slot: "BR",
        sources: REAR_SURROUND,
    },
];

/// One channel of a format, in the format's official order.
#[derive(Clone, Copy, Debug)]
pub struct FormatChannel {
    /// The label the format's own documents use.
    pub label: &'static str,
    /// Nominal azimuth and elevation (ITU convention).
    pub azimuth: f64,
    pub elevation: f64,
    pub layer: Layer,
    /// The Impulcifer slot this channel reads from (`LFE`/`LFE2` for LFE).
    pub slot: &'static str,
}

/// A format whose BRIR is also written in its official channel order.
#[derive(Clone, Copy, Debug)]
pub struct ImmersiveLayout {
    /// Identifier used by `layout_files` and the recorder (`22.2`).
    pub id: &'static str,
    /// Display name (`NHK 22.2`).
    pub name: &'static str,
    /// Output file written next to hrir.wav.
    pub file_name: &'static str,
    pub channels: &'static [FormatChannel],
}

fn ears(name: &str) -> [String; 2] {
    [format!("{name}-left"), format!("{name}-right")]
}

impl ImmersiveLayout {
    /// The format's channel labels in official order.
    pub fn labels(&self) -> Vec<&'static str> {
        self.channels.iter().map(|c| c.label).collect()
    }
    /// The file's tracks as Impulcifer track names: both ears of each
    /// channel's slot in official order, LFE as `LFE`/`LFE2` (never measured).
    pub fn track_names(&self) -> Vec<String> {
        self.channels.iter().flat_map(|c| ears(c.slot)).collect()
    }
    /// The file's tracks under the format's own labels (`TpFL-left`), as
    /// written to its `ICHL` chunk.
    pub fn label_track_names(&self) -> Vec<String> {
        self.channels.iter().flat_map(|c| ears(c.label)).collect()
    }
    /// The non-LFE channels whose slot is not measured.
    pub fn missing(&self, measured: impl Fn(&str) -> bool) -> Vec<&'static FormatChannel> {
        self.channels
            .iter()
            .filter(|c| c.layer != Layer::Lfe && !measured(c.slot))
            .collect()
    }
}

/// The `layout_files` option (ADR 0007 rule 4): `auto` writes every format
/// whose channels are all measured, `none` writes no format file, and a
/// comma-separated list of format ids also writes those formats when channels
/// are missing (silent) while the others stay automatic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayoutFiles {
    Auto,
    None,
    Listed(Vec<&'static str>),
}
impl LayoutFiles {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "auto" => return Ok(Self::Auto),
            "none" => return Ok(Self::None),
            _ => (),
        }
        let mut listed = Vec::new();
        for id in value.split(',').map(str::trim) {
            let known = layout(id).ok_or_else(|| {
                format!(
                    "layout_files must be auto, none or a list of {}; got \"{id}\".",
                    IMMERSIVE_LAYOUTS.map(|l| l.id).join(", ")
                )
            })?;
            if listed.contains(&known.id) {
                return Err(format!("layout_files lists {id} twice."));
            }
            listed.push(known.id);
        }
        Ok(Self::Listed(listed))
    }
    /// Whether the format's file is written, given whether all its channels
    /// are measured.
    pub fn writes(&self, id: &str, complete: bool) -> bool {
        match self {
            Self::Auto => complete,
            Self::None => false,
            Self::Listed(ids) => complete || ids.contains(&id),
        }
    }
    /// The formats written even when channels are missing.
    pub fn forced(&self) -> &[&'static str] {
        match self {
            Self::Listed(ids) => ids,
            _ => &[],
        }
    }
}

const fn ch(
    label: &'static str,
    azimuth: f64,
    elevation: f64,
    layer: Layer,
    slot: &'static str,
) -> FormatChannel {
    FormatChannel {
        label,
        azimuth,
        elevation,
        layer,
        slot,
    }
}
const fn lfe(label: &'static str, slot: &'static str) -> FormatChannel {
    ch(label, 0.0, 0.0, Layer::Lfe, slot)
}

use Layer::{Down, Ear, Height, Top};

/// NHK 22.2: SMPTE ST 2036-2 Table 1 (= ARIB STD-B59 Table 2-1 = ITU-R
/// BS.2051-3 System H Table 10 = BS.2094-2 AP_00010009). Nominal directions
/// from BS.2051-3 Table 1. 22.2's FL/FR are the ±60° pair; FLc/FRc are ±30°.
const NHK_22_2: [FormatChannel; 24] = [
    ch("FL", 60.0, 0.0, Ear, "WL"),
    ch("FR", -60.0, 0.0, Ear, "WR"),
    ch("FC", 0.0, 0.0, Ear, "FC"),
    lfe("LFE1", "LFE"),
    ch("BL", 135.0, 0.0, Ear, "BL"),
    ch("BR", -135.0, 0.0, Ear, "BR"),
    ch("FLc", 30.0, 0.0, Ear, "FL"),
    ch("FRc", -30.0, 0.0, Ear, "FR"),
    ch("BC", 180.0, 0.0, Ear, "BC"),
    lfe("LFE2", "LFE2"),
    ch("SiL", 90.0, 0.0, Ear, "SL"),
    ch("SiR", -90.0, 0.0, Ear, "SR"),
    ch("TpFL", 45.0, 30.0, Height, "HFL"),
    ch("TpFR", -45.0, 30.0, Height, "HFR"),
    ch("TpFC", 0.0, 30.0, Height, "HFC"),
    ch("TpC", 0.0, 90.0, Top, "TC"),
    ch("TpBL", 135.0, 30.0, Height, "HBL"),
    ch("TpBR", -135.0, 30.0, Height, "HBR"),
    ch("TpSiL", 90.0, 30.0, Height, "TSL"),
    ch("TpSiR", -90.0, 30.0, Height, "TSR"),
    ch("TpBC", 180.0, 30.0, Height, "HBC"),
    ch("BtFC", 0.0, -30.0, Down, "DFC"),
    ch("BtFL", 45.0, -30.0, Down, "DFL"),
    ch("BtFR", -45.0, -30.0, Down, "DFR"),
];

/// Auro-3D 13.1: "AURO-3D in multi-channel WAV-files" Rev 2 (2024-04-19)
/// channel map, WAVEX mask 0x2FE3F. Nominal directions from the Auro Home
/// Theater Setup Guidelines Rev 12 Table 3 (sign flipped to ITU).
const AURO_13_1: [FormatChannel; 14] = [
    ch("L", 30.0, 0.0, Ear, "FL"),
    ch("R", -30.0, 0.0, Ear, "FR"),
    ch("C", 0.0, 0.0, Ear, "FC"),
    lfe("LFE", "LFE"),
    ch("Lb", 150.0, 0.0, Ear, "BL"),
    ch("Rb", -150.0, 0.0, Ear, "BR"),
    ch("Ls", 110.0, 0.0, Ear, "SL"),
    ch("Rs", -110.0, 0.0, Ear, "SR"),
    ch("T", 0.0, 90.0, Top, "TC"),
    ch("HL", 30.0, 30.0, Height, "HFL"),
    ch("HC", 0.0, 30.0, Height, "HFC"),
    ch("HR", -30.0, 30.0, Height, "HFR"),
    ch("HLs", 110.0, 30.0, Height, "HBL"),
    ch("HRs", -110.0, 30.0, Height, "HBR"),
];

/// DTS:X Pro 30.2: ETSI TS 103 491 V1.2.1 Table B-5 (= Table C-4 = TS 103 584
/// Table 3), the per-speaker ChannelMask with all 32 bits set. DTS publishes
/// no PCM interleave for a 30.2 decoder; this is the mask's label order.
/// Nominal directions from TS 103 584 Table 3 (sign flipped to ITU); TS 103
/// 491 Table 7-28 gives ±45° for Lh, ±135° for Lhr and ±45° for Lb instead.
/// High (45°) is the height ring, Top (60°) the ceiling.
const DTSX_30_2: [FormatChannel; 32] = [
    ch("C", 0.0, 0.0, Ear, "FC"),
    ch("L", 30.0, 0.0, Ear, "FL"),
    ch("R", -30.0, 0.0, Ear, "FR"),
    ch("Ls", 110.0, 0.0, Ear, "SL2"),
    ch("Rs", -110.0, 0.0, Ear, "SR2"),
    lfe("LFE1", "LFE"),
    ch("Cs", 180.0, 0.0, Ear, "BC"),
    ch("Lsr", 150.0, 0.0, Ear, "BL"),
    ch("Rsr", -150.0, 0.0, Ear, "BR"),
    ch("Lss", 90.0, 0.0, Ear, "SL"),
    ch("Rss", -90.0, 0.0, Ear, "SR"),
    ch("Lc", 15.0, 0.0, Ear, "FCL"),
    ch("Rc", -15.0, 0.0, Ear, "FCR"),
    ch("Lh", 30.0, 45.0, Height, "HFL"),
    ch("Ch", 0.0, 45.0, Height, "HFC"),
    ch("Rh", -30.0, 45.0, Height, "HFR"),
    lfe("LFE2", "LFE2"),
    ch("Lw", 60.0, 0.0, Ear, "WL"),
    ch("Rw", -60.0, 0.0, Ear, "WR"),
    ch("Oh", 0.0, 90.0, Top, "TC"),
    ch("Lhs", 90.0, 45.0, Height, "TSL"),
    ch("Rhs", -90.0, 45.0, Height, "TSR"),
    ch("Chr", 180.0, 45.0, Height, "HBC"),
    ch("Lhr", 150.0, 45.0, Height, "HBL"),
    ch("Rhr", -150.0, 45.0, Height, "HBR"),
    ch("Cb", 0.0, -30.0, Down, "DFC"),
    ch("Lb", 30.0, -30.0, Down, "DFL"),
    ch("Rb", -30.0, -30.0, Down, "DFR"),
    ch("Ltf", 45.0, 60.0, Top, "TFL"),
    ch("Rtf", -45.0, 60.0, Top, "TFR"),
    ch("Ltr", 135.0, 60.0, Top, "TBL"),
    ch("Rtr", -135.0, 60.0, Top, "TBR"),
];

/// Dolby Atmos 24.1.10 (24 ear-level, LFE, 10 overhead): labels from the
/// Dolby Atmos Home Theater Installation Guidelines R3.1. Dolby publishes no
/// channel order above 9.1.6, so this order is Impulcifer's (ADR 0007):
/// channels 1-16 are the 9.1.6 interchange order (Apple
/// kAudioChannelLayoutTag_Atmos_9_1_6, AWS MediaConvert DD+ JOC input), the
/// rest follow the guidelines' section 4 listing. Ear-level angles are within
/// Dolby's ranges on an evenly spaced ring (Lsc 10, Lc 20, L 30, Lw 60, Ls1
/// 75, Ls 90, Ls2 105, Lrs1 120, Lrs 135, Lrs2 145, Lcs 165); overheads use
/// Dolby's side-view elevations.
const ATMOS_24_1_10: [FormatChannel; 35] = [
    ch("L", 30.0, 0.0, Ear, "FL"),
    ch("R", -30.0, 0.0, Ear, "FR"),
    ch("C", 0.0, 0.0, Ear, "FC"),
    lfe("LFE", "LFE"),
    ch("Ls", 90.0, 0.0, Ear, "SL"),
    ch("Rs", -90.0, 0.0, Ear, "SR"),
    ch("Lrs", 135.0, 0.0, Ear, "BL"),
    ch("Rrs", -135.0, 0.0, Ear, "BR"),
    ch("Lw", 60.0, 0.0, Ear, "WL"),
    ch("Rw", -60.0, 0.0, Ear, "WR"),
    ch("Ltf", 45.0, 45.0, Top, "TFL"),
    ch("Rtf", -45.0, 45.0, Top, "TFR"),
    ch("Ltm", 90.0, 65.0, Top, "TSL"),
    ch("Rtm", -90.0, 65.0, Top, "TSR"),
    ch("Ltr", 135.0, 45.0, Top, "TBL"),
    ch("Rtr", -135.0, 45.0, Top, "TBR"),
    ch("Lc", 20.0, 0.0, Ear, "FCL"),
    ch("Rc", -20.0, 0.0, Ear, "FCR"),
    ch("Lsc", 10.0, 0.0, Ear, "SCL"),
    ch("Rsc", -10.0, 0.0, Ear, "SCR"),
    ch("Ls1", 75.0, 0.0, Ear, "SL1"),
    ch("Rs1", -75.0, 0.0, Ear, "SR1"),
    ch("Ls2", 105.0, 0.0, Ear, "SL2"),
    ch("Rs2", -105.0, 0.0, Ear, "SR2"),
    ch("Lrs1", 120.0, 0.0, Ear, "BL1"),
    ch("Rrs1", -120.0, 0.0, Ear, "BR1"),
    ch("Lrs2", 145.0, 0.0, Ear, "BL2"),
    ch("Rrs2", -145.0, 0.0, Ear, "BR2"),
    ch("Lcs", 165.0, 0.0, Ear, "BCL"),
    ch("Rcs", -165.0, 0.0, Ear, "BCR"),
    ch("Cs", 180.0, 0.0, Ear, "BC"),
    ch("Lfh", 30.0, 35.0, Height, "HFL"),
    ch("Rfh", -30.0, 35.0, Height, "HFR"),
    ch("Lrh", 150.0, 35.0, Height, "HBL"),
    ch("Rrh", -150.0, 35.0, Height, "HBR"),
];

/// Every format Impulcifer writes in its own channel order.
pub const IMMERSIVE_LAYOUTS: [ImmersiveLayout; 4] = [
    ImmersiveLayout {
        id: "22.2",
        name: "NHK 22.2",
        file_name: "nhk_22.2.wav",
        channels: &NHK_22_2,
    },
    ImmersiveLayout {
        id: "13.1",
        name: "Auro-3D 13.1",
        file_name: "auro_13.1.wav",
        channels: &AURO_13_1,
    },
    ImmersiveLayout {
        id: "30.2",
        name: "DTS:X Pro 30.2",
        file_name: "dtsx_30.2.wav",
        channels: &DTSX_30_2,
    },
    ImmersiveLayout {
        id: "24.1.10",
        name: "Dolby Atmos 24.1.10",
        file_name: "atmos_24.1.10.wav",
        channels: &ATMOS_24_1_10,
    },
];

/// The layout with this identifier.
pub fn layout(id: &str) -> Option<&'static ImmersiveLayout> {
    IMMERSIVE_LAYOUTS.iter().find(|l| l.id == id)
}

/// The layout whose output file has this name.
pub fn layout_for_file(file_name: &str) -> Option<&'static ImmersiveLayout> {
    IMMERSIVE_LAYOUTS.iter().find(|l| l.file_name == file_name)
}
