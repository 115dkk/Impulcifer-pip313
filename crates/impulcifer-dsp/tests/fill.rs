#![forbid(unsafe_code)]
use impulcifer_dsp::{
    fft,
    hrir::{Hrir, SpeakerIrs},
    ir::ImpulseResponse,
    stages::fill::{fill_lfe, fill_missing_speakers, fill_output},
};

fn ir(data: Vec<f64>, fs: u32) -> ImpulseResponse {
    ImpulseResponse {
        data,
        fs,
        recording: None,
    }
}
fn hrir(speakers: &[(&str, Vec<f64>, Vec<f64>)], fs: u32) -> Hrir {
    Hrir {
        fs,
        speakers: speakers
            .iter()
            .map(|(speaker, left, right)| SpeakerIrs {
                speaker: (*speaker).into(),
                left: Some(ir(left.clone(), fs)),
                right: Some(ir(right.clone(), fs)),
            })
            .collect(),
    }
}
fn tagged(names: &[&str]) -> Hrir {
    hrir(
        &names
            .iter()
            .enumerate()
            .map(|(index, name)| (*name, vec![index as f64 + 1.0], vec![index as f64 + 1.5]))
            .collect::<Vec<_>>(),
        48_000,
    )
}
fn source<'a>(filled: &'a [(&str, String)], slot: &str) -> Option<&'a str> {
    filled
        .iter()
        .find(|(candidate, _)| *candidate == slot)
        .map(|(_, source)| source.as_str())
}

#[test]
fn neighbor_fill_picks_the_nearest_slot_on_the_same_side_and_layer() {
    struct Case<'a> {
        measured: &'a [&'a str],
        expected: &'a [(&'a str, Option<&'a str>)],
    }
    let cases = [
        Case {
            measured: &["FL", "FR", "FC", "BL", "BR", "SL", "SR"],
            expected: &[("WL", Some("FL")), ("WR", Some("FR"))],
        },
        Case {
            measured: &["FL", "FR", "BL", "BR"],
            expected: &[
                ("SL", Some("BL")),
                ("SR", Some("BR")),
                ("FC", Some("FL+FR")),
            ],
        },
        Case {
            measured: &["FL", "FR", "SL", "SR"],
            expected: &[("BL", Some("SL"))],
        },
        Case {
            measured: &["TFL", "TFR"],
            expected: &[("TBL", Some("TFL")), ("TSL", Some("TFL"))],
        },
        Case {
            measured: &["FL", "FR"],
            expected: &[("TFL", None)],
        },
        Case {
            measured: &["BL", "BR"],
            expected: &[("BC", Some("BL+BR"))],
        },
    ];
    for Case { measured, expected } in cases {
        let input = tagged(measured);
        let targets: Vec<_> = expected.iter().map(|(slot, _)| *slot).collect();
        let result = fill_missing_speakers(&input, &targets).unwrap();
        for (slot, expected_source) in expected {
            assert_eq!(
                source(&result.filled, slot),
                *expected_source,
                "{measured:?} -> {slot}"
            );
        }
    }

    let input = tagged(&["FL", "FR", "BL", "BR"]);
    let result = fill_missing_speakers(&input, &["FC"]).unwrap();
    let fc = result.hrir.pair("FC").unwrap();
    let fl = input.pair("FL").unwrap();
    let fr = input.pair("FR").unwrap();
    assert_eq!(
        fc.0.data[0],
        (fl.0.data[0] + fr.0.data[0]) / std::f64::consts::SQRT_2
    );
    assert_eq!(
        fc.1.data[0],
        (fl.1.data[0] + fr.1.data[0]) / std::f64::consts::SQRT_2
    );
}

#[test]
fn fill_never_uses_a_filled_slot_as_source() {
    let input = tagged(&["FL"]);
    let result = fill_missing_speakers(&input, &["SL", "BL"]).unwrap();
    assert_eq!(source(&result.filled, "SL"), Some("FL"));
    assert_eq!(source(&result.filled, "BL"), Some("FL"));
    assert_eq!(input.speakers.len(), 1);
}

#[test]
fn lfe_center_copies_fc_or_the_front_phantom() {
    let with_fc = tagged(&["FL", "FR", "FC"]);
    let (lfe, source) = fill_lfe(&with_fc, "LFE", "center").unwrap().unwrap();
    assert_eq!(source, "FC");
    assert_eq!(lfe.left.unwrap().data, with_fc.pair("FC").unwrap().0.data);

    let front = tagged(&["FL", "FR"]);
    let (lfe, source) = fill_lfe(&front, "LFE", "center").unwrap().unwrap();
    assert_eq!(source, "FL+FR");
    let pair = (front.pair("FL").unwrap(), front.pair("FR").unwrap());
    assert_eq!(
        lfe.left.unwrap().data[0],
        (pair.0.0.data[0] + pair.1.0.data[0]) / std::f64::consts::SQRT_2
    );
    assert!(
        fill_lfe(&tagged(&["FL"]), "LFE", "center")
            .unwrap()
            .is_none()
    );
}

#[test]
fn lfe_all_is_the_power_average_below_the_crossover() {
    let fs = 48_000;
    let length: usize = 4_096;
    let impulse = |gain: f64, peak: usize| {
        let mut data = vec![0.0; length];
        data[peak] = gain;
        data
    };
    let input = hrir(
        &[
            ("FL", impulse(1.0, 9), impulse(0.8, 11)),
            ("FR", impulse(0.5, 13), impulse(0.4, 15)),
        ],
        fs,
    );
    let first = fill_lfe(&input, "LFE", "all").unwrap().unwrap().0;
    let second = fill_lfe(&input, "LFE", "all").unwrap().unwrap().0;
    assert_eq!(
        first.left.as_ref().unwrap().data,
        second.left.as_ref().unwrap().data
    );
    assert_eq!(
        first.right.as_ref().unwrap().data,
        second.right.as_ref().unwrap().data
    );
    let data = &first.left.as_ref().unwrap().data;
    let peak = data
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .unwrap()
        .0;
    assert!(peak >= 11, "peak {peak} precedes common lower-median delay");

    let nfft = 16_384;
    let mut padded = vec![0.0; nfft];
    padded[..data.len()].copy_from_slice(data);
    let spectrum = fft::rfft(&padded);
    let bin = |frequency: f64| (frequency * nfft as f64 / fs as f64).round() as usize;
    let actual_40 = spectrum[bin(40.0)].norm();
    let expected_40 = {
        let magnitudes: Vec<_> = input
            .speakers
            .iter()
            .map(|speaker| {
                let mut padded = vec![0.0; nfft];
                padded[..length].copy_from_slice(&speaker.left.as_ref().unwrap().data);
                fft::rfft(&padded)[bin(40.0)].norm()
            })
            .collect();
        (magnitudes.iter().map(|value| value * value).sum::<f64>() / magnitudes.len() as f64).sqrt()
            / (1.0 + (40.0_f64 / 120.0).powi(4))
    };
    assert!((20.0 * (actual_40 / expected_40).log10()).abs() < 0.5);
    let attenuation = 20.0 * (spectrum[bin(1_000.0)].norm() / actual_40).log10();
    assert!(
        attenuation < -60.0,
        "1 kHz attenuation was {attenuation} dB"
    );
}

#[test]
fn measured_lfe_precedes_lfe_mode_and_fills_the_other_track() {
    let speakers = tagged(&["FL", "FR", "FC"]);
    let one = tagged(&["LFE"]).speakers;
    for mode in ["center", "all"] {
        let result = fill_output(
            &speakers,
            &one,
            &[("LFE".into(), "room".into())],
            &["LFE", "LFE2"],
            "silent",
            mode,
        )
        .unwrap();
        for target in ["LFE", "LFE2"] {
            assert_eq!(source(&result.filled, target), Some("room"));
            assert_eq!(
                result.hrir.pair(target).unwrap().0.data,
                one[0].left.as_ref().unwrap().data,
                "{target} with {mode}"
            );
        }
    }

    let two = tagged(&["LFE", "LFE2"]).speakers;
    let result = fill_output(
        &speakers,
        &two,
        &[
            ("LFE".into(), "measured_corrected".into()),
            ("LFE2".into(), "room".into()),
        ],
        &["LFE", "LFE2"],
        "silent",
        "all",
    )
    .unwrap();
    for (index, (target, label)) in [("LFE", "measured_corrected"), ("LFE2", "room")]
        .into_iter()
        .enumerate()
    {
        assert_eq!(source(&result.filled, target), Some(label));
        assert_eq!(
            result.hrir.pair(target).unwrap().0.data,
            two[index].left.as_ref().unwrap().data
        );
    }
}

#[test]
fn one_lfe_channel_sums_both_measured_subwoofers() {
    let speakers = tagged(&["FL", "FR", "FC"]);
    let two = hrir(
        &[
            ("LFE", vec![1.0, 2.0, 0.5], vec![-1.0, 0.25, 0.0]),
            ("LFE2", vec![0.5, -3.0, 0.25], vec![4.0, 0.5, -2.0]),
        ],
        48_000,
    )
    .speakers;
    let labels = [
        ("LFE".to_owned(), "measured_corrected".to_owned()),
        ("LFE2".to_owned(), "room".to_owned()),
    ];
    // A format with one LFE channel: the track is the two subwoofers together.
    let result = fill_output(&speakers, &two, &labels, &["FL", "LFE"], "silent", "all").unwrap();
    assert_eq!(
        source(&result.filled, "LFE"),
        Some("sum:measured_corrected+room")
    );
    let (left, right) = result.hrir.pair("LFE").unwrap();
    assert_eq!(left.data, vec![1.5, -1.0, 0.75]);
    assert_eq!(right.data, vec![3.0, 0.75, -2.0]);
    assert!(result.hrir.pair("LFE2").is_none());

    // A format with two LFE channels keeps them apart.
    let apart = fill_output(&speakers, &two, &labels, &["LFE", "LFE2"], "silent", "all").unwrap();
    assert_eq!(source(&apart.filled, "LFE"), Some("measured_corrected"));
    assert_eq!(source(&apart.filled, "LFE2"), Some("room"));
    assert_eq!(apart.hrir.pair("LFE").unwrap().0.data, vec![1.0, 2.0, 0.5]);

    // One subwoofer alone is used as it is.
    let alone = fill_output(
        &speakers,
        &two[1..],
        &labels[1..],
        &["FL", "LFE"],
        "silent",
        "none",
    )
    .unwrap();
    assert_eq!(source(&alone.filled, "LFE"), Some("room"));
    assert_eq!(
        alone.hrir.pair("LFE").unwrap().0.data,
        vec![0.5, -3.0, 0.25]
    );

    // Tracks of different lengths cannot be summed.
    let mut uneven = two.clone();
    uneven[1].left.as_mut().unwrap().data.push(0.0);
    assert!(fill_output(&speakers, &uneven, &labels, &["LFE"], "silent", "none").is_err());
}

#[test]
fn defaults_leave_every_output_unchanged() {
    let input = tagged(&["FL", "FR", "FC", "BL", "BR", "SL", "SR"]);
    let targets = ["FL", "FR", "FC", "BL", "BR", "SL", "SR"];
    let defaults = fill_output(&input, &[], &[], &targets, "silent", "none").unwrap();
    let enabled = fill_output(&input, &[], &[], &targets, "neighbor", "all").unwrap();
    let names: Vec<_> = targets
        .iter()
        .flat_map(|speaker| [format!("{speaker}-left"), format!("{speaker}-right")])
        .collect();
    let names: Vec<_> = names.iter().map(String::as_str).collect();
    assert_eq!(
        defaults.hrir.stack_tracks(&names, false).unwrap(),
        enabled.hrir.stack_tracks(&names, false).unwrap()
    );
    assert!(enabled.filled.is_empty());
}
