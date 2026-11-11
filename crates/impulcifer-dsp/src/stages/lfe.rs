//! Measured and room-derived LFE preparation (ADR 0017).
use super::{
    room::{FrCombination, RoomCorrectionOptions, RoomRange},
    room_v2,
};
use crate::{
    DspError,
    filters::{self, BType},
    fr::{CompensateOptions, FrequencyResponse, magnitude_to_frequency_response},
    hrir::Hrir,
    ir::ImpulseResponse,
    peaks, windows,
};

pub const LFE_BAND_LIMIT_HZ: f64 = 120.0;

pub fn onset_index(data: &[f64]) -> usize {
    let peak = peaks::first_peak_index(data, 0, None, 0.12589);
    let threshold = data.get(peak).map_or(0.0, |value| value.abs() * 0.1);
    let mut onset = peak;
    while onset > 0 && data[onset - 1].abs() >= threshold {
        onset -= 1;
    }
    onset
}

#[derive(Clone, Debug)]
pub struct LevelTransfer {
    pub left_db: f64,
    pub right_db: f64,
    pub speakers: usize,
    pub spread_db: f64,
}

fn percentile(mut values: Vec<f64>, fraction: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let rank = fraction * (values.len() - 1) as f64;
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    let weight = rank - lower as f64;
    Some(values[lower] * (1.0 - weight) + values[upper] * weight)
}

fn room_ear(speaker: &crate::hrir::SpeakerIrs, left: bool) -> Option<&ImpulseResponse> {
    if left {
        speaker.left.as_ref().or(speaker.right.as_ref())
    } else {
        speaker.right.as_ref().or(speaker.left.as_ref())
    }
}

fn band_level(ir: &ImpulseResponse, mic: Option<&FrequencyResponse>) -> Result<f64, DspError> {
    let margin = (0.005 * f64::from(ir.fs)).round() as usize;
    let start = onset_index(&ir.data).saturating_sub(margin);
    let length = (ir.fs as usize / 2).min(ir.data.len().saturating_sub(start));
    if length == 0 {
        return Err(DspError::InvalidArgument(
            "LFE level segment is empty".into(),
        ));
    }
    let mut fr =
        magnitude_to_frequency_response("LFE level", ir.fs, &ir.data[start..start + length])?;
    if let Some(mic) = mic {
        if mic.raw.len() != fr.raw.len() {
            return Err(DspError::InvalidArgument(
                "LFE calibration grid lengths differ".into(),
            ));
        }
        for (measured, calibration) in fr.raw.iter_mut().zip(&mic.raw) {
            *measured -= calibration;
        }
    }
    let smoothed = room_v2::smooth(&fr.frequency, &fr.raw, 1.0 / 6.0)?;
    let values: Vec<_> = fr
        .frequency
        .iter()
        .zip(smoothed)
        .filter(|(frequency, _)| (100.0..=250.0).contains(*frequency))
        .map(|(_, db)| 10.0_f64.powf(db / 10.0))
        .collect();
    if values.is_empty() {
        return Err(DspError::InvalidArgument(
            "LFE level band has no frequency points".into(),
        ));
    }
    Ok(10.0 * (values.iter().sum::<f64>() / values.len() as f64).log10())
}

/// Returns `None` when fewer than three speakers took part.
pub fn level_transfer(
    brir: &Hrir,
    room: &Hrir,
    mic: Option<&FrequencyResponse>,
) -> Result<Option<LevelTransfer>, DspError> {
    if brir.fs != room.fs {
        return Err(DspError::InvalidArgument(
            "BRIR and room sample rates differ for LFE level transfer".into(),
        ));
    }
    let mut left = Vec::new();
    let mut right = Vec::new();
    for speaker in &brir.speakers {
        let (Some(brir_left), Some(brir_right), Some(room_speaker)) = (
            speaker.left.as_ref(),
            speaker.right.as_ref(),
            room.get(&speaker.speaker),
        ) else {
            continue;
        };
        let (Some(room_left), Some(room_right)) =
            (room_ear(room_speaker, true), room_ear(room_speaker, false))
        else {
            continue;
        };
        left.push(band_level(brir_left, None)? - band_level(room_left, mic)?);
        right.push(band_level(brir_right, None)? - band_level(room_right, mic)?);
    }
    if left.len() < 3 {
        return Ok(None);
    }
    let left_db = room_v2::median(left.clone()).expect("three values");
    let right_db = room_v2::median(right.clone()).expect("three values");
    let spread = |values: Vec<f64>| {
        percentile(values.clone(), 0.75).unwrap() - percentile(values, 0.25).unwrap()
    };
    Ok(Some(LevelTransfer {
        left_db,
        right_db,
        speakers: left.len(),
        spread_db: spread(left).max(spread(right)),
    }))
}

#[derive(Clone, Debug)]
pub struct LfeCorrection {
    pub fr: FrequencyResponse,
    pub low: f64,
    pub high: f64,
    pub snr_available: bool,
}

pub fn upper_limit(frequency: &[f64], measured_db: &[f64]) -> f64 {
    let Ok(smoothed) = room_v2::smooth(frequency, measured_db, 1.0 / 3.0) else {
        return LFE_BAND_LIMIT_HZ;
    };
    let plateau = room_v2::median(
        frequency
            .iter()
            .zip(&smoothed)
            .filter(|(frequency, _)| (30.0..=80.0).contains(*frequency))
            .map(|(_, value)| *value)
            .collect(),
    );
    let Some(plateau) = plateau else {
        return LFE_BAND_LIMIT_HZ;
    };
    frequency
        .iter()
        .copied()
        .filter(|frequency| (40.0..=120.0).contains(frequency))
        .find(|limit| {
            frequency
                .iter()
                .zip(&smoothed)
                .filter(|(candidate, _)| **candidate >= *limit && **candidate <= 2.0 * limit)
                .all(|(_, value)| *value <= plateau - 6.0)
        })
        .unwrap_or(LFE_BAND_LIMIT_HZ)
}

pub fn room_correction(
    ir: &ImpulseResponse,
    target: &FrequencyResponse,
    mic: Option<&FrequencyResponse>,
) -> Result<LfeCorrection, DspError> {
    let margin = (0.005 * f64::from(ir.fs)).round() as usize;
    let start = onset_index(&ir.data).saturating_sub(margin);
    if start >= ir.data.len() {
        return Err(DspError::InvalidArgument(
            "LFE correction segment is empty".into(),
        ));
    }
    let mut fr = magnitude_to_frequency_response("LFE room correction", ir.fs, &ir.data[start..])?;
    if let Some(mic) = mic {
        if mic.raw.len() != fr.raw.len() {
            return Err(DspError::InvalidArgument(
                "LFE calibration grid lengths differ".into(),
            ));
        }
        for (measured, calibration) in fr.raw.iter_mut().zip(&mic.raw) {
            *measured -= calibration;
        }
    }
    let snr = room_v2::snr_db(ir, &fr.frequency);
    let detected = room_v2::detect_rolloff(&fr.frequency, &fr.raw, snr.as_deref());
    let rolloff = detected.map_or(
        room_v2::Rolloff {
            freq: 16.0,
            knee: 20.0,
            source: room_v2::RolloffSource::Slope,
        },
        |rolloff| room_v2::Rolloff {
            freq: rolloff.freq.max(16.0),
            knee: rolloff.knee.max(20.0),
            source: rolloff.source,
        },
    );
    let low = rolloff.knee;
    let high = upper_limit(&fr.frequency, &fr.raw);
    fr.compensate(target, &CompensateOptions::default())?;
    fr.error_smoothed = room_v2::smooth(&fr.frequency, &fr.error, 1.0 / 6.0)?;
    let centre = room_v2::median(
        fr.frequency
            .iter()
            .zip(&fr.error_smoothed)
            .filter(|(frequency, _)| {
                **frequency >= low && **frequency <= high / std::f64::consts::SQRT_2
            })
            .map(|(_, value)| *value)
            .collect(),
    )
    .unwrap_or(0.0);
    for value in fr.error.iter_mut().chain(&mut fr.error_smoothed) {
        *value -= centre;
    }
    room_v2::correction_gain(
        &mut fr,
        snr.as_deref(),
        Some(rolloff),
        high,
        false,
        ir.fs,
        &RoomCorrectionOptions {
            range: RoomRange::Schroeder,
            max_boost_db: 6.0,
            vbass_crossover: None,
            fr_combination_method: FrCombination::Average,
            ..Default::default()
        },
    )?;
    Ok(LfeCorrection {
        fr,
        low,
        high,
        snr_available: snr.is_some(),
    })
}

pub fn place_onsets(lfe: &mut Hrir, at: usize, head_ms: f64) {
    let head = (head_ms * f64::from(lfe.fs) / 1000.0).round() as usize;
    let window = (head > 0).then(|| windows::hann(head * 2, true));
    for speaker in &mut lfe.speakers {
        let onset = speaker
            .left
            .iter()
            .chain(&speaker.right)
            .map(|ir| onset_index(&ir.data))
            .min()
            .unwrap_or(at);
        let shift = at as i64 - onset as i64;
        for ir in speaker.left.iter_mut().chain(&mut speaker.right) {
            ir.shift(shift);
            if let Some(window) = &window {
                for (sample, gain) in ir.data.iter_mut().take(head).zip(window) {
                    *sample *= gain;
                }
            }
        }
    }
}

pub fn band_limit(lfe: &mut Hrir) {
    let sos = filters::butter(
        2,
        LFE_BAND_LIMIT_HZ / (f64::from(lfe.fs) / 2.0),
        BType::Lowpass,
    );
    lfe.for_each_ir(|ir| {
        ir.data = filters::sosfilt(&sos, &ir.data);
        ir.data = filters::sosfilt(&sos, &ir.data);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fft, hrir::SpeakerIrs};

    fn ir(data: Vec<f64>, fs: u32) -> ImpulseResponse {
        ImpulseResponse {
            data,
            fs,
            recording: None,
        }
    }

    #[test]
    fn lfe_onset_finds_the_start_of_the_first_lobe() {
        let fs = 48_000;
        for polarity in [-1.0, 1.0] {
            let mut impulse = vec![0.0; 4096];
            impulse[500] = polarity;
            let filtered = filters::sosfilt(
                &filters::butter(4, 120.0 / (f64::from(fs) / 2.0), BType::Lowpass),
                &impulse,
            );
            let peak_level = filtered.iter().map(|value| value.abs()).fold(0.0, f64::max);
            let noisy: Vec<_> = filtered
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    value + 0.001 * peak_level * ((index * 37 % 101) as f64 / 50.0 - 1.0)
                })
                .collect();
            let onset = onset_index(&noisy);
            let peak = peaks::first_peak_index(&noisy, 0, None, 0.12589);
            assert!(onset < peak, "onset {onset}, peak {peak}");
            let first_lobe = noisy[peak].abs();
            assert!(onset == 0 || noisy[onset - 1].abs() < first_lobe * 0.1);
        }
    }

    fn transfer_hrirs(count: usize, deviations: &[f64]) -> (Hrir, Hrir) {
        let fs = 48_000;
        let mut room = Vec::new();
        let mut brir = Vec::new();
        for index in 0..count {
            let mut data = vec![0.0; fs as usize / 2];
            data[100 + index] = 1.0;
            let deviation = deviations.get(index).copied().unwrap_or(0.0);
            let left = 10.0_f64.powf((6.0 + deviation) / 20.0);
            let right = 10.0_f64.powf((-4.0 - deviation) / 20.0);
            room.push(SpeakerIrs {
                speaker: format!("S{index}"),
                left: Some(ir(data.clone(), fs)),
                right: Some(ir(data.clone(), fs)),
            });
            brir.push(SpeakerIrs {
                speaker: format!("S{index}"),
                left: Some(ir(data.iter().map(|value| value * left).collect(), fs)),
                right: Some(ir(data.iter().map(|value| value * right).collect(), fs)),
            });
        }
        (Hrir { fs, speakers: brir }, Hrir { fs, speakers: room })
    }

    #[test]
    fn lfe_level_transfer_recovers_the_gain_between_microphones() {
        let (brir, room) = transfer_hrirs(4, &[-0.1, 0.0, 0.05, 0.1]);
        let transfer = level_transfer(&brir, &room, None).unwrap().unwrap();
        assert!(
            (transfer.left_db - 6.025).abs() < 0.1,
            "{}",
            transfer.left_db
        );
        assert!(
            (transfer.right_db + 4.025).abs() < 0.1,
            "{}",
            transfer.right_db
        );
        assert_eq!(transfer.speakers, 4);
        let (two_brir, two_room) = transfer_hrirs(2, &[0.0, 0.0]);
        assert!(
            level_transfer(&two_brir, &two_room, None)
                .unwrap()
                .is_none()
        );
        let (spread_brir, spread_room) = transfer_hrirs(4, &[-5.0, -2.0, 2.0, 5.0]);
        assert!(
            level_transfer(&spread_brir, &spread_room, None)
                .unwrap()
                .unwrap()
                .spread_db
                > 3.0
        );
    }

    #[test]
    fn lfe_room_correction_cuts_modes_within_the_subwoofer_band() {
        let fs = 48_000;
        let mut impulse = vec![0.0; fs as usize * 2];
        impulse[240] = 1.0;
        let peak = filters::rbj_peaking(50.0, 5.0, 10.0, f64::from(fs));
        let peaked = filters::sosfilt(&filters::tf2sos(&peak.b, &peak.a), &impulse);
        let target = super::super::room::prepare_room_target(None, fs).unwrap();
        let correction = room_correction(&ir(peaked, fs), &target, None).unwrap();
        let at = |frequency: f64| {
            correction
                .fr
                .frequency
                .iter()
                .position(|candidate| *candidate >= frequency)
                .unwrap()
        };
        let narrow =
            room_v2::smooth(&correction.fr.frequency, &correction.fr.raw, 1.0 / 12.0).unwrap();
        assert!(
            (correction.fr.equalization[at(50.0)] + correction.fr.raw[at(50.0)]).abs() < 1.0,
            "gain at 50 Hz was {} dB for a {} dB measured peak ({} dB narrow view)",
            correction.fr.equalization[at(50.0)],
            correction.fr.raw[at(50.0)],
            narrow[at(50.0)]
        );
        assert_eq!(correction.high, 120.0);
        let lowpass = filters::butter(2, 80.0 / (f64::from(fs) / 2.0), BType::Lowpass);
        let lowpassed = filters::sosfilt(&lowpass, &filters::sosfilt(&lowpass, &impulse));
        let limited = room_correction(&ir(lowpassed, fs), &target, None).unwrap();
        assert!(
            (limited.high / 80.0).log2().abs() <= 1.0 / 6.0,
            "upper limit was {} Hz",
            limited.high
        );
        assert!(
            limited
                .fr
                .frequency
                .iter()
                .zip(&limited.fr.equalization)
                .filter(|(frequency, _)| **frequency > limited.high)
                .all(|(_, gain)| gain.abs() < 1e-12)
        );
        assert!(
            limited
                .fr
                .equalization
                .iter()
                .all(|gain| *gain <= 6.0 + 1e-12)
        );
        let centred = room_v2::median(
            correction
                .fr
                .frequency
                .iter()
                .zip(&correction.fr.equalization)
                .filter(|(frequency, _)| {
                    **frequency >= correction.low
                        && **frequency <= correction.high / std::f64::consts::SQRT_2
                })
                .map(|(_, gain)| *gain)
                .collect(),
        )
        .unwrap();
        assert!(centred.abs() < 0.5, "median gain was {centred} dB");

        let full = room_correction(&ir(impulse.clone(), fs), &target, None).unwrap();
        assert_eq!(full.high, 120.0);

        let rolled_data = filters::sosfilt(
            &filters::butter(8, 20.0 / (f64::from(fs) / 2.0), BType::Highpass),
            &impulse,
        );
        let rolled_ir = ir(rolled_data, fs);
        let start =
            onset_index(&rolled_ir.data).saturating_sub((0.005 * f64::from(fs)).round() as usize);
        let measured =
            magnitude_to_frequency_response("rolloff", fs, &rolled_ir.data[start..]).unwrap();
        let snr = room_v2::snr_db(&rolled_ir, &measured.frequency);
        let rolloff = room_v2::detect_rolloff(&measured.frequency, &measured.raw, snr.as_deref())
            .unwrap()
            .freq
            .max(16.0);
        let rolled = room_correction(&rolled_ir, &target, None).unwrap();
        assert!(rolloff < 30.0, "detected rolloff was {rolloff} Hz");
        let below_rolloff = rolled
            .fr
            .frequency
            .iter()
            .zip(&rolled.fr.equalization)
            .filter(|(frequency, _)| **frequency < rolloff)
            .map(|(_, gain)| *gain)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            below_rolloff <= 1e-12,
            "rolloff received a {below_rolloff} dB boost (low {})",
            rolled.low
        );
    }

    #[test]
    fn lfe_band_limit_is_the_lr4_low_pass() {
        let fs = 48_000;
        let length = fs as usize * 4;
        let mut data = vec![0.0; length];
        data[0] = 1.0;
        let mut hrir = Hrir {
            fs,
            speakers: vec![SpeakerIrs {
                speaker: "LFE".into(),
                left: Some(ir(data.clone(), fs)),
                right: Some(ir(data, fs)),
            }],
        };
        band_limit(&mut hrir);
        let spectrum = fft::rfft(&hrir.pair("LFE").unwrap().0.data);
        let db = |frequency: f64| {
            let index = (frequency * length as f64 / f64::from(fs)).round() as usize;
            20.0 * spectrum[index].norm().log10()
        };
        assert!((db(120.0) + 6.0).abs() < 0.2, "{}", db(120.0));
        assert!((db(60.0) + 0.5).abs() < 0.2, "{}", db(60.0));
        assert!(db(1000.0) < -70.0, "{}", db(1000.0));
    }
}
