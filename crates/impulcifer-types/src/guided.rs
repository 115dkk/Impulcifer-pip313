//! Head-turn measurement plans. Angles follow the ITU convention (+left).
use crate::layouts::SLOTS;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GuidedSource {
    pub speaker: &'static str,
    pub azimuth: f64,
    pub output: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GuidedTake {
    pub speakers: Vec<&'static str>,
    pub tracks: &'static str,
    pub yaw: f64,
    pub sources: Vec<GuidedSource>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GuidedSetup {
    pub id: &'static str,
    pub outputs: &'static [f64],
    pub targets: &'static [&'static str],
}

pub const GUIDED_SETUPS: [GuidedSetup; 2] = [
    GuidedSetup {
        id: "mono",
        outputs: &[0.0],
        targets: &["stereo", "5.1", "7.1"],
    },
    GuidedSetup {
        id: "stereo",
        outputs: &[30.0, -30.0],
        targets: &["5.1", "7.1"],
    },
];

fn take(setup: &GuidedSetup, speakers: &[&'static str], yaw: f64) -> GuidedTake {
    GuidedTake {
        speakers: speakers.to_vec(),
        tracks: "stereo",
        yaw,
        sources: speakers
            .iter()
            .enumerate()
            .map(|(output, &speaker)| {
                let wrapped = (setup.outputs[output] - yaw).rem_euclid(360.0);
                GuidedSource {
                    speaker,
                    azimuth: if wrapped > 180.0 {
                        wrapped - 360.0
                    } else {
                        wrapped
                    },
                    output,
                }
            })
            .collect(),
    }
}

pub fn guided_plan(setup: &str, target: &str) -> Option<Vec<GuidedTake>> {
    let setup = GUIDED_SETUPS.iter().find(|s| s.id == setup)?;
    if !setup.targets.contains(&target) {
        return None;
    }
    if setup.id == "mono" {
        let speakers: &[&str] = match target {
            "stereo" => &["FL", "FR"],
            "5.1" => &["FC", "FL", "FR", "BL", "BR"],
            _ => &["FC", "FL", "FR", "SL", "SR", "BL", "BR"],
        };
        Some(
            speakers
                .iter()
                .map(|speaker| {
                    let slot = SLOTS
                        .iter()
                        .find(|s| s.code == *speaker)
                        .expect("guided slot");
                    take(setup, &[slot.code], -slot.azimuth)
                })
                .collect(),
        )
    } else {
        let mut plan = vec![
            take(setup, &["FL", "FR"], 0.0),
            take(setup, &["BR", "BL"], 180.0),
        ];
        if target == "7.1" {
            plan.push(take(setup, &["SL"], -60.0));
            plan.push(take(setup, &["SR"], 120.0));
        }
        plan.push(take(setup, &["FC"], 30.0));
        Some(plan)
    }
}
