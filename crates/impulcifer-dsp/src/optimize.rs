//! Bounded TRF, exact SVD solver, linear loss and bounded two-point differences.
//!
//! Port of SciPy 1.18.1 optimize/_lsq/{trf,common}.py and _numdiff.py,
//! Copyright SciPy Developers, BSD-3-Clause (see THIRD_PARTY_LICENSES.md).
//! Only x_scale=1 and ftol=xtol=gtol=1e-8 are supported. Jacobian evaluations
//! are deliberately excluded from nfev, as in scipy.optimize.least_squares.
use crate::DspError;
use faer::Mat;

#[derive(Debug)]
pub struct LeastSquaresResult {
    pub x: Vec<f64>,
    pub cost: f64,
    pub nfev: usize,
    pub status: i32,
}
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}
fn norm(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}
fn mv(a: &Mat<f64>, x: &[f64]) -> Vec<f64> {
    (0..a.nrows())
        .map(|i| (0..a.ncols()).map(|j| a[(i, j)] * x[j]).sum())
        .collect()
}
fn grad(a: &Mat<f64>, f: &[f64]) -> Vec<f64> {
    (0..a.ncols())
        .map(|j| (0..a.nrows()).map(|i| a[(i, j)] * f[i]).sum())
        .collect()
}
fn feasible(x: &mut [f64], lb: &[f64], ub: &[f64], initial: bool) {
    for ((x, &l), &u) in x.iter_mut().zip(lb).zip(ub) {
        if initial {
            let ld = *x - l;
            let ud = u - *x;
            if l.is_finite() && ld <= ud.min(1e-10 * l.abs().max(1.)) {
                *x = l + 1e-10 * l.abs().max(1.);
            }
            if u.is_finite() && ud <= ld.min(1e-10 * u.abs().max(1.)) {
                *x = u - 1e-10 * u.abs().max(1.);
            }
        } else {
            if *x <= l {
                *x = l.next_up();
            }
            if *x >= u {
                *x = u.next_down();
            }
        }
        if *x < l || *x > u {
            *x = 0.5 * (l + u);
        }
    }
}
fn jacobian<F: FnMut(&[f64]) -> Vec<f64>>(
    fun: &mut F,
    x: &[f64],
    f: &[f64],
    lb: &[f64],
    ub: &[f64],
) -> Result<Mat<f64>, DspError> {
    let mut j = Mat::zeros(f.len(), x.len());
    for k in 0..x.len() {
        let mut h = f64::EPSILON.sqrt() * if x[k] >= 0. { 1. } else { -1. } * x[k].abs().max(1.);
        let lower = x[k] - lb[k];
        let upper = ub[k] - x[k];
        let fitting = h.abs() <= lower.max(upper);
        if (x[k] + h < lb[k] || x[k] + h > ub[k]) && fitting {
            h = -h;
        }
        if !fitting {
            h = if upper >= lower { upper } else { -lower };
        }
        let mut y = x.to_vec();
        y[k] += h;
        let dx = y[k] - x[k];
        let fy = fun(&y);
        if fy.len() != f.len() || fy.iter().any(|x| !x.is_finite()) {
            return Err(DspError::InvalidArgument(
                "nonfinite or inconsistent residual Jacobian".into(),
            ));
        }
        for i in 0..f.len() {
            j[(i, k)] = (fy[i] - f[i]) / dx;
        }
    }
    Ok(j)
}
fn scaling(x: &[f64], g: &[f64], lb: &[f64], ub: &[f64]) -> (Vec<f64>, Vec<f64>) {
    (0..x.len())
        .map(|i| {
            if g[i] < 0. && ub[i].is_finite() {
                (ub[i] - x[i], -1.)
            } else if g[i] > 0. && lb[i].is_finite() {
                (x[i] - lb[i], 1.)
            } else {
                (1., 0.)
            }
        })
        .unzip()
}
fn bound_step(x: &[f64], s: &[f64], lb: &[f64], ub: &[f64]) -> (f64, Vec<bool>) {
    let steps: Vec<_> = (0..x.len())
        .map(|i| {
            if s[i] == 0. {
                f64::INFINITY
            } else {
                ((lb[i] - x[i]) / s[i]).max((ub[i] - x[i]) / s[i])
            }
        })
        .collect();
    let t = steps.iter().copied().fold(f64::INFINITY, f64::min);
    (t, steps.iter().map(|s| *s == t).collect())
}
fn quadratic(j: &Mat<f64>, g: &[f64], diag: &[f64], s: &[f64]) -> f64 {
    let js = mv(j, s);
    0.5 * (dot(&js, &js) + (0..s.len()).map(|i| s[i] * diag[i] * s[i]).sum::<f64>()) + dot(s, g)
}
fn quadratic_line(
    j: &Mat<f64>,
    g: &[f64],
    diag: &[f64],
    s: &[f64],
    s0: Option<&[f64]>,
) -> (f64, f64, f64) {
    let v = mv(j, s);
    let a = 0.5 * (dot(&v, &v) + (0..s.len()).map(|i| s[i] * diag[i] * s[i]).sum::<f64>());
    let mut b = dot(g, s);
    let mut c = 0.;
    if let Some(s0) = s0 {
        let u = mv(j, s0);
        b += dot(&u, &v);
        c = 0.5 * dot(&u, &u) + dot(g, s0);
        b += (0..s.len()).map(|i| s0[i] * diag[i] * s[i]).sum::<f64>();
        c += 0.5 * (0..s.len()).map(|i| s0[i] * diag[i] * s0[i]).sum::<f64>();
    }
    (a, b, c)
}
fn minimize((a, b, c): (f64, f64, f64), lo: f64, hi: f64) -> (f64, f64) {
    let value = |t: f64| t * (a * t + b) + c;
    let mut best = (lo, value(lo));
    if value(hi) < best.1 {
        best = (hi, value(hi));
    }
    let e = -0.5 * b / a;
    if a != 0. && lo < e && e < hi && value(e) < best.1 {
        best = (e, value(e));
    }
    best
}
#[allow(clippy::too_many_arguments)]
fn select(
    x: &[f64],
    j: &Mat<f64>,
    diag: &[f64],
    g: &[f64],
    mut p: Vec<f64>,
    mut ph: Vec<f64>,
    d: &[f64],
    delta: f64,
    lb: &[f64],
    ub: &[f64],
    theta: f64,
) -> (Vec<f64>, Vec<f64>, f64) {
    if (0..x.len()).all(|i| x[i] + p[i] >= lb[i] && x[i] + p[i] <= ub[i]) {
        let q = quadratic(j, g, diag, &ph);
        return (p, ph, -q);
    }
    let (stride, hits) = bound_step(x, &p, lb, ub);
    let mut rh: Vec<_> = ph
        .iter()
        .zip(hits)
        .map(|(v, h)| if h { -v } else { *v })
        .collect();
    let r: Vec<_> = rh.iter().zip(d).map(|(a, b)| a * b).collect();
    for v in &mut p {
        *v *= stride;
    }
    for v in &mut ph {
        *v *= stride;
    }
    let xb: Vec<_> = x.iter().zip(&p).map(|(a, b)| a + b).collect();
    let a = dot(&rh, &rh);
    let b = dot(&ph, &rh);
    let c = dot(&ph, &ph) - delta * delta;
    let root = (b * b - a * c).sqrt();
    let q = -(b + root.copysign(b));
    let tr = (q / a).max(c / q);
    let (bound, _) = bound_step(&xb, &r, lb, ub);
    let rs = bound.min(tr);
    let (lo, hi) = if rs > 0. {
        (
            (1. - theta) * stride / rs,
            if rs == bound { theta * bound } else { tr },
        )
    } else {
        (0., -1.)
    };
    let mut rv = f64::INFINITY;
    if lo <= hi {
        let (t, v) = minimize(quadratic_line(j, g, diag, &rh, Some(&ph)), lo, hi);
        rv = v;
        for i in 0..rh.len() {
            rh[i] = rh[i] * t + ph[i];
        }
    }
    for v in &mut p {
        *v *= theta;
    }
    for v in &mut ph {
        *v *= theta;
    }
    let pv = quadratic(j, g, diag, &ph);
    let mut ah: Vec<_> = g.iter().map(|v| -v).collect();
    let mut ag: Vec<_> = ah.iter().zip(d).map(|(a, b)| a * b).collect();
    let tr = delta / norm(&ah);
    let (bound, _) = bound_step(x, &ag, lb, ub);
    let upper = if bound < tr { theta * bound } else { tr };
    let (stride, av) = minimize(quadratic_line(j, g, diag, &ah, None), 0., upper);
    for v in &mut ah {
        *v *= stride;
    }
    for v in &mut ag {
        *v *= stride;
    }
    if pv < rv && pv < av {
        (p, ph, -pv)
    } else if rv < pv && rv < av {
        (rh.iter().zip(d).map(|(a, b)| a * b).collect(), rh, -rv)
    } else {
        (ag, ah, -av)
    }
}
fn trust(
    n: usize,
    m: usize,
    uf: &[f64],
    s: &[f64],
    v: &Mat<f64>,
    delta: f64,
    mut alpha: f64,
) -> (Vec<f64>, f64) {
    let suf: Vec<_> = s.iter().zip(uf).map(|(a, b)| a * b).collect();
    let full = m >= n && s[n - 1] > f64::EPSILON * m as f64 * s[0];
    if full {
        let p = mv(
            v,
            &uf.iter().zip(s).map(|(a, b)| -a / b).collect::<Vec<_>>(),
        );
        if norm(&p) <= delta {
            return (p, 0.);
        }
    }
    let phi = |a: f64| {
        let pn = norm(
            &suf.iter()
                .zip(s)
                .map(|(u, s)| u / (s * s + a))
                .collect::<Vec<_>>(),
        );
        (
            pn - delta,
            -suf.iter()
                .zip(s)
                .map(|(u, s)| u * u / (s * s + a).powi(3))
                .sum::<f64>()
                / pn,
        )
    };
    let mut upper = norm(&suf) / delta;
    let mut lower = if full {
        let (p, d) = phi(0.);
        -p / d
    } else {
        0.
    };
    if !full && alpha == 0. {
        alpha = (0.001 * upper).max((lower * upper).sqrt());
    }
    for _ in 0..10 {
        if alpha < lower || alpha > upper {
            alpha = (0.001 * upper).max((lower * upper).sqrt());
        }
        let (p, dp) = phi(alpha);
        if p < 0. {
            upper = alpha;
        }
        let ratio = p / dp;
        lower = lower.max(alpha - ratio);
        alpha -= (p + delta) * ratio / delta;
        if p.abs() < 0.01 * delta {
            break;
        }
    }
    let mut p = mv(
        v,
        &suf.iter()
            .zip(s)
            .map(|(u, s)| -u / (s * s + alpha))
            .collect::<Vec<_>>(),
    );
    let scale = delta / norm(&p);
    for v in &mut p {
        *v *= scale;
    }
    (p, alpha)
}
/// SciPy's bounded exact-TRF subset. The closure may cache computations but
/// must return the same residual at the same x. Bounds may be infinite.
pub fn least_squares<F: FnMut(&[f64]) -> Vec<f64>>(
    mut fun: F,
    x0: &[f64],
    lb: &[f64],
    ub: &[f64],
    max_nfev: usize,
) -> Result<LeastSquaresResult, DspError> {
    let n = x0.len();
    if n == 0
        || lb.len() != n
        || ub.len() != n
        || max_nfev == 0
        || (0..n).any(|i| {
            !x0[i].is_finite()
                || lb[i].is_nan()
                || ub[i].is_nan()
                || lb[i] >= ub[i]
                || x0[i] < lb[i]
                || x0[i] > ub[i]
        })
    {
        return Err(DspError::InvalidArgument(
            "invalid least-squares start/bounds/budget".into(),
        ));
    }
    let mut x = x0.to_vec();
    feasible(&mut x, lb, ub, true);
    let mut f = fun(&x);
    let m = f.len();
    if m == 0 || f.iter().any(|v| !v.is_finite()) {
        return Err(DspError::InvalidArgument("invalid initial residual".into()));
    }
    let mut j = jacobian(&mut fun, &x, &f, lb, ub)?;
    let mut cost = 0.5 * dot(&f, &f);
    let mut g = grad(&j, &f);
    let (v, _) = scaling(&x, &g, lb, ub);
    let mut delta = norm(
        &x.iter()
            .zip(v)
            .map(|(x, v)| x / v.sqrt())
            .collect::<Vec<_>>(),
    );
    if delta == 0. {
        delta = 1.;
    }
    let mut alpha = 0.;
    let mut nfev = 1;
    let mut status = 0;
    loop {
        let (v, dv) = scaling(&x, &g, lb, ub);
        let gn = g
            .iter()
            .zip(&v)
            .map(|(g, v)| (g * v).abs())
            .fold(0., f64::max);
        if gn < 1e-8 {
            status = 1;
        }
        if status != 0 || nfev == max_nfev {
            break;
        }
        let d: Vec<_> = v.iter().map(|v| v.sqrt()).collect();
        let diag: Vec<_> = g.iter().zip(dv).map(|(g, v)| g * v).collect();
        let gh: Vec<_> = d.iter().zip(&g).map(|(d, g)| d * g).collect();
        let jh = Mat::from_fn(m, n, |i, k| j[(i, k)] * d[k]);
        let aug = Mat::from_fn(m + n, n, |i, k| {
            if i < m {
                jh[(i, k)]
            } else if i - m == k {
                diag[k].sqrt()
            } else {
                0.
            }
        });
        // faer is built without rayon: get_global_parallelism() is Par::Seq.
        let svd = aug
            .thin_svd()
            .map_err(|_| DspError::InvalidArgument("TRF SVD did not converge".into()))?;
        let s: Vec<_> = (0..n).map(|i| svd.S().column_vector()[i]).collect();
        let uf: Vec<_> = (0..n)
            .map(|k| (0..m).map(|i| svd.U()[(i, k)] * f[i]).sum())
            .collect();
        let vv = svd.V().to_owned();
        let theta = 0.995_f64.max(1. - gn);
        let mut actual = -1.;
        let mut xn = x.clone();
        let mut fnn = f.clone();
        let mut cn = cost;
        while actual <= 0. && nfev < max_nfev {
            let (ph, a) = trust(n, m, &uf, &s, &vv, delta, alpha);
            alpha = a;
            let p = ph.iter().zip(&d).map(|(a, b)| a * b).collect();
            let (step, sh, pred) = select(&x, &jh, &diag, &gh, p, ph, &d, delta, lb, ub, theta);
            xn = x.iter().zip(&step).map(|(a, b)| a + b).collect();
            feasible(&mut xn, lb, ub, false);
            fnn = fun(&xn);
            nfev += 1;
            let shn = norm(&sh);
            if fnn.len() != m {
                return Err(DspError::InvalidArgument("residual length changed".into()));
            }
            if fnn.iter().any(|v| !v.is_finite()) {
                delta = 0.25 * shn;
                continue;
            }
            cn = 0.5 * dot(&fnn, &fnn);
            actual = cost - cn;
            let ratio = if pred > 0. {
                actual / pred
            } else if pred == 0. && actual == 0. {
                1.
            } else {
                0.
            };
            let dn = if ratio < 0.25 {
                0.25 * shn
            } else if ratio > 0.75 && shn > 0.95 * delta {
                2. * delta
            } else {
                delta
            };
            let ft = actual < 1e-8 * cost && ratio > 0.25;
            let xt = norm(&step) < 1e-8 * (1e-8 + norm(&x));
            status = match (ft, xt) {
                (true, true) => 4,
                (true, false) => 2,
                (false, true) => 3,
                _ => 0,
            };
            if status != 0 {
                break;
            }
            alpha *= delta / dn;
            delta = dn;
        }
        if actual > 0. {
            x = xn;
            f = fnn;
            cost = cn;
            j = jacobian(&mut fun, &x, &f, lb, ub)?;
            g = grad(&j, &f);
        }
    }
    Ok(LeastSquaresResult {
        x,
        cost,
        nfev,
        status,
    })
}
