#![forbid(unsafe_code)]
//! Immersive layouts (ADR 0007): official channel orders, the position-keyed
//! slot each format channel maps onto, and the invariants that keep a track
//! from landing on the wrong speaker or ear.

use impulcifer_types::layouts::{
    IMMERSIVE_LAYOUTS, Layer, MAPPING_EXCEPTIONS, SLOTS, Slot, layout,
};

/// SMPTE ST 2036-2 Table 1 = ARIB STD-B59 Table 2-1 = ITU-R BS.2051-3 System H.
#[test]
fn nhk_22_2_lists_the_smpte_st_2036_2_order() {
    let l = layout("22.2").unwrap();
    assert_eq!(l.file_name, "nhk_22.2.wav");
    assert_eq!(
        l.labels(),
        [
            "FL", "FR", "FC", "LFE1", "BL", "BR", "FLc", "FRc", "BC", "LFE2", "SiL", "SiR", "TpFL",
            "TpFR", "TpFC", "TpC", "TpBL", "TpBR", "TpSiL", "TpSiR", "TpBC", "BtFC", "BtFL",
            "BtFR",
        ]
    );
}

/// Auro "AURO-3D in multi-channel WAV-files" Rev 2 (2024-04-19), WAVEX mask
/// 0x2FE3F: back before side, Top before the height layer.
#[test]
fn auro_13_1_lists_the_auro_wav_white_paper_order() {
    let l = layout("13.1").unwrap();
    assert_eq!(l.file_name, "auro_13.1.wav");
    assert_eq!(
        l.labels(),
        [
            "L", "R", "C", "LFE", "Lb", "Rb", "Ls", "Rs", "T", "HL", "HC", "HR", "HLs", "HRs"
        ]
    );
}

/// ETSI TS 103 491 V1.2.1 Table B-5 (= Table C-4 = TS 103 584 Table 3): the
/// 32-bit ChannelMask, all bits set for 30.2. DTS's "Lb/Rb" are LOW front.
#[test]
fn dtsx_30_2_lists_the_ts_103_491_channel_mask_order() {
    let l = layout("30.2").unwrap();
    assert_eq!(l.file_name, "dtsx_30.2.wav");
    assert_eq!(
        l.labels(),
        [
            "C", "L", "R", "Ls", "Rs", "LFE1", "Cs", "Lsr", "Rsr", "Lss", "Rss", "Lc", "Rc", "Lh",
            "Ch", "Rh", "LFE2", "Lw", "Rw", "Oh", "Lhs", "Rhs", "Chr", "Lhr", "Rhr", "Cb", "Lb",
            "Rb", "Ltf", "Rtf", "Ltr", "Rtr",
        ]
    );
}

/// Dolby publishes no order above 9.1.6 (ADR 0007): channels 1-16 are the
/// 9.1.6 interchange order (Apple kAudioChannelLayoutTag_Atmos_9_1_6, AWS
/// MediaConvert DD+ JOC input), then the additional speakers in the order of
/// the Dolby Atmos Home Theater Installation Guidelines section 4.
#[test]
fn atmos_24_1_10_starts_with_the_9_1_6_interchange_order() {
    let l = layout("24.1.10").unwrap();
    assert_eq!(l.file_name, "atmos_24.1.10.wav");
    let labels = l.labels();
    assert_eq!(
        labels[..16],
        [
            "L", "R", "C", "LFE", "Ls", "Rs", "Lrs", "Rrs", "Lw", "Rw", "Ltf", "Rtf", "Ltm", "Rtm",
            "Ltr", "Rtr",
        ]
    );
    assert_eq!(
        labels[16..],
        [
            "Lc", "Rc", "Lsc", "Rsc", "Ls1", "Rs1", "Ls2", "Rs2", "Lrs1", "Rrs1", "Lrs2", "Rrs2",
            "Lcs", "Rcs", "Cs", "Lfh", "Rfh", "Lrh", "Rrh",
        ]
    );
}

/// Great-circle angle between two directions, in degrees.
fn angle(az1: f64, el1: f64, az2: f64, el2: f64) -> f64 {
    let (a1, e1, a2, e2) = (
        az1.to_radians(),
        el1.to_radians(),
        az2.to_radians(),
        el2.to_radians(),
    );
    let cos = e1.sin() * e2.sin() + e1.cos() * e2.cos() * (a1 - a2).cos();
    cos.clamp(-1.0, 1.0).acos().to_degrees()
}
/// The slot of `layer` closest to a direction; ties are a table error.
fn nearest(azimuth: f64, elevation: f64, layer: Layer) -> &'static Slot {
    let mut slots: Vec<_> = SLOTS
        .iter()
        .filter(|s| s.layer == layer)
        .map(|s| (angle(azimuth, elevation, s.azimuth, s.elevation), s))
        .collect();
    slots.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert!(
        slots[1].0 - slots[0].0 > 1e-9,
        "{azimuth}/{elevation} is equally close to {} and {}",
        slots[0].1.code,
        slots[1].1.code
    );
    slots[0].1
}

/// ADR 0007 rule 1: a channel reads from the nearest slot of its layer (by
/// role, not elevation); every departure is a cited exception, and an
/// exception that the rule already satisfies is stale.
#[test]
fn every_channel_maps_to_the_nearest_slot_of_its_layer_or_a_cited_exception() {
    for l in IMMERSIVE_LAYOUTS {
        for c in l.channels.iter().filter(|c| c.layer != Layer::Lfe) {
            let rule = nearest(c.azimuth, c.elevation, c.layer).code;
            let exception = MAPPING_EXCEPTIONS
                .iter()
                .find(|e| e.layout == l.id && e.label == c.label);
            match exception {
                Some(e) => {
                    assert_eq!(e.slot, c.slot, "{} {}", l.id, c.label);
                    assert_ne!(rule, c.slot, "{} {}: stale exception", l.id, c.label);
                    assert!(!e.sources.is_empty(), "{} {}", l.id, c.label);
                }
                None => assert_eq!(c.slot, rule, "{} {} is nearest to {rule}", l.id, c.label),
            }
        }
    }
    for e in MAPPING_EXCEPTIONS {
        let l = layout(e.layout).unwrap();
        assert!(l.channels.iter().any(|c| c.label == e.label), "{}", e.label);
    }
}

fn slot_of(code: &str) -> &'static Slot {
    SLOTS
        .iter()
        .find(|s| s.code == code)
        .unwrap_or_else(|| panic!("{code} is not a slot"))
}
/// The slot at the same height on the other side of the median plane.
fn mirror(code: &str) -> &'static str {
    let s = slot_of(code);
    if s.azimuth % 180.0 == 0.0 {
        return s.code;
    }
    SLOTS
        .iter()
        .find(|m| m.layer == s.layer && m.elevation == s.elevation && m.azimuth == -s.azimuth)
        .map(|m| m.code)
        .unwrap_or_else(|| panic!("{code} has no mirror"))
}

#[test]
fn slots_have_unique_codes_and_positions() {
    for (i, a) in SLOTS.iter().enumerate() {
        for b in &SLOTS[i + 1..] {
            assert_ne!(a.code, b.code);
            assert!(
                a.layer != b.layer || a.azimuth != b.azimuth || a.elevation != b.elevation,
                "{} and {} share a position",
                a.code,
                b.code
            );
        }
        mirror(a.code);
    }
}

/// Two channels of one format never read the same slot, so no track is
/// written twice and none is lost.
#[test]
fn no_format_reads_one_slot_twice() {
    for l in IMMERSIVE_LAYOUTS {
        for (i, a) in l.channels.iter().enumerate() {
            for b in &l.channels[i + 1..] {
                assert_ne!(a.slot, b.slot, "{}: {} and {}", l.id, a.label, b.label);
            }
        }
    }
}

/// A left channel and its right twin read mirrored slots: catches a pair
/// split across rows (Lhs -> TSL with Rhs -> TBR) and a left/right swap.
#[test]
fn left_and_right_channels_read_mirrored_slots() {
    for l in IMMERSIVE_LAYOUTS {
        for c in l.channels.iter().filter(|c| c.layer != Layer::Lfe) {
            let s = slot_of(c.slot);
            assert_eq!(
                c.azimuth.signum() * (c.azimuth.abs() % 180.0).signum(),
                s.azimuth.signum() * (s.azimuth.abs() % 180.0).signum(),
                "{} {} reads {} on the other side",
                l.id,
                c.label,
                c.slot
            );
            if c.azimuth.abs() % 180.0 == 0.0 {
                continue;
            }
            let twin = l
                .channels
                .iter()
                .find(|t| {
                    t.layer == c.layer
                        && t.elevation == c.elevation
                        && (t.azimuth + c.azimuth).abs() < 1e-9
                })
                .unwrap_or_else(|| panic!("{} {} has no twin", l.id, c.label));
            assert_eq!(
                twin.slot,
                mirror(c.slot),
                "{} {}/{}",
                l.id,
                c.label,
                twin.label
            );
        }
    }
}

/// A new slot code never spells a format label that means another speaker
/// (22.2's FLc is our FL, so Dolby's Lc is FCL, not FLC). The 2.x codes
/// predate the formats; 22.2's FL (±60° = WL) is documented instead.
#[test]
fn new_slot_codes_never_spell_a_label_of_another_speaker() {
    for s in &SLOTS[15..] {
        for l in IMMERSIVE_LAYOUTS {
            for c in l.channels {
                assert!(
                    !c.label.eq_ignore_ascii_case(s.code) || c.slot == s.code,
                    "{} spells {} {} which reads {}",
                    s.code,
                    l.id,
                    c.label,
                    c.slot
                );
            }
        }
    }
}

#[test]
fn every_added_slot_belongs_to_a_format() {
    for s in &SLOTS[15..] {
        assert!(
            IMMERSIVE_LAYOUTS
                .iter()
                .any(|l| l.channels.iter().any(|c| c.slot == s.code)),
            "{} is unused",
            s.code
        );
    }
}

mod constants {
    use super::{SLOTS, mirror, slot_of};
    use impulcifer_types::constants::*;

    fn tracks(speakers: &[&str]) -> Vec<String> {
        speakers
            .iter()
            .flat_map(|s| [format!("{s}-left"), format!("{s}-right")])
            .collect()
    }

    /// 3.x speaks every slot; the 2.x list stays its prefix, so recordings and
    /// outputs of 2.x speakers keep their meaning and bytes (ADR 0007 rule 5).
    #[test]
    fn speaker_names_are_the_slots_after_the_2x_names() {
        let codes: Vec<_> = SLOTS.iter().map(|s| s.code).collect();
        assert_eq!(SPEAKER_NAMES.to_vec(), codes);
        assert_eq!(SPEAKER_NAMES[..15], SPEAKER_NAMES_2X);
        assert_eq!(SPEAKER_DELAYS.len(), SPEAKER_NAMES.len());
    }

    #[test]
    fn hrir_and_hesuvi_append_every_added_slot_after_their_2x_tracks() {
        let added = tracks(&SPEAKER_NAMES[15..]);
        assert_eq!(
            HEXADECAGONAL_TRACK_ORDER[..32],
            HEXADECAGONAL_TRACK_ORDER_2X
        );
        assert_eq!(HEXADECAGONAL_TRACK_ORDER[32..].to_vec(), added);
        assert_eq!(HESUVI_TRACK_ORDER[..30], HESUVI_TRACK_ORDER_2X);
        assert_eq!(HESUVI_TRACK_ORDER[30..].to_vec(), added);
        assert_eq!(base_channel_count(&HEXADECAGONAL_TRACK_ORDER), 16);
        assert_eq!(base_channel_count(&HESUVI_TRACK_ORDER), 14);
    }

    /// Virtual bass, alignment and the README pick ears by speaker_side; an
    /// unlisted slot would silently count as a centre speaker.
    #[test]
    fn every_slot_is_on_the_side_of_its_azimuth() {
        for s in SLOTS {
            let expected = if s.azimuth % 180.0 == 0.0 {
                Side::Center
            } else if s.azimuth > 0.0 {
                Side::Left
            } else {
                Side::Right
            };
            assert_eq!(speaker_side(s.code), expected, "{}", s.code);
        }
        assert_eq!(speaker_side("LFE2"), Side::Center);
    }

    /// Ipsilateral alignment and channel balance skip unlisted speakers.
    #[test]
    fn every_slot_is_in_exactly_one_ipsilateral_pair() {
        assert_eq!(IPSILATERAL_PAIRS[..8], IPSILATERAL_PAIRS_2X);
        for s in SLOTS {
            let pairs: Vec<_> = IPSILATERAL_PAIRS
                .iter()
                .filter(|(a, b)| *a == s.code || *b == s.code)
                .collect();
            assert_eq!(pairs.len(), 1, "{}", s.code);
            let (a, b) = pairs[0];
            assert_eq!(*b, mirror(a), "{a}/{b}");
            assert_ne!(
                speaker_side(a),
                Side::Right,
                "{a} must be the left of its pair"
            );
            let _ = slot_of(b);
        }
    }
}

mod files {
    use impulcifer_types::layouts::{IMMERSIVE_LAYOUTS, LayoutFiles, layout, layout_for_file};

    /// Two tracks per channel, left ear first, LFE as silent slots in place.
    #[test]
    fn a_layout_file_reads_both_ears_of_each_slot_in_official_order() {
        let l = layout("22.2").unwrap();
        let names = l.track_names();
        assert_eq!(names.len(), 48);
        assert_eq!(names[..4], ["WL-left", "WL-right", "WR-left", "WR-right"]);
        assert_eq!(names[6..8], ["LFE-left", "LFE-right"]);
        assert_eq!(names[12..14], ["FL-left", "FL-right"]);
        assert_eq!(names[18..20], ["LFE2-left", "LFE2-right"]);
        let labels = l.label_track_names();
        assert_eq!(labels[..2], ["FL-left", "FL-right"]);
        assert_eq!(labels[12..14], ["FLc-left", "FLc-right"]);
        for l in IMMERSIVE_LAYOUTS {
            assert_eq!(l.track_names().len(), 2 * l.channels.len());
            assert_eq!(layout_for_file(l.file_name).unwrap().id, l.id);
        }
        assert!(layout_for_file("hrir.wav").is_none());
    }

    #[test]
    fn missing_lists_unmeasured_channels_and_never_lfe() {
        let l = layout("13.1").unwrap();
        let measured = [
            "FL", "FR", "FC", "SL", "SR", "BL", "BR", "TC", "HFL", "HFR", "HFC",
        ];
        let missing: Vec<_> = l
            .missing(|s| measured.contains(&s))
            .iter()
            .map(|c| c.label)
            .collect();
        assert_eq!(missing, ["HLs", "HRs"]);
        assert!(l.missing(|s| s != "LFE").is_empty());
    }

    /// `auto` writes complete formats, `none` nothing, and a list of ids also
    /// writes those formats when channels are missing (ADR 0007 rule 4).
    #[test]
    fn layout_files_option_selects_complete_or_listed_formats() {
        let auto = LayoutFiles::parse("auto").unwrap();
        assert!(auto.writes("22.2", true) && !auto.writes("22.2", false));
        let none = LayoutFiles::parse("none").unwrap();
        assert!(!none.writes("22.2", true));
        let listed = LayoutFiles::parse(" 24.1.10 , 30.2").unwrap();
        assert!(listed.writes("24.1.10", false) && listed.writes("30.2", false));
        assert!(listed.writes("22.2", true) && !listed.writes("22.2", false));
        assert_eq!(listed.forced(), ["24.1.10", "30.2"]);
        for bad in ["", "7.1.4", "auto,22.2", "none,13.1", "22.2,22.2"] {
            assert!(LayoutFiles::parse(bad).is_err(), "{bad:?}");
        }
    }
}
