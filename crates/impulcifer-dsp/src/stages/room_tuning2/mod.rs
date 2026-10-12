//! Virtual room tuning v2 channel correction, independently implemented from
//! docs/rust/ROOM_CORRECTION.md (ADR 0008); no SECS implementation is included.
mod peq;
mod primitives;
mod rc;
use crate::{DspError, fft::Complex64, filters, fr::FrequencyResponse, ir::ImpulseResponse};
use primitives::*;
pub use rc::design_rc;
use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tuning2Mode {
    Zero,
    Mixed,
    Linear,
    Long,
    Ultra(f64),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModeParams {
    pub taps: usize,
    pub peq_bands: usize,
    pub lift_ms: f64,
    pub phase_frac: f64,
}
impl Tuning2Mode {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "zero" => Some(Self::Zero),
            "mixed" => Some(Self::Mixed),
            "linear" => Some(Self::Linear),
            "long" => Some(Self::Long),
            _ => s
                .parse::<f64>()
                .ok()
                .filter(|x| x.is_finite() && (30. ..=200.).contains(x))
                .map(Self::Ultra),
        }
    }
    pub fn params(&self, fs: u32) -> ModeParams {
        let (taps, bands, lift, beta) = match *self {
            Self::Zero => (0, 32, 0., 0.),
            Self::Mixed => (2048, 16, 8., 6.),
            Self::Linear => (2048, 16, 28., 24.),
            Self::Long => (32768, 16, 200., 6.),
            Self::Ultra(ms) => {
                let mut n = 2048;
                while n < 16384 && 0.7 * n as f64 / 48. < ms - 2. {
                    n *= 2;
                }
                (n, 16, ms - 2., 24.)
            }
        };
        let taps = (taps as f64 * fs as f64 / 48000.).round_ties_even() as usize;
        ModeParams {
            taps,
            peq_bands: bands,
            lift_ms: lift.min(0.7 * taps as f64 / fs as f64 * 1000.),
            phase_frac: beta,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum PositionPolicy {
    Symmetric,
    Listening { index: usize, weight: f64 },
}
#[derive(Clone, Debug)]
pub struct RcOptions {
    pub mode: Tuning2Mode,
    pub max_boost: f64,
    pub low: f64,
    pub high: Option<f64>,
    pub schroeder: Option<f64>,
    pub room_volume: Option<f64>,
    pub policy: PositionPolicy,
}
impl Default for RcOptions {
    fn default() -> Self {
        Self {
            mode: Tuning2Mode::Mixed,
            max_boost: 10.,
            low: 20.,
            high: None,
            schroeder: None,
            room_volume: None,
            policy: PositionPolicy::Symmetric,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BandSource {
    Manual,
    Override,
    Volume,
    Default,
}
#[derive(Clone, Debug)]
pub struct CorrectionBand {
    pub low: f64,
    pub high: f64,
    pub source: BandSource,
    pub schroeder: Option<f64>,
}
#[derive(Clone, Debug)]
pub struct SpeakerRc {
    pub points: usize,
    pub level_db: f64,
    pub design_db: Vec<f64>,
    pub peq: Vec<(f64, f64, f64)>,
    pub peq_fits: Vec<Vec<(f64, f64, f64)>>,
    pub fir: Vec<f64>,
    pub phase_strength: f64,
    pub phase_frac: f64,
    pub long_modes: Vec<(f64, f64)>,
    pub response: Vec<Complex64>,
    pub lift_samples: usize,
}
pub fn analysis_len(fs: u32) -> usize {
    (131072. * fs as f64 / 48000. / 2.).round_ties_even() as usize * 2
}
fn chosen(policy: &PositionPolicy, n: usize) -> Vec<usize> {
    match policy {
        PositionPolicy::Symmetric => (0..n).collect(),
        PositionPolicy::Listening { index, .. } => vec![*index],
    }
}
fn weights(policy: &PositionPolicy, n: usize) -> Vec<f64> {
    let mut w = vec![1.; n];
    if let PositionPolicy::Listening { index, weight } = policy {
        if *index < n {
            w[*index] = *weight;
        }
    }
    w
}
fn curve(fr: Option<&FrequencyResponse>, f: &[f64]) -> Vec<f64> {
    match fr {
        None => vec![0.; f.len()],
        Some(fr) => f
            .iter()
            .map(|f| interp(*f, &fr.frequency, &fr.raw))
            .collect(),
    }
}
fn spectra(
    points: &[ImpulseResponse],
    calibration: Option<&FrequencyResponse>,
) -> (Vec<f64>, Vec<Vec<Complex64>>) {
    let fs = points[0].fs as f64;
    let n = analysis_len(points[0].fs);
    let f = grid(n, fs);
    let b = onset(&points[0].data);
    let cal = calibration.map(|c| {
        mp(
            &curve(Some(c), &f)
                .iter()
                .map(|x| 10_f64.powf(x / 20.))
                .collect::<Vec<_>>(),
            n,
        )
    });
    let h = points
        .iter()
        .map(|ir| {
            let a = onset(&ir.data).saturating_sub((0.002 * fs) as usize);
            let end = (a + (0.5 * fs) as usize).min(ir.len());
            let mut w = ir.data[a..end].to_vec();
            taper(&mut w, 0.2);
            let mut h = rfft(&w, n);
            for (i, h) in h.iter_mut().enumerate() {
                *h *= Complex64::from_polar(1., -2. * PI * f[i] * (a as f64 - b as f64) / fs);
                if let Some(c) = &cal {
                    *h /= c[i];
                }
            }
            h
        })
        .collect();
    (f, h)
}
fn power(h: &[Vec<Complex64>], policy: &PositionPolicy) -> Vec<f64> {
    let w = weights(policy, h.len());
    let total = w.iter().sum::<f64>();
    let mut p = vec![0.; h[0].len()];
    for (h, w) in h.iter().zip(w) {
        for (p, h) in p.iter_mut().zip(h) {
            *p += w * h.norm_sqr();
        }
    }
    for p in &mut p {
        *p /= total;
    }
    p
}
fn level(p: &[f64], f: &[f64], target: Option<&FrequencyResponse>) -> f64 {
    let t = curve(target, f);
    median(
        &f.iter()
            .enumerate()
            .filter(|(_, f)| **f >= 300. && **f <= 3000.)
            .map(|(i, _)| 10. * (p[i] + 1e-30).log10() - t[i])
            .collect::<Vec<_>>(),
    )
}
/// Invalid inputs return NaN; use design_rc for the fallible entry point.
pub fn channel_level(
    points: &[ImpulseResponse],
    policy: &PositionPolicy,
    target: Option<&FrequencyResponse>,
    calibration: Option<&FrequencyResponse>,
) -> f64 {
    let opts = RcOptions {
        policy: *policy,
        ..Default::default()
    };
    if validate(points, &opts, target, calibration).is_err() {
        return f64::NAN;
    }
    let (f, h) = spectra(points, calibration);
    level(&power(&h, policy), &f, target)
}
/// All measured speakers contribute to the automatic decay estimate. Invalid
/// inputs produce a NaN high edge; design_rc rejects that band.
pub fn correction_band(points: &[&[ImpulseResponse]], opts: &RcOptions) -> CorrectionBand {
    if points.is_empty()
        || points
            .iter()
            .any(|p| validate(p, opts, None, None).is_err())
        || points.iter().any(|p| p[0].fs != points[0][0].fs)
    {
        return CorrectionBand {
            low: opts.low,
            high: f64::NAN,
            source: BandSource::Default,
            schroeder: None,
        };
    }
    if let Some(high) = opts.high {
        return CorrectionBand {
            low: opts.low,
            high,
            source: BandSource::Manual,
            schroeder: None,
        };
    }
    let mut source = BandSource::Default;
    let mut schroeder = opts.schroeder;
    if schroeder.is_some() {
        source = BandSource::Override;
    } else if let Some(volume) = opts.room_volume {
        let mut times = Vec::new();
        for points in points {
            for index in chosen(&opts.policy, points.len()) {
                let ir = &points[index];
                for fc in [500., 1000.] {
                    let y = filters::sosfilt(
                        &filters::butter_bandpass(
                            4,
                            fc / 2_f64.sqrt() / (ir.fs as f64 / 2.),
                            fc * 2_f64.sqrt() / (ir.fs as f64 / 2.),
                        ),
                        &ir.data,
                    );
                    let mut e = vec![0.; y.len()];
                    let mut sum = 0.;
                    for i in (0..y.len()).rev() {
                        sum += y[i] * y[i];
                        e[i] = sum;
                    }
                    let d: Vec<_> = e
                        .iter()
                        .map(|e| 10. * (e / (sum + 1e-30) + 1e-30).log10())
                        .collect();
                    let i5 = d.iter().position(|d| *d < -5.).unwrap_or(0);
                    let i25 = d.iter().position(|d| *d < -25.).unwrap_or(0);
                    if i25 > i5 && i5 > 0 {
                        times.push(3. * (i25 - i5) as f64 / ir.fs as f64);
                    }
                }
            }
        }
        if !times.is_empty() {
            schroeder = Some(2000. * (median(&times) / volume).sqrt());
            source = BandSource::Volume;
        }
    }
    CorrectionBand {
        low: opts.low,
        high: schroeder
            .filter(|s| *s <= 500.)
            .map(|s| s.max(100.))
            .unwrap_or(500.),
        source,
        schroeder,
    }
}
fn validate(
    points: &[ImpulseResponse],
    opts: &RcOptions,
    target: Option<&FrequencyResponse>,
    cal: Option<&FrequencyResponse>,
) -> Result<(), DspError> {
    let invalid = || DspError::InvalidArgument("invalid tuning2 measurements/options/curve".into());
    if points.is_empty()
        || points[0].fs < 44100
        || points.iter().any(|p| {
            p.fs != points[0].fs || p.data.is_empty() || p.data.iter().any(|x| !x.is_finite())
        })
    {
        return Err(invalid());
    }
    if !opts.max_boost.is_finite()
        || !(0. ..=10.).contains(&opts.max_boost)
        || !opts.low.is_finite()
        || !(10. ..=200.).contains(&opts.low)
        || opts
            .high
            .is_some_and(|v| !v.is_finite() || !(100. ..=20000.).contains(&v) || v <= opts.low)
        || [opts.schroeder, opts.room_volume]
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || *v <= 0.)
    {
        return Err(invalid());
    }
    if let Tuning2Mode::Ultra(v) = opts.mode {
        if !v.is_finite() || !(30. ..=200.).contains(&v) {
            return Err(invalid());
        }
    }
    if let PositionPolicy::Listening { index, weight } = opts.policy {
        if index >= points.len() || !weight.is_finite() || weight <= 0. {
            return Err(invalid());
        }
    }
    for fr in [target, cal].into_iter().flatten() {
        if fr.frequency.is_empty()
            || fr.frequency.len() != fr.raw.len()
            || fr.frequency.iter().chain(&fr.raw).any(|x| !x.is_finite())
            || fr.frequency.windows(2).any(|w| w[0] >= w[1])
        {
            return Err(invalid());
        }
    }
    Ok(())
}
