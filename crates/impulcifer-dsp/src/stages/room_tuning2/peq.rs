use crate::{DspError, fft::Complex64, optimize};
use std::f64::consts::PI;
pub(super) type Bands = Vec<(f64, f64, f64)>;
fn ring(f: f64, g: f64) -> f64 {
    PI * f * 30. / 1000. / 10_f64.powf(g / 40.)
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
fn band(z: &[Complex64], f: f64, q: f64, g: f64, fs: f64) -> Vec<Complex64> {
    let a = 10_f64.powf(g / 40.);
    let w = 2. * PI * f / fs;
    let alpha = w.sin() / (2. * q);
    let b = [1. + alpha * a, -2. * w.cos(), 1. - alpha * a];
    let a = [1. + alpha / a, -2. * w.cos(), 1. - alpha / a];
    z.iter()
        .map(|z| {
            divide(
                b[0] + b[1] * z + b[2] * z * z,
                a[0] + a[1] * z + a[2] * z * z,
            )
        })
        .collect()
}
pub(super) fn response(f: &[f64], bands: &[(f64, f64, f64)], fs: f64) -> Vec<Complex64> {
    let z: Vec<_> = f
        .iter()
        .map(|f| Complex64::from_polar(1., -2. * PI * f / fs))
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
    let mut bands = Vec::new();
    for _ in 0..n {
        let current = response(grid, &bands, fs);
        let r: Vec<_> = grid
            .iter()
            .enumerate()
            .map(|(i, f)| {
                if *f >= lo && *f <= hi {
                    curve[i] - 20. * (current[i].norm() + 1e-12).log10()
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
        let l = 15_f64.ln().max((f / 1.2).ln());
        let u = top.ln().min((1.2 * f).ln()).max(l + 1e-3);
        let gu = if g > 0. {
            (g + 3.).min(boost)
        } else {
            (g / 2.).min(-0.25)
        };
        let gl = if g > 0. { 0. } else { -25. };
        lb.extend([l, 0.3_f64.ln(), gl]);
        ub.extend([u, 20_f64.min(ring(f / 1.2, gu)).max(0.31).ln(), gu]);
        x.extend([f.ln(), q.ln(), g]);
    }
    for i in 0..x.len() {
        x[i] = x[i].clamp(lb[i] + 1e-6, ub[i] - 1e-6);
    }
    let z: Vec<_> = grid
        .iter()
        .map(|f| Complex64::from_polar(1., -2. * PI * f / fs))
        .collect();
    let mut old = vec![f64::NAN; x.len()];
    let mut cache = vec![Vec::new(); bands.len()];
    let result = optimize::least_squares(
        |x| {
            for k in 0..cache.len() {
                if x[k * 3..k * 3 + 3] != old[k * 3..k * 3 + 3] {
                    cache[k] = band(&z, x[k * 3].exp(), x[k * 3 + 1].exp(), x[k * 3 + 2], fs);
                    old[k * 3..k * 3 + 3].copy_from_slice(&x[k * 3..k * 3 + 3]);
                }
            }
            let mut p = vec![Complex64::new(1., 0.); grid.len()];
            for band in &cache {
                for (p, b) in p.iter_mut().zip(band) {
                    *p *= b;
                }
            }
            p.iter()
                .enumerate()
                .map(|(i, p)| {
                    (if grid[i] >= lo && grid[i] <= hi {
                        1.
                    } else {
                        0.15
                    }) * (20. * (p.norm() + 1e-12).log10() - curve[i])
                })
                .collect()
        },
        &x,
        &lb,
        &ub,
        150,
    )?;
    Ok(result
        .x
        .chunks_exact(3)
        .map(|x| {
            let f = x[0].exp();
            (f, x[1].exp().min(ring(f, x[2])), x[2])
        })
        .collect())
}
