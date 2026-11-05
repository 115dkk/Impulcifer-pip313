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
    let cases: &[(&[&str], &[(&str, Option<&str>)])] = &[
        (
            &["FL", "FR", "FC", "BL", "BR", "SL", "SR"],
            &[("WL", Some("FL")), ("WR", Some("FR"))],
        ),
        (
            &["FL", "FR", "BL", "BR"],
            &[
                ("SL", Some("BL")),
                ("SR", Some("BR")),
                ("FC", Some("FL+FR")),
            ],
        ),
        (&["FL", "FR", "SL", "SR"], &[("BL", Some("SL"))]),
        (
            &["TFL", "TFR"],
            &[("TBL", Some("TFL")), ("TSL", Some("TFL"))],
        ),
        (&["FL", "FR"], &[("TFL", None)]),
        (&["BL", "BR"], &[("BC", Some("BL+BR"))]),
    ];
    for (measured, expected) in cases {
        let input = tagged(measured);
        let targets: Vec<_> = expected.iter().map(|(slot, _)| *slot).collect();
        let result = fill_missing_speakers(&input, &targets).unwrap();
        for (slot, expected_source) in *expected {
            assert_eq!(source(&result.filled, slot), *expected_source, "{measured:?} -> {slot}");
        }
    }

    let input = tagged(&["FL", "FR", "BL", "BR"]);
    let result = fill_missing_speakers(&input, &["FC"]).unwrap();
    let fc = result.hrir.pair("FC").unwrap();
    let fl = input.pair("FL").unwrap();
    let fr = input.pair("FR").unwrap();
    assert_eq!(fc.0.data[0], (fl.0.data[0] + fr.0.data[0]) / std::f64::consts::SQRT_2);
    assert_eq!(fc.1.data[0], (fl.1.data[0] + fr.1.data[0]) / std::f64::consts::SQRT_2);
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
    assert!(fill_lfe(&tagged(&["FL"]), "LFE", "center").unwrap().is_none());
}

#[test]
fn lfe_all_is_the_power_average_below_the_crossover() {
    let fs = 48_000;
    let length = 4_096;
    let tone = |frequency: f64, gain: f64, peak: usize| {
        (0..length)
            .map(|n| {
                let shifted = n.saturating_sub(peak) as f64;
                if n < peak { 0.0 } else { gain * (std::f64::consts::TAU * frequency * shifted / fs as f64).sin() }
            })
            .collect::<Vec<_>>()
    };
    let input = hrir(
        &[
            ("FL", tone(40.0, 1.0, 9), tone(40.0, 0.8, 11)),
            ("FR", tone(40.0, 0.5, 13), tone(40.0, 0.4, 15)),
        ],
        fs,
    );
    let first = fill_lfe(&input, "LFE", "all").unwrap().unwrap().0;
    let second = fill_lfe(&input, "LFE", "all").unwrap().unwrap().0;
    assert_eq!(first.left.as_ref().unwrap().data, second.left.as_ref().unwrap().data);
    assert_eq!(first.right.as_ref().unwrap().data, second.right.as_ref().unwrap().data);
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
    assert!(attenuation < -60.0, "1 kHz attenuation was {attenuation} dB");
}

#[test]
fn defaults_leave_every_output_unchanged() {
    let input = tagged(&["FL", "FR", "FC", "BL", "BR", "SL", "SR"]);
    let targets = ["FL", "FR", "FC", "BL", "BR", "SL", "SR"];
    let defaults = fill_output(&input, &targets, "silent", "none").unwrap();
    let enabled = fill_output(&input, &targets, "neighbor", "all").unwrap();
    let names: Vec<_> = targets
        .iter()
        .flat_map(|speaker| [format!("{speaker}-left"), format!("{speaker}-right")])
        .collect();
    let names: Vec<_> = names.iter().map(String::as_str).collect();
    assert_eq!(defaults.hrir.stack_tracks(&names, false).unwrap(), enabled.hrir.stack_tracks(&names, false).unwrap());
    assert!(enabled.filled.is_empty());
}
