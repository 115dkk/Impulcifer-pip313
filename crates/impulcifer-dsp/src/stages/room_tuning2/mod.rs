//! Virtual room tuning v2 channel correction, independently implemented from
//! docs/rust/ROOM_CORRECTION.md (ADR 0008); no SECS implementation is included.
mod apply;
pub use apply::{
    PlannedSpeaker, Speaker2Report, Tuning2Options, Tuning2Plan, Tuning2Report, apply_tuning2,
    apply_tuning2_full, prepare_tuning2,
};
pub mod mimo;
pub use mimo::{
    MimoDesign, MimoInput, MimoInputResult, MimoOptions, MimoSupport, design_supports,
    support_response,
};
pub mod peq;
pub mod primitives;
pub mod profile;
pub mod protect;
mod rc;
use crate::{DspError, fft::Complex64, filters, fr::FrequencyResponse, ir::ImpulseResponse};
use primitives::*;
pub use rc::{
    DipLimitResult, ModeDecayStages, RcSession, RcStages, RealizeDiagnostics, ResonanceRefinement,
    blend_lf_excess, design_rc, design_rc_with_stages, dip_limit_curve, limited_dips,
    realize_for_fixture, refine_resonance, select_mode_decay_cuts, smoothed_phase_delay,
};
use std::f64::consts::PI;

pub use impulcifer_types::config::{ModeParams, Tuning2Mode};
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
    pub dip_limit: bool,
    pub low_protect: bool,
    pub extra_bands: Vec<(f64, f64)>,
    pub lf_align: bool,
    pub mode_decay: bool,
    pub hf_phase: bool,
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
            dip_limit: false,
            low_protect: false,
            extra_bands: Vec::new(),
            lf_align: false,
            mode_decay: false,
            hf_phase: false,
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
    pub bands: Vec<(f64, f64)>,
}
#[derive(Clone, Debug)]
pub struct LfAlignResult {
    pub f_lin_hz: f64,
    pub arrival_ms: Option<f64>,
    pub limited: bool,
}

#[derive(Clone, Debug)]
pub struct ModeDecayResult {
    pub target_t60_s: f64,
    pub cuts: Vec<ModeDecayCut>,
}

#[derive(Clone, Debug)]
pub struct HfPhaseResult {
    pub range_hz: Option<(f64, f64)>,
    pub rotation_deg: Option<f64>,
    pub limited: bool,
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
    pub phase_candidates: Vec<(f64, f64)>,
    pub long_modes: Vec<(f64, f64)>,
    pub response: Vec<Complex64>,
    pub lift_samples: usize,
    pub rolloff_hz: Option<f64>,
    pub limit_outcome: protect::LimitOutcome,
    pub low_limited: bool,
    pub limited_dips: Vec<(f64, f64, f64)>,
    pub bands: Vec<(f64, f64)>,
    pub lf_align: Option<LfAlignResult>,
    pub mode_decay: Option<ModeDecayResult>,
    pub hf_phase: Option<HfPhaseResult>,
}

fn merge_bands(mut bands: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    bands.sort_by(|left, right| left.0.total_cmp(&right.0).then(left.1.total_cmp(&right.1)));
    let mut merged: Vec<(f64, f64)> = Vec::with_capacity(bands.len());
    for (low, high) in bands {
        if let Some(previous) = merged.last_mut()
            && low <= previous.1
        {
            previous.1 = previous.1.max(high);
        } else {
            merged.push((low, high));
        }
    }
    merged
}

/// Sort and merge the primary correction band with optional extra bands.
pub fn merge_band_set(primary: (f64, f64), extras: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut bands = Vec::with_capacity(extras.len() + 1);
    bands.push(primary);
    bands.extend_from_slice(extras);
    merge_bands(bands)
}

/// Frequency fade used by the tuning2 design curve over the merged band set.
pub fn magnitude_mask(frequencies: &[f64], bands: &[(f64, f64)], fs: f64) -> Vec<f64> {
    frequencies
        .iter()
        .map(|frequency| {
            let log_frequency = frequency.max(1e-3).log2();
            bands
                .iter()
                .map(|(low, high)| {
                    let end = (high * 2_f64.sqrt()).min(0.99 * fs / 2.0);
                    ((log_frequency - (low / 2_f64.sqrt()).log2()) / 0.5).clamp(0.0, 1.0)
                        * ((end.log2() - log_frequency) / (end / high).log2().max(1e-3))
                            .clamp(0.0, 1.0)
                })
                .fold(0.0, f64::max)
        })
        .collect()
}

/// Phase-delay fade, retaining the 3.12.0 arithmetic when no extras are set.
pub fn phase_fade(
    frequencies: &[f64],
    primary_high: f64,
    bands: &[(f64, f64)],
    has_extras: bool,
) -> Vec<f64> {
    let end = primary_high * 2_f64.sqrt();
    frequencies
        .iter()
        .map(|frequency| {
            let base = if *frequency < 15.0 {
                (frequency / 15.0).clamp(0.0, 1.0)
            } else {
                ((end.log2() - frequency.max(1e-3).log2()) / 0.5).clamp(0.0, 1.0)
            };
            if !has_extras {
                return base;
            }
            let log_frequency = frequency.max(1e-3).log2();
            bands
                .iter()
                .map(|(low, high)| {
                    ((log_frequency - (low / 2_f64.sqrt()).log2()) / 0.5).clamp(0.0, 1.0)
                        * (((high * 2_f64.sqrt()).log2() - log_frequency) / 0.5).clamp(0.0, 1.0)
                })
                .fold(base, f64::max)
        })
        .collect()
}

/// Bins used to measure FIR realization error.
/// Low-frequency arrival-alignment weight before smoothing and exclusions.
pub fn lf_alignment_weight(frequencies: &[f64], f_lin: f64) -> Vec<f64> {
    frequencies
        .iter()
        .map(|frequency| {
            if f_lin >= 200.0 || *frequency <= 0.0 {
                return 0.0;
            }
            let a = (2.0 * (*frequency / f_lin).log2() + 1.0).clamp(0.0, 1.0);
            let b = (1.0 - 2.0 * (*frequency / 200.0).log2()).clamp(0.0, 1.0);
            0.5 * (1.0 - (PI * a).cos()) * 0.5 * (1.0 - (PI * b).cos())
        })
        .collect()
}

/// Per-bin upper bound for low-frequency arrival alignment in a realization.
pub fn lf_pull_bound(value: f64, lift: f64, strength: f64, frequency: f64, weight: f64) -> f64 {
    let upper = if frequency > 0.0 {
        (1.0 - weight) * lift * strength + weight * (lift * strength).min(1.5 / frequency)
    } else {
        lift * strength
    };
    value.clamp(-0.005, upper)
}

/// A mode-decay cut and the mode measurement from which it was designed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModeDecayCut {
    pub frequency_hz: f64,
    pub measured_t60_s: f64,
    pub depth_db: f64,
    pub q: f64,
}

/// Build one decay cut, or return `None` when the mode does not need one.
pub fn mode_decay_cut(
    frequency_hz: f64,
    measured_t60_s: f64,
    target_t60_s: f64,
    height_db: f64,
    long_boost_db: f64,
) -> Option<ModeDecayCut> {
    if !frequency_hz.is_finite()
        || !measured_t60_s.is_finite()
        || !target_t60_s.is_finite()
        || !height_db.is_finite()
        || frequency_hz <= 0.0
        || target_t60_s <= 0.0
        || measured_t60_s <= 1.5 * target_t60_s
    {
        return None;
    }
    let depth_db = 12.0_f64
        .min(20.0 * (measured_t60_s / target_t60_s).log10())
        .min(height_db.max(0.0) + long_boost_db);
    if depth_db < 1.0 {
        return None;
    }
    Some(ModeDecayCut {
        frequency_hz,
        measured_t60_s,
        depth_db,
        q: 10_f64.powf(-depth_db / 40.0) * PI * frequency_hz * measured_t60_s / 6.91,
    })
}

#[derive(Clone, Debug)]
pub struct HfPhaseRegion {
    pub low_hz: f64,
    pub high_hz: f64,
    pub log_width: f64,
    pub weight_sum: f64,
    pub weight: Vec<f64>,
}

/// Raised-cosine range used to unwind the constant FIR phase above the band.
pub fn hf_phase_region(
    frequencies: &[f64],
    fs: f64,
    primary_high: f64,
    bands: &[(f64, f64)],
) -> Option<HfPhaseRegion> {
    let last_high = bands.last().map_or(primary_high, |band| band.1);
    let low_hz = last_high * 2_f64.sqrt();
    let high_hz = (8.0 * low_hz).min(0.45 * fs);
    if high_hz < 2.0 * low_hz {
        return None;
    }
    let log_width = (high_hz / low_hz).ln();
    let weight: Vec<_> = frequencies
        .iter()
        .map(|frequency| {
            if *frequency >= low_hz && *frequency <= high_hz {
                0.5 * (1.0 - (2.0 * PI * (*frequency / low_hz).ln() / log_width).cos())
            } else {
                0.0
            }
        })
        .collect();
    let weight_sum = weight.iter().sum();
    Some(HfPhaseRegion {
        low_hz,
        high_hz,
        log_width,
        weight_sum,
        weight,
    })
}

pub fn phase_error_band(
    frequencies: &[f64],
    primary_high: f64,
    extras: &[(f64, f64)],
    fir_low: f64,
    floor: f64,
    fs: f64,
) -> Vec<f64> {
    let primary_end = primary_high * 2_f64.sqrt();
    frequencies
        .iter()
        .map(|frequency| {
            let inside_primary = *frequency >= floor.max(fir_low) && *frequency <= primary_end;
            let inside_extra = extras.iter().any(|(low, high)| {
                *frequency >= *low && *frequency <= (high * 2_f64.sqrt()).min(0.99 * fs / 2.0)
            });
            f64::from(inside_primary || inside_extra)
        })
        .collect()
}

/// Concatenated phase-score grid over the merged correction bands.
pub fn band_grid(bands: &[(f64, f64)]) -> Vec<f64> {
    bands
        .iter()
        .filter(|(low, high)| low.max(20.0) < *high)
        .flat_map(|(low, high)| geomspace(low.max(20.0), *high, 1200))
        .collect()
}

/// Optional zero-mode PEQ reach set for a fit touched by an extra band.
pub fn fit_reach(
    fit_low: f64,
    fit_high: f64,
    primary_high: f64,
    extras: &[(f64, f64)],
) -> Option<Vec<(f64, f64)>> {
    if !extras
        .iter()
        .any(|(low, high)| *low <= fit_high && *high >= fit_low)
    {
        return None;
    }
    let mut expanded: Vec<_> = extras
        .iter()
        .map(|(low, high)| (low / 2_f64.sqrt(), high * 2_f64.sqrt()))
        .filter(|(low, high)| low < high)
        .collect();
    let primary = (fit_low, primary_high * 2_f64.sqrt());
    if primary.0 < primary.1 {
        expanded.push(primary);
    }
    Some(merge_bands(expanded))
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
    if n == 1 {
        return w;
    }
    if let PositionPolicy::Listening { index, weight } = policy
        && *index < n
    {
        w[*index] = *weight;
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
    _policy: &PositionPolicy,
    calibration: Option<&FrequencyResponse>,
) -> (Vec<f64>, Vec<Vec<Complex64>>) {
    let fs = points[0].fs as f64;
    let n = analysis_len(points[0].fs);
    let f = grid(n, fs);
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
            // Recording latencies are unrelated, under either position policy.
            let b = onset(&ir.data);
            let a = b.saturating_sub((0.002 * fs) as usize);
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
/// Channel level in dB; invalid measurements, policies and curves are rejected.
pub fn channel_level(
    points: &[ImpulseResponse],
    policy: &PositionPolicy,
    target: Option<&FrequencyResponse>,
    calibration: Option<&FrequencyResponse>,
) -> Result<f64, DspError> {
    let opts = RcOptions {
        policy: *policy,
        ..Default::default()
    };
    validate(points, &opts, target, calibration)?;
    let (f, h) = spectra(points, policy, calibration);
    Ok(level(&power(&h, policy), &f, target))
}
/// All measured speakers contribute to the automatic decay estimate.
pub fn correction_band(
    points: &[&[ImpulseResponse]],
    opts: &RcOptions,
) -> Result<CorrectionBand, DspError> {
    if points.is_empty() {
        return Err(DspError::InvalidArgument(
            "empty tuning2 measurements".into(),
        ));
    }
    for p in points {
        validate(p, opts, None, None)?;
    }
    if points.iter().any(|p| p[0].fs != points[0][0].fs) {
        return Err(DspError::InvalidArgument(
            "inconsistent tuning2 sample rates".into(),
        ));
    }
    if let Some(high) = opts.high {
        return Ok(CorrectionBand {
            low: opts.low,
            high,
            source: BandSource::Manual,
            schroeder: None,
            bands: merge_band_set((opts.low, high), &opts.extra_bands),
        });
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
    let high = schroeder
        .filter(|s| *s <= 500.)
        .map(|s| s.max(100.))
        .unwrap_or(500.);
    Ok(CorrectionBand {
        low: opts.low,
        high,
        source,
        schroeder,
        bands: merge_band_set((opts.low, high), &opts.extra_bands),
    })
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
        || opts.extra_bands.len() > 4
        || opts.extra_bands.iter().any(|(low, high)| {
            !low.is_finite() || !high.is_finite() || *low < 20.0 || *high > 20_000.0 || low >= high
        })
    {
        return Err(invalid());
    }
    if let Tuning2Mode::Ultra(v) = opts.mode
        && (!v.is_finite() || !(30. ..=200.).contains(&v))
    {
        return Err(invalid());
    }
    if let PositionPolicy::Listening { index, weight } = opts.policy
        && (index >= points.len() || !weight.is_finite() || weight <= 0.)
    {
        return Err(invalid());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn point(padding: usize, gain: f64) -> ImpulseResponse {
        let mut data = vec![0.; padding + 48000];
        data[padding] = gain;
        for k in 1..24000 {
            data[padding + k] = gain
                * 0.09
                * (-(k as f64) / 4000.).exp()
                * (2. * PI * 73. * k as f64 / 48000.).cos();
        }
        ImpulseResponse {
            fs: 48000,
            data,
            recording: None,
        }
    }

    #[test]
    fn independent_padding_preserves_spectra_levels_and_long_input() {
        let points = [point(200, 1.), point(400, 0.7)];
        // Reverse which position is earliest, without changing either measurement.
        let shifted = [point(1200, 1.), point(500, 0.7)];
        for policy in [
            PositionPolicy::Symmetric,
            PositionPolicy::Listening {
                index: 0,
                weight: 5.,
            },
            PositionPolicy::Listening {
                index: 1,
                weight: 5.,
            },
        ] {
            let (f, h) = spectra(&points, &policy, None);
            let (sf, sh) = spectra(&shifted, &policy, None);
            assert_eq!(f, sf);
            for (a, b) in h.iter().zip(&sh) {
                assert_eq!(a, b, "each position must retain its own complex spectrum");
                for n in [300, 720] {
                    assert_eq!(low_rate(a, &f, n), low_rate(b, &f, n));
                }
            }
            for (a, b) in points.iter().zip(&shifted) {
                assert_eq!(
                    channel_level(
                        std::slice::from_ref(a),
                        &PositionPolicy::Symmetric,
                        None,
                        None
                    )
                    .unwrap(),
                    channel_level(
                        std::slice::from_ref(b),
                        &PositionPolicy::Symmetric,
                        None,
                        None
                    )
                    .unwrap()
                );
            }
            assert_eq!(
                channel_level(&points, &policy, None, None).unwrap(),
                channel_level(&shifted, &policy, None, None).unwrap()
            );
        }
    }

    #[test]
    fn measurement_apis_reject_invalid_inputs() {
        let opts = RcOptions::default();
        let valid = point(200, 1.);
        let mut nonfinite = valid.clone();
        nonfinite.data[0] = f64::NAN;
        let mut empty = valid.clone();
        empty.data.clear();
        let mut low_rate = valid.clone();
        low_rate.fs = 32000;
        let mut other_rate = valid.clone();
        other_rate.fs = 44100;
        for p in [
            vec![],
            vec![nonfinite],
            vec![empty],
            vec![low_rate],
            vec![valid.clone(), other_rate.clone()],
        ] {
            assert!(matches!(
                channel_level(&p, &opts.policy, None, None),
                Err(DspError::InvalidArgument(_))
            ));
            assert!(matches!(
                correction_band(&[&p], &opts),
                Err(DspError::InvalidArgument(_))
            ));
        }
        assert!(correction_band(&[], &opts).is_err());
        assert!(correction_band(&[std::slice::from_ref(&valid), &[other_rate]], &opts).is_err());
        for policy in [
            PositionPolicy::Listening {
                index: 1,
                weight: 1.,
            },
            PositionPolicy::Listening {
                index: 0,
                weight: f64::NAN,
            },
            PositionPolicy::Listening {
                index: 0,
                weight: 0.,
            },
        ] {
            assert!(channel_level(std::slice::from_ref(&valid), &policy, None, None).is_err());
            assert!(
                correction_band(
                    &[std::slice::from_ref(&valid)],
                    &RcOptions {
                        policy,
                        ..opts.clone()
                    }
                )
                .is_err()
            );
        }
        assert!(
            correction_band(
                &[&[valid]],
                &RcOptions {
                    high: Some(f64::NAN),
                    ..opts
                }
            )
            .is_err()
        );
    }
}
