use std::collections::{HashMap, HashSet};

use impulcifer_types::layouts::SLOTS;

use super::{ExternalError, ExternalFile, ExternalFormat};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum EarSide {
    Left,
    Right,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Assignment {
    pub slot: &'static str,
    pub side: EarSide,
    pub id: String,
}

pub fn slot_for_label(label: &str) -> Option<&'static str> {
    let normalized = normalize(label);
    if let Some(slot) = SLOTS.iter().find(|slot| normalize(slot.code) == normalized) {
        return Some(slot.code);
    }
    let slot = match normalized.as_str() {
        "left" | "l" | "frontleft" | "leftfront" => "FL",
        "right" | "r" | "frontright" | "rightfront" => "FR",
        "center" | "centre" | "c" | "frontcenter" | "frontcentre" | "centerfront" => "FC",
        "leftsurround" | "surroundleft" | "leftside" | "sideleft" | "leftsidesurround" | "ls"
        | "lss" => "SL",
        "rightsurround" | "surroundright" | "rightside" | "sideright" | "rightsidesurround"
        | "rs" | "rss" => "SR",
        "leftback" | "backleft" | "leftrear" | "rearleft" | "leftbacksurround"
        | "leftrearsurround" | "surroundbackleft" | "lb" | "lrs" | "lsb" | "lbs" => "BL",
        "rightback" | "backright" | "rightrear" | "rearright" | "rightbacksurround"
        | "rightrearsurround" | "surroundbackright" | "rb" | "rrs" | "rsb" | "rbs" => "BR",
        "leftwide" | "wideleft" | "frontwideleft" | "lw" | "flw" => "WL",
        "rightwide" | "wideright" | "frontwideright" | "rw" | "frw" => "WR",
        "leftfrontheight" | "frontleftheight" | "frontheightleft" | "heightfrontleft"
        | "leftheight" | "lfh" | "flh" | "fhl" => "HFL",
        "rightfrontheight" | "frontrightheight" | "frontheightright" | "heightfrontright"
        | "rightheight" | "rfh" | "frh" | "fhr" => "HFR",
        "centerfrontheight" | "frontcenterheight" | "centerheight" | "centreheight" | "cfh"
        | "fch" | "fhc" => "HFC",
        "leftbackheight" | "leftrearheight" | "backheightleft" | "rearheightleft" | "lbh"
        | "lrh" | "blh" => "HBL",
        "rightbackheight" | "rightrearheight" | "backheightright" | "rearheightright" | "rbh"
        | "rrh" | "brh" => "HBR",
        "topfrontleft" | "lefttopfront" | "leftfronttop" | "lft" | "ltf" => "TFL",
        "topfrontright" | "righttopfront" | "rightfronttop" | "rft" | "rtf" => "TFR",
        "topmiddleleft" | "lefttopmiddle" | "leftmiddletop" | "topsideleft" | "lmt" | "ltm"
        | "tml" => "TSL",
        "topmiddleright" | "righttopmiddle" | "rightmiddletop" | "topsideright" | "rmt" | "rtm"
        | "tmr" => "TSR",
        "toprearleft" | "topbackleft" | "lefttoprear" | "leftbacktop" | "leftreartop" | "lbt"
        | "ltr" | "ltb" => "TBL",
        "toprearright" | "topbackright" | "righttoprear" | "rightbacktop" | "rightreartop"
        | "rbt" | "rtr" | "rtb" => "TBR",
        "top" | "topcenter" | "topcentre" | "centertop" | "voiceofgod" | "vog" => "TC",
        _ => return None,
    };
    SLOTS
        .iter()
        .find(|candidate| candidate.code == slot)
        .map(|slot| slot.code)
}

pub(crate) fn is_subwoofer_name(label: &str) -> bool {
    let normalized = normalize(label);
    for prefix in ["sub", "subwoofer", "lfe"] {
        if let Some(suffix) = normalized.strip_prefix(prefix)
            && (suffix.is_empty() || suffix.chars().all(|c| c.is_ascii_digit()))
        {
            return true;
        }
    }
    false
}

pub fn auto_assign(file: &ExternalFile) -> Vec<Assignment> {
    let assignments = match file.format {
        ExternalFormat::Rew => auto_rew(file),
        ExternalFormat::Dirac => auto_dirac(file),
    };
    canonical(assignments)
}

fn auto_rew(file: &ExternalFile) -> Vec<Assignment> {
    let mut assigned = HashSet::new();
    let mut result = Vec::new();
    for measurement in &file.measurements {
        if measurement.skipped.is_some() {
            continue;
        }
        let Some((slot, ear)) = rew_title(&measurement.name) else {
            continue;
        };
        let sides: &[EarSide] = match ear {
            Some(EarSide::Left) => &[EarSide::Left],
            Some(EarSide::Right) => &[EarSide::Right],
            None => &[EarSide::Left, EarSide::Right],
        };
        for &side in sides {
            if assigned.insert((slot, side)) {
                result.push(Assignment {
                    slot,
                    side,
                    id: measurement.id.clone(),
                });
            }
        }
    }
    result
}

fn rew_title(title: &str) -> Option<(&'static str, Option<EarSide>)> {
    let tokens: Vec<String> = title
        .split(|c: char| c.is_whitespace() || "_-,.;:/()[]".contains(c))
        .filter(|token| !token.is_empty())
        .map(|token| token.to_lowercase())
        .collect();
    if tokens.iter().any(|token| is_subwoofer_name(token)) {
        return None;
    }
    let ambiguous = |token: &str| matches!(token, "left" | "right" | "l" | "r");
    if let Some((index, slot)) = tokens.iter().enumerate().find_map(|(index, token)| {
        (!ambiguous(token))
            .then(|| slot_for_label(token))
            .flatten()
            .map(|slot| (index, slot))
    }) {
        let ear = tokens
            .iter()
            .enumerate()
            .find_map(|(i, token)| (i != index).then(|| ear_word(token)).flatten());
        return Some((slot, ear));
    }
    let (index, token) = tokens
        .iter()
        .enumerate()
        .find(|(_, token)| ambiguous(token))?;
    let slot = slot_for_label(token)?;
    let ear = tokens
        .iter()
        .skip(index + 1)
        .find_map(|token| ear_word(token));
    Some((slot, ear))
}

fn ear_word(token: &str) -> Option<EarSide> {
    match token {
        "left" | "l" | "le" | "왼쪽" | "왼" | "좌" | "좌측" => Some(EarSide::Left),
        "right" | "r" | "re" | "오른쪽" | "오른" | "우" | "우측" => Some(EarSide::Right),
        _ => None,
    }
}

fn auto_dirac(file: &ExternalFile) -> Vec<Assignment> {
    let position = if file.positions.contains(&1) {
        Some(1)
    } else {
        file.positions.first().copied()
    };
    let Some(position) = position else {
        return Vec::new();
    };
    let mut used = HashSet::new();
    let mut result = Vec::new();
    for speaker in &file.speakers {
        if speaker.subwoofer {
            continue;
        }
        let Some(slot) = slot_for_label(&speaker.name) else {
            continue;
        };
        if !used.insert(slot) {
            continue;
        }
        let id = format!("{}@{position}", speaker.index);
        if file
            .measurements
            .iter()
            .any(|measurement| measurement.id == id && measurement.skipped.is_none())
        {
            for side in [EarSide::Left, EarSide::Right] {
                result.push(Assignment {
                    slot,
                    side,
                    id: id.clone(),
                });
            }
        }
    }
    result
}

pub fn parse_map(file: &ExternalFile, map: &str) -> Result<Vec<Assignment>, ExternalError> {
    let map = map.trim();
    if map.is_empty() || map.eq_ignore_ascii_case("auto") {
        return Ok(auto_assign(file));
    }
    let measurements: HashMap<_, _> = file
        .measurements
        .iter()
        .map(|m| (m.id.as_str(), m))
        .collect();
    let mut assigned = HashSet::new();
    let mut result: Vec<Assignment> = Vec::new();
    for entry in map.split(',') {
        let (target, id) = entry
            .split_once('=')
            .ok_or_else(|| ExternalError::new("invalid measurement map entry"))?;
        let id = id.trim();
        let measurement = measurements
            .get(id)
            .ok_or_else(|| ExternalError::new(format!("unknown measurement id: {id}")))?;
        if measurement.skipped.is_some() {
            return Err(ExternalError::new(format!("measurement is skipped: {id}")));
        }
        let (raw_slot, sides): (&str, &[EarSide]) =
            if let Some(slot) = target.trim().strip_suffix(".left") {
                (slot, &[EarSide::Left])
            } else if let Some(slot) = target.trim().strip_suffix(".right") {
                (slot, &[EarSide::Right])
            } else {
                (target.trim(), &[EarSide::Left, EarSide::Right])
            };
        let slot = SLOTS
            .iter()
            .find(|slot| slot.code.eq_ignore_ascii_case(raw_slot))
            .map(|slot| slot.code)
            .ok_or_else(|| ExternalError::new(format!("unknown speaker slot: {raw_slot}")))?;
        for &side in sides {
            if !assigned.insert((slot, side)) {
                return Err(ExternalError::new(format!(
                    "speaker slot side assigned more than once: {slot}"
                )));
            }
            result.push(Assignment {
                slot,
                side,
                id: id.to_owned(),
            });
        }
    }
    Ok(canonical(result))
}

pub fn format_map(assignments: &[Assignment]) -> String {
    let sorted = canonical(assignments.to_vec());
    let mut output = Vec::new();
    for slot in &SLOTS {
        let left = sorted
            .iter()
            .find(|a| a.slot == slot.code && a.side == EarSide::Left);
        let right = sorted
            .iter()
            .find(|a| a.slot == slot.code && a.side == EarSide::Right);
        match (left, right) {
            (Some(left), Some(right)) if left.id == right.id => {
                output.push(format!("{}={}", slot.code, left.id))
            }
            (Some(left), right) => {
                output.push(format!("{}.left={}", slot.code, left.id));
                if let Some(right) = right {
                    output.push(format!("{}.right={}", slot.code, right.id));
                }
            }
            (None, Some(right)) => output.push(format!("{}.right={}", slot.code, right.id)),
            (None, None) => {}
        }
    }
    output.join(",")
}

fn canonical(mut assignments: Vec<Assignment>) -> Vec<Assignment> {
    assignments.sort_by_key(|assignment| {
        let slot = SLOTS
            .iter()
            .position(|slot| slot.code == assignment.slot)
            .unwrap_or(usize::MAX);
        let side = match assignment.side {
            EarSide::Left => 0,
            EarSide::Right => 1,
        };
        (slot, side)
    });
    assignments
}

fn normalize(label: &str) -> String {
    label
        .chars()
        .filter(|c| !matches!(c, ' ' | '_' | '-' | '.'))
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external::{ExternalMeasurement, ExternalSpeaker, Skip};

    fn rew(names: &[(&str, Option<Skip>)]) -> ExternalFile {
        ExternalFile {
            format: ExternalFormat::Rew,
            title: None,
            sample_rate: 48000,
            microphone: None,
            mic_calibration: None,
            speakers: vec![],
            positions: vec![],
            measurements: names
                .iter()
                .enumerate()
                .map(|(i, (name, skipped))| ExternalMeasurement {
                    id: (i + 1).to_string(),
                    name: (*name).into(),
                    speaker: None,
                    position: None,
                    start_time: 0.0,
                    samples: if skipped.is_none() { vec![0.0] } else { vec![] },
                    skipped: *skipped,
                })
                .collect(),
        }
    }
    fn dirac(names: &[(&str, bool)], positions: &[usize]) -> ExternalFile {
        let speakers: Vec<_> = names
            .iter()
            .enumerate()
            .map(|(i, (n, s))| ExternalSpeaker {
                index: i + 1,
                name: (*n).into(),
                subwoofer: *s,
            })
            .collect();
        let p = *positions.first().unwrap();
        ExternalFile {
            format: ExternalFormat::Dirac,
            title: None,
            sample_rate: 48000,
            microphone: None,
            mic_calibration: None,
            positions: positions.to_vec(),
            measurements: speakers
                .iter()
                .map(|s| ExternalMeasurement {
                    id: format!("{}@{p}", s.index),
                    name: s.name.clone(),
                    speaker: Some(s.index),
                    position: Some(p),
                    start_time: 0.0,
                    samples: vec![0.0],
                    skipped: None,
                })
                .collect(),
            speakers,
        }
    }

    #[test]
    fn rew_examples_assign_as_specified() {
        for (name, expected) in [
            ("FL left", Some(("FL", Some(EarSide::Left)))),
            ("FL 오른쪽 🎧", Some(("FL", Some(EarSide::Right)))),
            ("L P1", Some(("FL", None))),
            ("R ART", Some(("FR", None))),
            ("C 왼쪽 귀", Some(("FC", Some(EarSide::Left)))),
            ("LS R", Some(("SL", Some(EarSide::Right)))),
            ("secs aver", None),
        ] {
            assert_eq!(rew_title(name), expected, "{name}");
        }
    }

    #[test]
    fn rew_skipped_sub_and_partial_both_ears_follow_rules() {
        let file = rew(&[
            ("FR only", Some(Skip::NoImpulseResponse)),
            ("Sub", None),
            ("FL left", None),
            ("FL", None),
        ]);
        let a = auto_assign(&file);
        assert_eq!(
            a.iter()
                .map(|a| (a.slot, a.side, a.id.as_str()))
                .collect::<Vec<_>>(),
            [("FL", EarSide::Left, "3"), ("FL", EarSide::Right, "4")]
        );
    }

    #[test]
    fn dirac_assigns_known_unique_speakers_at_lowest_measured_position() {
        let file = dirac(
            &[
                ("Left", false),
                ("Right", false),
                ("LeftSurround", false),
                ("RightSurround", false),
                ("Subwoofer", true),
                ("Subwoofer", true),
            ],
            &[2],
        );
        assert_eq!(
            format_map(&auto_assign(&file)),
            "FL=1@2,FR=2@2,SL=3@2,SR=4@2"
        );
        let duplicate = dirac(&[("Left", false), ("Left", false)], &[1]);
        assert_eq!(format_map(&auto_assign(&duplicate)), "FL=1@1");
    }

    #[test]
    fn parse_map_accepts_and_rejects_required_cases() {
        let file = rew(&[
            ("FL", None),
            ("FR", None),
            ("skip", Some(Skip::NoImpulseResponse)),
        ]);
        assert_eq!(
            format_map(&parse_map(&file, "fl = 1, FR.right=2").unwrap()),
            "FL=1,FR.right=2"
        );
        // A side given twice is an error, also when one entry is for both ears.
        for bad in [
            "FL=9",
            "FL=3",
            "XX=1",
            "FL.left=1,FL.left=2",
            "FL=1,FL.right=2",
        ] {
            assert!(parse_map(&file, bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn format_map_pairs_equal_ids_and_round_trips() {
        let file = dirac(&[("Left", false), ("Right", false)], &[1]);
        let paired = auto_assign(&file);
        assert_eq!(format_map(&paired), "FL=1@1,FR=2@1");
        assert_eq!(parse_map(&file, &format_map(&paired)).unwrap(), paired);
        let split = vec![
            Assignment {
                slot: "FL",
                side: EarSide::Left,
                id: "1@1".into(),
            },
            Assignment {
                slot: "FL",
                side: EarSide::Right,
                id: "2@1".into(),
            },
        ];
        assert_eq!(format_map(&split), "FL.left=1@1,FL.right=2@1");
        assert_eq!(parse_map(&file, &format_map(&split)).unwrap(), split);
    }
}
