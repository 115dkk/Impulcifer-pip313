//! MIMO FIR supports, from the clean-room specification in ROOM_CORRECTION.md.
use super::{analysis_len, primitives::*};
use crate::{DspError, fft::Complex64, filters, hrir::Hrir};
use faer::{
    Mat, Par,
    diag::Diag,
    dyn_stack::{MemBuffer, MemStack},
    linalg::evd,
};
use rayon::prelude::*;
use std::f64::consts::PI;

pub const MIMO_EARLY_MS_DEFAULT: f64 = 60.0;
const MIMO_EARLY_SAMPLES_DEFAULT: usize = 36;
const GUARD_BAND_SAMPLES: usize = 36;
const GUARD_TAIL_SAMPLES: usize = 180;

#[derive(Clone, Debug)]
pub struct MimoOptions {
    pub fsiso: f64,
    pub low: Option<f64>,
    pub input_hp: Option<f64>,
    pub early_ms: f64,
    pub ear_guard: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MimoRule {
    pub early: usize,
    pub ear_guard: bool,
}
impl Default for MimoRule {
    fn default() -> Self {
        Self {
            early: MIMO_EARLY_SAMPLES_DEFAULT,
            ear_guard: false,
        }
    }
}

/// Convert the configurable 600 Hz MIMO early window from milliseconds to samples.
pub fn early_samples(early_ms: f64) -> usize {
    (early_ms * 600.0 / 1000.0).round().max(3.0) as usize
}
pub struct MimoInput<'a> {
    pub speaker: &'a str,
    pub gain_db: f64,
    pub response: &'a [Complex64],
}
#[derive(Clone, Debug, PartialEq)]
pub struct MimoSupport {
    pub speaker: String,
    pub low: f64,
    pub fir: Vec<f64>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct MimoEarBand {
    pub ear: usize,
    pub lo: f64,
    pub hi: f64,
    pub db: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct MimoEarTail {
    pub ear: usize,
    pub hz: f64,
    pub db: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct MimoEarLimit {
    pub scale_db: f64,
    pub band: Option<MimoEarBand>,
    pub tail: Option<MimoEarTail>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct MimoInputResult {
    pub speaker: String,
    pub candidates: Vec<String>,
    pub supports: Vec<MimoSupport>,
    pub level_db: f64,
    pub scale_db: Option<f64>,
    pub band_change_db: f64,
    pub tail_change_db: f64,
    pub ear_limit: Option<MimoEarLimit>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct MimoDesign {
    pub ranges: Vec<(String, Option<(f64, f64)>)>,
    pub inputs: Vec<MimoInputResult>,
    pub early_ms: f64,
    pub ear_guard: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GuardEvaluation {
    pub band_db: f64,
    pub band_hz: Option<(f64, f64)>,
    pub band_ear: Option<usize>,
    pub tail_db: f64,
    pub mode_hz: Option<f64>,
    pub tail_ear: Option<usize>,
}
/// Stage boundaries retained for the deterministic oracle gate.
#[derive(Clone, Debug)]
pub struct MimoStages {
    pub x: Vec<Vec<f64>>,
    pub g: Vec<Vec<Vec<f64>>>,
    pub lp: Vec<f64>,
    pub direct: f64,
    pub normal: Mat<f64>,
    pub rhs: Vec<f64>,
    pub logs: Vec<f64>,
    pub ratios: Vec<f64>,
    pub kept: usize,
    pub u: Vec<f64>,
    pub guard_band: Vec<f64>,
    pub guard_tail: Vec<f64>,
    pub guard_steps: Vec<GuardEvaluation>,
}
const N: usize = 720;
const TAPS: usize = 180;
/// Memory the parallel MIMO designs may hold at once.
const MIMO_MEMORY_BUDGET: usize = 1 << 30;
/// How many inputs may be designed in parallel. A design holds its normal
/// matrix, the eigenvectors and the solver's scratch at once, about four n x n
/// f64 arrays (n = 180 per candidate: 39 candidates make 394 MB each). The
/// grouping does not change any result.
fn designs_at_once(candidates: usize) -> usize {
    let unknowns = candidates * TAPS;
    let per_input = 4 * unknowns * unknowns * std::mem::size_of::<f64>();
    (MIMO_MEMORY_BUDGET / per_input.max(1)).max(1)
}
fn convolution(a: &[f64], b: &[f64]) -> Vec<f64> {
    // Short low-rate filters: direct convolution avoids FFT roundoff at onset.
    let mut y = vec![0.; a.len() + b.len() - 1];
    for (i, a) in a.iter().enumerate() {
        for (j, b) in b.iter().enumerate() {
            y[i + j] += a * b;
        }
    }
    y
}
fn first(x: &[f64]) -> usize {
    let peak = x.iter().map(|v| v.abs()).fold(0., f64::max);
    x.iter().position(|v| v.abs() > 0.1 * peak).unwrap_or(0)
}
/// Build truncated convolution Gram blocks along diagonals. Both prefix sums
/// start at zero; no entry is a difference between long accumulated sums.
pub fn normal_equations(
    x: &[Vec<f64>],
    c: &[Vec<Vec<f64>>],
) -> Result<(Mat<f64>, Vec<f64>), DspError> {
    normal_equations_with(x, c, MIMO_EARLY_SAMPLES_DEFAULT)
}

pub fn normal_equations_with(
    x: &[Vec<f64>],
    c: &[Vec<Vec<f64>>],
    early_samples: usize,
) -> Result<(Mat<f64>, Vec<f64>), DspError> {
    validate_paths(x, c)?;
    let r = c[0].len();
    let m = r * TAPS;
    let mut a = Mat::zeros(m, m);
    let mut b = vec![0.; m];
    for (x, c) in x.iter().zip(c) {
        let early = first(x).saturating_add(early_samples).min(N);
        for j in 0..r {
            for k in 0..TAPS {
                b[j * TAPS + k] -= (early.max(k)..N).map(|t| c[j][t - k] * x[t]).sum::<f64>();
            }
            for h in j..r {
                for delta in -(TAPS as isize - 1)..TAPS as isize {
                    let dj = delta.max(0) as usize;
                    let dh = (-delta).max(0) as usize;
                    let mut sums = vec![0.; N + 1];
                    for t in 0..N - dj.max(dh) {
                        sums[t + 1] = sums[t] + c[j][t + dh] * c[h][t + dj];
                    }
                    for base in 0..TAPS - dj.max(dh) {
                        let k = base + dj;
                        let l = base + dh;
                        let shift = k.max(l);
                        let value = sums[N - shift] + 9. * sums[early.saturating_sub(shift)];
                        a[(j * TAPS + k, h * TAPS + l)] += value;
                    }
                }
            }
        }
    }
    for j in 0..m {
        for i in j + 1..m {
            a[(i, j)] = a[(j, i)];
        }
    }
    Ok((a, b))
}
fn validate_paths(x: &[Vec<f64>], g: &[Vec<Vec<f64>>]) -> Result<(), DspError> {
    if x.len() != 2
        || g.len() != 2
        || g[0].is_empty()
        || x.iter()
            .any(|x| x.len() != N || x.iter().any(|v| !v.is_finite()))
        || g.iter().any(|ear| {
            ear.len() != g[0].len()
                || ear
                    .iter()
                    .any(|p| p.len() != N || p.iter().any(|v| !v.is_finite()))
        })
    {
        return Err(DspError::InvalidArgument(
            "inconsistent MIMO path lengths or values".into(),
        ));
    }
    Ok(())
}
fn matvec(a: &Mat<f64>, x: &[f64]) -> Vec<f64> {
    let mut result = vec![0.; a.nrows()];
    // faer is column-major. Traverse each column contiguously while retaining
    // ascending column accumulation for every output, independent of SIMD width.
    for (j, x) in x.iter().enumerate() {
        for (out, value) in result.iter_mut().zip(a.col(j).iter()) {
            *out += value * x;
        }
    }
    result
}
pub type BisectionResult = (Vec<f64>, Vec<f64>, usize, Vec<f64>);
/// Sequential symmetric factorization, reused for all nineteen regularizations.
pub fn bisect(
    a: &Mat<f64>,
    b: &[f64],
    lp: &[f64],
    direct: f64,
) -> Result<BisectionResult, DspError> {
    let m = b.len();
    if m == 0
        || !m.is_multiple_of(TAPS)
        || a.nrows() != m
        || a.ncols() != m
        || lp.len() != 31
        || lp.iter().chain(b).any(|v| !v.is_finite())
        || !direct.is_finite()
        || direct <= 0.
        || a.col_iter().any(|c| c.iter().any(|v| !v.is_finite()))
    {
        return Err(DspError::InvalidArgument(
            "invalid MIMO solve dimensions or values".into(),
        ));
    }
    let sigma = (0..m).map(|i| a[(i, i)]).sum::<f64>() / m as f64;
    let mut vectors = Mat::zeros(m, m);
    let mut values = Diag::zeros(m);
    let mut memory = MemBuffer::new(evd::self_adjoint_evd_scratch::<f64>(
        m,
        evd::ComputeEigenvectors::Yes,
        Par::Seq,
        Default::default(),
    ));
    evd::self_adjoint_evd(
        a.as_ref(),
        values.as_mut(),
        Some(vectors.as_mut()),
        Par::Seq,
        MemStack::new(&mut memory),
        Default::default(),
    )
    .map_err(|e| DspError::InvalidArgument(format!("MIMO eigendecomposition: {e:?}")))?;
    let projected: Vec<_> = (0..m)
        .map(|j| (0..m).map(|i| vectors[(i, j)] * b[i]).sum::<f64>())
        .collect();
    let solve = |log: f64| {
        let lambda = (sigma * 10_f64.powf(log)).max(f64::MIN_POSITIVE);
        let p: Vec<_> = (0..m)
            .map(|i| projected[i] / (values[i] + lambda))
            .collect();
        let mut u = matvec(&vectors, &p);
        // Refine against the original matrix, not its rounded eigensystem.
        // The least-regularized trial is ill-conditioned even when the kept
        // trial is not; two corrections recover the dense-solve accuracy.
        for _ in 0..2 {
            let au = matvec(a, &u);
            let residual: Vec<_> = (0..m).map(|i| b[i] - lambda * u[i] - au[i]).collect();
            let correction: Vec<_> = (0..m)
                .map(|j| {
                    (0..m).map(|i| vectors[(i, j)] * residual[i]).sum::<f64>()
                        / (values[j] + lambda)
                })
                .collect();
            for (u, correction) in u.iter_mut().zip(matvec(&vectors, &correction)) {
                *u += correction;
            }
        }
        let ratio = u
            .chunks(TAPS)
            .map(|u| convolution(lp, u).iter().map(|v| v * v).sum::<f64>())
            .sum::<f64>()
            / direct.max(1e-30);
        (u, ratio)
    };
    let (mut best, ratio) = solve(4.);
    let mut logs = vec![4.];
    let mut ratios = vec![ratio];
    let mut kept = 0;
    let (mut lo, mut hi) = (-8., 4.);
    for i in 1..=18 {
        let mid = (lo + hi) / 2.;
        let (u, ratio) = solve(mid);
        logs.push(mid);
        ratios.push(ratio);
        if ratio > 10_f64.powf(-1.2) {
            lo = mid;
        } else {
            hi = mid;
            best = u;
            kept = i;
        }
    }
    Ok((logs, ratios, kept, best))
}
/// The ear guard's per-ear change `c(a, b)` in dB: the floor on both sides when
/// the ear had energy before, `+99` for energy where it had none, no value when
/// it had none before and after. An overflowing ratio stays `+inf`; only the
/// energies, not the ratio, select the `+99` case.
pub fn seat_change(a: f64, b: f64) -> Option<f64> {
    if a > 0.0 {
        Some(10.0 * ((b + 1e-30) / (a + 1e-30)).log10())
    } else if b > 0.0 {
        Some(99.0)
    } else {
        None
    }
}

pub fn guard_evaluation(
    x: &[Vec<f64>],
    y: &[Vec<f64>],
    fsiso: f64,
    ear_guard: bool,
) -> GuardEvaluation {
    let peaks = late_mode_frequencies(x, 6);
    let mut edges = Vec::new();
    let mut i = 0;
    while i as f64 / 3. <= (fsiso / 20.).log2() + 1e-9 {
        edges.push(20. * 2_f64.powf(i as f64 / 3.));
        i += 1;
    }
    let mut bands: Vec<_> = edges.windows(2).map(|p| (p[0], p[1])).collect();
    bands.extend(peaks.iter().map(|p| (p - 1.5, p + 1.5)));
    let f = grid(2 * N, 600.);

    let mut band_db = -99.0;
    let mut band_hz = None;
    let mut band_ear = None;
    if ear_guard {
        let mut before = Vec::with_capacity(x.len());
        let mut after = Vec::with_capacity(y.len());
        for (x, y) in x.iter().zip(y) {
            let end = (first(x) + GUARD_BAND_SAMPLES).min(N);
            let mut a = x.clone();
            let mut b = y.clone();
            a[..end].fill(0.);
            b[..end].fill(0.);
            before.push(
                rfft(&a, 2 * N)
                    .into_iter()
                    .map(|v| v.norm_sqr())
                    .collect::<Vec<_>>(),
            );
            after.push(
                rfft(&b, 2 * N)
                    .into_iter()
                    .map(|v| v.norm_sqr())
                    .collect::<Vec<_>>(),
            );
        }
        for ear in 0..before.len() {
            for &(lo, hi) in &bands {
                let mut a = 0.0;
                let mut b = 0.0;
                for k in 0..f.len() {
                    if f[k] >= lo && f[k] < hi {
                        a += before[ear][k];
                        b += after[ear][k];
                    }
                }
                if let Some(value) = seat_change(a, b)
                    && value > band_db
                {
                    band_db = value;
                    band_hz = Some((lo, hi));
                    band_ear = Some(ear);
                }
            }
        }
    } else {
        // Keep the pre-3.16.0 accumulation order bit for bit.
        let mut before = vec![0.; N + 1];
        let mut after = before.clone();
        for (x, y) in x.iter().zip(y) {
            let end = (first(x) + GUARD_BAND_SAMPLES).min(N);
            let mut a = x.clone();
            let mut b = y.clone();
            a[..end].fill(0.);
            b[..end].fill(0.);
            for (out, v) in before.iter_mut().zip(rfft(&a, 2 * N)) {
                *out += v.norm_sqr();
            }
            for (out, v) in after.iter_mut().zip(rfft(&b, 2 * N)) {
                *out += v.norm_sqr();
            }
        }
        for &(lo, hi) in &bands {
            let mut a = 0.;
            let mut b = 0.;
            for k in 0..f.len() {
                if f[k] >= lo && f[k] < hi {
                    a += before[k];
                    b += after[k];
                }
            }
            if a > 0. {
                let value = 10. * ((b + 1e-30) / a).log10();
                if value > band_db {
                    band_db = value;
                    band_hz = Some((lo, hi));
                }
            }
        }
    }

    let mut modes: Vec<_> = peaks
        .into_iter()
        .chain(late_mode_frequencies(y, 6))
        .map(|p| (p * 10.).round_ties_even() / 10.)
        .collect();
    modes.sort_by(f64::total_cmp);
    modes.dedup();
    let energy: Vec<Vec<(f64, f64)>> = modes
        .iter()
        .map(|p| {
            let sos = filters::butter_bandpass(2, (p - 1.5).max(1.) / 300., (p + 1.5) / 300.);
            x.iter()
                .zip(y)
                .map(|(x, y)| {
                    let k = (first(x) + GUARD_TAIL_SAMPLES).min(N);
                    let a = filters::sosfilt(&sos, x)[k..]
                        .iter()
                        .map(|v| v * v)
                        .sum::<f64>();
                    let b = filters::sosfilt(&sos, y)[k..]
                        .iter()
                        .map(|v| v * v)
                        .sum::<f64>();
                    (a, b)
                })
                .collect()
        })
        .collect();
    let summed: Vec<_> = energy
        .iter()
        .map(|ears| {
            let mut a = 0.0;
            let mut b = 0.0;
            for &(ear_a, ear_b) in ears {
                a += ear_a;
                b += ear_b;
            }
            (a, b)
        })
        .collect();
    let max = summed.iter().map(|v| v.0).fold(0., f64::max);
    let mut tail_db = -99.0;
    let mut mode_hz = None;
    let mut tail_ear = None;
    for (index, &(a, b)) in summed.iter().enumerate() {
        if a <= 0.0 || a < 0.01 * max {
            continue;
        }
        if ear_guard {
            let mut mode_db = -99.0;
            let mut mode_ear = None;
            for (ear, &(ear_a, ear_b)) in energy[index].iter().enumerate() {
                if let Some(value) = seat_change(ear_a, ear_b)
                    && value > mode_db
                {
                    mode_db = value;
                    mode_ear = Some(ear);
                }
            }
            if mode_db > tail_db {
                tail_db = mode_db;
                mode_hz = Some(modes[index]);
                tail_ear = mode_ear;
            }
        } else {
            let value = 10. * ((b + 1e-30) / a).log10();
            if value > tail_db {
                tail_db = value;
                mode_hz = Some(modes[index]);
            }
        }
    }
    GuardEvaluation {
        band_db,
        band_hz,
        band_ear,
        tail_db,
        mode_hz,
        tail_ear,
    }
}

/// Design from oracle path boundaries, also used by the production chain.
pub fn design_paths(
    speaker: &str,
    candidates: &[(String, f64)],
    x: Vec<Vec<f64>>,
    g: Vec<Vec<Vec<f64>>>,
    lp: Vec<f64>,
    direct: f64,
    fsiso: f64,
) -> Result<(MimoInputResult, MimoStages), DspError> {
    design_paths_with(
        speaker,
        candidates,
        x,
        g,
        lp,
        direct,
        fsiso,
        MimoRule::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn design_paths_with(
    speaker: &str,
    candidates: &[(String, f64)],
    x: Vec<Vec<f64>>,
    g: Vec<Vec<Vec<f64>>>,
    lp: Vec<f64>,
    direct: f64,
    fsiso: f64,
    rule: MimoRule,
) -> Result<(MimoInputResult, MimoStages), DspError> {
    validate_paths(&x, &g)?;
    if candidates.len() != g[0].len()
        || candidates
            .iter()
            .any(|(_, low)| !low.is_finite() || *low <= 0.)
        || lp.len() != 31
        || lp.iter().any(|v| !v.is_finite())
        || !fsiso.is_finite()
        || !(50. ..=150.).contains(&fsiso)
        || !direct.is_finite()
        || direct <= 0.
    {
        return Err(DspError::InvalidArgument(
            "invalid MIMO design dimensions or values".into(),
        ));
    }
    let c: Vec<Vec<_>> = g
        .iter()
        .map(|ear| {
            ear.iter()
                .map(|path| {
                    let mut c = convolution(path, &lp);
                    c.truncate(N);
                    c
                })
                .collect()
        })
        .collect();
    let (normal, rhs) = normal_equations_with(&x, &c, rule.early)?;
    let (logs, ratios, kept, u) = bisect(&normal, &rhs, &lp, direct)?;
    let supports: Vec<_> = u.chunks(TAPS).map(|u| convolution(&lp, u)).collect();
    let mut result = MimoInputResult {
        speaker: speaker.into(),
        candidates: candidates.iter().map(|c| c.0.clone()).collect(),
        supports: vec![],
        level_db: -300.,
        scale_db: None,
        band_change_db: -99.,
        tail_change_db: -99.,
        ear_limit: None,
    };
    let mut guard_band = vec![];
    let mut guard_tail = vec![];
    let mut guard_steps = vec![];
    let mut last_rejected = None;
    for db in [0., -3., -6., -9., -12.] {
        let scale = 10_f64.powf(db / 20.);
        let mut y = x.clone();
        for (y, g) in y.iter_mut().zip(&g) {
            for (g, s) in g.iter().zip(&supports) {
                let scaled: Vec<_> = s.iter().map(|v| v * scale).collect();
                for (y, v) in y.iter_mut().zip(convolution(g, &scaled)) {
                    *y += v;
                }
            }
        }
        let evaluation = guard_evaluation(&x, &y, fsiso, rule.ear_guard);
        guard_band.push(evaluation.band_db);
        guard_tail.push(evaluation.tail_db);
        guard_steps.push(evaluation.clone());
        let accepted = evaluation.band_db <= 1. && evaluation.tail_db <= 1.;
        if !accepted && rule.ear_guard {
            last_rejected = Some(MimoEarLimit {
                scale_db: db,
                band: (evaluation.band_db > 1.0).then(|| MimoEarBand {
                    ear: evaluation.band_ear.expect("ear guard band has an ear"),
                    lo: evaluation.band_hz.expect("ear guard band has a range").0,
                    hi: evaluation.band_hz.expect("ear guard band has a range").1,
                    db: evaluation.band_db,
                }),
                tail: (evaluation.tail_db > 1.0).then(|| MimoEarTail {
                    ear: evaluation.tail_ear.expect("ear guard tail has an ear"),
                    hz: evaluation.mode_hz.expect("ear guard tail has a mode"),
                    db: evaluation.tail_db,
                }),
            });
        }
        if accepted {
            result.scale_db = Some(db);
            result.band_change_db = evaluation.band_db;
            result.tail_change_db = evaluation.tail_db;
            result.level_db = 10. * (ratios[kept] * scale * scale).max(1e-30).log10();
            result.supports = candidates
                .iter()
                .zip(&supports)
                .map(|((speaker, low), fir)| MimoSupport {
                    speaker: speaker.clone(),
                    low: *low,
                    fir: fir.iter().map(|v| v * scale).collect(),
                })
                .collect();
            break;
        }
    }
    result.ear_limit = last_rejected;
    Ok((
        result,
        MimoStages {
            x,
            g,
            lp,
            direct,
            normal,
            rhs,
            logs,
            ratios,
            kept,
            u,
            guard_band,
            guard_tail,
            guard_steps,
        },
    ))
}
fn apply_input_high_pass(spectra: &mut [Vec<Vec<Complex64>>], response: &[Complex64]) {
    for spectrum in spectra.iter_mut().flatten() {
        assert_eq!(spectrum.len(), response.len());
        for (value, high_pass) in spectrum.iter_mut().zip(response) {
            *value *= high_pass;
        }
    }
}

pub(super) fn long_spectrum(ir: &crate::ir::ImpulseResponse, b: usize, n: usize) -> Vec<Complex64> {
    let a = onset(&ir.data).saturating_sub((0.002 * ir.fs as f64) as usize);
    let mut w = ir.data[a..(a + 2 * ir.fs as usize).min(ir.len())].to_vec();
    taper(&mut w, 0.1);
    rfft(&w, n)
        .into_iter()
        .zip(grid(n, ir.fs as f64))
        .map(|(v, f)| {
            v * Complex64::from_polar(1., -2. * PI * f * (a as f64 - b as f64) / ir.fs as f64)
        })
        .collect()
}
pub fn design_supports(
    brirs: &Hrir,
    inputs: &[MimoInput<'_>],
    opts: &MimoOptions,
) -> Result<MimoDesign, DspError> {
    design_supports_observed(brirs, inputs, opts, &|_, _| {})
}
/// Observe frozen-input stage boundaries without retaining all normal matrices.
/// Called in input workers; observers must not change the design.
pub fn design_supports_observed(
    brirs: &Hrir,
    inputs: &[MimoInput<'_>],
    opts: &MimoOptions,
    observe: &(impl Fn(&str, &MimoStages) + Sync),
) -> Result<MimoDesign, DspError> {
    design_supports_observed_with_spectra(brirs, inputs, opts, None, observe)
}

fn design_supports_observed_with_spectra(
    brirs: &Hrir,
    inputs: &[MimoInput<'_>],
    opts: &MimoOptions,
    spectra_override: Option<Vec<Vec<Vec<Complex64>>>>,
    observe: &(impl Fn(&str, &MimoStages) + Sync),
) -> Result<MimoDesign, DspError> {
    let n = analysis_len(brirs.fs);
    let f = grid(n, brirs.fs as f64);
    if brirs.fs < 44100
        || !opts.fsiso.is_finite()
        || !(50. ..=150.).contains(&opts.fsiso)
        || opts.low.is_some_and(|v| !v.is_finite() || v <= 0.)
        || opts
            .input_hp
            .is_some_and(|value| !value.is_finite() || !(10.0..=50.0).contains(&value))
        || !opts.early_ms.is_finite()
        || !(5.0..=60.0).contains(&opts.early_ms)
        || brirs
            .speakers
            .iter()
            .flat_map(|s| s.left.iter().chain(&s.right))
            .any(|ir| {
                ir.fs != brirs.fs || ir.data.is_empty() || ir.data.iter().any(|v| !v.is_finite())
            })
        || inputs.iter().any(|i| {
            !i.gain_db.is_finite()
                || i.response.len() != n / 2 + 1
                || i.response
                    .iter()
                    .any(|v| !v.re.is_finite() || !v.im.is_finite())
                || brirs.get(i.speaker).is_none()
        })
    {
        return Err(DspError::InvalidArgument(
            "invalid MIMO measurements/options".into(),
        ));
    }
    let speakers: Vec<_> = brirs
        .speakers
        .iter()
        .filter(|s| s.left.is_some() && s.right.is_some())
        .collect();
    let b = speakers
        .iter()
        .map(|s| onset(&s.left.as_ref().unwrap().data))
        .min()
        .unwrap_or(0);
    let mut spectra: Vec<Vec<_>> = spectra_override.unwrap_or_else(|| {
        speakers
            .iter()
            .map(|s| {
                [s.left.as_ref().unwrap(), s.right.as_ref().unwrap()]
                    .iter()
                    .map(|ir| long_spectrum(ir, b, n))
                    .collect()
            })
            .collect()
    });
    if spectra.len() != speakers.len()
        || spectra.iter().any(|ears| {
            ears.len() != 2
                || ears.iter().any(|spectrum| {
                    spectrum.len() != n / 2 + 1
                        || spectrum
                            .iter()
                            .any(|value| !value.re.is_finite() || !value.im.is_finite())
                })
        })
    {
        return Err(DspError::InvalidArgument(
            "invalid MIMO spectrum override".into(),
        ));
    }
    if let Some(cutoff) = opts.input_hp {
        apply_input_high_pass(&mut spectra, &hp8(&f, cutoff));
    }
    let ranges: Vec<_> = speakers
        .iter()
        .zip(&spectra)
        .map(|(s, h)| {
            let p: Vec<_> = h[0]
                .iter()
                .zip(&h[1])
                .map(|(a, b)| (a.norm_sqr() + b.norm_sqr()) / 2.)
                .collect();
            let d: Vec<_> = smooth(&p, &f, &[3.])
                .iter()
                .map(|p| 10. * (p + 1e-30).log10())
                .collect();
            let reference = median(
                &f.iter()
                    .zip(&d)
                    .filter(|(f, _)| **f >= 200. && **f <= 1000.)
                    .map(|(_, v)| *v)
                    .collect::<Vec<_>>(),
            );
            let low = geomspace(opts.fsiso.min(200.), 15., 200)
                .into_iter()
                .find(|v| interp(*v, &f, &d) < reference - 10.)
                .unwrap_or(15.)
                .max(50.);
            let high = geomspace(opts.fsiso.min(200.), opts.fsiso, 200)
                .into_iter()
                .find(|v| interp(*v, &f, &d) < reference - 10.)
                .unwrap_or(opts.fsiso);
            let range = if low < 0.8 * high.min(opts.fsiso) {
                let lo = ((low * 10.).round_ties_even() / 10.).max(opts.low.unwrap_or(0.));
                let hi = (high * 10.).round_ties_even() / 10.;
                (lo < 0.8 * hi).then_some((lo, hi))
            } else {
                None
            };
            (s.speaker.clone(), range)
        })
        .collect();
    let paths: Vec<_> = spectra
        .iter()
        .zip(&ranges)
        .map(|(h, (_, range))| {
            range.map(|(lo, _)| {
                let hp = hp4(&f, lo);
                h.iter()
                    .map(|h| {
                        low_rate(
                            &h.iter().zip(&hp).map(|(h, hp)| h * hp).collect::<Vec<_>>(),
                            &f,
                            N,
                        )
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let design_one = |input: &MimoInput| {
        let index = speakers
            .iter()
            .position(|s| s.speaker == input.speaker)
            .ok_or_else(|| DspError::InvalidArgument("MIMO input requires both ears".into()))?;
        let indices: Vec<_> = paths
            .iter()
            .enumerate()
            .filter(|(i, p)| *i != index && p.is_some())
            .map(|(i, _)| i)
            .collect();
        if indices.is_empty() {
            return Ok(MimoInputResult {
                speaker: input.speaker.into(),
                candidates: vec![],
                supports: vec![],
                level_db: -300.,
                scale_db: None,
                band_change_db: -99.,
                tail_change_db: -99.,
                ear_limit: None,
            });
        }
        let candidates: Vec<_> = indices
            .iter()
            .map(|i| (ranges[*i].0.clone(), ranges[*i].1.unwrap().0))
            .collect();
        let x = spectra[index]
            .iter()
            .map(|h| {
                low_rate(
                    &h.iter()
                        .zip(input.response)
                        .map(|(h, r)| h * 10_f64.powf(input.gain_db / 20.) * r)
                        .collect::<Vec<_>>(),
                    &f,
                    N,
                )
            })
            .collect();
        let g = (0..2)
            .map(|e| {
                indices
                    .iter()
                    .map(|i| paths[*i].as_ref().unwrap()[e].clone())
                    .collect()
            })
            .collect();
        let direct = low_rate(input.response, &f, N).iter().map(|v| v * v).sum();
        design_paths_with(
            input.speaker,
            &candidates,
            x,
            g,
            firwin(31, opts.fsiso, 600.),
            direct,
            opts.fsiso,
            MimoRule {
                early: early_samples(opts.early_ms),
                ear_guard: opts.ear_guard,
            },
        )
        .map(|(result, stages)| {
            observe(input.speaker, &stages);
            result
        })
    };
    let at_once = designs_at_once(
        paths
            .iter()
            .filter(|p| p.is_some())
            .count()
            .saturating_sub(1),
    );
    let mut results = Vec::with_capacity(inputs.len());
    for group in inputs.chunks(at_once) {
        results.extend(group.par_iter().map(design_one).collect::<Vec<_>>());
    }
    Ok(MimoDesign {
        ranges,
        inputs: results.into_iter().collect::<Result<_, _>>()?,
        early_ms: opts.early_ms,
        ear_guard: opts.ear_guard,
    })
}
pub fn support_response(
    support: &MimoSupport,
    fs: u32,
    n: usize,
) -> Result<Vec<Complex64>, DspError> {
    if fs < 44100
        || n < 2
        || !n.is_multiple_of(2)
        || support.fir.len() != 210
        || support.fir.iter().any(|v| !v.is_finite())
        || !support.low.is_finite()
        || support.low <= 0.
    {
        return Err(DspError::InvalidArgument(
            "invalid MIMO support dimensions or values".into(),
        ));
    }
    let f = grid(n, fs as f64);
    let hp = hp4(&f, support.low);
    let response: Vec<_> = f
        .iter()
        .zip(hp)
        .map(|(f, hp)| {
            if *f >= 285. {
                return Complex64::new(0., 0.);
            }
            let s = support
                .fir
                .iter()
                .enumerate()
                .map(|(i, v)| Complex64::from_polar(*v, -2. * PI * f * i as f64 / 600.))
                .sum::<Complex64>();
            hp * s * ((285. - f) / 60.).clamp(0., 1.)
        })
        .collect();
    Ok(response)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configurable_early_window_and_default_wrappers_match() {
        assert_eq!(early_samples(5.0), 3);
        assert_eq!(early_samples(7.5), 5);
        assert_eq!(early_samples(60.0), 36);

        let mut x = vec![vec![0.0; N]; 2];
        x[0][4] = 1.0;
        x[0][90] = 0.2;
        x[1][7] = 0.8;
        x[1][110] = -0.15;
        let mut c = vec![vec![vec![0.0; N]]; 2];
        c[0][0][3] = 0.7;
        c[0][0][80] = 0.1;
        c[1][0][5] = -0.6;
        c[1][0][100] = 0.12;
        let (default_matrix, default_rhs) = normal_equations(&x, &c).unwrap();
        let (explicit_matrix, explicit_rhs) =
            normal_equations_with(&x, &c, MIMO_EARLY_SAMPLES_DEFAULT).unwrap();
        assert_eq!(default_rhs, explicit_rhs);
        for column in 0..default_matrix.ncols() {
            assert!(
                default_matrix
                    .col(column)
                    .iter()
                    .zip(explicit_matrix.col(column).iter())
                    .all(|(left, right)| left.to_bits() == right.to_bits())
            );
        }
    }

    #[test]
    fn seat_change_reserves_ninety_nine_for_new_energy_only() {
        assert_eq!(seat_change(0.0, 0.0), None);
        assert_eq!(seat_change(0.0, 1e-300), Some(99.0));
        assert_eq!(seat_change(1.0, 1.0), Some(0.0));
        // Finite energies whose ratio overflows keep the logarithmic branch.
        assert_eq!(seat_change(1e-31, 1e300), Some(f64::INFINITY));
        assert!(seat_change(1e-31, 1e300) > Some(120.0));
        // An early window far past the response leaves no late samples.
        let mut x = vec![vec![0.0; N]; 2];
        x[0][1] = 1.0;
        x[1][1] = 1.0;
        let c = vec![vec![vec![0.5; N]]; 2];
        let (_, rhs) = normal_equations_with(&x, &c, usize::MAX).unwrap();
        assert!(rhs.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn ear_guard_rejects_a_change_hidden_by_the_ear_sum() {
        let mut x = vec![vec![0.0; N]; 2];
        let mut y = vec![vec![0.0; N]; 2];
        for ear in 0..2 {
            x[ear][0] = 1.0;
            x[ear][100] = 0.5;
            x[ear][250] = 0.5;
            y[ear][0] = 1.0;
        }
        for index in [100, 250] {
            y[0][index] = 0.25;
            y[1][index] = 0.65;
        }
        let summed = guard_evaluation(&x, &y, 150.0, false);
        let per_ear = guard_evaluation(&x, &y, 150.0, true);
        assert!(summed.band_db <= 1.0 && summed.tail_db <= 1.0);
        assert!(per_ear.band_db > 1.0 || per_ear.tail_db > 1.0);
        assert_eq!(per_ear.band_ear, Some(1));
    }

    #[test]
    fn default_design_wrapper_matches_explicit_rule() {
        let mut x = vec![vec![0.0; N]; 2];
        let mut g = vec![vec![vec![0.0; N]]; 2];
        for ear in 0..2 {
            x[ear][0] = 1.0;
            x[ear][100 + ear * 10] = 0.2;
            g[ear][0][5 + ear] = 0.8;
            g[ear][0][120 + ear * 10] = 0.1;
        }
        let mut lp = vec![0.0; 31];
        lp[15] = 1.0;
        let candidates = vec![("FR".to_owned(), 50.0)];
        let default = design_paths(
            "FL",
            &candidates,
            x.clone(),
            g.clone(),
            lp.clone(),
            1.0,
            150.0,
        )
        .unwrap();
        let explicit =
            design_paths_with("FL", &candidates, x, g, lp, 1.0, 150.0, MimoRule::default())
                .unwrap();
        assert_eq!(default.0, explicit.0);
        assert_eq!(default.1.rhs, explicit.1.rhs);
        assert_eq!(default.1.logs, explicit.1.logs);
        assert_eq!(default.1.ratios, explicit.1.ratios);
        assert_eq!(default.1.guard_steps, explicit.1.guard_steps);
        assert!(default.0.ear_limit.is_none());
    }

    #[test]
    fn input_high_pass_reaches_every_mimo_design_boundary_bit_for_bit() {
        use crate::{hrir::SpeakerIrs, ir::ImpulseResponse};
        use std::sync::Mutex;

        let impulse = |offset: usize, gain: f64| {
            let mut data = vec![0.0; 4096];
            data[offset] = gain;
            ImpulseResponse {
                fs: 48_000,
                data,
                recording: None,
            }
        };
        let brirs = Hrir {
            fs: 48_000,
            speakers: [("FL", 100, 1.0), ("FR", 110, 0.8), ("FC", 120, 0.6)]
                .into_iter()
                .map(|(speaker, offset, gain)| SpeakerIrs {
                    speaker: speaker.into(),
                    left: Some(impulse(offset, gain)),
                    right: Some(impulse(offset + 3, gain * 0.9)),
                })
                .collect(),
        };
        let n = analysis_len(brirs.fs);
        let frequencies = grid(n, brirs.fs as f64);
        let common_onset = brirs
            .speakers
            .iter()
            .map(|speaker| onset(&speaker.left.as_ref().unwrap().data))
            .min()
            .unwrap();
        let raw: Vec<Vec<Vec<_>>> = brirs
            .speakers
            .iter()
            .map(|speaker| {
                [
                    speaker.left.as_ref().unwrap(),
                    speaker.right.as_ref().unwrap(),
                ]
                .iter()
                .map(|ir| long_spectrum(ir, common_onset, n))
                .collect()
            })
            .collect();
        let correction = vec![Complex64::new(1.0, 0.0); n / 2 + 1];
        let inputs = [MimoInput {
            speaker: "FL",
            gain_db: -1.5,
            response: &correction,
        }];
        let cut_stages = Mutex::new(None);
        let cut = design_supports_observed_with_spectra(
            &brirs,
            &inputs,
            &MimoOptions {
                fsiso: 120.0,
                low: None,
                input_hp: Some(25.0),
                early_ms: MIMO_EARLY_MS_DEFAULT,
                ear_guard: false,
            },
            Some(raw.clone()),
            &|_, stages| *cut_stages.lock().unwrap() = Some(stages.clone()),
        )
        .unwrap();

        let high_pass = hp8(&frequencies, 25.0);
        let mut premultiplied = raw;
        for spectrum in premultiplied.iter_mut().flatten() {
            for (value, filter) in spectrum.iter_mut().zip(&high_pass) {
                *value *= filter;
            }
        }
        let premultiplied_stages = Mutex::new(None);
        let premultiplied_result = design_supports_observed_with_spectra(
            &brirs,
            &inputs,
            &MimoOptions {
                fsiso: 120.0,
                low: None,
                input_hp: None,
                early_ms: MIMO_EARLY_MS_DEFAULT,
                ear_guard: false,
            },
            Some(premultiplied),
            &|_, stages| *premultiplied_stages.lock().unwrap() = Some(stages.clone()),
        )
        .unwrap();

        assert_eq!(cut, premultiplied_result, "ranges and supports");
        let cut = cut_stages.into_inner().unwrap().unwrap();
        let premultiplied = premultiplied_stages.into_inner().unwrap().unwrap();
        assert_eq!(cut.x, premultiplied.x, "MIMO input x");
        assert_eq!(cut.g, premultiplied.g, "support paths");
        assert_eq!(cut.guard_band, premultiplied.guard_band, "guard bands");
        assert_eq!(cut.guard_tail, premultiplied.guard_tail, "guard tails");
        assert_eq!(cut.ratios, premultiplied.ratios, "strength ratios");
        assert_eq!(cut.direct, premultiplied.direct, "ratio denominator");
        let expected_direct: f64 = low_rate(&correction, &frequencies, N)
            .iter()
            .map(|value| value * value)
            .sum();
        assert_eq!(cut.direct, expected_direct);
    }

    #[test]
    fn mimo_designs_in_parallel_stay_within_the_memory_budget() {
        assert_eq!(super::designs_at_once(39), 1);
        assert_eq!(super::designs_at_once(20), 2);
        assert_eq!(super::designs_at_once(6), 28);
        assert_eq!(super::designs_at_once(0), super::MIMO_MEMORY_BUDGET);
    }
}
