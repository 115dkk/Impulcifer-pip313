use super::*;
use crate::fft;
/// Optional full-grid capture of the production chain, in execution order.
#[derive(Default, Debug)]
pub struct RcStages {
    pub power: Vec<f64>,
    pub smoothed_variable: Vec<f64>,
    pub smoothed_third: Vec<f64>,
    pub design_db: Vec<f64>,
    pub long_mode_bins: Vec<usize>,
    pub mode_decay: Option<ModeDecayStages>,
    pub lf_f_lin_hz: Option<f64>,
    pub lf_weight_raw: Vec<f64>,
    pub lf_weight: Vec<f64>,
    pub lf_smoothed_db: Vec<f64>,
    pub lf_target_magnitude: Vec<f64>,
    pub lf_target_delay: Vec<f64>,
    pub lf_target: Vec<f64>,
    pub sessions: Vec<RcSession>,
}
#[derive(Default, Debug)]
pub struct RcSession {
    pub fit_inputs: Vec<Vec<f64>>,
    pub m: Vec<Complex64>,
    pub v: Vec<Complex64>,
    pub v_smoothed: Vec<Complex64>,
    pub tau: Vec<f64>,
    pub tau_smoothed: Vec<f64>,
    pub v_total: Vec<Complex64>,
    pub v_total_smoothed: Vec<Complex64>,
    pub total_delay: Vec<f64>,
    pub total_reference: Option<f64>,
    pub total_error: Vec<f64>,
    pub phase_fade: Vec<f64>,
    pub excess: Vec<f64>,
    pub realizations: Vec<RealizeDiagnostics>,
    pub firs: Vec<Vec<f64>>,
    pub touchup_m: Vec<Vec<Complex64>>,
    pub fir: Vec<f64>,
    pub response: Vec<Complex64>,
    pub peq: Vec<(f64, f64, f64)>,
    pub strength: f64,
    pub beta: f64,
}
fn db(x: &[Complex64]) -> Vec<f64> {
    x.iter().map(|x| 20. * (x.norm() + 1e-12).log10()).collect()
}
fn sample(g: &[f64], f: &[f64], v: &[f64]) -> Vec<f64> {
    g.iter().map(|g| interp(*g, f, v)).collect()
}

#[derive(Clone, Debug)]
pub struct DipLimitResult {
    pub envelope: Vec<f64>,
    pub depth: Vec<f64>,
    pub fill: Vec<f64>,
    pub correction: Vec<f64>,
}

/// Apply the one-octave-envelope dip limit before the other design masks.
pub fn dip_limit_curve(
    power: &[f64],
    frequencies: &[f64],
    smoothed_db: &[f64],
    target_db: &[f64],
    level_db: f64,
    correction: &[f64],
) -> DipLimitResult {
    assert_eq!(power.len(), frequencies.len());
    assert_eq!(power.len(), smoothed_db.len());
    assert_eq!(power.len(), target_db.len());
    assert_eq!(power.len(), correction.len());
    let envelope: Vec<_> = smooth(power, frequencies, &[1.0])
        .iter()
        .map(|value| 10.0 * (value + 1e-30).log10())
        .collect();
    let depth: Vec<_> = envelope
        .iter()
        .zip(smoothed_db)
        .map(|(envelope, smoothed)| (envelope - smoothed).max(0.0))
        .collect();
    let fill: Vec<_> = depth
        .iter()
        .map(|depth| {
            if *depth <= 2.0 {
                *depth
            } else {
                2.0 + (depth - 2.0) / 2.0
            }
            .min(6.0)
        })
        .collect();
    let correction = correction
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value.min((target_db[index] + level_db - envelope[index] + fill[index]).max(-25.0))
        })
        .collect();
    DipLimitResult {
        envelope,
        depth,
        fill,
        correction,
    }
}

pub fn limited_dips(
    frequencies: &[f64],
    depth: &[f64],
    fill: &[f64],
    limited: &[f64],
    unlimited: &[f64],
) -> Vec<(f64, f64, f64)> {
    let selected: Vec<_> = depth
        .iter()
        .zip(unlimited.iter().zip(limited))
        .map(|(depth, (unlimited, limited))| *depth >= 4.0 && unlimited - limited >= 0.5)
        .collect();
    let mut dips = Vec::new();
    let mut index = 0;
    while index < selected.len() {
        if !selected[index] {
            index += 1;
            continue;
        }
        let start = index;
        while index + 1 < selected.len() && selected[index + 1] {
            index += 1;
        }
        let end = index;
        let deepest = (start..=end)
            .max_by(|left, right| depth[*left].total_cmp(&depth[*right]))
            .unwrap();
        dips.push((frequencies[deepest], depth[deepest], fill[deepest]));
        index += 1;
    }
    dips.sort_by(|left, right| right.1.total_cmp(&left.1));
    dips.truncate(6);
    dips
}

fn long_mode_candidates(
    h: &[Vec<Complex64>],
    f: &[f64],
    band: &CorrectionBand,
    policy: &PositionPolicy,
) -> Vec<(usize, Vec<(f64, f64)>)> {
    chosen(policy, h.len())
        .into_iter()
        .map(|position| {
            let x = low_rate(&h[position], f, 300);
            let modes = late_mode_frequencies(std::slice::from_ref(&x), 8)
                .into_iter()
                .filter(|peak| {
                    *peak <= 160.0
                        && band
                            .bands
                            .iter()
                            .any(|(low, high)| *peak >= *low && *peak <= high.min(160.0))
                })
                .filter_map(|peak| {
                    mode_decay(&x, peak, 1.5)
                        .filter(|time| *time > 0.0)
                        .map(|time| (peak, time))
                })
                .collect();
            (position, modes)
        })
        .collect()
}

fn long_modes(
    h: &[Vec<Complex64>],
    f: &[f64],
    band: &CorrectionBand,
    policy: &PositionPolicy,
) -> Vec<(f64, f64)> {
    let _timer = profile::timer("long_modes");
    let mut out = Vec::new();
    for (_, modes) in long_mode_candidates(h, f, band, policy) {
        if modes.len() < 2 {
            continue;
        }
        let cutoff =
            (1.3 * median(&modes.iter().map(|(_, time)| *time).collect::<Vec<_>>())).max(0.3);
        for (peak, time) in modes {
            if time >= cutoff {
                out.push((peak, time));
            }
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    out.dedup_by(|a, b| a == b);
    out
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResonanceRefinement {
    pub low_index: usize,
    pub high_index: usize,
    pub peak_index: Option<usize>,
    pub left_db: Option<f64>,
    pub middle_db: Option<f64>,
    pub right_db: Option<f64>,
    pub curvature: Option<f64>,
    pub offset: Option<f64>,
    pub mode: Option<(f64, f64)>,
    pub rejection: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub struct ModeDecayStages {
    pub candidates: Vec<(f64, f64)>,
    pub band_t60: Vec<f64>,
    pub refinements: Vec<(f64, ResonanceRefinement)>,
    pub merged: Vec<(f64, f64)>,
    pub heights: Vec<f64>,
    pub decay_cuts: Vec<Option<ModeDecayCut>>,
}

/// Refine a low-rate candidate on the full design-grid response.
pub fn refine_resonance(
    response: &[Complex64],
    frequencies: &[f64],
    candidate_hz: f64,
) -> ResonanceRefinement {
    let low_index = frequencies.partition_point(|frequency| *frequency < candidate_hz - 3.0);
    let high_index = frequencies.partition_point(|frequency| *frequency <= candidate_hz + 3.0);
    let rejected = |reason| ResonanceRefinement {
        low_index,
        high_index,
        peak_index: None,
        left_db: None,
        middle_db: None,
        right_db: None,
        curvature: None,
        offset: None,
        mode: None,
        rejection: Some(reason),
    };
    if high_index <= low_index + 2 || high_index > response.len() {
        return rejected("window_too_short");
    }
    let mut peak = low_index;
    let mut largest = response[peak].norm();
    for (index, value) in response
        .iter()
        .enumerate()
        .take(high_index)
        .skip(low_index + 1)
    {
        let magnitude = value.norm();
        if (magnitude.is_nan() && !largest.is_nan())
            || (!magnitude.is_nan() && !largest.is_nan() && magnitude > largest)
        {
            peak = index;
            largest = magnitude;
        }
    }
    if peak == low_index || peak + 1 == high_index {
        let mut result = rejected("no_interior_maximum");
        result.peak_index = Some(peak);
        return result;
    }
    let db = |value: Complex64| 20.0 * (value.norm() + 1e-12).log10();
    let left = db(response[peak - 1]);
    let middle = db(response[peak]);
    let right = db(response[peak + 1]);
    let curvature = left - 2.0 * middle + right;
    let offset = 0.5 * (left - right) / curvature;
    let mut result = ResonanceRefinement {
        low_index,
        high_index,
        peak_index: Some(peak),
        left_db: Some(left),
        middle_db: Some(middle),
        right_db: Some(right),
        curvature: Some(curvature),
        offset: Some(offset),
        mode: None,
        rejection: None,
    };
    if !curvature.is_finite() || curvature >= 0.0 || middle <= left || middle <= right {
        result.rejection = Some("invalid_curvature");
        return result;
    }
    let frequency = frequencies[peak] + offset * (frequencies[peak + 1] - frequencies[peak]);
    let low = low_rate(response, frequencies, 1200);
    match mode_decay(&low, frequency, 1.5).filter(|time| time.is_finite() && *time > 0.0) {
        Some(time) => result.mode = Some((frequency, time)),
        None => result.rejection = Some("invalid_decay"),
    }
    result
}

fn target_mode_decay(
    h: &[Vec<Complex64>],
    policy: &PositionPolicy,
    fs: f64,
    n: usize,
) -> (f64, Vec<f64>) {
    let mut times = Vec::new();
    for position in chosen(policy, h.len()) {
        let impulse = fft::irfft(&h[position], n);
        for center in [200.0, 250.0, 315.0, 400.0, 500.0] {
            if let Some(time) = mode_decay_at_rate(&impulse, center, 0.115 * center, fs) {
                times.push(time);
            }
        }
    }
    let target = if times.is_empty() {
        0.25
    } else {
        median(&times).max(0.25)
    };
    (target, times)
}

fn merge_refined_modes(mut refined: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    refined.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut merged: Vec<(f64, f64)> = Vec::new();
    let mut previous = f64::NEG_INFINITY;
    for mode in refined {
        if mode.0 - previous <= 1.5 {
            if mode.1 > merged.last().unwrap().1 {
                *merged.last_mut().unwrap() = mode;
            }
        } else {
            merged.push(mode);
        }
        previous = mode.0;
    }
    merged.sort_by(|left, right| right.1.total_cmp(&left.1));
    merged
}

pub fn select_mode_decay_cuts(
    modes: &[(f64, f64)],
    heights: &[f64],
    target_t60_s: f64,
) -> (Vec<Option<ModeDecayCut>>, Vec<ModeDecayCut>) {
    assert_eq!(modes.len(), heights.len());
    let mut attempts = Vec::new();
    let mut selected = Vec::new();
    for ((frequency, time), height) in modes.iter().zip(heights) {
        let cut = mode_decay_cut(*frequency, *time, target_t60_s, *height, 3.0);
        attempts.push(cut);
        if let Some(cut) = cut {
            selected.push(cut);
            if selected.len() == 4 {
                break;
            }
        }
    }
    (attempts, selected)
}

#[allow(clippy::too_many_arguments)]
fn mode_decay_design(
    h: &[Vec<Complex64>],
    f: &[f64],
    band: &CorrectionBand,
    policy: &PositionPolicy,
    smoothed_db: &[f64],
    target_db: &[f64],
    level_db: f64,
    fs: f64,
) -> (ModeDecayResult, ModeDecayStages) {
    let mut candidates_flat = Vec::new();
    let mut refinements = Vec::new();
    let mut refined = Vec::new();
    for (position, candidates) in long_mode_candidates(h, f, band, policy) {
        for (candidate, time) in candidates {
            candidates_flat.push((candidate, time));
            let refinement = refine_resonance(&h[position], f, candidate);
            if let Some(mode) = refinement.mode {
                refined.push(mode);
            }
            refinements.push((candidate, refinement));
        }
    }
    let merged = merge_refined_modes(refined);
    let (target_t60_s, band_t60) = target_mode_decay(h, policy, fs, (f.len() - 1) * 2);
    let heights: Vec<_> = merged
        .iter()
        .map(|(frequency, _)| {
            interp(*frequency, f, smoothed_db) - interp(*frequency, f, target_db) - level_db
        })
        .collect();
    let (decay_cuts, cuts) = select_mode_decay_cuts(&merged, &heights, target_t60_s);
    (
        ModeDecayResult { target_t60_s, cuts },
        ModeDecayStages {
            candidates: candidates_flat,
            band_t60,
            refinements,
            merged,
            heights,
            decay_cuts,
        },
    )
}
/// Blend excess-phase and total-phase delay errors for LF arrival alignment.
pub fn blend_lf_excess(
    tau: &[f64],
    reference: f64,
    total_error: &[f64],
    total_valid: &[bool],
    kappa: &[f64],
    fade: &[f64],
) -> Vec<f64> {
    assert_eq!(tau.len(), total_error.len());
    assert_eq!(tau.len(), total_valid.len());
    assert_eq!(tau.len(), kappa.len());
    assert_eq!(tau.len(), fade.len());
    (0..tau.len())
        .map(|index| {
            let fallback = (tau[index] - reference) * fade[index];
            let candidate = fade[index]
                * ((1.0 - kappa[index]) * (tau[index] - reference)
                    + kappa[index] * total_error[index]);
            if total_valid[index]
                && total_error[index].is_finite()
                && kappa[index].is_finite()
                && candidate.is_finite()
            {
                candidate
            } else {
                fallback
            }
        })
        .collect()
}

#[derive(Clone)]
struct LfAlignPrep {
    f_lin_hz: f64,
    raw_weight: Vec<f64>,
    kappa: Vec<f64>,
    target_magnitude: Vec<f64>,
    target_delay_raw: Vec<f64>,
    target_delay: Vec<f64>,
}

struct FirRun {
    fir: Vec<f64>,
    fir_response: Vec<Complex64>,
    response: Vec<Complex64>,
    peq: peq::Bands,
    strength: f64,
    beta: f64,
    fits: Vec<peq::Bands>,
    limit_outcome: protect::LimitOutcome,
    low_limited: bool,
    lf_limited: bool,
    hf_rotation_deg: Option<f64>,
    hf_limited: bool,
}

#[derive(Clone)]
struct RealizeOutput {
    fir: Vec<f64>,
    response: Vec<Complex64>,
    rms: f64,
    max: f64,
    applied_before: Vec<f64>,
    applied_after: Vec<f64>,
    total: Option<f64>,
    rho: Option<f64>,
    wrapped: Option<f64>,
    correction: Option<f64>,
    rotation: Option<f64>,
    rotation_deg: Option<f64>,
    hf_limited: bool,
}

struct Realizer<'a> {
    f: &'a [f64],
    x: &'a [f64],
    n: usize,
    taps: usize,
    lambda: f64,
    ff: f64,
    fe: f64,
    extras: &'a [(f64, f64)],
    rolloff_hz: Option<f64>,
    fs: f64,
    kappa: Option<&'a [f64]>,
    hf_region: Option<&'a HfPhaseRegion>,
    error_mask: Option<&'a [bool]>,
}
impl Realizer<'_> {
    fn run(&self, s: f64, m: &[Complex64]) -> RealizeOutput {
        let _timer = profile::timer("realization");
        let delta = self.f[1] - self.f[0];
        if self.kappa.is_none() && self.hf_region.is_none() {
            let mut sum = 0.0;
            let mut applied = Vec::with_capacity(self.x.len());
            let spec: Vec<_> = m
                .iter()
                .zip(self.x)
                .map(|(m, x)| {
                    let adv = x.clamp(-0.005, self.lambda * s);
                    applied.push(adv);
                    sum += self.lambda - adv;
                    *m * Complex64::from_polar(1.0, -2.0 * PI * sum * delta)
                })
                .collect();
            return self.finish(m, spec, applied.clone(), applied, None, false);
        }
        let mut applied_before = Vec::with_capacity(self.x.len());
        for (index, value) in self.x.iter().enumerate() {
            let adv = self.kappa.map_or_else(
                || value.clamp(-0.005, self.lambda * s),
                |kappa| lf_pull_bound(*value, self.lambda, s, self.f[index], kappa[index]),
            );
            applied_before.push(adv);
        }
        let mut applied_after = applied_before.clone();
        let mut hf_scalars = None;
        let mut hf_limited = false;
        if let Some(region) = self.hf_region {
            let total: f64 = applied_before.iter().sum();
            let rho = 2.0 * PI * delta * (total - self.lambda);
            let wrapped = rho.rem_euclid(2.0 * PI);
            let correction = wrapped / (2.0 * PI * delta);
            let rotation = if wrapped > PI {
                wrapped - 2.0 * PI
            } else {
                wrapped
            };
            hf_scalars = Some((total, rho, wrapped, correction, rotation));
            for (index, adv) in applied_after.iter_mut().enumerate() {
                if self.f[index] >= region.low_hz && self.f[index] <= region.high_hz {
                    let candidate = *adv - correction * region.weight[index] / region.weight_sum;
                    let clipped = candidate.clamp(-0.005, self.lambda);
                    hf_limited |= clipped.to_bits() != candidate.to_bits();
                    *adv = clipped;
                }
            }
        }
        let mut sum = 0.0;
        let spec = m
            .iter()
            .zip(&applied_after)
            .map(|(m, adv)| {
                sum += self.lambda - adv;
                *m * Complex64::from_polar(1.0, -2.0 * PI * sum * delta)
            })
            .collect();
        self.finish(
            m,
            spec,
            applied_before,
            applied_after,
            hf_scalars,
            hf_limited,
        )
    }

    fn finish(
        &self,
        m: &[Complex64],
        spec: Vec<Complex64>,
        applied_before: Vec<f64>,
        applied_after: Vec<f64>,
        hf_scalars: Option<(f64, f64, f64, f64, f64)>,
        hf_limited: bool,
    ) -> RealizeOutput {
        let mut h = fft::irfft(&spec, self.n);
        h.truncate(self.taps);
        taper(&mut h, 0.1);
        if self.rolloff_hz.is_some() {
            protect::correct_fir_dc(&mut h);
        }
        let response = rfft(&h, self.n);
        let error: Vec<_> = self
            .f
            .iter()
            .enumerate()
            .filter(|(index, frequency)| {
                self.error_mask.map_or_else(
                    || {
                        (**frequency >= 20_f64.max(self.ff) && **frequency <= self.fe)
                            || self.extras.iter().any(|(low, high)| {
                                **frequency >= *low
                                    && **frequency
                                        <= (high * 2_f64.sqrt()).min(0.99 * self.fs / 2.0)
                            })
                    },
                    |mask| mask[*index],
                )
            })
            .map(|(index, _)| {
                20.0 * (response[index].norm() + 1e-12).log10()
                    - 20.0 * (m[index].norm() + 1e-12).log10()
            })
            .collect();
        let rms = (error.iter().map(|value| value * value).sum::<f64>()
            / error.len().max(1) as f64)
            .sqrt();
        let max = error.iter().map(|value| value.abs()).fold(0.0, f64::max);
        let (total, rho, wrapped, correction, rotation) = hf_scalars.map_or(
            (None, None, None, None, None),
            |(total, rho, wrapped, correction, rotation)| {
                (
                    Some(total),
                    Some(rho),
                    Some(wrapped),
                    Some(correction),
                    Some(rotation),
                )
            },
        );
        RealizeOutput {
            fir: h,
            response,
            rms,
            max,
            applied_before,
            applied_after,
            total,
            rho,
            wrapped,
            correction,
            rotation,
            rotation_deg: rotation.map(f64::to_degrees),
            hf_limited,
        }
    }
}
pub fn smoothed_phase_delay(value: &[Complex64], frequencies: &[f64]) -> Vec<f64> {
    let delta = frequencies[1] - frequencies[0];
    let phase = unwrap(
        &value
            .iter()
            .map(|value| {
                if value.re.is_finite() && value.im.is_finite() {
                    value.arg()
                } else {
                    0.0
                }
            })
            .collect::<Vec<_>>(),
    );
    let coordinate: Vec<_> = frequencies
        .iter()
        .map(|frequency| 2.0 * PI * frequency.max(delta))
        .collect();
    let raw: Vec<_> = gradient(&phase, &coordinate)
        .iter()
        .map(|value| if value.is_finite() { -*value } else { 0.0 })
        .collect();
    smooth(&raw, frequencies, &[3.0])
}

#[allow(clippy::too_many_arguments)]
fn prepare_lf_alignment(
    frequencies: &[f64],
    smoothed_db: &[f64],
    design_db: &[f64],
    long_modes: &[(f64, f64)],
    low: f64,
    rolloff_hz: Option<f64>,
    low_protect: bool,
    n: usize,
) -> LfAlignPrep {
    let f_lin_hz = rolloff_hz.map_or(low, |rolloff| (1.5 * rolloff).max(low));
    let mut raw = lf_alignment_weight(frequencies, f_lin_hz);
    for (index, frequency) in frequencies.iter().enumerate() {
        if long_modes.iter().any(|(peak, _)| {
            *frequency >= peak * 2_f64.powf(-1.0 / 24.0)
                && *frequency <= peak * 2_f64.powf(1.0 / 24.0)
        }) || (low_protect && rolloff_hz.is_some_and(|rolloff| *frequency < rolloff))
        {
            raw[index] = 0.0;
        }
    }
    let kappa = smooth(&raw, frequencies, &[6.0])
        .into_iter()
        .map(|value| value.clamp(0.0, 1.0))
        .collect();
    let magnitude: Vec<_> = smoothed_db
        .iter()
        .zip(design_db)
        .map(|(smoothed, design)| 10_f64.powf((smoothed + design) / 20.0))
        .collect();
    let target_phase = mp(&magnitude, n);
    let target_delay_raw = smoothed_phase_delay(&target_phase, frequencies);
    let at_linear = interp(f_lin_hz, frequencies, &target_delay_raw);
    let target_delay = frequencies
        .iter()
        .enumerate()
        .map(|(index, frequency)| {
            if *frequency < f_lin_hz {
                (target_delay_raw[index] - at_linear).max(0.0)
            } else {
                0.0
            }
        })
        .collect();
    LfAlignPrep {
        f_lin_hz,
        raw_weight: raw,
        kappa,
        target_magnitude: magnitude,
        target_delay_raw,
        target_delay,
    }
}

#[derive(Clone, Debug)]
pub struct RealizeDiagnostics {
    pub strength: f64,
    pub fir: Vec<f64>,
    pub response: Vec<Complex64>,
    pub rms: f64,
    pub max: f64,
    pub applied_before: Vec<f64>,
    pub applied_after: Vec<f64>,
    pub total: Option<f64>,
    pub rho: Option<f64>,
    pub wrapped: Option<f64>,
    pub correction: Option<f64>,
    pub rotation: Option<f64>,
    pub rotation_deg: Option<f64>,
    pub limited: bool,
}

fn realize_diagnostics(output: &RealizeOutput, strength: f64) -> RealizeDiagnostics {
    RealizeDiagnostics {
        strength,
        fir: output.fir.clone(),
        response: output.response.clone(),
        rms: output.rms,
        max: output.max,
        applied_before: output.applied_before.clone(),
        applied_after: output.applied_after.clone(),
        total: output.total,
        rho: output.rho,
        wrapped: output.wrapped,
        correction: output.correction,
        rotation: output.rotation,
        rotation_deg: output.rotation_deg,
        limited: output.hf_limited,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn realize_for_fixture(
    frequencies: &[f64],
    excess: &[f64],
    magnitude: &[Complex64],
    taps: usize,
    lift_seconds: f64,
    fir_low_hz: f64,
    error_high_hz: f64,
    extras: &[(f64, f64)],
    rolloff_hz: Option<f64>,
    fs: f64,
    strength: f64,
    kappa: Option<&[f64]>,
    hf_region: Option<&HfPhaseRegion>,
    error_mask: Option<&[bool]>,
) -> RealizeDiagnostics {
    let output = Realizer {
        f: frequencies,
        x: excess,
        n: (frequencies.len() - 1) * 2,
        taps,
        lambda: lift_seconds,
        ff: fir_low_hz,
        fe: error_high_hz,
        extras,
        rolloff_hz,
        fs,
        kappa,
        hf_region,
        error_mask,
    }
    .run(strength, magnitude);
    realize_diagnostics(&output, strength)
}

#[allow(clippy::too_many_arguments)]
fn fir_run(
    h: &[Vec<Complex64>],
    f: &[f64],
    u0: &[f64],
    opts: &RcOptions,
    band: &CorrectionBand,
    params: ModeParams,
    beta: f64,
    fs: f64,
    rolloff_hz: Option<f64>,
    decay_bands: &[(f64, f64, f64)],
    decay_response: &[Complex64],
    lf_align: Option<&LfAlignPrep>,
    hf_region: Option<&HfPhaseRegion>,
    mut capture: Option<&mut RcSession>,
) -> Result<FirRun, DspError> {
    let n = (f.len() - 1) * 2;
    let g = geomspace(15., 20000., 700);
    let ff = 1.5 * fs / params.taps as f64;
    let ramp: Vec<_> = f
        .iter()
        .map(|f| ((f - ff / 2.) / (ff / 2.)).clamp(0., 1.))
        .collect();
    let fe = band.high * 2_f64.sqrt();
    let c = sample(&g, f, u0);
    if let Some(t) = capture.as_deref_mut() {
        t.fit_inputs.push(c.clone());
    }
    let fit_low = rolloff_hz.unwrap_or(20.0).max(20.0);
    let mut b1 = peq::fit(
        &g,
        &c,
        16,
        fit_low,
        500.,
        opts.max_boost,
        Some(band.high),
        fs,
    )?;
    if let Some(rolloff_hz) = rolloff_hz {
        protect::clamp(&mut b1, rolloff_hz);
        let unity = vec![Complex64::new(1.0, 0.0); f.len()];
        let _ = protect::limit(
            &mut b1,
            &unity,
            f,
            rolloff_hz,
            protect::LimitOutcome::default(),
            fs,
        );
    }
    let p1 = peq::response(f, &b1, fs);
    let pdb = db(&p1);
    let mut m = mp(
        &u0.iter()
            .enumerate()
            .map(|(i, u)| 10_f64.powf((u - pdb[i]) * ramp[i] / 20.))
            .collect::<Vec<_>>(),
        n,
    );
    if let Some(t) = capture.as_deref_mut() {
        t.m = m.clone();
    }
    let weights = weights(&opts.policy, h.len());
    let total = weights.iter().sum::<f64>();
    let mut v = vec![Complex64::new(0.0, 0.0); f.len()];
    let mut v_total = lf_align.map(|_| vec![Complex64::new(0.0, 0.0); f.len()]);
    let res = resolution(f);
    for (h, w) in h.iter().zip(weights) {
        let mut r: Vec<_> = if opts.mode_decay {
            h.iter()
                .zip(decay_response)
                .zip(&p1)
                .zip(&m)
                .map(|(((h, decay), p), m)| ((h * decay) * p) * m)
                .collect()
        } else {
            h.iter()
                .zip(&p1)
                .zip(&m)
                .map(|((h, p), m)| h * p * m)
                .collect()
        };
        let t = argmax(
            &fft::irfft(&r, n)
                .iter()
                .map(|x| x.abs())
                .collect::<Vec<_>>(),
        ) as f64
            / fs;
        for (r, f) in r.iter_mut().zip(f) {
            *r *= Complex64::from_polar(1., 2. * PI * f * t);
        }
        if let Some(total_phase) = &mut v_total {
            for index in 0..f.len() {
                total_phase[index] += w * r[index] / (r[index].norm() + 1e-30);
            }
        }
        let min = mp(
            &smooth(&r.iter().map(|r| r.norm_sqr()).collect::<Vec<_>>(), f, &res)
                .iter()
                .map(|x| x.sqrt() + 1e-12)
                .collect::<Vec<_>>(),
            n,
        );
        for i in 0..f.len() {
            let e = r[i] / (min[i] + 1e-30);
            v[i] += w * e / (e.norm() + 1e-30);
        }
    }
    for v in &mut v {
        *v /= total;
    }
    if let Some(total_phase) = &mut v_total {
        for value in total_phase {
            *value /= total;
        }
    }
    let vs = smooth_phasors(&v, f, &[beta]);
    if let Some(t) = capture.as_deref_mut() {
        t.v = v.clone();
        t.v_smoothed = vs.clone();
    }
    let phi = unwrap(
        &vs.iter()
            .map(|v| {
                if v.re.is_finite() && v.im.is_finite() {
                    v.arg()
                } else {
                    0.
                }
            })
            .collect::<Vec<_>>(),
    );
    let tau: Vec<_> = gradient(
        &phi,
        &f.iter()
            .map(|f| 2. * PI * f.max(f64::MIN_POSITIVE))
            .enumerate()
            .map(|(i, x)| if i == 0 { 2. * PI * f[1] } else { x })
            .collect::<Vec<_>>(),
    )
    .iter()
    .map(|v| if v.is_finite() { -v } else { 0. })
    .collect();
    if let Some(t) = capture.as_deref_mut() {
        t.tau = tau.clone();
    }
    let tau = smooth(&tau, f, &[beta]);
    if let Some(t) = capture.as_deref_mut() {
        t.tau_smoothed = tau.clone();
    }
    let med = median(
        &f.iter()
            .enumerate()
            .filter(|(_, f)| **f >= 3000. && **f <= 6000.)
            .map(|(i, _)| tau[i])
            .collect::<Vec<_>>(),
    );
    let mut x: Vec<_> = if opts.extra_bands.is_empty() {
        f.iter()
            .enumerate()
            .map(|(index, frequency)| {
                let gamma = if *frequency < 15.0 {
                    (frequency / 15.0).clamp(0.0, 1.0)
                } else {
                    ((fe.log2() - frequency.max(1e-3).log2()) / 0.5).clamp(0.0, 1.0)
                };
                (tau[index] - med) * gamma
            })
            .collect()
    } else {
        tau.iter()
            .zip(phase_fade(f, band.high, &band.bands, true))
            .map(|(tau, gamma)| (tau - med) * gamma)
            .collect()
    };
    if let (Some(prep), Some(total_phase)) = (lf_align, v_total.as_ref()) {
        let total_smoothed = smooth_phasors(total_phase, f, &[beta]);
        let total_delay = smoothed_phase_delay(&total_smoothed, f);
        let reference = median(
            &f.iter()
                .enumerate()
                .filter(|(_, frequency)| **frequency >= 3000.0 && **frequency <= 6000.0)
                .map(|(index, _)| total_delay[index])
                .collect::<Vec<_>>(),
        );
        let fade = phase_fade(f, band.high, &band.bands, !opts.extra_bands.is_empty());
        let total_valid: Vec<_> = (0..f.len())
            .map(|index| {
                total_phase[index].re.is_finite()
                    && total_phase[index].im.is_finite()
                    && total_smoothed[index].re.is_finite()
                    && total_smoothed[index].im.is_finite()
            })
            .collect();
        let total_error: Vec<_> = (0..f.len())
            .map(|index| {
                if total_valid[index] {
                    (total_delay[index] - reference) - prep.target_delay[index]
                } else {
                    f64::NAN
                }
            })
            .collect();
        x = blend_lf_excess(&tau, med, &total_error, &total_valid, &prep.kappa, &fade);
        if let Some(stages) = capture.as_deref_mut() {
            stages.v_total = total_phase.clone();
            stages.v_total_smoothed = total_smoothed;
            stages.total_delay = total_delay;
            stages.total_reference = Some(reference);
            stages.total_error = total_error;
            stages.phase_fade = fade;
            stages.excess = x.clone();
        }
    }
    let realizer = Realizer {
        f,
        x: &x,
        n,
        taps: params.taps,
        lambda: params.lift_ms / 1000.,
        ff,
        fe,
        extras: &opts.extra_bands,
        rolloff_hz,
        fs,
        kappa: lf_align.map(|prep| prep.kappa.as_slice()),
        hf_region,
        error_mask: None,
    };
    let baseline = realizer.run(0.0, &m);
    let rms0 = baseline.rms;
    let max0 = baseline.max;
    let mut selected = baseline;
    let mut fir = selected.fir.clone();
    let mut response = selected.response.clone();
    if let Some(t) = capture.as_deref_mut() {
        t.realizations.push(realize_diagnostics(&selected, 0.0));
        t.firs.push(fir.clone());
    }
    let mut strength = 0.;
    for s in [0.9, 0.7, 0.5, 0.3] {
        let candidate = realizer.run(s, &m);
        if let Some(t) = capture.as_deref_mut() {
            t.realizations.push(realize_diagnostics(&candidate, s));
            t.firs.push(candidate.fir.clone());
        }
        if candidate.rms <= 0.5_f64.max(rms0 + 0.3) && candidate.max <= 2_f64.max(max0 + 1.0) {
            fir = candidate.fir.clone();
            response = candidate.response.clone();
            selected = candidate;
            strength = s;
            break;
        }
    }
    let fd = db(&response);
    let c = sample(
        &g,
        f,
        &u0.iter().zip(fd).map(|(u, f)| u - f).collect::<Vec<_>>(),
    );
    if let Some(t) = capture.as_deref_mut() {
        t.fit_inputs.push(c.clone());
    }
    let mut b2 = peq::fit(
        &g,
        &c,
        16,
        fit_low,
        500.,
        opts.max_boost,
        Some(band.high),
        fs,
    )?;
    let mut limit_outcome = protect::LimitOutcome::default();
    if let Some(rolloff_hz) = rolloff_hz {
        protect::clamp(&mut b2, rolloff_hz);
        let unity = vec![Complex64::new(1.0, 0.0); f.len()];
        limit_outcome = protect::limit(&mut b2, &unity, f, rolloff_hz, limit_outcome, fs);
    }
    let p2 = peq::response(f, &b2, fs);
    let rho: Vec<_> = f
        .iter()
        .map(|frequency| {
            band.bands
                .iter()
                .any(|(low, high)| *frequency >= low.max(20.0) && *frequency <= *high)
        })
        .collect();
    let err = |response: &[Complex64]| {
        let sm = smooth(
            &response
                .iter()
                .zip(&p2)
                .map(|(f, p)| (f * p).norm_sqr())
                .collect::<Vec<_>>(),
            f,
            &[24.],
        );
        let e: Vec<_> = (0..f.len())
            .filter(|i| rho[*i])
            .map(|i| 10. * (sm[i] + 1e-30).log10() - u0[i])
            .collect();
        let med = median(&e);
        (e.iter().map(|e| (e - med).powi(2)).sum::<f64>() / e.len() as f64).sqrt()
    };
    let touchup_timer = profile::timer("touchup");
    for _ in 0..2 {
        let r: Vec<_> = (0..f.len())
            .map(|i| {
                if rho[i] {
                    u0[i] - 20. * ((response[i] * p2[i]).norm() + 1e-12).log10()
                } else {
                    0.
                }
            })
            .collect();
        let mut z = smooth(&r, f, &[3.]);
        if let Some(rolloff_hz) = rolloff_hz {
            for (value, frequency) in z.iter_mut().zip(f) {
                if *frequency < rolloff_hz {
                    *value = value.min(0.0);
                }
            }
        }
        let m2 = mp(
            &(0..f.len())
                .map(|i| {
                    m[i].norm() * 10_f64.powf(z[i] * ramp[i] * if rho[i] { 1. } else { 0. } / 20.)
                })
                .collect::<Vec<_>>(),
            n,
        );
        if let Some(t) = capture.as_deref_mut() {
            t.touchup_m.push(m2.clone());
        }
        let touchup = realizer.run(strength, &m2);
        if let Some(t) = capture.as_deref_mut() {
            t.realizations.push(realize_diagnostics(&touchup, strength));
            t.firs.push(touchup.fir.clone());
        }
        if err(&touchup.response) >= err(&response) {
            break;
        }
        fir = touchup.fir;
        response = touchup.response;
        m = m2;
    }
    drop(touchup_timer);
    let fir_response = response.clone();
    let mut combined = if opts.mode_decay {
        let mut bands = decay_bands.to_vec();
        bands.extend_from_slice(&b2);
        bands
    } else {
        b2.clone()
    };
    if opts.mode_decay {
        for ((value, peq), decay) in response.iter_mut().zip(&p2).zip(decay_response) {
            *value = (*value * peq) * decay;
        }
    } else {
        for (value, peq) in response.iter_mut().zip(&p2) {
            *value *= peq;
        }
    }
    let lf_limited = lf_align.is_some_and(|prep| {
        prep.f_lin_hz >= 200.0
            || strength == 0.0
            || prep.kappa.iter().enumerate().any(|(index, weight)| {
                *weight >= 0.5 && x[index] - selected.applied_before[index] > 0.001
            })
    });
    if let Some(t) = capture {
        t.fir = fir.clone();
        t.response = response.clone();
        t.peq = combined.clone();
        t.strength = strength;
        t.beta = beta;
    }
    Ok(FirRun {
        fir,
        fir_response,
        response,
        peq: std::mem::take(&mut combined),
        strength,
        beta,
        fits: vec![b1, b2],
        limit_outcome,
        low_limited: rolloff_hz.is_none(),
        lf_limited,
        hf_rotation_deg: selected.rotation_deg,
        hf_limited: selected.hf_limited,
    })
}
fn finalize_run(
    run: &mut FirRun,
    frequencies: &[f64],
    rolloff_hz: Option<f64>,
    decay_count: usize,
    fs: f64,
) {
    if let Some(rolloff_hz) = rolloff_hz {
        if decay_count > 0 {
            for index in &mut run.limit_outcome.reduced {
                *index += decay_count;
            }
        }
        run.limit_outcome = protect::limit(
            &mut run.peq,
            &run.fir_response,
            frequencies,
            rolloff_hz,
            run.limit_outcome.clone(),
            fs,
        );
        run.low_limited =
            protect::limited(&run.peq, &run.fir_response, frequencies, rolloff_hz, fs);
        let peq_response = peq::response(frequencies, &run.peq, fs);
        run.response = run
            .fir_response
            .iter()
            .zip(peq_response)
            .map(|(fir, peq)| fir * peq)
            .collect();
    } else {
        run.low_limited = true;
    }
}

fn score(
    run: &FirRun,
    h: &[Vec<Complex64>],
    f: &[f64],
    band: &CorrectionBand,
    policy: &PositionPolicy,
    fs: f64,
) -> f64 {
    let n = (f.len() - 1) * 2;
    let g = band_grid(&band.bands);
    let mut worst = f64::NEG_INFINITY;
    for i in chosen(policy, h.len()) {
        let mut r: Vec<_> = h[i].iter().zip(&run.response).map(|(a, b)| a * b).collect();
        let t = argmax(
            &fft::irfft(&r, n)
                .iter()
                .map(|x| x.abs())
                .collect::<Vec<_>>(),
        ) as f64
            / fs;
        for (r, f) in r.iter_mut().zip(f) {
            *r *= Complex64::from_polar(1., 2. * PI * f * t);
        }
        let s = smooth_phasors(&r, f, &[24.]);
        let re: Vec<_> = s.iter().map(|s| s.re).collect();
        let im: Vec<_> = s.iter().map(|s| s.im).collect();
        let phase = unwrap(
            &g.iter()
                .map(|g| Complex64::new(interp(*g, f, &re), interp(*g, f, &im)).arg())
                .collect::<Vec<_>>(),
        );
        let gd: Vec<_> = gradient(&phase, &g.iter().map(|g| 2. * PI * g).collect::<Vec<_>>())
            .iter()
            .map(|g| -g * 1000.)
            .collect();
        let med = median(&gd);
        worst = worst.max(gd.iter().map(|g| g - med).fold(f64::NEG_INFINITY, f64::max));
    }
    worst
}
pub fn design_rc(
    points: &[ImpulseResponse],
    opts: &RcOptions,
    band: &CorrectionBand,
    target: Option<&FrequencyResponse>,
    calibration: Option<&FrequencyResponse>,
) -> Result<SpeakerRc, DspError> {
    design_rc_with_stages(points, opts, band, target, calibration, None)
}
pub fn design_rc_with_stages(
    points: &[ImpulseResponse],
    opts: &RcOptions,
    band: &CorrectionBand,
    target: Option<&FrequencyResponse>,
    calibration: Option<&FrequencyResponse>,
    mut capture: Option<&mut RcStages>,
) -> Result<SpeakerRc, DspError> {
    if let Some(t) = capture.as_deref_mut() {
        *t = RcStages::default();
    }
    validate(points, opts, target, calibration)?;
    let fs = points[0].fs as f64;
    if !band.low.is_finite()
        || !band.high.is_finite()
        || band.low <= 0.
        || band.high <= band.low
        || band.high >= fs / 2.
    {
        return Err(DspError::InvalidArgument("invalid correction band".into()));
    }
    let spectra_timer = profile::timer("spectra");
    let (f, h) = spectra(points, &opts.policy, calibration);
    drop(spectra_timer);
    let base_power = power(&h, &opts.policy);
    let level_db = level(&base_power, &f, target);
    let res = resolution(&f);
    let long_modes = long_modes(&h, &f, band, &opts.policy);
    let target = curve(target, &f.iter().map(|f| f.max(1.0)).collect::<Vec<_>>());
    let (mode_decay, mode_decay_stages) = if opts.mode_decay {
        let base_variable = smooth(&base_power, &f, &res);
        let base_third = smooth(&base_power, &f, &[3.0]);
        let base_smoothed: Vec<_> = base_variable
            .iter()
            .zip(base_third)
            .map(|(variable, third)| {
                (10.0 * (variable + 1e-30).log10()).max(10.0 * (third + 1e-30).log10() - 3.0)
            })
            .collect();
        let (result, stages) = mode_decay_design(
            &h,
            &f,
            band,
            &opts.policy,
            &base_smoothed,
            &target,
            level_db,
            fs,
        );
        (Some(result), Some(stages))
    } else {
        (None, None)
    };
    let decay_bands: Vec<_> = mode_decay
        .as_ref()
        .map(|result| {
            result
                .cuts
                .iter()
                .map(|cut| (cut.frequency_hz, cut.q, -cut.depth_db))
                .collect()
        })
        .unwrap_or_default();
    let decay_response = if opts.mode_decay {
        peq::response(&f, &decay_bands, fs)
    } else {
        vec![Complex64::new(1.0, 0.0); f.len()]
    };
    let p = if opts.mode_decay {
        let weights = weights(&opts.policy, h.len());
        let total = weights.iter().sum::<f64>();
        let mut adjusted = vec![0.0; f.len()];
        for (response, weight) in h.iter().zip(weights) {
            for index in 0..f.len() {
                adjusted[index] += weight * (response[index] * decay_response[index]).norm_sqr();
            }
        }
        for value in &mut adjusted {
            *value /= total;
        }
        adjusted
    } else {
        base_power.clone()
    };
    let s = smooth(&p, &f, &res);
    let s3 = smooth(&p, &f, &[3.0]);
    let measured_rolloff = if opts.low_protect || opts.lf_align {
        protect::rolloff(&base_power, &f).frequency
    } else {
        None
    };
    let rolloff_hz = opts.low_protect.then_some(measured_rolloff).flatten();
    let mut dips = Vec::new();
    let u0: Vec<_> = if !opts.dip_limit
        && !opts.low_protect
        && opts.extra_bands.is_empty()
        && !opts.mode_decay
    {
        let end = (band.high * 2_f64.sqrt()).min(0.99 * fs / 2.);
        f.iter()
            .enumerate()
            .map(|(i, f)| {
                let sm = (10. * (s[i] + 1e-30).log10()).max(10. * (s3[i] + 1e-30).log10() - 3.);
                let cap = if *f > 16000. {
                    opts.max_boost.min(2.)
                } else {
                    opts.max_boost
                };
                let mut u = (target[i] + level_db - sm).clamp(-25., cap);
                if long_modes
                    .iter()
                    .any(|(p, _)| *f >= p * 2_f64.powf(-1. / 24.) && *f <= p * 2_f64.powf(1. / 24.))
                {
                    u = u.min(3.);
                }
                let l = f.max(1e-3).log2();
                u * ((l - (band.low / 2_f64.sqrt()).log2()) / 0.5).clamp(0., 1.)
                    * ((end.log2() - l) / (end / band.high).log2().max(1e-3)).clamp(0., 1.)
            })
            .collect()
    } else {
        let smoothed_db: Vec<_> = s
            .iter()
            .zip(&s3)
            .map(|(variable, third)| {
                (10. * (variable + 1e-30).log10()).max(10. * (third + 1e-30).log10() - 3.)
            })
            .collect();
        let mut correction: Vec<_> = f
            .iter()
            .enumerate()
            .map(|(index, frequency)| {
                let cap = if *frequency > 16000.0 {
                    opts.max_boost.min(2.0)
                } else {
                    opts.max_boost
                };
                (target[index] + level_db - smoothed_db[index]).clamp(-25.0, cap)
            })
            .collect();
        let mut unlimited = correction.clone();
        let dip = opts.dip_limit.then(|| {
            let result = dip_limit_curve(&p, &f, &smoothed_db, &target, level_db, &correction);
            correction = result.correction.clone();
            result
        });
        if let Some(rolloff_hz) = rolloff_hz {
            for (index, frequency) in f.iter().enumerate() {
                let cap = if *frequency < rolloff_hz {
                    0.0
                } else if *frequency < 2.0 * rolloff_hz {
                    opts.max_boost * (frequency / rolloff_hz).log2()
                } else {
                    f64::INFINITY
                };
                correction[index] = correction[index].min(cap);
                unlimited[index] = unlimited[index].min(cap);
            }
        }
        for (index, frequency) in f.iter().enumerate() {
            if long_modes.iter().any(|(peak, _)| {
                *frequency >= peak * 2_f64.powf(-1. / 24.)
                    && *frequency <= peak * 2_f64.powf(1. / 24.)
            }) || mode_decay.as_ref().is_some_and(|result| {
                result.cuts.iter().any(|cut| {
                    *frequency >= cut.frequency_hz * 2_f64.powf(-1.0 / 24.0)
                        && *frequency <= cut.frequency_hz * 2_f64.powf(1.0 / 24.0)
                })
            }) {
                correction[index] = correction[index].min(3.0);
                unlimited[index] = unlimited[index].min(3.0);
            }
        }
        let mask = magnitude_mask(&f, &band.bands, fs);
        for index in 0..f.len() {
            correction[index] *= mask[index];
            unlimited[index] *= mask[index];
        }
        if let Some(dip) = dip {
            dips = limited_dips(&f, &dip.depth, &dip.fill, &correction, &unlimited);
        }
        correction
    };
    let reported_design = if opts.mode_decay {
        u0.iter()
            .zip(&decay_response)
            .map(|(design, decay)| design + 20.0 * (decay.norm() + 1e-12).log10())
            .collect()
    } else {
        u0.clone()
    };
    let smoothed_db: Vec<_> = s
        .iter()
        .zip(&s3)
        .map(|(variable, third)| {
            (10.0 * (variable + 1e-30).log10()).max(10.0 * (third + 1e-30).log10() - 3.0)
        })
        .collect();
    let lf_alignment = opts.lf_align.then(|| {
        prepare_lf_alignment(
            &f,
            &smoothed_db,
            &u0,
            &long_modes,
            band.low,
            measured_rolloff,
            opts.low_protect,
            (f.len() - 1) * 2,
        )
    });
    let hf_region = opts
        .hf_phase
        .then(|| hf_phase_region(&f, fs, band.high, &band.bands))
        .flatten();
    if let Some(t) = capture.as_deref_mut() {
        t.power = p.clone();
        t.smoothed_variable = s.clone();
        t.smoothed_third = s3.clone();
        t.design_db = u0.clone();
        t.mode_decay = mode_decay_stages.clone();
        if let Some(prep) = &lf_alignment {
            t.lf_f_lin_hz = Some(prep.f_lin_hz);
            t.lf_weight_raw = prep.raw_weight.clone();
            t.lf_weight = prep.kappa.clone();
            t.lf_smoothed_db = smoothed_db.clone();
            t.lf_target_magnitude = prep.target_magnitude.clone();
            t.lf_target_delay = prep.target_delay_raw.clone();
            t.lf_target = prep.target_delay.clone();
        }
        t.long_mode_bins = f
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                long_modes.iter().any(|(p, _)| {
                    **f >= p * 2_f64.powf(-1. / 24.) && **f <= p * 2_f64.powf(1. / 24.)
                }) || mode_decay.as_ref().is_some_and(|result| {
                    result.cuts.iter().any(|cut| {
                        **f >= cut.frequency_hz * 2_f64.powf(-1.0 / 24.0)
                            && **f <= cut.frequency_hz * 2_f64.powf(1.0 / 24.0)
                    })
                })
            })
            .map(|(k, _)| k)
            .collect();
    }
    let mut session = capture.as_ref().map(|_| RcSession::default());
    let params = opts.mode.params(points[0].fs);
    let mut phase_candidates = Vec::new();
    let (
        fir,
        response,
        peq,
        strength,
        beta,
        fits,
        limit_outcome,
        low_limited,
        lf_limited,
        hf_rotation_deg,
        hf_limited,
    ) = if opts.mode == Tuning2Mode::Zero {
        let g = geomspace(15., 20000., 900);
        let c = sample(&g, &f, &u0);
        if let Some(t) = session.as_mut() {
            t.fit_inputs.push(
                g.iter()
                    .zip(&c)
                    .map(|(g, c)| if *g <= 500. { *c } else { 0. })
                    .collect(),
            );
        }
        let low_fit = rolloff_hz.unwrap_or(15.0).max(15_f64.max(band.low / 1.4));
        let low_reach = fit_reach(low_fit, 400.0, band.high, &opts.extra_bands);
        let blo = peq::fit_with_reach(
            &g,
            &g.iter()
                .zip(&c)
                .map(|(g, c)| if *g <= 500. { *c } else { 0. })
                .collect::<Vec<_>>(),
            16,
            low_fit,
            400.,
            opts.max_boost,
            None,
            fs,
            low_reach.as_deref(),
        )?;
        let d = db(&peq::response(&g, &blo, fs));
        let high_end = 20000_f64.min(band.bands.last().map_or(band.high, |band| band.1));
        let high_reach = fit_reach(400.0, high_end, band.high, &opts.extra_bands);
        let bhi = peq::fit_with_reach(
            &g,
            &g.iter()
                .enumerate()
                .map(|(i, g)| if *g >= 300. { c[i] - d[i] } else { 0. })
                .collect::<Vec<_>>(),
            16,
            400.,
            high_end,
            opts.max_boost,
            None,
            fs,
            high_reach.as_deref(),
        )?;
        let mut bands = blo.clone();
        bands.extend_from_slice(&bhi);
        if let Some(rolloff_hz) = rolloff_hz {
            protect::clamp(&mut bands, rolloff_hz);
        }
        if opts.mode_decay {
            let mut combined = decay_bands.clone();
            combined.extend(bands);
            bands = combined;
        }
        let response = peq::response(&f, &bands, fs);
        let low_limited = rolloff_hz.is_none_or(|rolloff_hz| {
            let unity = vec![Complex64::new(1.0, 0.0); f.len()];
            protect::limited(&bands, &unity, &f, rolloff_hz, fs)
        });
        (
            Vec::new(),
            response,
            bands,
            0.,
            0.,
            vec![blo, bhi],
            protect::LimitOutcome::default(),
            low_limited,
            false,
            None,
            false,
        )
    } else {
        let dual = matches!(opts.mode, Tuning2Mode::Linear | Tuning2Mode::Ultra(_));
        let mut other_session = capture.as_ref().map(|_| RcSession::default());
        let (first, other) = if dual {
            let profiling = profile::enabled();
            let ((first, first_times), (other, times)) = rayon::join(
                || {
                    profile::capture(profiling, || {
                        fir_run(
                            &h,
                            &f,
                            &u0,
                            opts,
                            band,
                            params,
                            params.phase_frac,
                            fs,
                            rolloff_hz,
                            &decay_bands,
                            &decay_response,
                            lf_alignment.as_ref(),
                            hf_region.as_ref(),
                            session.as_mut(),
                        )
                    })
                },
                || {
                    profile::capture(profiling, || {
                        fir_run(
                            &h,
                            &f,
                            &u0,
                            opts,
                            band,
                            params,
                            6.,
                            fs,
                            rolloff_hz,
                            &decay_bands,
                            &decay_response,
                            lf_alignment.as_ref(),
                            hf_region.as_ref(),
                            other_session.as_mut(),
                        )
                    })
                },
            );
            profile::merge(first_times);
            profile::merge(times);
            (first, Some(other))
        } else {
            (
                fir_run(
                    &h,
                    &f,
                    &u0,
                    opts,
                    band,
                    params,
                    params.phase_frac,
                    fs,
                    rolloff_hz,
                    &decay_bands,
                    &decay_response,
                    lf_alignment.as_ref(),
                    hf_region.as_ref(),
                    session.as_mut(),
                ),
                None,
            )
        };
        let mut run = first?;
        if let Some(t) = capture.as_deref_mut() {
            t.sessions.push(session.take().unwrap());
        }
        let mut fits = run.fits.clone();
        if let Some(other) = other {
            let other = other?;
            if let Some(t) = capture.as_deref_mut() {
                t.sessions.push(other_session.unwrap());
            }
            fits.extend(other.fits.clone());
            let requested_score = score(&run, &h, &f, band, &opts.policy, fs);
            let fallback_score = score(&other, &h, &f, band, &opts.policy, fs);
            phase_candidates.push((run.beta, requested_score));
            phase_candidates.push((other.beta, fallback_score));
            if fallback_score < requested_score {
                run = other;
            }
        }
        finalize_run(&mut run, &f, rolloff_hz, decay_bands.len(), fs);
        (
            run.fir,
            run.response,
            run.peq,
            run.strength,
            run.beta,
            fits,
            run.limit_outcome,
            run.low_limited,
            run.lf_limited,
            run.hf_rotation_deg,
            run.hf_limited,
        )
    };
    if let (Some(t), Some(mut s)) = (capture, session) {
        s.fir = fir.clone();
        s.response = response.clone();
        s.peq = peq.clone();
        s.strength = strength;
        s.beta = beta;
        t.sessions.push(s);
    }
    let lf_align_result = if opts.lf_align && opts.mode != Tuning2Mode::Zero {
        let prep = lf_alignment.as_ref().unwrap();
        let mut arrival: Option<f64> = None;
        for position in chosen(&opts.policy, h.len()) {
            let corrected: Vec<_> = h[position]
                .iter()
                .zip(&response)
                .map(|(measurement, correction)| measurement * correction)
                .collect();
            let delay = smoothed_phase_delay(&corrected, &f);
            let low: Vec<_> = f
                .iter()
                .enumerate()
                .filter(|(_, frequency)| {
                    **frequency >= prep.f_lin_hz.max(40.0) && **frequency <= 150.0
                })
                .map(|(index, _)| delay[index])
                .filter(|value| value.is_finite())
                .collect();
            let high: Vec<_> = f
                .iter()
                .enumerate()
                .filter(|(_, frequency)| **frequency >= 3000.0 && **frequency <= 6000.0)
                .map(|(index, _)| delay[index])
                .filter(|value| value.is_finite())
                .collect();
            if !low.is_empty() && !high.is_empty() {
                let value = 1000.0 * (median(&low) - median(&high));
                if arrival.is_none_or(|current| value.abs() > current.abs()) {
                    arrival = Some(value);
                }
            }
        }
        Some(LfAlignResult {
            f_lin_hz: prep.f_lin_hz,
            arrival_ms: arrival,
            limited: lf_limited,
        })
    } else {
        None
    };
    let hf_phase_result = if opts.hf_phase && opts.mode != Tuning2Mode::Zero {
        Some(HfPhaseResult {
            range_hz: hf_region
                .as_ref()
                .map(|region| (region.low_hz, region.high_hz)),
            rotation_deg: hf_rotation_deg,
            limited: hf_limited,
        })
    } else {
        None
    };
    Ok(SpeakerRc {
        points: points.len(),
        level_db,
        design_db: reported_design,
        peq,
        peq_fits: fits,
        fir,
        phase_strength: strength,
        phase_frac: beta,
        phase_candidates,
        long_modes,
        response,
        lift_samples: (params.lift_ms * fs / 1000.).round_ties_even() as usize,
        rolloff_hz,
        limit_outcome,
        low_limited,
        limited_dips: dips,
        bands: band.bands.clone(),
        lf_align: lf_align_result,
        mode_decay,
        hf_phase: hf_phase_result,
    })
}

#[cfg(test)]
mod bundle2_tests {
    use super::*;

    #[test]
    fn nearby_modes_merge_and_four_cut_limit_is_enforced() {
        let merged = merge_refined_modes(vec![(60.0, 0.5), (61.4, 0.8), (63.0, 0.7)]);
        assert_eq!(merged, vec![(61.4, 0.8), (63.0, 0.7)]);
        let modes: Vec<_> = (0..5)
            .map(|index| (40.0 + index as f64 * 5.0, 1.0))
            .collect();
        let (attempts, cuts) = select_mode_decay_cuts(&modes, &[20.0; 5], 0.4);
        assert_eq!(
            attempts.len(),
            4,
            "the production loop must stop at four cuts"
        );
        assert_eq!(cuts.len(), 4, "a fifth mode must not create a cut");
        let capped = mode_decay_cut(80.0, 4.0, 0.4, 2.0, 3.0).unwrap();
        assert_eq!(
            capped.depth_db, 5.0,
            "height plus the 3 dB boost caps depth"
        );
    }

    #[test]
    fn resonance_refinement_keeps_the_first_nan_maximum() {
        let frequencies: Vec<_> = (0..10).map(|value| value as f64).collect();
        let mut response = vec![Complex64::new(1.0, 0.0); frequencies.len()];
        response[3] = Complex64::new(f64::NAN, 0.0);
        response[5] = Complex64::new(f64::NAN, 0.0);
        let result = refine_resonance(&response, &frequencies, 4.0);
        assert_eq!(result.peak_index, Some(3));
    }

    #[test]
    fn lf_weight_excludes_long_modes_and_protected_rolloff() {
        let frequencies = grid(4096, 48_000.0);
        let prep = prepare_lf_alignment(
            &frequencies,
            &vec![0.0; frequencies.len()],
            &vec![0.0; frequencies.len()],
            &[(93.75, 1.0)],
            20.0,
            Some(50.0),
            true,
            4096,
        );
        let below = frequencies.partition_point(|frequency| *frequency < 40.0);
        assert!(prep.kappa[..below].iter().all(|weight| *weight < 1e-12));
        let mode = frequencies
            .iter()
            .position(|frequency| (*frequency - 93.75).abs() < 1e-12)
            .unwrap();
        assert!(prep.kappa[mode] < 1.0);
    }

    #[test]
    fn arrival_bound_and_hf_unwind_limits_are_reported() {
        assert_eq!(lf_pull_bound(0.020, 0.028, 1.0, 125.0, 1.0), 0.012);
        let frequencies = grid(8, 8.0);
        let region = HfPhaseRegion {
            low_hz: 2.0,
            high_hz: 2.0,
            log_width: 1.0,
            weight_sum: 1.0,
            weight: vec![0.0, 0.0, 1.0, 0.0, 0.0],
        };
        let x = vec![0.008; frequencies.len()];
        let m = vec![Complex64::new(1.0, 0.0); frequencies.len()];
        let realizer = Realizer {
            f: &frequencies,
            x: &x,
            n: 8,
            taps: 8,
            lambda: 0.008,
            ff: 0.0,
            fe: 4.0,
            extras: &[],
            rolloff_hz: None,
            fs: 8.0,
            kappa: None,
            hf_region: Some(&region),
            error_mask: None,
        };
        let result = realizer.run(1.0, &m);
        assert!(result.hf_limited);
        assert_eq!(result.applied_after[2], -0.005);
        assert!(
            hf_phase_region(&grid(8, 48_000.0), 48_000.0, 20_000.0, &[(20.0, 20_000.0)]).is_none()
        );
    }
}
