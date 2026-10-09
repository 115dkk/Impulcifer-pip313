//! Independent speaker-feed processor, derived only from omni room measurements.
use crate::{
    DspError, conv,
    fft::{self, Complex64},
    fr::FrequencyResponse,
    hrir::Hrir,
    ir::ImpulseResponse,
};
use impulcifer_types::{
    config::{PhaseLimit, TuningDelay},
    constants::{IPSILATERAL_PAIRS, Side},
};
use std::f64::consts::{PI, SQRT_2};

/// Normal and Low fractional-octave denominators.
pub const RESOLUTIONS: [[f64; 5]; 2] = [[24., 12., 6., 3., 1.], [12., 6., 3., 2., 1.]];
#[derive(Clone, Debug)]
pub struct TuningOptions {
    pub delay: TuningDelay,
    pub phase_limit: PhaseLimit,
    pub schroeder: f64,
    pub max_boost: f64,
    pub curtain: f64,
    pub level_match: bool,
    pub pairs: Vec<PairMeasurement>,
}
impl Default for TuningOptions {
    fn default() -> Self {
        Self {
            delay: TuningDelay::Ms(10.),
            phase_limit: PhaseLimit::Full,
            schroeder: 200.,
            max_boost: 6.,
            curtain: 300.,
            level_match: true,
            pairs: Vec::new(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct PairMeasurement {
    pub speakers: [String; 2],
    pub side: Side,
    pub irs: [ImpulseResponse; 2],
}
#[derive(Clone, Debug)]
pub struct PairCheck {
    pub speakers: [String; 2],
    pub side: Side,
    pub omni_ms: f64,
    pub mismatch: bool,
}
#[derive(Clone, Debug)]
pub struct AutoDelayScores {
    pub speakers: [String; 2],
    pub side: Side,
    pub metrics: Vec<(u32, f64)>,
}
#[derive(Clone, Debug)]
pub struct TuningReport {
    pub fs: u32,
    pub f_ph: f64,
    pub delay_samples: usize,
    pub speakers: Vec<SpeakerTuning>,
    pub auto_scores: Vec<(u32, f64)>,
    pub auto_file_scores: Vec<AutoDelayScores>,
    pub pair_checks: Vec<PairCheck>,
}
#[derive(Clone, Debug)]
pub struct TuningPlan {
    pub fs: u32,
    pub report: TuningReport,
    pub level_match: bool,
    pub pairs: Vec<PairMeasurement>,
}
#[derive(Clone, Debug)]
pub struct SpeakerTuning {
    pub speaker: String,
    pub points: usize,
    pub reference: f64,
    pub low_cutoff: f64,
    pub high_cutoff: f64,
    pub trim_db: f64,
    pub trim_source: &'static str,
    pub level_difference_db: f64,
    pub corrected_share: f64,
    pub pre_echo_db: f64,
    pub pre_echo_channels: Vec<PreEcho>,
    pub design_origin: usize,
    pub target: Vec<f64>,
    pub phase_corrected: bool,
    pub mismatch: bool,
    pub lf_edt_before_ms: f64,
    pub lf_edt_after_ms: f64,
    pub excess_median_before_ms: f64,
    pub excess_median_after_ms: f64,
    pub representation_rms_db: Vec<(Side, f64)>,
    pub filter: Vec<f64>,
    pub weights: Vec<f64>,
    pub full_filter: Vec<f64>,
    pub excess: Vec<Complex64>,
    pub inverse: Vec<f64>,
    pub macro_gain: Vec<f64>,
    pub omni: Vec<(Side, ImpulseResponse)>,
}
#[derive(Clone, Debug)]
pub struct PreEcho {
    pub side: Side,
    pub before_db: f64,
    pub after_db: f64,
}
impl PreEcho {
    pub fn warns(&self) -> bool {
        self.after_db > -30. && self.after_db >= self.before_db + 6.
    }
}
fn unit(v: Complex64) -> Complex64 {
    if v.norm() < 1e-12 {
        Complex64::new(1., 0.)
    } else {
        v / v.norm()
    }
}
/// Positive FFT half; negative bins are supplied by irfft's conjugate mirror.
pub fn smooth_bins(data: &[Complex64], width: f64) -> Vec<Complex64> {
    if data.len() < 3 {
        return data.to_vec();
    }
    let mut prefix = vec![Complex64::default(); data.len() + 1];
    for (i, v) in data.iter().enumerate() {
        prefix[i + 1] = prefix[i] + v;
    }
    let mut out = data.to_vec();
    let span = 2_f64.powf(width / 2.) - 2_f64.powf(-width / 2.);
    for (k, v) in out.iter_mut().enumerate().skip(1) {
        let m = ((k as f64 * span / 2.).floor() as usize).max(1);
        let lo = k.saturating_sub(m).max(1);
        let hi = (k + m + 1).min(data.len());
        *v = (prefix[hi] - prefix[lo]) / (hi - lo) as f64;
    }
    out
}
pub fn smooth_real(data: &[f64], width: f64) -> Vec<f64> {
    smooth_bins(
        &data
            .iter()
            .map(|v| Complex64::new(*v, 0.))
            .collect::<Vec<_>>(),
        width,
    )
    .into_iter()
    .map(|v| v.re)
    .collect()
}
// SECS accumulates its float32 input magnitudes in float32. Keeping that
// boundary matters at threshold crossings (SL's high cutoff differs by one bin
// with an f64 cumulative sum). Later complex/tone smoothing stays f64.
fn smooth_input_magnitude(data: &[f64], width: f64) -> Vec<f64> {
    let mut prefix = vec![0_f32; data.len() + 1];
    for (i, v) in data.iter().enumerate() {
        prefix[i + 1] = prefix[i] + *v as f32;
    }
    let span = 2_f64.powf(width / 2.) - 2_f64.powf(-width / 2.);
    let mut out = data.to_vec();
    for (k, v) in out.iter_mut().enumerate().skip(1) {
        let m = ((k as f64 * span / 2.).floor() as usize).max(1);
        let lo = k.saturating_sub(m).max(1);
        let hi = (k + m + 1).min(data.len());
        *v = (prefix[hi] - prefix[lo]) as f64 / (hi - lo) as f64;
    }
    out
}
fn bands(f: f64) -> [f64; 5] {
    let l = |fc: f64| 1. / (1. + (f / fc).powi(4));
    [
        l(100.),
        (1. - l(100.)) * l(150.),
        (1. - l(150.)) * l(200.),
        (1. - l(200.)) * l(300.),
        1. - l(300.),
    ]
}
fn five(data: &[Complex64], low: bool, df: f64) -> Vec<Complex64> {
    let sm = RESOLUTIONS[usize::from(low)].map(|b| smooth_bins(data, 1. / b));
    (0..data.len())
        .map(|k| {
            bands(k as f64 * df)
                .iter()
                .enumerate()
                .map(|(i, w)| sm[i][k] * w)
                .sum()
        })
        .collect()
}
fn five_real(data: &[f64], low: bool, df: f64) -> Vec<f64> {
    five(
        &data
            .iter()
            .map(|v| Complex64::new(*v, 0.))
            .collect::<Vec<_>>(),
        low,
        df,
    )
    .into_iter()
    .map(|v| v.re)
    .collect()
}
pub fn spectrum(data: &[f64], n: usize) -> Vec<Complex64> {
    let mut x = vec![0.; n];
    let len = n.min(data.len());
    x[..len].copy_from_slice(&data[..len]);
    fft::rfft(&x)
}
pub fn minimum_phase(magnitude: &[f64], n: usize) -> Vec<Complex64> {
    let log: Vec<_> = magnitude
        .iter()
        .map(|v| Complex64::new((v + 1e-12).ln(), 0.))
        .collect();
    let mut cep = fft::irfft(&log, n);
    for v in &mut cep[1..n.div_ceil(2)] {
        *v *= 2.;
    }
    cep[n / 2 + 1..].fill(0.);
    fft::rfft(&cep).into_iter().map(|v| v.exp()).collect()
}
fn peak(data: &[f64]) -> usize {
    data.iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .map_or(0, |v| v.0)
}
pub fn excess_spectrum(ir: &ImpulseResponse, n: usize) -> (Vec<Complex64>, Vec<f64>) {
    let mut input = vec![0_f32; n];
    for (v, sample) in input.iter_mut().zip(&ir.data) {
        *v = *sample as f32;
    }
    let plan = realfft::RealFftPlanner::<f32>::new().plan_fft_forward(n);
    let mut raw = plan.make_output_vec();
    plan.process(&mut input, &mut raw)
        .expect("valid tuning FFT buffers");
    let mag: Vec<_> = raw.iter().map(|v| v.norm() as f64).collect();
    let h: Vec<_> = raw
        .iter()
        .map(|v| Complex64::new(v.re as f64, v.im as f64))
        .collect();
    let min = minimum_phase(&mag, n);
    let p = peak(&ir.data[..ir.data.len().min(n)]);
    (
        h.iter()
            .zip(min)
            .enumerate()
            .map(|(k, (h, m))| {
                h / m * Complex64::from_polar(1., 2. * PI * k as f64 * p as f64 / n as f64)
            })
            .collect(),
        mag,
    )
}
fn interp(f: f64, frequency: &[f64], values: &[f64]) -> f64 {
    let hi = frequency.partition_point(|v| *v < f);
    if hi == 0 {
        return values[0];
    }
    if hi == frequency.len() {
        return values[hi - 1];
    }
    let t = (f - frequency[hi - 1]) / (frequency[hi] - frequency[hi - 1]);
    values[hi - 1] * (1. - t) + values[hi] * t
}
fn percentile(mut x: Vec<f64>, q: f64) -> f64 {
    if x.is_empty() {
        return 1e-12;
    }
    x.sort_by(f64::total_cmp);
    let p = (x.len() - 1) as f64 * q;
    x[p.floor() as usize] * (1. - p.fract()) + x[p.ceil() as usize] * p.fract()
}
fn mean_band(x: &[f64], df: f64, lo: f64, hi: f64) -> f64 {
    let v: Vec<_> = x
        .iter()
        .enumerate()
        .filter(|(k, _)| *k as f64 * df >= lo && (*k as f64 * df) < hi)
        .map(|(_, v)| *v)
        .collect();
    if v.is_empty() {
        1e-12
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}
fn select(x: &[f64], df: f64, lo: f64, hi: f64) -> Vec<f64> {
    x.iter()
        .enumerate()
        .filter(|(k, _)| *k as f64 * df >= lo && (*k as f64 * df) < hi)
        .map(|(_, v)| *v)
        .collect()
}
fn window(x: &[f64], fs: u32, pre_ms: f64, post_ms: Option<f64>) -> Vec<f64> {
    let n = x.len();
    let pre = pre_ms * fs as f64 / 1000.;
    x.iter()
        .enumerate()
        .map(|(i, v)| {
            let t = ((i + n / 2) % n) as f64 - (n / 2) as f64;
            let w = if t < 0. {
                if t < -pre {
                    0.
                } else {
                    0.5 * (1. + (PI * t / pre).cos())
                }
            } else if let Some(post) = post_ms {
                let p = post * fs as f64 / 1000.;
                if t > p {
                    0.
                } else {
                    0.5 * (1. + (PI * t / p).cos())
                }
            } else {
                1.
            };
            v * w
        })
        .collect()
}
/// Intermediate values from the same production chain, for black-box stage checks.
#[derive(Default)]
pub struct TuningStages {
    pub smoothed_magnitude: Vec<f64>,
    pub track_weight: Vec<f64>,
    pub windowed_minimum: Vec<f64>,
    pub crush: Vec<f64>,
}

pub fn design_speaker_with_stages(
    speaker: &str,
    points: &[(Side, ImpulseResponse)],
    options: &TuningOptions,
    delay_ms: f64,
) -> Result<(SpeakerTuning, TuningStages), DspError> {
    let mut stages = TuningStages::default();
    let result = design_speaker_inner(
        speaker,
        points,
        options,
        delay_ms,
        None,
        None,
        Some(&mut stages),
    )?;
    Ok((result, stages))
}

/// One processor calculation, before the final 510 ms FIR crop.
pub fn design_speaker(
    speaker: &str,
    points: &[(Side, ImpulseResponse)],
    options: &TuningOptions,
    delay_ms: f64,
    calibration: Option<&FrequencyResponse>,
    target_curve: Option<&FrequencyResponse>,
) -> Result<SpeakerTuning, DspError> {
    design_speaker_inner(
        speaker,
        points,
        options,
        delay_ms,
        calibration,
        target_curve,
        None,
    )
}

fn design_speaker_inner(
    speaker: &str,
    points: &[(Side, ImpulseResponse)],
    options: &TuningOptions,
    delay_ms: f64,
    calibration: Option<&FrequencyResponse>,
    target_curve: Option<&FrequencyResponse>,
    mut stages: Option<&mut TuningStages>,
) -> Result<SpeakerTuning, DspError> {
    let fs = points
        .first()
        .ok_or_else(|| DspError::InvalidArgument("tuning requires a point".into()))?
        .1
        .fs;
    let n = fs as usize;
    let df = fs as f64 / n as f64;
    if n < 4
        || points.len() > 2
        || points.iter().any(|(_, ir)| {
            ir.fs != fs || ir.data.is_empty() || ir.data.iter().any(|v| !v.is_finite())
        })
    {
        return Err(DspError::InvalidArgument("invalid tuning IR".into()));
    }
    let count = n / 2 + 1;
    let f: Vec<_> = (0..count).map(|k| k as f64 * df).collect();
    let spectra: Vec<_> = points
        .iter()
        .map(|(_, ir)| excess_spectrum(ir, n))
        .collect();
    let mut mags: Vec<Vec<f64>> = spectra.iter().map(|(_, m)| m.clone()).collect();
    if let Some(mic) = calibration {
        for m in &mut mags {
            for (k, v) in m.iter_mut().enumerate() {
                *v *= 10_f64.powf(-interp(f[k], &mic.frequency, &mic.raw) / 20.);
            }
        }
    }
    let mag: Vec<_> = (0..count)
        .map(|k| (mags.iter().map(|m| m[k].powi(2)).sum::<f64>() / mags.len() as f64).sqrt())
        .collect();
    let weights_level: Vec<_> = mags
        .iter()
        .map(|m| smooth_input_magnitude(m, 1. / 3.))
        .collect();
    let refs: Vec<_> = weights_level
        .iter()
        .map(|m| mean_band(m, df, 100., 10000.).max(1e-12))
        .collect();
    let phase_on = options.phase_limit != PhaseLimit::Off;
    let f_ph = options.phase_limit.frequency(fs, options.schroeder);
    let mut weights = vec![0.; count];
    let mut x = vec![Complex64::new(1., 0.); count];
    if phase_on {
        for k in 0..count {
            let r = weights_level
                .iter()
                .zip(&refs)
                .map(|(m, r)| ((m[k] / r - 0.177) / 0.5).clamp(0., 1.))
                .fold(1., f64::min);
            let mut g = r;
            let inverse = if points.len() == 1 {
                spectra[0].0[k].conj()
            } else {
                g *= ((120.
                    - (spectra[0].0[k] * spectra[1].0[k].conj())
                        .arg()
                        .abs()
                        .to_degrees())
                    / 60.)
                    .clamp(0., 1.);
                unit(weights_level[0][k] * spectra[0].0[k] + weights_level[1][k] * spectra[1].0[k])
                    .conj()
            };
            if f_ph < fs as f64 / 2. {
                let start = f_ph / SQRT_2;
                let t = ((f[k] - start) / (f_ph - start)).clamp(0., 1.);
                g *= 0.5 * (1. + (PI * t).cos());
            }
            weights[k] = g;
            x[k] = inverse * g + Complex64::new(1. - g, 0.);
        }
        x = five(&x, false, df).into_iter().map(unit).collect();
    }
    let pr_low = delay_ms.clamp(2., 25.);
    let pr_mid = 10_f64.min(pr_low / 2.);
    let pr_high = 5_f64.min(pr_low / 4.);
    let excess = if phase_on {
        let impulse = fft::irfft(&x, n);
        let copies =
            [pr_low, pr_mid, pr_high].map(|pre| fft::rfft(&window(&impulse, fs, pre, None)));
        (0..count)
            .map(|k| {
                let w = bands(f[k]);
                unit(
                    copies[0][k] * (w[0] + w[1])
                        + copies[1][k] * (w[2] + w[3])
                        + copies[2][k] * w[4],
                )
            })
            .collect::<Vec<_>>()
    } else {
        vec![Complex64::new(1., 0.); count]
    };
    let sm = RESOLUTIONS[0].map(|b| smooth_input_magnitude(&mag, 1. / b));
    let mut m: Vec<f64> = (0..count)
        .map(|k| {
            bands(f[k])
                .iter()
                .enumerate()
                .map(|(i, w)| sm[i][k] * w)
                .sum()
        })
        .collect();
    let bottom = m
        .iter()
        .enumerate()
        .filter(|(k, _)| (15. ..=20.).contains(&f[*k]))
        .map(|(_, v)| *v)
        .fold(0., f64::max);
    for (k, v) in m.iter_mut().enumerate() {
        if f[k] < 15. {
            *v = v.min(bottom);
        }
    }
    let reference = percentile(select(&mag, df, 100., 10000.), 0.35).max(1e-12);
    let db: Vec<_> = m.iter().map(|v| 20. * v.max(1e-12).log10()).collect();
    let low_ref = mean_band(&db, df, 1000., 10000.);
    let low_cutoff = (0..count)
        .find(|k| (10. ..=1000.).contains(&f[*k]) && db[*k] >= low_ref - 6.)
        .map_or(300., |k| f[k])
        .clamp(10., 300.);
    let high_ref = 20.
        * percentile(select(&m, df, 2000., 22050.), 0.95)
            .max(1e-12)
            .log10();
    let high_cutoff = (0..count)
        .rev()
        .find(|k| (2000. ..=22050.).contains(&f[*k]) && db[*k] >= high_ref - 10.)
        .map_or(16000., |k| f[k])
        .clamp(6000., 20000.);
    let thr = (mean_band(&m, df, 200., 1000.) * 10_f64.powf(-30. / 20.)).max(1e-12);
    let target_db: Vec<_> = f
        .iter()
        .map(|f| target_curve.map_or(0., |t| interp(*f, &t.frequency, &t.raw)))
        .collect();
    let target_center = mean_band(&target_db, df, 100., 10000.);
    if let Some(s) = stages.as_deref_mut() {
        s.smoothed_magnitude = m.clone();
        s.track_weight = vec![0.; count];
    }
    let mut target = vec![0.; count];
    let mut inv = vec![0.; count];
    let mut fade = vec![0.; count];
    for k in 0..count {
        let f = f[k];
        let tl = ((1.15 * low_cutoff - f) / (0.15 * low_cutoff)).clamp(0., 1.);
        let th = ((f - 0.8 * high_cutoff) / (0.2 * high_cutoff)).clamp(0., 1.);
        let tn = if f < 200. {
            ((2. * thr - m[k]) / thr).clamp(0., 1.)
        } else {
            0.
        };
        let t = (tl + th + tn).clamp(0., 1.);
        if let Some(s) = stages.as_deref_mut() {
            s.track_weight[k] = t;
        }
        let natural = (m[k] / reference).max(1e-12);
        let flat =
            f.powi(4) / (f.powi(4) + 1.) * 20000_f64.powi(4) / (f.powi(4) + 20000_f64.powi(4));
        target[k] = natural.min(1.).powf(t) * flat.powf(1. - t);
        if f > 1000. {
            target[k] = target[k].min(flat);
        }
        target[k] *= 10_f64.powf((target_db[k] - target_center) / 20.);
        inv[k] = (target[k] / natural).clamp(0., 10_f64.powf(options.max_boost / 20.));
        let end = (2. * options.curtain).min(fs as f64 / 2. - 1.);
        fade[k] = ((f - options.curtain) / (end - options.curtain).max(1.)).clamp(0., 1.);
    }
    inv[0] = inv[1];
    for k in 0..count {
        inv[k] = inv[k] * (1. - fade[k]) + fade[k];
    }
    let minimum = minimum_phase(&inv, n);
    let hm = if phase_on {
        let impulse = fft::irfft(&minimum, n);
        let long = fft::rfft(&window(&impulse, fs, pr_low, Some(500.)));
        let short = fft::rfft(&window(&impulse, fs, pr_low, Some(10.)));
        (0..count)
            .map(|k| long[k] * (1. - fade[k]) + short[k] * fade[k])
            .collect::<Vec<_>>()
    } else {
        minimum
    };
    if let Some(s) = stages.as_deref_mut() {
        s.windowed_minimum = hm.iter().map(|v| v.norm()).collect();
    }
    let delay = if phase_on {
        (delay_ms * fs as f64 / 1000.).round() as usize
    } else {
        0
    };
    let initial: Vec<_> = (0..count)
        .map(|k| {
            excess[k]
                * hm[k]
                * Complex64::from_polar(1., -2. * PI * k as f64 * delay as f64 / n as f64)
        })
        .collect();
    let sim: Vec<_> = (0..count).map(|k| mag[k] * initial[k].norm()).collect();
    let tonal = five_real(&sim, true, df);
    let macro_raw: Vec<_> = (0..count)
        .map(|k| {
            let v = (reference * target[k] / tonal[k].max(1e-12)).clamp(0.5, 2.);
            if v > 1. {
                1. + (v - 1.) * ((20000. - f[k]) / 10000.).clamp(0., 1.)
            } else {
                v
            }
        })
        .collect();
    let macro_gain = smooth_real(&macro_raw, 1.);
    let residual: Vec<_> = (0..count)
        .map(|k| (sim[k] * macro_gain[k] / (reference * target[k]).max(1e-12)).max(1.))
        .collect();
    let peaks = smooth_real(&residual, 1. / 48.);
    let mut post: Vec<_> = (0..count)
        .map(|k| {
            let cw = ((500. - f[k]) / 200.).clamp(0., 1.);
            macro_gain[k] * ((1. - cw) + (1. / peaks[k]).powf(0.6) * cw)
        })
        .collect();
    if let Some(s) = stages {
        s.crush = post.iter().zip(&macro_gain).map(|(p, m)| p / m).collect();
    }
    post[0] = post[1];
    let post = minimum_phase(&post, n);
    let full_filter = fft::irfft(
        &initial
            .iter()
            .zip(post)
            .map(|(a, b)| a * b)
            .collect::<Vec<_>>(),
        n,
    );
    let mut filter = full_filter[..((0.51 * fs as f64).round() as usize).min(n)].to_vec();
    let fade_len = (0.005 * fs as f64).round() as usize;
    let start = filter.len().saturating_sub(fade_len);
    for (i, v) in filter[start..].iter_mut().enumerate() {
        *v *= (1. - i as f64 / fade_len as f64).powi(2);
    }
    let corrected_share =
        weights.iter().filter(|v| **v >= 0.5).count() as f64 / weights.len() as f64;
    Ok(SpeakerTuning {
        speaker: speaker.into(),
        points: points.len(),
        reference,
        low_cutoff,
        high_cutoff,
        trim_db: 0.,
        trim_source: "skipped",
        level_difference_db: 0.,
        corrected_share,
        pre_echo_db: -600.,
        pre_echo_channels: Vec::new(),
        design_origin: delay,
        target,
        phase_corrected: phase_on,
        mismatch: false,
        lf_edt_before_ms: 0.,
        lf_edt_after_ms: 0.,
        excess_median_before_ms: 0.,
        excess_median_after_ms: 0.,
        representation_rms_db: Vec::new(),
        filter,
        weights,
        full_filter,
        excess,
        inverse: inv,
        macro_gain,
        omni: points.to_vec(),
    })
}

pub fn prepare_tuning(
    hrir: &Hrir,
    options: &TuningOptions,
    calibration: Option<&FrequencyResponse>,
    target: Option<&FrequencyResponse>,
    pairs: &[PairMeasurement],
) -> Result<TuningPlan, DspError> {
    let build = |d| {
        hrir.speakers
            .iter()
            .filter(|s| s.left.is_some() || s.right.is_some())
            .map(|s| {
                let points: Vec<_> = [(Side::Left, &s.left), (Side::Right, &s.right)]
                    .into_iter()
                    .filter_map(|(side, ir)| ir.clone().map(|ir| (side, ir)))
                    .collect();
                design_speaker(&s.speaker, &points, options, d, calibration, target)
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let mut scores = Vec::new();
    let mut file_scores: Vec<_> = pairs
        .iter()
        .map(|p| AutoDelayScores {
            speakers: p.speakers.clone(),
            side: p.side,
            metrics: Vec::new(),
        })
        .collect();
    let delay_ms = match options.delay {
        TuningDelay::Ms(d) => d,
        TuningDelay::Auto if options.phase_limit == PhaseLimit::Off => 0.,
        TuningDelay::Auto => {
            let mut best = (10, f64::INFINITY);
            for d in 2..=10 {
                if !pairs.is_empty() {
                    let mut score = 0.;
                    for (pair, curve) in pairs.iter().zip(&mut file_scores) {
                        let metric =
                            auto_delay_metric(pair, options, d as f64, calibration, target)?;
                        curve.metrics.push((d, metric));
                        score += metric;
                    }
                    scores.push((d, score));
                    if score < best.1 {
                        best = (d, score);
                    }
                }
            }
            best.0 as f64
        }
    };
    let speakers = build(delay_ms)?;
    let delay_samples = if speakers.is_empty() || options.phase_limit == PhaseLimit::Off {
        0
    } else {
        (delay_ms * hrir.fs as f64 / 1000.).round() as usize
    };
    Ok(TuningPlan {
        fs: hrir.fs,
        report: TuningReport {
            fs: hrir.fs,
            f_ph: options.phase_limit.frequency(hrir.fs, options.schroeder),
            delay_samples,
            speakers,
            auto_scores: scores,
            auto_file_scores: file_scores,
            pair_checks: Vec::new(),
        },
        level_match: options.level_match,
        pairs: pairs.to_vec(),
    })
}
/// SECS stereo scoring only: common-start inputs, gain/arrival matching and joint crop.
/// None of these pair-specific shifts or gains is applied to the production FIRs.
pub fn auto_delay_metric(
    pair: &PairMeasurement,
    options: &TuningOptions,
    delay_ms: f64,
    calibration: Option<&FrequencyResponse>,
    target: Option<&FrequencyResponse>,
) -> Result<f64, DspError> {
    let fs = pair.irs[0].fs;
    let n = fs as usize;
    let start = peak(&pair.irs[0].data)
        .min(peak(&pair.irs[1].data))
        .saturating_sub((0.001 * fs as f64).round() as usize);
    let irs = pair.irs.each_ref().map(|ir| {
        let mut data = ir.data.get(start..).unwrap_or_default().to_vec();
        data.resize(n, 0.);
        ImpulseResponse {
            fs: ir.fs,
            data,
            recording: None,
        }
    });
    let scoring = TuningOptions {
        phase_limit: PhaseLimit::Full,
        level_match: false,
        ..options.clone()
    };
    let a = design_speaker(
        &pair.speakers[0],
        &[(pair.side, irs[0].clone())],
        &scoring,
        delay_ms,
        calibration,
        target,
    )?;
    let b = design_speaker(
        &pair.speakers[1],
        &[(pair.side, irs[1].clone())],
        &scoring,
        delay_ms,
        calibration,
        target,
    )?;
    let peaks = irs.each_ref().map(|ir| peak(&ir.data));
    let width = (0.005 * fs as f64).round() as usize;
    let rms = |i: usize| {
        let values = &irs[i].data[peaks[i].saturating_sub(width)..(peaks[i] + width).min(n)];
        (values.iter().map(|v| v * v).sum::<f64>() / values.len() as f64).sqrt()
    };
    let gain = rms(0) / rms(1).max(1e-12);
    let latest = peaks[0].max(peaks[1]);
    let shifted: [Vec<f64>; 2] = [&a.full_filter, &b.full_filter]
        .into_iter()
        .enumerate()
        .map(|(i, filter)| {
            let shift = latest - peaks[i];
            (0..n)
                .map(|j| {
                    j.checked_sub(shift).map_or(0., |j| filter[j]) * if i == 1 { gain } else { 1. }
                })
                .collect()
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let q = shifted
        .iter()
        .flat_map(|f| f.iter().enumerate())
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .unwrap()
        .0;
    // Scoring uses truncation after seconds-to-samples conversion (9 ms at
    // 48 kHz becomes 431). The production design origin remains rounded D_s.
    let crop_start = q as isize - (delay_ms / 1000. * fs as f64) as isize;
    let filters = shifted.each_ref().map(|f| {
        let crop: Vec<_> = (0..(0.51 * fs as f64).round() as usize)
            .map(|i| {
                let j = crop_start + i as isize;
                if j < 0 {
                    0.
                } else {
                    f.get(j as usize).copied().unwrap_or(0.)
                }
            })
            .collect();
        spectrum(&crop, n)
    });
    let h = irs.each_ref().map(|ir| spectrum(&ir.data, n));
    let summed: Vec<_> = (0..h[0].len())
        .map(|k| (h[0][k] * filters[0][k] + h[1][k] * filters[1][k]).norm())
        .collect();
    let after = five_real(&summed, false, 1.);
    let lo = ((a.low_cutoff + b.low_cutoff) / 2.).max(10.);
    // The black-box cutoff carries a positive sub-bin epsilon, so the lower
    // scoring edge is open, including when the average cutoff is an integer.
    let errors: Vec<_> = (lo.floor() as usize + 1..=300.min(after.len() - 1))
        .map(|k| {
            let w = 1. + ((300. - k as f64) / (300. - lo).max(1e-12)).clamp(0., 1.);
            let e = 20. * (after[k] + 1e-12).log10()
                - (20. * (a.reference * a.target[k]).max(1e-12).log10() + 6.);
            (w, e)
        })
        .collect();
    let weight = errors.iter().map(|v| v.0).sum::<f64>();
    let mean = errors.iter().map(|(w, e)| w * e).sum::<f64>() / weight;
    Ok(errors
        .iter()
        .map(|(w, e)| w * (e.abs() + 0.35 * (e - mean).abs()))
        .sum::<f64>()
        / weight)
}
/// Preserve only within-recording comparisons; independent recordings share no clock.
pub fn recording_pairs(hrir: &Hrir, names: &[String], side: Option<Side>) -> Vec<PairMeasurement> {
    let mut out = Vec::new();
    if names.len() != 2 {
        return out;
    }
    for i in 0..names.len() {
        for j in i + 1..names.len() {
            for ear in [Side::Left, Side::Right] {
                if side.is_some_and(|s| s != ear) {
                    continue;
                }
                let get = |name: &str| {
                    hrir.get(name)
                        .and_then(|s| {
                            if ear == Side::Left {
                                s.left.as_ref()
                            } else {
                                s.right.as_ref()
                            }
                        })
                        .cloned()
                };
                if let (Some(a), Some(b)) = (get(&names[i]), get(&names[j])) {
                    out.push(PairMeasurement {
                        speakers: [names[i].clone(), names[j].clone()],
                        side: ear,
                        irs: [a, b],
                    });
                }
            }
        }
    }
    out
}
pub fn geometry_check(pair: &PairMeasurement) -> Option<PairCheck> {
    let &(left, right) = IPSILATERAL_PAIRS.iter().find(|(a, b)| {
        a != b
            && ((pair.speakers[0] == *a && pair.speakers[1] == *b)
                || (pair.speakers[0] == *b && pair.speakers[1] == *a))
    })?;
    let arrivals = pair.irs.each_ref().map(|ir| peak(&ir.data));
    let delta = (arrivals[1] as f64 - arrivals[0] as f64) * 1000. / pair.irs[0].fs as f64;
    let expected_first = if pair.side == Side::Left { left } else { right };
    let mismatch = delta.abs() > 0.05 && ((delta > 0.) != (pair.speakers[0] == expected_first));
    Some(PairCheck {
        speakers: pair.speakers.clone(),
        side: pair.side,
        omni_ms: delta,
        mismatch,
    })
}
pub fn excess_group_delay_median(ir: &ImpulseResponse) -> f64 {
    let (e, _) = excess_spectrum(ir, ir.fs as usize);
    let e = smooth_bins(&e, 1. / 6.);
    let values: Vec<_> = (30..=300.min(e.len() - 2))
        .map(|k| {
            // Central difference of the locally unwrapped complex-smoothed phase.
            let phase = (e[k] * e[k - 1].conj()).arg() + (e[k + 1] * e[k].conj()).arg();
            (phase * 1000. / (4. * PI)).abs()
        })
        .collect();
    percentile(values, 0.5)
}
pub const EDT_BANDS: [f64; 7] = [40., 50., 63., 80., 100., 125., 160.];
/// Zero-phase, fourth-order Butterworth prototype transformed to a band-pass.
/// Squared digital magnitude implements forward/backward filtering; zero padding
/// prevents the finite impulse's tail from wrapping into the decay fit.
pub fn lf_decay_bands(ir: &ImpulseResponse) -> [f64; 7] {
    let n = (2 * ir.data.len().max(ir.fs as usize)).next_power_of_two();
    let h = spectrum(&ir.data, n);
    let start = peak(&ir.data);
    EDT_BANDS.map(|fc| {
        let edge = 2_f64.powf(1. / 6.);
        let lo = (PI * fc / edge / ir.fs as f64).tan();
        let hi = (PI * fc * edge / ir.fs as f64).tan();
        let filtered: Vec<_> = h
            .iter()
            .enumerate()
            .map(|(k, v)| {
                let omega = (PI * k as f64 / n as f64).tan();
                let gain = if k == 0 || k == n / 2 {
                    0.
                } else {
                    let ratio = (omega * omega - lo * hi) / ((hi - lo) * omega);
                    1. / (1. + ratio.powi(8))
                };
                v * gain
            })
            .collect();
        let data = fft::irfft(&filtered, n);
        let mut energy = vec![0.; ir.data.len() - start];
        let mut total = 0.;
        for (out, v) in energy
            .iter_mut()
            .rev()
            .zip(data[start..ir.data.len()].iter().rev())
        {
            total += v * v;
            *out = total;
        }
        if total <= 1e-30 {
            return 0.;
        }
        let mut count = 0.;
        let mut sx = 0.;
        let mut sy = 0.;
        let mut sxx = 0.;
        let mut sxy = 0.;
        for (i, e) in energy.iter().enumerate() {
            let db = 10. * (e / total).max(1e-30).log10();
            if db < -10. {
                break;
            }
            let t = i as f64 / ir.fs as f64;
            count += 1.;
            sx += t;
            sy += db;
            sxx += t * t;
            sxy += t * db;
        }
        let denominator = count * sxx - sx * sx;
        if denominator <= 0. {
            return 0.;
        }
        let slope = (count * sxy - sx * sy) / denominator;
        if slope < 0. { -60000. / slope } else { 0. }
    })
}
impl SpeakerTuning {
    pub fn weak(&self) -> bool {
        self.representation_mean_db().is_some_and(|v| v > 4.)
    }
    pub fn representation_mean_db(&self) -> Option<f64> {
        (!self.representation_rms_db.is_empty()).then(|| {
            self.representation_rms_db
                .iter()
                .map(|(_, v)| v)
                .sum::<f64>()
                / self.representation_rms_db.len() as f64
        })
    }
    pub fn pre_echo_warning_db(&self) -> Option<f64> {
        self.pre_echo_channels
            .iter()
            .filter(|v| v.warns())
            .map(|v| v.after_db)
            .max_by(f64::total_cmp)
    }
}
/// Largest level before the direct sound, relative to it. The last 2 ms
/// before the direct sound are excluded: a full-band mixed-phase correction
/// applied at the ear always leaves −20 to −45 dB there (SECS's own filter
/// does the same on the demo BRIRs), heard as a softer onset, not as an echo.
pub fn pre_echo(ir: &ImpulseResponse) -> f64 {
    let (before, direct) = pre_echo_parts(ir);
    level_db(before, direct)
}
/// Side, then `pre_echo_parts` before and after tuning.
type EarEcho = (Side, (f64, f64), (f64, f64));
/// `(largest |h| before the guard, |h| at the direct sound)` of one ear.
fn pre_echo_parts(ir: &ImpulseResponse) -> (f64, f64) {
    if ir.data.is_empty() {
        return (0., 0.);
    }
    let p = crate::peaks::first_peak_index(&ir.data, 0, None, 0.12589);
    let guard = (0.002 * ir.fs as f64).round() as usize;
    let before = p.checked_sub(guard).map_or(0., |end| {
        ir.data[..=end].iter().map(|v| v.abs()).fold(0., f64::max)
    });
    (before, ir.data[p].abs())
}
fn level_db(value: f64, reference: f64) -> f64 {
    if reference <= 0. {
        return -600.;
    }
    20. * (value / reference.max(1e-12)).max(1e-30).log10()
}
pub fn level_trims(references: &[f64], ear_levels: &[f64]) -> Vec<(f64, bool, f64)> {
    let reference = percentile(references.to_vec(), 0.5);
    let ear_ref = percentile(ear_levels.to_vec(), 0.5);
    references
        .iter()
        .zip(ear_levels)
        .map(|(r, e)| {
            let omni = 20. * (reference / r.max(1e-12)).log10();
            let ear = 20. * (ear_ref / e.max(1e-12)).log10();
            let diff = omni - ear;
            (
                if diff.abs() > 2. {
                    0.
                } else {
                    omni.clamp(-6., 6.)
                },
                diff.abs() > 2.,
                diff,
            )
        })
        .collect()
}
pub fn apply_tuning(hrir: &mut Hrir, plan: &TuningPlan) -> Result<TuningReport, DspError> {
    if hrir.fs != plan.fs {
        return Err(DspError::InvalidArgument(
            "tuning sample rate differs".into(),
        ));
    }
    let mut report = plan.report.clone();
    report.speakers.retain(|s| hrir.get(&s.speaker).is_some());
    if report.speakers.is_empty() {
        report.delay_samples = 0;
        return Ok(report);
    }
    for pair in &plan.pairs {
        if let Some(check) = geometry_check(pair) {
            if check.mismatch {
                for s in &mut report.speakers {
                    if pair.speakers.contains(&s.speaker) {
                        s.mismatch = true;
                    }
                }
            }
            report.pair_checks.push(check);
        }
    }
    let refs: Vec<_> = report.speakers.iter().map(|s| s.reference).collect();
    let ear_levels: Vec<_> = report
        .speakers
        .iter()
        .map(|s| {
            let ears = hrir.get(&s.speaker).unwrap();
            ears.left
                .iter()
                .chain(&ears.right)
                .map(|ir| {
                    spectrum(&ir.data, hrir.fs as usize)[100..501.min(hrir.fs as usize / 2 + 1)]
                        .iter()
                        .map(|v| v.norm_sqr())
                        .sum::<f64>()
                })
                .sum::<f64>()
                .sqrt()
        })
        .collect();
    let trims = level_trims(&refs, &ear_levels);
    for (s, (trim, skipped, diff)) in report.speakers.iter_mut().zip(trims) {
        if plan.level_match {
            s.trim_db = trim;
            s.trim_source = if skipped { "skipped" } else { "omni" };
            s.level_difference_db = diff;
            let gain = 10_f64.powf(trim / 20.);
            for v in &mut s.filter {
                *v *= gain;
            }
        }
    }
    for speaker in &mut hrir.speakers {
        let mut entry = report
            .speakers
            .iter_mut()
            .find(|s| s.speaker == speaker.speaker);
        let mut decay_before = [0.; 7];
        let mut decay_after = [0.; 7];
        let mut ear_count = 0.;
        // Per ear (pre-echo, direct) before and after tuning. The pre-echo of
        // a speaker is heard against its louder direct sound, so both ears are
        // referred to the larger of the two direct levels: the head-shadowed
        // contralateral direct sound would otherwise inflate the ratio.
        let mut echo_parts: Vec<EarEcho> = Vec::new();
        for (side, ir) in [
            (Side::Left, &mut speaker.left),
            (Side::Right, &mut speaker.right),
        ] {
            let Some(ir) = ir else {
                continue;
            };
            let length = ir.data.len() + report.delay_samples;
            if let Some(s) = entry.as_deref_mut() {
                ear_count += 1.;
                let before_parts = pre_echo_parts(ir);
                s.excess_median_before_ms += excess_group_delay_median(ir);
                for (sum, value) in decay_before.iter_mut().zip(lf_decay_bands(ir)) {
                    *sum += value;
                }
                if let Some((_, omni)) = s.omni.iter().find(|(e, _)| *e == side) {
                    let mag = |ir: &ImpulseResponse| {
                        smooth_real(
                            &spectrum(&ir.data, ir.fs as usize)
                                .iter()
                                .map(|v| 20. * v.norm().max(1e-12).log10())
                                .collect::<Vec<_>>(),
                            1. / 6.,
                        )
                    };
                    let a = mag(omni);
                    let b = mag(ir);
                    let diff: Vec<_> = a.iter().zip(b).map(|(a, b)| a - b).collect();
                    let center = mean_band(&diff, 1., 100., 300.);
                    let rms = (diff[30..301]
                        .iter()
                        .map(|v| (v - center).powi(2))
                        .sum::<f64>()
                        / 271.)
                        .sqrt();
                    s.representation_rms_db.push((side, rms));
                }
                ir.data = conv::convolve(&ir.data, &s.filter, conv::Mode::Full);
                ir.data.resize(length, 0.);
                let len = (0.005 * hrir.fs as f64).round() as usize;
                let start = ir.data.len().saturating_sub(len);
                for (i, v) in ir.data[start..].iter_mut().enumerate() {
                    *v *= 0.5 * (1. + (PI * i as f64 / len as f64).cos());
                }
                s.excess_median_after_ms += excess_group_delay_median(ir);
                for (sum, value) in decay_after.iter_mut().zip(lf_decay_bands(ir)) {
                    *sum += value;
                }
                echo_parts.push((side, before_parts, pre_echo_parts(ir)));
            } else if report.delay_samples > 0 {
                let mut data = vec![0.; report.delay_samples];
                data.extend_from_slice(&ir.data);
                ir.data = data;
            }
        }
        if let Some(s) = entry
            && ear_count > 0.
        {
            let reference_before = echo_parts.iter().map(|e| e.1.1).fold(0., f64::max);
            let reference_after = echo_parts.iter().map(|e| e.2.1).fold(0., f64::max);
            for (side, before, after) in &echo_parts {
                let after_db = level_db(after.0, reference_after);
                s.pre_echo_db = s.pre_echo_db.max(after_db);
                s.pre_echo_channels.push(PreEcho {
                    side: *side,
                    before_db: level_db(before.0, reference_before),
                    after_db,
                });
            }
            s.lf_edt_before_ms =
                percentile(decay_before.iter().map(|v| v / ear_count).collect(), 0.5);
            s.lf_edt_after_ms =
                percentile(decay_after.iter().map(|v| v / ear_count).collect(), 0.5);
            s.excess_median_before_ms /= ear_count;
            s.excess_median_after_ms /= ear_count;
        }
    }
    Ok(report)
}
