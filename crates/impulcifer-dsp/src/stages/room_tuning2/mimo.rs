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

#[derive(Clone, Debug)]
pub struct MimoOptions {
    pub fsiso: f64,
    pub low: Option<f64>,
}
pub struct MimoInput<'a> {
    pub speaker: &'a str,
    pub gain_db: f64,
    pub response: &'a [Complex64],
}
#[derive(Clone, Debug)]
pub struct MimoSupport {
    pub speaker: String,
    pub low: f64,
    pub fir: Vec<f64>,
}
#[derive(Clone, Debug)]
pub struct MimoInputResult {
    pub speaker: String,
    pub candidates: Vec<String>,
    pub supports: Vec<MimoSupport>,
    pub level_db: f64,
    pub scale_db: Option<f64>,
    pub band_change_db: f64,
    pub tail_change_db: f64,
}
#[derive(Clone, Debug)]
pub struct MimoDesign {
    pub ranges: Vec<(String, Option<(f64, f64)>)>,
    pub inputs: Vec<MimoInputResult>,
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
}
const N: usize = 720;
const TAPS: usize = 180;
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
    validate_paths(x, c)?;
    let r = c[0].len();
    let m = r * TAPS;
    let mut a = Mat::zeros(m, m);
    let mut b = vec![0.; m];
    for (x, c) in x.iter().zip(c) {
        let early = (first(x) + 36).min(N);
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
fn guard(x: &[Vec<f64>], y: &[Vec<f64>], fsiso: f64) -> (f64, f64) {
    let peaks = late_peaks(x, 6);
    let mut edges = Vec::new();
    let mut i = 0;
    while i as f64 / 3. <= (fsiso / 20.).log2() + 1e-9 {
        edges.push(20. * 2_f64.powf(i as f64 / 3.));
        i += 1;
    }
    let mut bands: Vec<_> = edges.windows(2).map(|p| (p[0], p[1])).collect();
    bands.extend(peaks.iter().map(|p| (p - 1.5, p + 1.5)));
    let mut before = vec![0.; N + 1];
    let mut after = before.clone();
    for (x, y) in x.iter().zip(y) {
        let end = (first(x) + 36).min(N);
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
    let f = grid(2 * N, 600.);
    let mut band: f64 = -99.;
    for (lo, hi) in bands {
        let mut a = 0.;
        let mut b = 0.;
        for k in 0..f.len() {
            if f[k] >= lo && f[k] < hi {
                a += before[k];
                b += after[k];
            }
        }
        if a > 0. {
            band = band.max(10. * ((b + 1e-30) / a).log10());
        }
    }
    let mut modes: Vec<_> = peaks
        .into_iter()
        .chain(late_peaks(y, 6))
        .map(|p| (p * 10.).round_ties_even() / 10.)
        .collect();
    modes.sort_by(f64::total_cmp);
    modes.dedup();
    let energy: Vec<_> = modes
        .iter()
        .map(|p| {
            let sos = filters::butter_bandpass(2, (p - 1.5).max(1.) / 300., (p + 1.5) / 300.);
            let mut a = 0.;
            let mut b = 0.;
            for (x, y) in x.iter().zip(y) {
                let k = (first(x) + 180).min(N);
                a += filters::sosfilt(&sos, x)[k..]
                    .iter()
                    .map(|v| v * v)
                    .sum::<f64>();
                b += filters::sosfilt(&sos, y)[k..]
                    .iter()
                    .map(|v| v * v)
                    .sum::<f64>();
            }
            (a, b)
        })
        .collect();
    let max = energy.iter().map(|v| v.0).fold(0., f64::max);
    let tail = energy
        .iter()
        .filter(|(a, _)| *a > 0. && *a >= 0.01 * max)
        .map(|(a, b)| 10. * ((b + 1e-30) / a).log10())
        .fold(-99., f64::max);
    (band, tail)
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
    let (normal, rhs) = normal_equations(&x, &c)?;
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
    };
    let mut guard_band = vec![];
    let mut guard_tail = vec![];
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
        let (band, tail) = guard(&x, &y, fsiso);
        guard_band.push(band);
        guard_tail.push(tail);
        if band <= 1. && tail <= 1. {
            result.scale_db = Some(db);
            result.band_change_db = band;
            result.tail_change_db = tail;
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
        },
    ))
}
fn long_spectrum(ir: &crate::ir::ImpulseResponse, b: usize, n: usize) -> Vec<Complex64> {
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
pub fn design_mimo(
    brirs: &Hrir,
    inputs: &[MimoInput<'_>],
    opts: &MimoOptions,
) -> Result<MimoDesign, DspError> {
    design_mimo_observed(brirs, inputs, opts, &|_, _| {})
}
/// Observe frozen-input stage boundaries without retaining all normal matrices.
/// Called in input workers; observers must not change the design.
pub fn design_mimo_observed(
    brirs: &Hrir,
    inputs: &[MimoInput<'_>],
    opts: &MimoOptions,
    observe: &(impl Fn(&str, &MimoStages) + Sync),
) -> Result<MimoDesign, DspError> {
    let n = analysis_len(brirs.fs);
    let f = grid(n, brirs.fs as f64);
    if brirs.fs < 44100
        || !opts.fsiso.is_finite()
        || !(50. ..=150.).contains(&opts.fsiso)
        || opts.low.is_some_and(|v| !v.is_finite() || v <= 0.)
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
    let spectra: Vec<Vec<_>> = speakers
        .iter()
        .map(|s| {
            [s.left.as_ref().unwrap(), s.right.as_ref().unwrap()]
                .iter()
                .map(|ir| long_spectrum(ir, b, n))
                .collect()
        })
        .collect();
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
    let results: Result<Vec<_>, _> = inputs
        .par_iter()
        .map(|input| {
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
            design_paths(
                input.speaker,
                &candidates,
                x,
                g,
                firwin(31, opts.fsiso, 600.),
                direct,
                opts.fsiso,
            )
            .map(|(result, stages)| {
                observe(input.speaker, &stages);
                result
            })
        })
        .collect();
    Ok(MimoDesign {
        ranges,
        inputs: results?,
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
    Ok(f.iter()
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
        .collect())
}
