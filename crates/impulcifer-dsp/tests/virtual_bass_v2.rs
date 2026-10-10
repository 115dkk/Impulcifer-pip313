#![forbid(unsafe_code)]

use impulcifer_dsp::{
    filters::{self, BType, Sos},
    fr::{FrequencyResponse, magnitude_to_frequency_response},
    hrir::{Hrir, SpeakerIrs},
    ir::ImpulseResponse,
    stages::room::{RoomFrs, RoomTerm},
    virtual_bass::{
        VirtualBassMode, VirtualBassOptions, VirtualBassPlan, VirtualBassV2Options,
        apply_virtual_bass, apply_virtual_bass_v2, auto_crossover, detect_speaker_rolloffs,
    },
};
use impulcifer_types::constants::Side;

const FS: u32 = 48_000;
const N: usize = 24_000;

fn delayed(data: &[f64], delay: usize) -> Vec<f64> {
    let mut out = vec![0.0; N];
    let count = data.len().min(N.saturating_sub(delay));
    out[delay..delay + count].copy_from_slice(&data[..count]);
    out
}

fn impulse() -> Vec<f64> {
    let mut data = vec![0.0; N];
    data[48] = 1.0;
    data
}

fn filtered_impulse(sos: &Sos) -> Vec<f64> {
    filters::sosfilt(sos, &impulse())
}

fn ir(data: Vec<f64>) -> ImpulseResponse {
    ImpulseResponse {
        data,
        fs: FS,
        recording: None,
    }
}

fn hrir(speakers: &[(&str, Vec<f64>)]) -> Hrir {
    Hrir {
        fs: FS,
        speakers: speakers
            .iter()
            .map(|(name, data)| SpeakerIrs {
                speaker: (*name).into(),
                left: Some(ir(data.clone())),
                right: Some(ir(data.clone())),
            })
            .collect(),
    }
}

fn options<'a>(
    room_gains: Option<&'a RoomFrs>,
    target: Option<&'a FrequencyResponse>,
) -> VirtualBassV2Options<'a> {
    VirtualBassV2Options {
        crossover: 250.0,
        head_ms: 1.0,
        hp_freq: 15.0,
        invert_polarity: false,
        room_gains,
        room_target: target,
    }
}

fn response_at(data: &[f64], frequency: f64) -> f64 {
    let fr = magnitude_to_frequency_response("test response", FS, data).unwrap();
    let upper = fr.frequency.partition_point(|f| *f < frequency);
    let index = upper.min(fr.frequency.len() - 1);
    fr.raw[index]
}

fn peaking(data: &[f64], gain_db: f64) -> Vec<f64> {
    let biquad = filters::rbj_peaking(280.0, 4.0, gain_db, FS as f64);
    filters::sosfilt(
        &Sos(vec![[
            biquad.b[0],
            biquad.b[1],
            biquad.b[2],
            biquad.a[0],
            biquad.a[1],
            biquad.a[2],
        ]]),
        data,
    )
}

#[test]
fn vbass_v2_auto_crossover_follows_the_highest_rolloff() {
    let hp40 = filters::butter(4, 40.0 / (FS as f64 / 2.0), BType::Highpass);
    let hp80 = filters::butter(4, 80.0 / (FS as f64 / 2.0), BType::Highpass);
    let measured = hrir(&[
        ("FL", filtered_impulse(&hp40)),
        ("FR", filtered_impulse(&hp80)),
    ]);
    let rolloffs = detect_speaker_rolloffs(&measured).unwrap();
    assert_eq!(rolloffs.len(), 2, "{rolloffs:?}");
    let fl = rolloffs.iter().find(|r| r.speaker == "FL").unwrap().freq;
    let fr = rolloffs.iter().find(|r| r.speaker == "FR").unwrap().freq;
    assert!((30.0..=50.0).contains(&fl), "FL rolloff {fl}");
    assert!((60.0..=100.0).contains(&fr), "FR rolloff {fr}");
    assert_eq!(auto_crossover(&rolloffs), Some((2.0 * fr).round()));
    let plan = VirtualBassPlan::resolve(VirtualBassMode::Auto, 250.0, &measured).unwrap();
    assert_eq!(plan.crossover, auto_crossover(&rolloffs));
    assert_eq!(plan.limiting_rolloff().unwrap().speaker, "FR");

    let full_range = hrir(&[("FL", impulse()), ("FR", impulse())]);
    let rolloffs = detect_speaker_rolloffs(&full_range).unwrap();
    assert!(rolloffs.is_empty(), "{rolloffs:?}");
    assert_eq!(auto_crossover(&rolloffs), None);
    assert_eq!(
        VirtualBassPlan::resolve(VirtualBassMode::Auto, 250.0, &full_range)
            .unwrap()
            .crossover,
        None
    );
}

#[test]
fn vbass_v2_level_ignores_a_mode_above_the_crossover() {
    let flat = impulse();
    let mode = peaking(&flat, 10.0);
    let mut flat_v2 = hrir(&[("FL", flat.clone()), ("FR", flat.clone())]);
    let mut mode_v2 = hrir(&[("FL", mode.clone()), ("FR", mode.clone())]);
    let flat_report = apply_virtual_bass_v2(&mut flat_v2, &options(None, None))
        .unwrap()
        .unwrap();
    let mode_report = apply_virtual_bass_v2(&mut mode_v2, &options(None, None))
        .unwrap()
        .unwrap();
    let v2_shift = (mode_report.level_db - flat_report.level_db).abs();
    assert!(v2_shift <= 2.0, "v2 level shift {v2_shift} dB");

    let legacy = VirtualBassOptions {
        crossover_freq: 250,
        head_ms: 1.0,
        hp_freq: 15.0,
        invert_polarity: Some(false),
    };
    let mut flat_legacy = hrir(&[("FL", flat.clone()), ("FR", flat)]);
    let mut mode_legacy = hrir(&[("FL", mode.clone()), ("FR", mode)]);
    apply_virtual_bass(&mut flat_legacy, &legacy).unwrap();
    apply_virtual_bass(&mut mode_legacy, &legacy).unwrap();
    let flat_level = response_at(&flat_legacy.speakers[0].left.as_ref().unwrap().data, 80.0);
    let mode_level = response_at(&mode_legacy.speakers[0].left.as_ref().unwrap().data, 80.0);
    let legacy_shift = (mode_level - flat_level).abs();
    assert!(
        v2_shift < legacy_shift,
        "v2 {v2_shift} dB, legacy {legacy_shift} dB"
    );
}

#[test]
fn vbass_v2_level_uses_the_room_gain() {
    let flat = impulse();
    let mode = peaking(&flat, 10.0);
    let mut baseline = hrir(&[("FL", flat.clone()), ("FR", flat)]);
    let baseline_level = apply_virtual_bass_v2(&mut baseline, &options(None, None))
        .unwrap()
        .unwrap()
        .level_db;

    let measured = magnitude_to_frequency_response("mode", FS, &mode).unwrap();
    let mode_response = filters::biquad_response_db(
        &filters::rbj_peaking(280.0, 4.0, 10.0, FS as f64),
        &measured.frequency,
        FS as f64,
    );
    let mut correction = FrequencyResponse::new(
        "room gain",
        Some(measured.frequency.clone()),
        Some(vec![0.0; measured.frequency.len()]),
    )
    .unwrap();
    correction.equalization = mode_response.into_iter().map(|value| -value).collect();
    let gains = RoomFrs {
        term: RoomTerm::Gain,
        entries: vec![
            ("FL".into(), Side::Left, correction.clone()),
            ("FR".into(), Side::Right, correction),
        ],
    };
    let mut corrected = hrir(&[("FL", mode.clone()), ("FR", mode)]);
    let corrected_level = apply_virtual_bass_v2(&mut corrected, &options(Some(&gains), None))
        .unwrap()
        .unwrap()
        .level_db;
    assert!(
        (corrected_level - baseline_level).abs() <= 0.3,
        "baseline {baseline_level} dB, corrected {corrected_level} dB"
    );
}

#[test]
fn vbass_v2_alignment_finds_the_speaker_low_end_delay() {
    let xo = 250.0;
    let measured = delayed(&[1.0], 48 + 96);
    let mut measured = hrir(&[("FL", measured.clone()), ("FR", measured)]);
    let report = apply_virtual_bass_v2(&mut measured, &options(None, None))
        .unwrap()
        .unwrap();
    let tolerance = (FS as f64 / xo / 24.0).ceil() as usize;
    assert!(
        report.delay_samples.abs_diff(96) <= tolerance,
        "delay {}, expected 96 ± {tolerance}",
        report.delay_samples
    );
    let response = magnitude_to_frequency_response(
        "aligned response",
        FS,
        &measured.speakers[0].left.as_ref().unwrap().data,
    )
    .unwrap();
    let band = |lo, hi| {
        response
            .frequency
            .iter()
            .zip(&response.raw)
            .filter(|(frequency, _)| **frequency >= lo && **frequency <= hi)
            .map(|(_, value)| *value)
            .collect::<Vec<_>>()
    };
    let reference = {
        let mut values = band(2.0 * xo, 4.0 * xo);
        values.sort_by(f64::total_cmp);
        (values[(values.len() - 1) / 2] + values[values.len() / 2]) / 2.0
    };
    let worst = band(xo / 2.0, 2.0 * xo)
        .into_iter()
        .map(|value| (value - reference).abs())
        .max_by(f64::total_cmp)
        .unwrap();
    assert!(worst <= 1.5, "aligned crossover deviation {worst} dB");
}

#[test]
fn vbass_v2_follows_the_room_target_below_the_crossover() {
    let target = FrequencyResponse::new(
        "bass target",
        Some(vec![10.0, 40.0, 100.0, 250.0, 500.0, 1000.0, 20_000.0]),
        Some(vec![6.0, 6.0, 6.0, 0.0, 0.0, 0.0, 0.0]),
    )
    .unwrap();
    let measured = impulse();
    let mut flat = hrir(&[("FL", measured.clone()), ("FR", measured.clone())]);
    let mut shaped = hrir(&[("FL", measured.clone()), ("FR", measured)]);
    let flat_report = apply_virtual_bass_v2(&mut flat, &options(None, None))
        .unwrap()
        .unwrap();
    let shaped_report = apply_virtual_bass_v2(&mut shaped, &options(None, Some(&target)))
        .unwrap()
        .unwrap();
    assert!(!flat_report.target_shaped);
    assert!(shaped_report.target_shaped);
    let flat_left = &flat.speakers[0].left.as_ref().unwrap().data;
    let shaped_left = &shaped.speakers[0].left.as_ref().unwrap().data;
    let bass_delta = response_at(shaped_left, 40.0) - response_at(flat_left, 40.0);
    assert!(
        (bass_delta - 6.0).abs() <= 0.7,
        "40 Hz target delta {bass_delta} dB"
    );
    let high_delta = response_at(shaped_left, 1000.0) - response_at(flat_left, 1000.0);
    assert!(high_delta.abs() < 0.1, "1 kHz target delta {high_delta} dB");
}
