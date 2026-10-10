#![forbid(unsafe_code)]
//! Virtual bass synthesis, core/virtual_bass.py:31-198.
use crate::{
    DspError, fft,
    filters::{self, BType, Sos},
    fr::{FrequencyResponse, magnitude_to_frequency_response},
    hrir::Hrir,
    stages::{
        room::{RoomFrs, RoomTerm},
        room_tuning,
        room_v2::{detect_rolloff, median, smooth},
    },
};
use impulcifer_types::constants::{Side, speaker_side};
#[derive(Clone, Debug)]
pub struct VirtualBassOptions {
    pub crossover_freq: u32,
    pub head_ms: f64,
    pub hp_freq: f64,
    pub invert_polarity: Option<bool>,
}
/// Python _delay_signal, core/virtual_bass.py:31-43; p10_vbass.
fn delay_signal(sig: &[f64], delay: i64, length: usize) -> Vec<f64> {
    let mut out = vec![0.0; length];
    if delay >= 0 {
        let start = (delay as usize).min(length);
        let n = (length - start).min(sig.len());
        out[start..start + n].copy_from_slice(&sig[..n]);
    } else {
        let start = delay.unsigned_abs().min(sig.len() as u64) as usize;
        let n = length.min(sig.len() - start);
        out[..n].copy_from_slice(&sig[start..start + n]);
    }
    out
}
/// Python _mag_at, core/virtual_bass.py:46-57; p10_vbass.
fn mag_at(ir: &[f64], fs: u32, freq: f64) -> f64 {
    let spectrum = fft::rfft(ir);
    let index = (0..spectrum.len())
        .min_by(|&a, &b| {
            (a as f64 * (fs as f64 / ir.len() as f64) - freq)
                .abs()
                .total_cmp(&(b as f64 * (fs as f64 / ir.len() as f64) - freq).abs())
        })
        .unwrap();
    spectrum[index].norm()
}
/// Python _duplicate_sos, core/virtual_bass.py:60-62; p10_vbass.
fn duplicate_sos(sos: &Sos, times: usize) -> Sos {
    Sos(sos.0.repeat(times))
}
/// LR8 high-pass shared with the room correction hand-off.
pub(crate) fn highpass_sos(crossover: f64, fs: u32) -> Sos {
    duplicate_sos(
        &filters::butter(4, crossover / (fs as f64 / 2.0), BType::Highpass),
        2,
    )
}
/// Python _rbj_high_shelf, core/virtual_bass.py:65-79; p10_vbass.
fn rbj_high_shelf(fc: f64, fs: u32, gain: f64, q: f64) -> Sos {
    let a = 10.0_f64.powf(gain / 40.0);
    let w = 2.0 * std::f64::consts::PI * fc / fs as f64;
    let alpha = w.sin() / (2.0 * q);
    let c = w.cos();
    filters::tf2sos(
        &[
            a * ((a + 1.0) + (a - 1.0) * c + 2.0 * a.sqrt() * alpha),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * c),
            a * ((a + 1.0) + (a - 1.0) * c - 2.0 * a.sqrt() * alpha),
        ],
        &[
            (a + 1.0) - (a - 1.0) * c + 2.0 * a.sqrt() * alpha,
            2.0 * ((a - 1.0) - (a + 1.0) * c),
            (a + 1.0) - (a - 1.0) * c - 2.0 * a.sqrt() * alpha,
        ],
    )
}
/// Python synthesize_virtual_bass/apply_virtual_bass_to_hrir,
/// core/virtual_bass.py:82-198; p10_vbass. Nyquist rejection returns unchanged,
/// matching Python's logged early return, not an invented DSP error.
pub fn apply_virtual_bass(hrir: &mut Hrir, options: &VirtualBassOptions) -> Result<(), DspError> {
    let fs = hrir.fs;
    let nyq = fs as f64 / 2.0;
    let xo = options.crossover_freq as f64;
    if xo >= nyq {
        return Ok(());
    }
    if xo <= 0.0
        || !options.hp_freq.is_finite()
        || options.hp_freq <= 0.0
        || options.hp_freq >= nyq
        || !options.head_ms.is_finite()
    {
        return Err(DspError::InvalidArgument(
            "invalid virtual bass cutoff or head duration".into(),
        ));
    }
    let n = hrir
        .speakers
        .iter()
        .flat_map(|s| s.left.iter().chain(s.right.iter()))
        .map(|ir| ir.len())
        .max()
        .unwrap_or(0);
    if n == 0 {
        return Err(DspError::InvalidArgument(
            "virtual bass requires nonempty responses".into(),
        ));
    }
    hrir.for_each_ir(|ir| ir.data.resize(n, 0.0));
    let mut impulse = vec![0.0; n];
    impulse[0] = 1.0;
    let sub = filters::sosfilt(
        &filters::butter(4, options.hp_freq / nyq, BType::Highpass),
        &impulse,
    );
    let bass = filters::sosfilt(
        &duplicate_sos(&filters::butter(4, xo / nyq, BType::Lowpass), 2),
        &sub,
    );
    let ild = Sos([
        (150.0, -1.5, 0.760),
        (400.0, -3.0, 0.660),
        (800.0, -3.5, 0.610),
    ]
    .into_iter()
    .flat_map(|(fc, g, q)| rbj_high_shelf(fc, fs, g, q).0)
    .collect());
    let high = highpass_sos(xo, fs);
    let mut mags = Vec::new();
    for s in &hrir.speakers {
        if let (Some(l), Some(r)) = (&s.left, &s.right) {
            for ir in [l, r] {
                mags.push(mag_at(&filters::sosfilt(&high, &ir.data), fs, xo));
            }
        }
    }
    if mags.is_empty() {
        return Ok(());
    }
    let gain = (mags.iter().sum::<f64>() / mags.len() as f64) / (mag_at(&bass, fs, xo) + 1e-20);
    let head = (options.head_ms * 1e-3 * fs as f64).round_ties_even() as i64;
    let polarity = if options.invert_polarity == Some(true) {
        -1.0
    } else {
        1.0
    };
    for s in &mut hrir.speakers {
        if let (Some(l), Some(r)) = (&mut s.left, &mut s.right) {
            let on_left = speaker_side(&s.speaker) == Side::Left;
            let itd = r.peak_index(0, None, 0.12589) as i64 - l.peak_index(0, None, 0.12589) as i64;
            let direct: Vec<_> = bass.iter().map(|v| v * gain * polarity).collect();
            let cross = filters::sosfilt(&ild, &direct);
            let direct = delay_signal(&direct, head, n);
            let cross = delay_signal(&cross, head + if on_left { itd } else { -itd }, n);
            l.data = filters::sosfilt(&high, &l.data)
                .iter()
                .zip(if on_left { &direct } else { &cross })
                .map(|(h, b)| h + b)
                .collect();
            r.data = filters::sosfilt(&high, &r.data)
                .iter()
                .zip(if on_left { &cross } else { &direct })
                .map(|(h, b)| h + b)
                .collect();
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtualBassMode {
    Auto,
    Manual,
    Legacy,
}
impl VirtualBassMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "auto" => Some(Self::Auto),
            "manual" => Some(Self::Manual),
            "legacy" => Some(Self::Legacy),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Manual => "manual",
            Self::Legacy => "legacy",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpeakerRolloff {
    pub speaker: String,
    pub freq: f64,
}

/// Find the bass rolloff of each speaker that has both ears.
pub fn detect_speaker_rolloffs(hrir: &Hrir) -> Result<Vec<SpeakerRolloff>, DspError> {
    let mut result = Vec::new();
    for speaker in &hrir.speakers {
        let (Some(left), Some(right)) = (&speaker.left, &speaker.right) else {
            continue;
        };
        let left = magnitude_to_frequency_response("vbass rolloff", hrir.fs, &left.data)?;
        let right = magnitude_to_frequency_response("vbass rolloff", hrir.fs, &right.data)?;
        let average: Vec<_> = left
            .raw
            .iter()
            .zip(&right.raw)
            .map(|(l, r)| {
                10.0 * ((10.0_f64.powf(l / 10.0) + 10.0_f64.powf(r / 10.0)) / 2.0).log10()
            })
            .collect();
        if let Some(rolloff) = detect_rolloff(&left.frequency, &average, None) {
            result.push(SpeakerRolloff {
                speaker: speaker.speaker.clone(),
                freq: rolloff.freq,
            });
        }
    }
    Ok(result)
}

/// Pick one octave above the highest detected speaker rolloff.
pub fn auto_crossover(rolloffs: &[SpeakerRolloff]) -> Option<f64> {
    rolloffs
        .iter()
        .map(|r| r.freq)
        .max_by(f64::total_cmp)
        .map(|freq| (2.0 * freq).round().clamp(30.0, 500.0))
}

#[derive(Clone, Debug)]
pub struct VirtualBassPlan {
    pub mode: VirtualBassMode,
    pub crossover: Option<f64>,
    pub rolloffs: Vec<SpeakerRolloff>,
    pub room_target: Option<FrequencyResponse>,
}
impl VirtualBassPlan {
    pub fn resolve(
        mode: VirtualBassMode,
        manual_crossover: f64,
        hrir: &Hrir,
    ) -> Result<Self, DspError> {
        if mode == VirtualBassMode::Legacy {
            return Err(DspError::InvalidArgument(
                "legacy virtual bass does not use a plan".into(),
            ));
        }
        let rolloffs = detect_speaker_rolloffs(hrir)?;
        let crossover = match mode {
            VirtualBassMode::Auto => auto_crossover(&rolloffs),
            VirtualBassMode::Manual => Some(manual_crossover),
            VirtualBassMode::Legacy => unreachable!(),
        };
        Ok(Self {
            mode,
            crossover,
            rolloffs,
            room_target: None,
        })
    }
    pub fn limiting_rolloff(&self) -> Option<&SpeakerRolloff> {
        self.rolloffs
            .iter()
            .max_by(|a, b| a.freq.total_cmp(&b.freq))
    }
    pub fn crossover_below_rolloff(&self) -> Option<&SpeakerRolloff> {
        (self.mode == VirtualBassMode::Manual)
            .then(|| self.limiting_rolloff())
            .flatten()
            .filter(|r| {
                self.crossover
                    .is_some_and(|xo| xo < r.freq * 2.0_f64.sqrt())
            })
    }
}

pub struct VirtualBassV2Options<'a> {
    pub crossover: f64,
    pub head_ms: f64,
    pub hp_freq: f64,
    pub invert_polarity: bool,
    pub room_gains: Option<&'a RoomFrs>,
    pub room_target: Option<&'a FrequencyResponse>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VirtualBassReport {
    pub crossover: f64,
    pub band: (f64, f64),
    pub level_db: f64,
    pub delay_samples: usize,
    pub target_shaped: bool,
}

fn same_grid(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(a, b)| (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(f64::MIN_POSITIVE))
}

fn interp_log_clamped(frequency: &[f64], values: &[f64], at: f64) -> f64 {
    if at <= frequency[0] {
        return values[0];
    }
    if at >= frequency[frequency.len() - 1] {
        return values[values.len() - 1];
    }
    let upper = frequency.partition_point(|f| *f < at);
    let lower = upper - 1;
    let t = (at / frequency[lower]).log2() / (frequency[upper] / frequency[lower]).log2();
    values[lower] + t * (values[upper] - values[lower])
}

struct Branches {
    index: usize,
    hi_left: Vec<f64>,
    hi_right: Vec<f64>,
    syn_left: Vec<f64>,
    syn_right: Vec<f64>,
}

/// Apply virtual bass with robust level matching, room-target shaping and alignment.
pub fn apply_virtual_bass_v2(
    hrir: &mut Hrir,
    options: &VirtualBassV2Options,
) -> Result<Option<VirtualBassReport>, DspError> {
    let fs = hrir.fs;
    let nyq = fs as f64 / 2.0;
    let xo = options.crossover;
    if xo >= nyq {
        return Ok(None);
    }
    if xo <= 0.0
        || !options.hp_freq.is_finite()
        || options.hp_freq <= 0.0
        || options.hp_freq >= nyq
        || !options.head_ms.is_finite()
    {
        return Err(DspError::InvalidArgument(
            "invalid virtual bass cutoff or head duration".into(),
        ));
    }
    let n = hrir
        .speakers
        .iter()
        .flat_map(|s| s.left.iter().chain(s.right.iter()))
        .map(|ir| ir.len())
        .max()
        .unwrap_or(0);
    if n == 0 {
        return Err(DspError::InvalidArgument(
            "virtual bass requires nonempty responses".into(),
        ));
    }
    if !hrir
        .speakers
        .iter()
        .any(|s| s.left.is_some() && s.right.is_some())
    {
        return Ok(None);
    }
    hrir.for_each_ir(|ir| ir.data.resize(n, 0.0));
    let mut impulse = vec![0.0; n];
    impulse[0] = 1.0;
    let sub = filters::sosfilt(
        &filters::butter(4, options.hp_freq / nyq, BType::Highpass),
        &impulse,
    );
    let mut bass = filters::sosfilt(
        &duplicate_sos(&filters::butter(4, xo / nyq, BType::Lowpass), 2),
        &sub,
    );
    let ild = Sos([
        (150.0, -1.5, 0.760),
        (400.0, -3.0, 0.660),
        (800.0, -3.5, 0.610),
    ]
    .into_iter()
    .flat_map(|(fc, g, q)| rbj_high_shelf(fc, fs, g, q).0)
    .collect());
    let high = highpass_sos(xo, fs);
    let band = (xo, (4.0 * xo).min(0.45 * fs as f64));
    let mut levels = Vec::new();
    for speaker in &hrir.speakers {
        let (Some(left), Some(right)) = (&speaker.left, &speaker.right) else {
            continue;
        };
        let side = if speaker_side(&speaker.speaker) == Side::Left {
            Side::Left
        } else {
            Side::Right
        };
        let ear = if side == Side::Left { left } else { right };
        let mut fr = magnitude_to_frequency_response("vbass level", fs, &ear.data)?;
        if let Some(gain) = options
            .room_gains
            .filter(|r| r.term == RoomTerm::Gain)
            .and_then(|r| {
                r.entries
                    .iter()
                    .find(|(name, ear_side, _)| name == &speaker.speaker && *ear_side == side)
                    .map(|(_, _, fr)| fr)
            })
        {
            if same_grid(&fr.frequency, &gain.frequency) {
                for (value, gain) in fr.raw.iter_mut().zip(&gain.equalization) {
                    *value += gain;
                }
            } else {
                for (value, frequency) in fr.raw.iter_mut().zip(&fr.frequency) {
                    *value += interp_log_clamped(&gain.frequency, &gain.equalization, *frequency);
                }
            }
        }
        let smoothed = smooth(&fr.frequency, &fr.raw, 1.0 / 6.0)?;
        if let Some(level) = median(
            fr.frequency
                .iter()
                .zip(smoothed)
                .filter(|(f, _)| **f >= band.0 && **f <= band.1)
                .map(|(_, value)| value)
                .collect(),
        ) {
            levels.push(level);
        }
    }
    let unit = magnitude_to_frequency_response("vbass unit", fs, &sub)?;
    let unit_smoothed = smooth(&unit.frequency, &unit.raw, 1.0 / 6.0)?;
    let unit_level = median(
        unit.frequency
            .iter()
            .zip(unit_smoothed)
            .filter(|(f, _)| **f >= band.0 && **f <= band.1)
            .map(|(_, value)| value)
            .collect(),
    )
    .ok_or_else(|| DspError::InvalidArgument("virtual bass level band is empty".into()))?;
    let level_db = median(levels)
        .ok_or_else(|| DspError::InvalidArgument("virtual bass level band is empty".into()))?
        - unit_level;
    let gain = 10.0_f64.powf(level_db / 20.0);

    let mut target_shaped = false;
    if let Some(target) = options
        .room_target
        .filter(|target| target.raw.iter().any(|v| v.abs() > 1e-9))
    {
        let target_band = median(
            target
                .frequency
                .iter()
                .zip(&target.raw)
                .filter(|(f, _)| **f >= band.0 && **f <= band.1)
                .map(|(_, value)| *value)
                .collect(),
        )
        .ok_or_else(|| DspError::InvalidArgument("virtual bass target band is empty".into()))?;
        let nf = 2 * n;
        let magnitude: Vec<_> = (0..=nf / 2)
            .map(|k| {
                let frequency =
                    (k as f64 * fs as f64 / nf as f64).clamp(target.frequency[0], 2.0 * xo);
                10.0_f64.powf(
                    (interp_log_clamped(&target.frequency, &target.raw, frequency) - target_band)
                        / 20.0,
                )
            })
            .collect();
        let shape = room_tuning::minimum_phase(&magnitude, nf);
        let mut padded = vec![0.0; nf];
        padded[..n].copy_from_slice(&bass);
        let shaped: Vec<_> = fft::rfft(&padded)
            .iter()
            .zip(shape)
            .map(|(sample, shape)| sample * shape)
            .collect();
        bass = fft::irfft(&shaped, nf)[..n].to_vec();
        target_shaped = true;
    }

    let head = (options.head_ms * 1e-3 * fs as f64).round_ties_even() as i64;
    let polarity = if options.invert_polarity { -1.0 } else { 1.0 };
    let mut branches = Vec::new();
    for (index, speaker) in hrir.speakers.iter().enumerate() {
        let (Some(left), Some(right)) = (&speaker.left, &speaker.right) else {
            continue;
        };
        let on_left = speaker_side(&speaker.speaker) == Side::Left;
        let itd =
            right.peak_index(0, None, 0.12589) as i64 - left.peak_index(0, None, 0.12589) as i64;
        let direct: Vec<_> = bass.iter().map(|v| v * gain * polarity).collect();
        let cross = filters::sosfilt(&ild, &direct);
        let direct = delay_signal(&direct, head, n);
        let cross = delay_signal(&cross, head + if on_left { itd } else { -itd }, n);
        branches.push(Branches {
            index,
            hi_left: filters::sosfilt(&high, &left.data),
            hi_right: filters::sosfilt(&high, &right.data),
            syn_left: if on_left {
                direct.clone()
            } else {
                cross.clone()
            },
            syn_right: if on_left { cross } else { direct },
        });
    }
    let nf = 2 * n;
    let mut accumulated = vec![fft::Complex64::new(0.0, 0.0); nf / 2 + 1];
    for branch in &branches {
        for (measured, synthetic) in [
            (&branch.hi_left, &branch.syn_left),
            (&branch.hi_right, &branch.syn_right),
        ] {
            let mut measured_padded = vec![0.0; nf];
            let mut synthetic_padded = vec![0.0; nf];
            measured_padded[..n].copy_from_slice(measured);
            synthetic_padded[..n].copy_from_slice(synthetic);
            let measured = fft::rfft(&measured_padded);
            let synthetic = fft::rfft(&synthetic_padded);
            for (k, value) in accumulated.iter_mut().enumerate() {
                let frequency = k as f64 * fs as f64 / nf as f64;
                if frequency >= xo / 2.0_f64.sqrt() && frequency <= xo * 2.0_f64.sqrt() {
                    *value += measured[k] * synthetic[k].conj();
                }
            }
        }
    }
    let correlation = fft::irfft(&accumulated, nf);
    // One period at the crossover covers every phase there.
    let period = ((fs as f64 / xo).floor() as usize).clamp(1, nf);
    let delay_samples = correlation[..period]
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1).then_with(|| b.0.cmp(&a.0)))
        .map_or(0, |(index, _)| index);
    for branch in branches {
        let speaker = &mut hrir.speakers[branch.index];
        let left = speaker.left.as_mut().unwrap();
        let right = speaker.right.as_mut().unwrap();
        left.data = branch
            .hi_left
            .iter()
            .zip(delay_signal(&branch.syn_left, delay_samples as i64, n))
            .map(|(high, bass)| high + bass)
            .collect();
        right.data = branch
            .hi_right
            .iter()
            .zip(delay_signal(&branch.syn_right, delay_samples as i64, n))
            .map(|(high, bass)| high + bass)
            .collect();
    }
    Ok(Some(VirtualBassReport {
        crossover: xo,
        band,
        level_db,
        delay_samples,
        target_shaped,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Python zero-padding, truncation and non-wrapping bounds; core/virtual_bass.py:31-43; p10_vbass.
    #[test]
    fn delay_signal_boundaries() {
        let x = [1.0, 2.0, 3.0];
        for (delay, length, expected) in [
            (0, 5, vec![1.0, 2.0, 3.0, 0.0, 0.0]),
            (0, 2, vec![1.0, 2.0]),
            (1, 3, vec![0.0, 1.0, 2.0]),
            (-1, 3, vec![2.0, 3.0, 0.0]),
            (3, 3, vec![0.0; 3]),
            (-3, 3, vec![0.0; 3]),
            (i64::MAX, 3, vec![0.0; 3]),
            (i64::MIN, 3, vec![0.0; 3]),
            (0, 0, vec![]),
        ] {
            assert_eq!(delay_signal(&x, delay, length), expected);
        }
        assert_eq!(delay_signal(&[], 0, 3), vec![0.0; 3]);
    }

    /// Python nearest rfft bin, first-bin tie and endpoint clamps; core/virtual_bass.py:53-57; p10_vbass.
    #[test]
    fn mag_at_bin_boundaries() {
        // Eight-sample DC signal: only the first bin has magnitude eight.
        let x = [1.0; 8];
        assert_eq!(mag_at(&x, 8000, -100.0), 8.0);
        assert_eq!(mag_at(&x, 8000, 500.0), 8.0);
        assert_eq!(mag_at(&x, 8000, 500.1), 0.0);
        assert_eq!(mag_at(&x, 8000, 10000.0), 0.0);
        assert_eq!(mag_at(&[2.0], 8000, 4000.0), 2.0);
        for freq in [0.0, 500.0, 4000.0, 10000.0] {
            assert!((mag_at(&[1.0, 0.0, 0.0, 0.0, 0.0], 8000, freq) - 1.0).abs() < 1e-14);
        }
    }

    /// Python duplicates entire chains in order, not individual sections; core/virtual_bass.py:60-62; p10_vbass.
    #[test]
    fn duplicate_sos_preserves_chain_order() {
        let sos = filters::butter(4, 0.25, BType::Lowpass);
        assert_eq!(duplicate_sos(&sos, 1).0, sos.0);
        let got = duplicate_sos(&sos, 3);
        assert_eq!(got.0.len(), 3 * sos.0.len());
        for chain in got.0.chunks(sos.0.len()) {
            assert_eq!(chain, sos.0);
        }
    }

    /// Python RBJ shelf has unit DC and requested Nyquist gain; core/virtual_bass.py:65-79; p10_vbass.
    #[test]
    fn rbj_high_shelf_endpoint_gains() {
        for fs in [44100, 48000] {
            for (fc, gain, q) in [
                (150.0, -1.5, 0.760),
                (400.0, -3.0, 0.660),
                (800.0, -3.5, 0.610),
                (400.0, 0.0, 0.660),
                (400.0, 3.0, 0.660),
            ] {
                let sos = rbj_high_shelf(fc, fs, gain, q);
                assert_eq!(sos.0.len(), 1);
                let c = &sos.0[0];
                let dc = (c[0] + c[1] + c[2]) / (c[3] + c[4] + c[5]);
                let nyquist = (c[0] - c[1] + c[2]) / (c[3] - c[4] + c[5]);
                assert!((dc - 1.0).abs() < 1e-9);
                assert!((nyquist - 10.0_f64.powf(gain / 20.0)).abs() < 1e-12);
            }
        }
    }
}
