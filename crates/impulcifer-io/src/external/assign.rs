use std::collections::{HashMap, HashSet};

use impulcifer_types::layouts::SLOTS;

use super::{ExternalError, ExternalFile, ExternalFormat};

const LFE_SLOTS: [&str; 2] = ["LFE", "LFE2"];

fn lfe_slot(value: &str) -> Option<&'static str> {
    LFE_SLOTS
        .into_iter()
        .find(|slot| slot.eq_ignore_ascii_case(value.trim()))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum EarSide {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Purpose {
    Room,
    Brir,
    Headphones,
}

impl Purpose {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "room" => Some(Self::Room),
            "brir" => Some(Self::Brir),
            "headphones" => Some(Self::Headphones),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Room => "room",
            Self::Brir => "brir",
            Self::Headphones => "headphones",
        }
    }
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
    auto_assign_for(file, Purpose::Room)
}

pub fn auto_assign_for(file: &ExternalFile, purpose: Purpose) -> Vec<Assignment> {
    let assignments = match (file.format, purpose) {
        (ExternalFormat::Rew, Purpose::Room) => auto_rew_room(file),
        (ExternalFormat::Dirac, Purpose::Room) => auto_dirac_room(file),
        (ExternalFormat::Rew, Purpose::Brir) => auto_rew_brir(file),
        (ExternalFormat::Dirac, Purpose::Brir) => auto_dirac_brir(file),
        (ExternalFormat::Rew, Purpose::Headphones) => auto_rew_headphones(file),
        (ExternalFormat::Dirac, Purpose::Headphones) => auto_dirac_headphones(file),
    };
    canonical(assignments)
}

fn auto_rew_room(file: &ExternalFile) -> Vec<Assignment> {
    let mut assigned = HashSet::new();
    let mut result = Vec::new();
    for measurement in usable(file) {
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

fn auto_rew_brir(file: &ExternalFile) -> Vec<Assignment> {
    let mut assigned = HashSet::new();
    usable(file)
        .filter_map(|measurement| {
            let (slot, side) = if let Some(target) = rew_subwoofer_title(&measurement.name) {
                target
            } else {
                let (slot, side) = rew_title(&measurement.name)?;
                (slot, side?)
            };
            assigned.insert((slot, side)).then(|| Assignment {
                slot,
                side,
                id: measurement.id.clone(),
            })
        })
        .collect()
}

fn auto_rew_headphones(file: &ExternalFile) -> Vec<Assignment> {
    let measurements: Vec<_> = usable(file).collect();
    let mut assigned = HashSet::new();
    let mut result = Vec::new();
    for measurement in &measurements {
        let Some(side) = headphone_side(&measurement.name) else {
            continue;
        };
        if assigned.insert(side) {
            result.push(headphone_assignment(side, measurement.id.clone()));
        }
    }
    if result.is_empty() && measurements.len() == 2 {
        result.push(headphone_assignment(
            EarSide::Left,
            measurements[0].id.clone(),
        ));
        result.push(headphone_assignment(
            EarSide::Right,
            measurements[1].id.clone(),
        ));
    }
    result
}

fn usable(file: &ExternalFile) -> impl Iterator<Item = &super::ExternalMeasurement> {
    file.measurements.iter().filter(|m| m.skipped.is_none())
}

fn rew_subwoofer_title(title: &str) -> Option<(&'static str, EarSide)> {
    let tokens = title_tokens(title);
    let (index, token) = tokens
        .iter()
        .enumerate()
        .find(|(_, token)| is_subwoofer_name(token))?;
    let slot = if token.ends_with('2') { "LFE2" } else { "LFE" };
    let side = tokens
        .iter()
        .enumerate()
        .find_map(|(i, token)| (i != index).then(|| ear_word(token)).flatten())?;
    Some((slot, side))
}

fn rew_title(title: &str) -> Option<(&'static str, Option<EarSide>)> {
    let tokens = title_tokens(title);
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

fn title_tokens(title: &str) -> Vec<String> {
    title
        .split(|c: char| c.is_whitespace() || "_-,.;:/()[]".contains(c))
        .filter(|token| !token.is_empty())
        .map(|token| token.to_lowercase())
        .collect()
}

fn headphone_side(title: &str) -> Option<EarSide> {
    let tokens = title_tokens(title);
    tokens.iter().find_map(|token| ear_word(token)).or_else(|| {
        tokens.iter().find_map(|token| match slot_for_label(token) {
            Some("FL") => Some(EarSide::Left),
            Some("FR") => Some(EarSide::Right),
            _ => None,
        })
    })
}

fn ear_word(token: &str) -> Option<EarSide> {
    match token {
        "left" | "l" | "le" | "왼쪽" | "왼" | "좌" | "좌측" => Some(EarSide::Left),
        "right" | "r" | "re" | "오른쪽" | "오른" | "우" | "우측" => Some(EarSide::Right),
        _ => None,
    }
}

fn ear_positions(file: &ExternalFile) -> Option<(usize, usize)> {
    let mut positions = file.positions.clone();
    positions.sort_unstable();
    positions.dedup();
    let left = *positions.first()?;
    let right = positions.get(1).copied().unwrap_or(left);
    Some((left, right))
}

fn auto_dirac_room(file: &ExternalFile) -> Vec<Assignment> {
    let position = if file.positions.contains(&1) {
        Some(1)
    } else {
        file.positions.iter().min().copied()
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
        if measurement_exists(file, &id) {
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

fn auto_dirac_brir(file: &ExternalFile) -> Vec<Assignment> {
    let Some((left, right)) = ear_positions(file) else {
        return Vec::new();
    };
    let mut used = HashSet::new();
    let mut lfe_count = 0;
    let mut result = Vec::new();
    for speaker in &file.speakers {
        if speaker.subwoofer {
            if lfe_count == LFE_SLOTS.len() {
                continue;
            }
            let left_id = format!("{}@{left}", speaker.index);
            let right_id = format!("{}@{right}", speaker.index);
            if measurement_exists(file, &left_id) && measurement_exists(file, &right_id) {
                let slot = LFE_SLOTS[lfe_count];
                lfe_count += 1;
                result.push(Assignment {
                    slot,
                    side: EarSide::Left,
                    id: left_id,
                });
                result.push(Assignment {
                    slot,
                    side: EarSide::Right,
                    id: right_id,
                });
            }
            continue;
        }
        let Some(slot) = slot_for_label(&speaker.name) else {
            continue;
        };
        if !used.insert(slot) {
            continue;
        }
        for (side, position) in [(EarSide::Left, left), (EarSide::Right, right)] {
            let id = format!("{}@{position}", speaker.index);
            if measurement_exists(file, &id) {
                result.push(Assignment { slot, side, id });
            }
        }
    }
    result
}

fn auto_dirac_headphones(file: &ExternalFile) -> Vec<Assignment> {
    let Some((left_position, right_position)) = ear_positions(file) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for (wanted, side, position) in [
        ("FL", EarSide::Left, left_position),
        ("FR", EarSide::Right, right_position),
    ] {
        let Some(speaker) = file
            .speakers
            .iter()
            .find(|speaker| !speaker.subwoofer && slot_for_label(&speaker.name) == Some(wanted))
        else {
            continue;
        };
        let id = format!("{}@{position}", speaker.index);
        if measurement_exists(file, &id) {
            result.push(headphone_assignment(side, id));
        }
    }
    result
}

fn measurement_exists(file: &ExternalFile, id: &str) -> bool {
    file.measurements
        .iter()
        .any(|measurement| measurement.id == id && measurement.skipped.is_none())
}

fn headphone_assignment(side: EarSide, id: String) -> Assignment {
    Assignment {
        slot: match side {
            EarSide::Left => "FL",
            EarSide::Right => "FR",
        },
        side,
        id,
    }
}

pub fn parse_map(file: &ExternalFile, map: &str) -> Result<Vec<Assignment>, ExternalError> {
    parse_map_for(file, map, Purpose::Room)
}

pub fn parse_map_for(
    file: &ExternalFile,
    map: &str,
    purpose: Purpose,
) -> Result<Vec<Assignment>, ExternalError> {
    let map = map.trim();
    if map.is_empty() || map.eq_ignore_ascii_case("auto") {
        return Ok(auto_assign_for(file, purpose));
    }
    let measurements: HashMap<_, _> = file
        .measurements
        .iter()
        .map(|m| (m.id.as_str(), m))
        .collect();
    let mut assigned = HashSet::new();
    let mut result = Vec::new();
    for entry in map.split(',') {
        let (target, id) = entry
            .split_once('=')
            .ok_or_else(|| ExternalError::new("invalid measurement map entry"))?;
        let target = target.trim();
        let id = id.trim();
        let measurement = measurements
            .get(id)
            .ok_or_else(|| ExternalError::new(format!("unknown measurement id: {id}")))?;
        if measurement.skipped.is_some() {
            return Err(ExternalError::new(format!("measurement is skipped: {id}")));
        }
        let raw_target = target
            .strip_suffix(".left")
            .or_else(|| target.strip_suffix(".right"))
            .unwrap_or(target);
        if let Some(slot) = lfe_slot(raw_target)
            && purpose != Purpose::Brir
        {
            return Err(ExternalError::new(format!(
                "speaker slot {slot} is not valid for {} measurements",
                purpose.as_str()
            )));
        }
        let targets: Vec<(&'static str, EarSide)> = match purpose {
            Purpose::Headphones => match target.to_ascii_lowercase().as_str() {
                "left" => vec![("FL", EarSide::Left)],
                "right" => vec![("FR", EarSide::Right)],
                _ => {
                    return Err(ExternalError::new(
                        "a headphone map entry must be left=ID or right=ID",
                    ));
                }
            },
            Purpose::Room | Purpose::Brir => {
                let (raw_slot, sides): (&str, &[EarSide]) =
                    if let Some(slot) = target.strip_suffix(".left") {
                        (slot, &[EarSide::Left])
                    } else if let Some(slot) = target.strip_suffix(".right") {
                        (slot, &[EarSide::Right])
                    } else if purpose == Purpose::Brir {
                        return Err(ExternalError::new(
                            "a BRIR map entry names one ear: use SLOT.left=ID or SLOT.right=ID",
                        ));
                    } else {
                        (target, &[EarSide::Left, EarSide::Right])
                    };
                let slot = lfe_slot(raw_slot).or_else(|| {
                    SLOTS
                        .iter()
                        .find(|slot| slot.code.eq_ignore_ascii_case(raw_slot.trim()))
                        .map(|slot| slot.code)
                });
                let slot = slot.ok_or_else(|| {
                    ExternalError::new(format!("unknown speaker slot: {raw_slot}"))
                })?;
                sides.iter().map(|side| (slot, *side)).collect()
            }
        };
        for (slot, side) in targets {
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
    format_map_for(assignments, Purpose::Room)
}

pub fn format_map_for(assignments: &[Assignment], purpose: Purpose) -> String {
    let sorted = canonical(assignments.to_vec());
    if purpose == Purpose::Headphones {
        return [EarSide::Left, EarSide::Right]
            .iter()
            .filter_map(|side| {
                sorted.iter().find(|a| a.side == *side).map(|a| {
                    format!(
                        "{}={}",
                        if *side == EarSide::Left {
                            "left"
                        } else {
                            "right"
                        },
                        a.id
                    )
                })
            })
            .collect::<Vec<_>>()
            .join(",");
    }
    let mut output = Vec::new();
    for slot in &SLOTS {
        let left = sorted
            .iter()
            .find(|a| a.slot == slot.code && a.side == EarSide::Left);
        let right = sorted
            .iter()
            .find(|a| a.slot == slot.code && a.side == EarSide::Right);
        if purpose == Purpose::Room {
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
        } else {
            if let Some(left) = left {
                output.push(format!("{}.left={}", slot.code, left.id));
            }
            if let Some(right) = right {
                output.push(format!("{}.right={}", slot.code, right.id));
            }
        }
    }
    if purpose == Purpose::Brir {
        for slot in LFE_SLOTS {
            if let Some(left) = sorted
                .iter()
                .find(|a| a.slot == slot && a.side == EarSide::Left)
            {
                output.push(format!("{slot}.left={}", left.id));
            }
            if let Some(right) = sorted
                .iter()
                .find(|a| a.slot == slot && a.side == EarSide::Right)
            {
                output.push(format!("{slot}.right={}", right.id));
            }
        }
    }
    output.join(",")
}

pub fn check_assignments(
    assignments: &[Assignment],
    purpose: Purpose,
) -> Result<(), ExternalError> {
    match purpose {
        Purpose::Room => Ok(()),
        Purpose::Brir => {
            if assignments.is_empty() {
                return Err(ExternalError::new("no BRIR measurements assigned"));
            }
            for slot in SLOTS.iter().map(|s| s.code).chain(LFE_SLOTS) {
                let sides: HashSet<_> = assignments
                    .iter()
                    .filter(|a| a.slot == slot)
                    .map(|a| a.side)
                    .collect();
                if !sides.is_empty() {
                    if !sides.contains(&EarSide::Left) {
                        return Err(ExternalError::new(format!(
                            "speaker slot {slot} has no left ear measurement"
                        )));
                    }
                    if !sides.contains(&EarSide::Right) {
                        return Err(ExternalError::new(format!(
                            "speaker slot {slot} has no right ear measurement"
                        )));
                    }
                }
            }
            check_unique_ids(assignments)?;
            if !assignments.iter().any(|a| a.slot == "FL") {
                return Err(ExternalError::new(
                    "the BRIR measurements must include FL, the reference for alignment",
                ));
            }
            Ok(())
        }
        Purpose::Headphones => {
            let left = assignments.iter().find(|a| a.side == EarSide::Left);
            let right = assignments.iter().find(|a| a.side == EarSide::Right);
            match (left, right) {
                (None, None) => Err(ExternalError::new(
                    "headphone measurement needs a left and a right ear; both ears are missing",
                )),
                (None, Some(_)) => Err(ExternalError::new(
                    "headphone measurement needs a left and a right ear; the left ear is missing",
                )),
                (Some(_), None) => Err(ExternalError::new(
                    "headphone measurement needs a left and a right ear; the right ear is missing",
                )),
                (Some(_), Some(_)) => check_unique_ids(assignments),
            }
        }
    }
}

fn check_unique_ids(assignments: &[Assignment]) -> Result<(), ExternalError> {
    let mut ids = HashSet::new();
    for assignment in assignments {
        if !ids.insert(&assignment.id) {
            return Err(ExternalError::new(format!(
                "measurement {} is assigned more than once",
                assignment.id
            )));
        }
    }
    Ok(())
}

fn canonical(mut assignments: Vec<Assignment>) -> Vec<Assignment> {
    assignments.sort_by_key(|assignment| {
        let slot = SLOTS
            .iter()
            .position(|slot| slot.code == assignment.slot)
            .or_else(|| {
                LFE_SLOTS
                    .iter()
                    .position(|slot| *slot == assignment.slot)
                    .map(|index| SLOTS.len() + index)
            })
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
            .map(|(i, (name, subwoofer))| ExternalSpeaker {
                index: i + 1,
                name: (*name).into(),
                subwoofer: *subwoofer,
            })
            .collect();
        let measurements = positions
            .iter()
            .flat_map(|position| {
                speakers.iter().map(move |speaker| ExternalMeasurement {
                    id: format!("{}@{position}", speaker.index),
                    name: speaker.name.clone(),
                    speaker: Some(speaker.index),
                    position: Some(*position),
                    start_time: 0.0,
                    samples: vec![0.0],
                    skipped: None,
                })
            })
            .collect();
        ExternalFile {
            format: ExternalFormat::Dirac,
            title: None,
            sample_rate: 48000,
            microphone: None,
            mic_calibration: None,
            positions: positions.to_vec(),
            measurements,
            speakers,
        }
    }

    #[test]
    fn rew_examples_assign_as_specified() {
        let file = rew(&[("FL left", None), ("FL right", None), ("FL", None)]);
        assert_eq!(format_map(&auto_assign(&file)), "FL.left=1,FL.right=2");
    }

    #[test]
    fn dirac_assigns_known_unique_speakers_at_lowest_measured_position() {
        let file = dirac(&[("Left", false), ("Right", false)], &[2]);
        assert_eq!(format_map(&auto_assign(&file)), "FL=1@2,FR=2@2");
    }

    #[test]
    fn parse_map_accepts_and_rejects_required_cases() {
        let file = rew(&[("FL", None), ("FR", None)]);
        assert_eq!(
            format_map(&parse_map(&file, "FL=1,FR.right=2").unwrap()),
            "FL=1,FR.right=2"
        );
        assert!(parse_map(&file, "XX=1").is_err());
    }

    #[test]
    fn purpose_parse_and_room_wrappers_are_exact() {
        assert_eq!(Purpose::parse("room"), Some(Purpose::Room));
        assert_eq!(Purpose::parse("BRIR"), None);
        let files = [
            rew(&[("FL left", None), ("FL right", None)]),
            dirac(&[("Left", false)], &[1, 2]),
        ];
        for file in files {
            assert_eq!(auto_assign(&file), auto_assign_for(&file, Purpose::Room));
            let a = auto_assign(&file);
            assert_eq!(format_map(&a), format_map_for(&a, Purpose::Room));
            assert_eq!(
                parse_map(&file, "auto").unwrap(),
                parse_map_for(&file, "auto", Purpose::Room).unwrap()
            );
        }
    }

    #[test]
    fn brir_automatic_assignment_follows_rew_and_dirac_rules() {
        let file = rew(&[
            ("FL", None),
            ("FL left", None),
            ("FL left", None),
            ("FL right", None),
            ("FR left", None),
        ]);
        assert_eq!(
            format_map_for(&auto_assign_for(&file, Purpose::Brir), Purpose::Brir),
            "FL.left=2,FL.right=4,FR.left=5"
        );
        let file = dirac(&[("Left", false), ("Right", false)], &[3, 7]);
        assert_eq!(
            format_map_for(&auto_assign_for(&file, Purpose::Brir), Purpose::Brir),
            "FL.left=1@3,FL.right=1@7,FR.left=2@3,FR.right=2@7"
        );
        let one = dirac(&[("Left", false)], &[4]);
        assert_eq!(
            format_map_for(&auto_assign_for(&one, Purpose::Brir), Purpose::Brir),
            "FL.left=1@4,FL.right=1@4"
        );
    }

    #[test]
    fn headphone_automatic_assignment_follows_rew_and_dirac_rules() {
        let names = rew(&[
            ("measurement left", None),
            ("FR response", None),
            ("right duplicate", None),
        ]);
        assert_eq!(
            format_map_for(
                &auto_assign_for(&names, Purpose::Headphones),
                Purpose::Headphones
            ),
            "left=1,right=2"
        );
        let fallback = rew(&[("one", None), ("two", None)]);
        assert_eq!(
            format_map_for(
                &auto_assign_for(&fallback, Purpose::Headphones),
                Purpose::Headphones
            ),
            "left=1,right=2"
        );
        assert!(
            auto_assign_for(
                &rew(&[("one", None), ("two", None), ("three", None)]),
                Purpose::Headphones
            )
            .is_empty()
        );
        let dirac = dirac(&[("Left", false), ("Right", false)], &[2, 5]);
        assert_eq!(
            format_map_for(
                &auto_assign_for(&dirac, Purpose::Headphones),
                Purpose::Headphones
            ),
            "left=1@2,right=2@5"
        );
    }

    #[test]
    fn purpose_maps_reject_invalid_forms_and_format_canonically() {
        let file = rew(&[
            ("FL left", None),
            ("FL right", None),
            ("skip", Some(Skip::NoImpulseResponse)),
        ]);
        for bad in ["FL=1", "FL.left=9", "FL.left=3"] {
            assert!(parse_map_for(&file, bad, Purpose::Brir).is_err(), "{bad}");
        }
        for bad in ["FL.left=1", "left=1,left=2", "left=9", "right=3"] {
            assert!(
                parse_map_for(&file, bad, Purpose::Headphones).is_err(),
                "{bad}"
            );
        }
        let brir = parse_map_for(&file, "fl.right=2,FL.left=1", Purpose::Brir).unwrap();
        assert_eq!(format_map_for(&brir, Purpose::Brir), "FL.left=1,FL.right=2");
        let hp = parse_map_for(&file, "RIGHT=2, LEFT=1", Purpose::Headphones).unwrap();
        assert_eq!(format_map_for(&hp, Purpose::Headphones), "left=1,right=2");
    }

    #[test]
    fn lfe_targets_are_brir_only_and_need_both_ears() {
        let file = rew(&[
            ("FL left", None),
            ("FL right", None),
            ("sub left", None),
            ("sub right", None),
            ("sub2 left", None),
            ("sub2 right", None),
        ]);
        let assignments = parse_map_for(
            &file,
            "LFE2.right=6,FL.right=2,LFE.left=3,FL.left=1,LFE2.left=5,LFE.right=4",
            Purpose::Brir,
        )
        .unwrap();
        assert_eq!(
            format_map_for(&assignments, Purpose::Brir),
            "FL.left=1,FL.right=2,LFE.left=3,LFE.right=4,LFE2.left=5,LFE2.right=6"
        );
        check_assignments(&assignments, Purpose::Brir).unwrap();
        assert_eq!(
            parse_map_for(&file, "LFE.left=3", Purpose::Room)
                .unwrap_err()
                .to_string(),
            "speaker slot LFE is not valid for room measurements"
        );
        assert_eq!(
            parse_map_for(&file, "LFE2.right=6", Purpose::Headphones)
                .unwrap_err()
                .to_string(),
            "speaker slot LFE2 is not valid for headphones measurements"
        );
        assert_eq!(
            parse_map_for(&file, "LFE=3", Purpose::Brir)
                .unwrap_err()
                .to_string(),
            "a BRIR map entry names one ear: use SLOT.left=ID or SLOT.right=ID"
        );
        let one_ear =
            parse_map_for(&file, "FL.left=1,FL.right=2,LFE.left=3", Purpose::Brir).unwrap();
        assert_eq!(
            check_assignments(&one_ear, Purpose::Brir)
                .unwrap_err()
                .to_string(),
            "speaker slot LFE has no right ear measurement"
        );
        let lfe_only = parse_map_for(&file, "LFE.left=3,LFE.right=4", Purpose::Brir).unwrap();
        assert_eq!(
            check_assignments(&lfe_only, Purpose::Brir)
                .unwrap_err()
                .to_string(),
            "the BRIR measurements must include FL, the reference for alignment"
        );
    }

    #[test]
    fn brir_automatic_assignment_puts_subwoofers_on_lfe() {
        let file = dirac(
            &[
                ("Left", false),
                ("Sub A", true),
                ("Sub B", true),
                ("Sub C", true),
            ],
            &[2, 5],
        );
        assert_eq!(
            format_map_for(&auto_assign_for(&file, Purpose::Brir), Purpose::Brir),
            "FL.left=1@2,FL.right=1@5,LFE.left=2@2,LFE.right=2@5,LFE2.left=3@2,LFE2.right=3@5"
        );

        let file = rew(&[
            ("FL left", None),
            ("FL right", None),
            ("subwoofer left", None),
            ("subwoofer right", None),
            ("LFE2 left", None),
            ("LFE2 right", None),
        ]);
        assert_eq!(
            format_map_for(&auto_assign_for(&file, Purpose::Brir), Purpose::Brir),
            "FL.left=1,FL.right=2,LFE.left=3,LFE.right=4,LFE2.left=5,LFE2.right=6"
        );
    }

    #[test]
    fn assignment_checks_return_contract_messages() {
        let a = |slot, side, id: &str| Assignment {
            slot,
            side,
            id: id.into(),
        };
        assert_eq!(
            check_assignments(&[], Purpose::Brir)
                .unwrap_err()
                .to_string(),
            "no BRIR measurements assigned"
        );
        assert_eq!(
            check_assignments(&[a("FL", EarSide::Left, "1")], Purpose::Brir)
                .unwrap_err()
                .to_string(),
            "speaker slot FL has no right ear measurement"
        );
        assert_eq!(
            check_assignments(
                &[a("FL", EarSide::Left, "3"), a("FL", EarSide::Right, "3")],
                Purpose::Brir
            )
            .unwrap_err()
            .to_string(),
            "measurement 3 is assigned more than once"
        );
        assert_eq!(
            check_assignments(
                &[a("FR", EarSide::Left, "1"), a("FR", EarSide::Right, "2")],
                Purpose::Brir
            )
            .unwrap_err()
            .to_string(),
            "the BRIR measurements must include FL, the reference for alignment"
        );
        assert_eq!(
            check_assignments(&[a("FL", EarSide::Left, "1")], Purpose::Headphones)
                .unwrap_err()
                .to_string(),
            "headphone measurement needs a left and a right ear; the right ear is missing"
        );
        assert_eq!(
            check_assignments(
                &[a("FL", EarSide::Left, "3"), a("FR", EarSide::Right, "3")],
                Purpose::Headphones
            )
            .unwrap_err()
            .to_string(),
            "measurement 3 is assigned more than once"
        );
    }
}
