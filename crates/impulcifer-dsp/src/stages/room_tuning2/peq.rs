use crate::{DspError, fft::Complex64, optimize};
use std::f64::consts::PI;
pub(super) type Bands = Vec<(f64, f64, f64)>;
fn ring(f: f64, g: f64) -> f64 {
    max_q_for_ring(f, g, 30.)
}
pub fn max_q_for_ring(f: f64, g: f64, ring_ms: f64) -> f64 {
    PI * f * ring_ms / 1000. / 10_f64.powf(g / 40.)
}
/// Narrow the supplied per-band logarithmic bounds using the ring-time cap.
pub fn ring_time_bounds(bands: &[(f64, f64, f64)], lb: &mut [f64], ub: &mut [f64], ring_ms: f64) {
    assert_eq!(lb.len(), bands.len() * 3);
    assert_eq!(ub.len(), lb.len());
    for (i, &(f, _, g)) in bands.iter().enumerate() {
        let k = i * 3;
        lb[k] = lb[k].max((f / 1.2).ln());
        ub[k] = ub[k].min((1.2 * f).ln()).max(lb[k] + 1e-3);
        if g > 0. {
            lb[k + 2] = 0.;
            ub[k + 2] = ub[k + 2].min(g + 3.);
        } else {
            ub[k + 2] = (g / 2.).min(-0.25);
        }
        ub[k + 1] = ub[k + 1]
            .min(max_q_for_ring(f / 1.2, ub[k + 2], ring_ms).max(0.31).ln())
            .max(0.31_f64.ln());
    }
}
// NumPy's complex divide avoids squaring the denominator and has a different
// rounding path from num_complex's default division.
fn divide(a: Complex64, b: Complex64) -> Complex64 {
    if b.re.abs() >= b.im.abs() {
        let r = b.im / b.re;
        let d = 1. / (b.re + b.im * r);
        Complex64::new((a.re + a.im * r) * d, (a.im - a.re * r) * d)
    } else {
        let r = b.re / b.im;
        let d = 1. / (b.re * r + b.im);
        Complex64::new((a.re * r + a.im) * d, (a.im * r - a.re) * d)
    }
}
/// NumPy's baseline SIMD complex absolute uses a scaled square root, not libm hypot.
pub fn magnitude(z: Complex64) -> f64 {
    let re = z.re.abs();
    let im = z.im.abs();
    if re.is_infinite() || im.is_infinite() {
        return f64::INFINITY;
    }
    if re.is_nan() || im.is_nan() {
        return f64::NAN;
    }
    let larger = re.max(im);
    let smaller = re.min(im);
    if larger == 0. {
        return 0.;
    }
    let ratio = smaller / larger;
    (ratio * ratio + 1.).sqrt() * larger
}
fn band(z: &[Complex64], f: f64, q: f64, g: f64, fs: f64) -> Vec<Complex64> {
    let mut out = vec![Complex64::new(0., 0.); z.len()];
    band_into(z, f, q, g, fs, &mut out);
    out
}
fn band_into(z: &[Complex64], f: f64, q: f64, g: f64, fs: f64, out: &mut [Complex64]) {
    let a = 10_f64.powf(g / 40.);
    let w = 2. * PI * f / fs;
    let alpha = w.sin() / (2. * q);
    let b = [1. + alpha * a, -2. * w.cos(), 1. - alpha * a];
    let a = [1. + alpha / a, -2. * w.cos(), 1. - alpha / a];
    pulp::Arch::new().dispatch(|| {
        for (out, z) in out.iter_mut().zip(z) {
            *out = divide(
                b[0] + b[1] * z + b[2] * z * z,
                a[0] + a[1] * z + a[2] * z * z,
            );
        }
    });
}
pub fn response(f: &[f64], bands: &[(f64, f64, f64)], fs: f64) -> Vec<Complex64> {
    let z: Vec<_> = f
        .iter()
        // NumPy divides the complex exponent by fs via its reciprocal.
        .map(|f| Complex64::from_polar(1., (-2. * PI * f) * (1. / fs)))
        .collect();
    let mut out = vec![Complex64::new(1., 0.); f.len()];
    for &(f, q, g) in bands {
        for (o, p) in out.iter_mut().zip(band(&z, f, q, g, fs)) {
            *o *= p;
        }
    }
    out
}
#[allow(clippy::too_many_arguments)]
pub(super) fn fit(
    grid: &[f64],
    curve: &[f64],
    n: usize,
    lo: f64,
    hi: f64,
    boost: f64,
    fmax: Option<f64>,
    fs: f64,
) -> Result<Bands, DspError> {
    fit_with_trace(grid, curve, n, lo, hi, boost, fmax, fs, None)
}
/// A fit has no trace when its greedy start selects no bands.
#[derive(Debug)]
pub struct FitTrace {
    pub x0: Vec<f64>,
    pub lb: Vec<f64>,
    pub ub: Vec<f64>,
    pub solver: optimize::LeastSquaresTrace,
    pub result: optimize::LeastSquaresResult,
    pub bands: Vec<(f64, f64, f64)>,
}
#[allow(clippy::too_many_arguments)]
pub fn fit_with_trace(
    grid: &[f64],
    curve: &[f64],
    n: usize,
    lo: f64,
    hi: f64,
    boost: f64,
    fmax: Option<f64>,
    fs: f64,
    mut trace: Option<&mut Option<FitTrace>>,
) -> Result<Vec<(f64, f64, f64)>, DspError> {
    if let Some(t) = trace.as_deref_mut() {
        *t = None;
    }
    let mut bands = Vec::new();
    for _ in 0..n {
        let current = response(grid, &bands, fs);
        let r: Vec<_> = grid
            .iter()
            .enumerate()
            .map(|(i, f)| {
                if *f >= lo && *f <= hi {
                    curve[i] - 20. * (magnitude(current[i]) + 1e-12).log10()
                } else {
                    0.
                }
            })
            .collect();
        let k = super::primitives::argmax(&r.iter().map(|x| x.abs()).collect::<Vec<_>>());
        if r[k].abs() < 0.5 {
            break;
        }
        let gain = r[k].clamp(-25., boost);
        let mut i0 = k;
        let mut i1 = k;
        while i0 > 0 && r[i0 - 1].abs() >= r[k].abs() / 2. && r[i0 - 1].signum() == r[k].signum() {
            i0 -= 1;
        }
        while i1 + 1 < r.len()
            && r[i1 + 1].abs() >= r[k].abs() / 2.
            && r[i1 + 1].signum() == r[k].signum()
        {
            i1 += 1;
        }
        let q = (grid[k] / (grid[i1] - grid[i0]).max(0.02 * grid[k]))
            .clamp(0.5, 15.)
            .min(ring(grid[k], gain))
            .max(0.31);
        bands.push((grid[k], q, gain));
    }
    if bands.is_empty() {
        return Ok(bands);
    }
    let mut x = Vec::new();
    let mut lb = Vec::new();
    let mut ub = Vec::new();
    let top = fmax.unwrap_or((1.2 * hi).max(2000.));
    for &(f, q, g) in &bands {
        lb.extend([15_f64.ln(), 0.3_f64.ln(), -25.]);
        ub.extend([top.ln(), 20_f64.ln(), boost]);
        x.extend([f.ln(), q.ln(), g]);
    }
    ring_time_bounds(&bands, &mut lb, &mut ub, 30.);
    for i in 0..x.len() {
        x[i] = x[i].clamp(lb[i] + 1e-6, ub[i] - 1e-6);
    }
    let z: Vec<_> = grid
        .iter()
        // NumPy divides the complex exponent by fs via its reciprocal.
        .map(|f| Complex64::from_polar(1., (-2. * PI * f) * (1. / fs)))
        .collect();
    let mut cache = vec![vec![Complex64::new(1., 0.); grid.len()]; bands.len()];
    let mut prefix = vec![vec![Complex64::new(1., 0.); grid.len()]; bands.len() + 1];
    let mut changed = vec![Complex64::new(0., 0.); grid.len()];
    let mut product = vec![Complex64::new(1., 0.); grid.len()];
    let mut solver_trace = trace
        .as_ref()
        .map(|_| optimize::LeastSquaresTrace::default());
    let result = optimize::least_squares_in_place(
        |x, column, out| {
            let _timer = super::profile::timer("residual");
            pulp::Arch::new().dispatch(|| {
                if let Some(column) = column {
                    let k = column / 3;
                    band_into(
                        &z,
                        x[k * 3].exp(),
                        x[k * 3 + 1].exp(),
                        x[k * 3 + 2],
                        fs,
                        &mut changed,
                    );
                    product.copy_from_slice(&prefix[k]);
                    for (p, b) in product.iter_mut().zip(&changed) {
                        *p *= b;
                    }
                    for band in &cache[k + 1..] {
                        for (p, b) in product.iter_mut().zip(band) {
                            *p *= b;
                        }
                    }
                } else {
                    for k in 0..cache.len() {
                        band_into(
                            &z,
                            x[k * 3].exp(),
                            x[k * 3 + 1].exp(),
                            x[k * 3 + 2],
                            fs,
                            &mut cache[k],
                        );
                        let (before, after) = prefix.split_at_mut(k + 1);
                        after[0].copy_from_slice(&before[k]);
                        for (p, b) in after[0].iter_mut().zip(&cache[k]) {
                            *p *= b;
                        }
                    }
                    product.copy_from_slice(prefix.last().unwrap());
                }
                out.resize(grid.len(), 0.);
                for (o, p) in out.iter_mut().zip(&product) {
                    *o = magnitude(*p) + 1e-12;
                }
            });
            for (i, o) in out.iter_mut().enumerate() {
                *o = (if grid[i] >= lo && grid[i] <= hi {
                    1.
                } else {
                    0.15
                }) * (20. * o.log10() - curve[i]);
            }
        },
        &x,
        &lb,
        &ub,
        150,
        solver_trace.as_mut(),
    )?;
    let bands: Vec<_> = result
        .x
        .as_chunks::<3>()
        .0
        .iter()
        .map(|x| {
            let f = x[0].exp();
            (f, x[1].exp().min(ring(f, x[2])), x[2])
        })
        .collect();
    if let Some(t) = trace {
        *t = Some(FitTrace {
            x0: x,
            lb,
            ub,
            solver: solver_trace.unwrap(),
            result,
            bands: bands.clone(),
        });
    }
    Ok(bands)
}
