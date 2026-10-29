#![forbid(unsafe_code)]

use impulcifer_types::{
    constants::SPEAKER_NAMES,
    guided::{GUIDED_SETUPS, guided_plan},
    layouts::SLOTS,
};

fn wrap(angle: f64) -> f64 {
    let wrapped = angle.rem_euclid(360.0);
    if wrapped > 180.0 {
        wrapped - 360.0
    } else {
        wrapped
    }
}

#[test]
fn guided_plans_match_the_measurement_table() {
    let cases = [
        (
            "mono",
            "stereo",
            vec![(vec!["FL"], -30.0), (vec!["FR"], 30.0)],
        ),
        (
            "mono",
            "5.1",
            vec![
                (vec!["FC"], 0.0),
                (vec!["FL"], -30.0),
                (vec!["FR"], 30.0),
                (vec!["BL"], -135.0),
                (vec!["BR"], 135.0),
            ],
        ),
        (
            "mono",
            "7.1",
            vec![
                (vec!["FC"], 0.0),
                (vec!["FL"], -30.0),
                (vec!["FR"], 30.0),
                (vec!["SL"], -90.0),
                (vec!["SR"], 90.0),
                (vec!["BL"], -135.0),
                (vec!["BR"], 135.0),
            ],
        ),
        (
            "mono",
            "9.1",
            vec![
                (vec!["FC"], 0.0),
                (vec!["FL"], -30.0),
                (vec!["FR"], 30.0),
                (vec!["WL"], -60.0),
                (vec!["WR"], 60.0),
                (vec!["SL"], -90.0),
                (vec!["SR"], 90.0),
                (vec!["BL"], -135.0),
                (vec!["BR"], 135.0),
            ],
        ),
        (
            "stereo",
            "5.1",
            vec![
                (vec!["FL", "FR"], 0.0),
                (vec!["BR", "BL"], 180.0),
                (vec!["FC"], 30.0),
            ],
        ),
        (
            "stereo",
            "7.1",
            vec![
                (vec!["FL", "FR"], 0.0),
                (vec!["BR", "BL"], 180.0),
                (vec!["SL"], -60.0),
                (vec!["SR"], 120.0),
                (vec!["FC"], 30.0),
            ],
        ),
        (
            "stereo",
            "9.1",
            vec![
                (vec!["FL", "FR"], 0.0),
                (vec!["BR", "BL"], 180.0),
                (vec!["SL"], -60.0),
                (vec!["SR"], 120.0),
                (vec!["FC", "WR"], 30.0),
                (vec!["WL"], -30.0),
            ],
        ),
    ];
    for (setup, target, expected) in cases {
        let plan = guided_plan(setup, target).unwrap();
        let actual: Vec<_> = plan
            .iter()
            .map(|take| (take.speakers.clone(), take.yaw))
            .collect();
        assert_eq!(actual, expected, "{setup} {target}");
        assert!(plan.iter().all(|take| take.tracks == "stereo"));
    }
    assert!(guided_plan("mono", "22.2").is_none());
    assert!(guided_plan("unknown", "5.1").is_none());
}

#[test]
fn guided_sources_follow_output_minus_yaw_and_valid_stereo_rules() {
    for setup in &GUIDED_SETUPS {
        for target in setup.targets {
            for take in guided_plan(setup.id, target).unwrap() {
                assert!((1..=2).contains(&take.speakers.len()));
                assert_eq!(take.sources.len(), take.speakers.len());
                assert!(
                    take.speakers
                        .iter()
                        .all(|speaker| SPEAKER_NAMES.contains(speaker))
                );
                if take.speakers.len() == 2 {
                    assert_ne!(take.speakers[0], take.speakers[1]);
                }
                for (output, source) in take.sources.iter().enumerate() {
                    assert_eq!(source.speaker, take.speakers[output]);
                    assert_eq!(source.output, output);
                    assert_eq!(
                        source.azimuth,
                        wrap(setup.outputs[output] - take.yaw),
                        "{} {target} {}",
                        setup.id,
                        source.speaker
                    );
                }
            }
        }
    }
}

#[test]
fn mono_guided_sources_use_slot_azimuths() {
    for target in GUIDED_SETUPS[0].targets {
        for take in guided_plan("mono", target).unwrap() {
            let source = &take.sources[0];
            let slot = SLOTS
                .iter()
                .find(|slot| slot.code == source.speaker)
                .unwrap();
            assert_eq!(source.azimuth, slot.azimuth, "{} {target}", source.speaker);
        }
    }
}
