#![forbid(unsafe_code)]
use impulcifer_dsp::{
    channel_balance::ChannelBalance,
    hrir::{Hrir, SpeakerIrs},
    ir::ImpulseResponse,
    mic_deviation::{Anchor, MicDeviationOptions, MicMatching, apply_mic_deviation_correction},
    pipeline::total_steps,
    stages::{readme::readme_data, room::correction_limit_mask},
    virtual_bass::{VirtualBassOptions, apply_virtual_bass},
};
use impulcifer_types::config::ProcessingConfig;
/// Python default stage table; core/pipeline.py:424-470; p10_stage_table.
#[test]
fn stage_table_total_is_eleven_by_default() {
    assert_eq!(total_steps(&ProcessingConfig::default()), 11);
}
/// Python full Hann (not half Hann); core/room_correction.py:295-302; p10_generic_room.
#[test]
fn correction_limit_mask_is_full_hann_over_one_octave() {
    let f = [10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0];
    let mask = correction_limit_mask(&f, 80.0);
    assert_eq!(mask[..4], [1.0; 4]);
    assert!(mask[4].abs() < 1e-15);
    assert!((mask[5] - 0.75).abs() < 1e-15);
    assert!((mask[6] - 0.75).abs() < 1e-15);
    assert!(mask[7].abs() < 1e-15);
    assert_eq!(mask[8], 0.0);
}
/// Python synthetic identical ear pair; core/hrir.py:368-381; p10_mic_deviation.
fn hrir() -> Hrir {
    let mut data = vec![0.0; 8192];
    data[48] = 1.0;
    let ir = ImpulseResponse {
        data,
        fs: 48000,
        recording: None,
    };
    Hrir {
        fs: 48000,
        speakers: vec![SpeakerIrs {
            speaker: "FL".into(),
            left: Some(ir.clone()),
            right: Some(ir),
        }],
    }
}
/// Python Nyquist early return, unchanged data; core/virtual_bass.py:93-95; p10_vbass.
#[test]
fn virtual_bass_guards_reject_crossover_at_nyquist() {
    let mut h = hrir();
    let before = h.speakers[0].left.as_ref().unwrap().data.clone();
    apply_virtual_bass(
        &mut h,
        &VirtualBassOptions {
            crossover_freq: 24000,
            head_ms: 1.0,
            hp_freq: 15.0,
            invert_polarity: None,
        },
    )
    .unwrap();
    assert_eq!(h.speakers[0].left.as_ref().unwrap().data, before);
}
/// Python <0.05 dB skip; core/microphone_deviation_correction.py:433-436; p10_mic_deviation.
#[test]
fn mic_deviation_skips_below_threshold() {
    let mut h = hrir();
    let before = h.speakers[0].left.as_ref().unwrap().data.clone();
    let summary = apply_mic_deviation_correction(&mut h, &MicDeviationOptions::default()).unwrap();
    assert!(summary.max_error_db < 0.05);
    assert!(summary.speakers_processed.is_empty());
    assert_eq!(h.speakers[0].left.as_ref().unwrap().data, before);
}
/// Python averages powers, not dB, from every selected anchor; microphone_deviation_correction.py:195-238; p10_mic_deviation.
fn check_anchor(
    anchor: Anchor,
    powers: &[(&str, f64, f64)],
    selected: &[usize],
    expected_anchor: &str,
) {
    let mut matching = MicMatching::new(
        48000,
        &MicDeviationOptions {
            anchor,
            ..Default::default()
        },
    );
    let n = matching.frequency.len();
    matching.speaker_power = powers
        .iter()
        .map(|(s, l, r)| (s.to_string(), [vec![*l; n], vec![*r; n]]))
        .collect();
    let left = selected.iter().map(|i| powers[*i].1).sum::<f64>() / selected.len() as f64;
    let right = selected.iter().map(|i| powers[*i].2).sum::<f64>() / selected.len() as f64;
    let delta = 10.0 * ((left + 1e-20) / (right + 1e-20)).log10();
    let expected: Vec<_> = matching
        .band_weight()
        .iter()
        .map(|w| (delta * w).clamp(-12.0, 12.0))
        .collect();
    let got = matching.estimate_interaural_mismatch().unwrap();
    for (a, b) in got.iter().zip(expected) {
        assert!((a - b).abs() < 1e-9);
    }
    assert_eq!(matching.anchor_used, expected_anchor);
}
/// Python _CENTER_SPEAKERS includes all three recordings; microphone_deviation_correction.py:43,205-212; p10_mic_deviation.
#[test]
fn mic_multiple_centers_average_power() {
    let powers = [
        ("FL", 100.0, 1.0),
        ("BC", 9.0, 4.0),
        ("FC", 1.0, 16.0),
        ("TFC", 4.0, 1.0),
    ];
    for anchor in [Anchor::Auto, Anchor::Frontal] {
        check_anchor(anchor, &powers, &[1, 2, 3], "frontal");
    }
    check_anchor(Anchor::Diffuse, &powers, &[0, 1, 2, 3], "diffuse");
}
/// Python TFC/BC without FC still select frontal; microphone_deviation_correction.py:205-213; p10_mic_deviation.
#[test]
fn mic_tfc_bc_only_select_frontal() {
    for anchor in [Anchor::Auto, Anchor::Frontal] {
        for center in ["TFC", "BC"] {
            check_anchor(
                anchor,
                &[("FR", 100.0, 1.0), (center, 4.0, 1.0)],
                &[1],
                "frontal",
            );
        }
        check_anchor(
            anchor,
            &[("TFC", 4.0, 1.0), ("BC", 1.0, 9.0)],
            &[0, 1],
            "frontal",
        );
    }
}
/// Python frontal falls back to all recordings without centers; microphone_deviation_correction.py:214-218; p10_mic_deviation.
#[test]
fn mic_absent_center_falls_back_to_diffuse() {
    for anchor in [Anchor::Auto, Anchor::Frontal, Anchor::Diffuse] {
        check_anchor(
            anchor,
            &[("FL", 4.0, 1.0), ("FR", 1.0, 9.0), ("LFE", 2.0, 3.0)],
            &[0, 1, 2],
            "diffuse",
        );
    }
}
/// Python numeric fallback; core/hrir.py:772-781; p10_channel_balance.
#[test]
fn channel_balance_parse_accepts_numbers_and_rejects_words() {
    assert_eq!(
        ChannelBalance::parse("3").unwrap(),
        ChannelBalance::GainDb(3.0)
    );
    assert_eq!(
        ChannelBalance::parse("-1.5").unwrap(),
        ChannelBalance::GainDb(-1.5)
    );
    assert!(ChannelBalance::parse("banana").is_err());
}
/// Python SPEAKER_NAMES stable ordering; core/pipeline_stages.py:548-553; p10_readme.
#[test]
fn readme_rows_sorted_by_speaker_names() {
    let mut h = hrir();
    let mut b = h.speakers[0].clone();
    b.speaker = "BL".into();
    h.speakers.insert(0, b);
    let rows = readme_data(&h, 48000, 0.0).rows;
    assert_eq!(
        rows.iter().map(|r| r.speaker.as_str()).collect::<Vec<_>>(),
        ["FL", "FL", "BL", "BL"]
    );
}
