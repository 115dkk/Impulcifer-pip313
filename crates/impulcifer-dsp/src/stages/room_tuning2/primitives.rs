use crate::{
    fft::{self, Complex64},
    filters,
};
use std::f64::consts::PI;
pub fn grid(n: usize, fs: f64) -> Vec<f64> {
    let step = 1. / (n as f64 * (1. / fs));
    (0..=n / 2).map(|k| k as f64 * step).collect()
}
pub fn median(x: &[f64]) -> f64 {
    if x.is_empty() {
        return f64::NAN;
    }
    let mut x = x.to_vec();
    x.sort_by(f64::total_cmp);
    (x[(x.len() - 1) / 2] + x[x.len() / 2]) / 2.
}
pub fn interp(x: f64, g: &[f64], v: &[f64]) -> f64 {
    if x <= g[0] {
        return v[0];
    }
    if x >= g[g.len() - 1] {
        return v[v.len() - 1];
    }
    let k = g.partition_point(|a| *a <= x) - 1;
    v[k] + (v[k + 1] - v[k]) / (g[k + 1] - g[k]) * (x - g[k])
}
pub fn geomspace(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    let a = lo.log10();
    let d = (hi.log10() - a) / (n - 1) as f64;
    let mut g: Vec<_> = (0..n).map(|i| 10_f64.powf(a + i as f64 * d)).collect();
    g[0] = lo;
    g[n - 1] = hi;
    g
}
pub fn smooth(x: &[f64], f: &[f64], b: &[f64]) -> Vec<f64> {
    let _timer = super::profile::timer("smoothing");
    let mut c = Vec::with_capacity(x.len() + 1);
    c.push(0.);
    for x in x {
        c.push(c.last().unwrap() + x);
    }
    let delta = f[1] - f[0];
    let scalar_h = (b.len() == 1).then(|| 2_f64.powf(0.5 / b[0]));
    f.iter()
        .enumerate()
        .map(|(k, f)| {
            let h = scalar_h.unwrap_or_else(|| 2_f64.powf(0.5 / b[k]));
            let lo = ((f / h / delta).floor() as usize).min(x.len() - 1);
            let hi = ((f * h / delta).ceil() as usize)
                .min(x.len() - 1)
                .max(lo + 1)
                .min(x.len() - 1);
            (c[hi + 1] - c[lo]) / (hi + 1 - lo) as f64
        })
        .collect()
}
pub fn smooth_complex(x: &[Complex64], f: &[f64], b: &[f64]) -> Vec<Complex64> {
    let re = smooth(&x.iter().map(|v| v.re).collect::<Vec<_>>(), f, b);
    let im = smooth(&x.iter().map(|v| v.im).collect::<Vec<_>>(), f, b);
    re.into_iter()
        .zip(im)
        .map(|(re, im)| Complex64::new(re, im))
        .collect()
}
pub fn resolution(f: &[f64]) -> Vec<f64> {
    f.iter()
        .map(|f| {
            24. * 0.25_f64.powf(
                ((f.max(1e-3).log10() - 200_f64.log10()) / (2000_f64.log10() - 200_f64.log10()))
                    .clamp(0., 1.),
            )
        })
        .collect()
}
pub fn mp(a: &[f64], n: usize) -> Vec<Complex64> {
    let _timer = super::profile::timer("minimum_phase");
    let mut c = fft::irfft(
        &a.iter()
            .map(|a| Complex64::new(a.max(1e-8).ln(), 0.))
            .collect::<Vec<_>>(),
        n,
    );
    for x in &mut c[1..n / 2] {
        *x *= 2.;
    }
    c[n / 2 + 1..].fill(0.);
    fft::rfft(&c).into_iter().map(|x| x.exp()).collect()
}
pub fn rfft(x: &[f64], n: usize) -> Vec<Complex64> {
    let mut p = vec![0.; n];
    let len = x.len().min(n);
    p[..len].copy_from_slice(&x[..len]);
    fft::rfft_owned(p)
}
pub fn taper(x: &mut [f64], fraction: f64) {
    let k = (fraction * x.len() as f64) as usize;
    let start = x.len() - k;
    for i in 0..k {
        x[start + i] *= 0.5 * (1. + (PI * i as f64 / k as f64).cos());
    }
}
pub fn argmax(x: &[f64]) -> usize {
    let mut best = 0;
    for i in 1..x.len() {
        if x[i] > x[best] {
            best = i;
        }
    }
    best
}
pub fn onset(x: &[f64]) -> usize {
    let a: Vec<_> = x.iter().map(|x| x.abs()).collect();
    let k = argmax(&a);
    a[..=k].iter().position(|x| *x >= 0.1 * a[k]).unwrap_or(0)
}
pub fn unwrap(p: &[f64]) -> Vec<f64> {
    let mut out = p.to_vec();
    let mut corr = 0.;
    for i in 1..p.len() {
        let d = p[i] - p[i - 1];
        let mut dm = (d + PI).rem_euclid(2. * PI) - PI;
        if dm == -PI && d > 0. {
            dm = PI;
        }
        if d.abs() >= PI {
            corr += dm - d;
        }
        out[i] += corr;
    }
    out
}
pub fn gradient(y: &[f64], x: &[f64]) -> Vec<f64> {
    let n = y.len();
    let mut out = vec![0.; n];
    out[0] = (y[1] - y[0]) / (x[1] - x[0]);
    out[n - 1] = (y[n - 1] - y[n - 2]) / (x[n - 1] - x[n - 2]);
    for i in 1..n - 1 {
        let d1 = x[i] - x[i - 1];
        let d2 = x[i + 1] - x[i];
        let a = -d2 / (d1 * (d1 + d2));
        let b = (d2 - d1) / (d1 * d2);
        let c = d1 / (d2 * (d1 + d2));
        out[i] = a * y[i - 1] + b * y[i] + c * y[i + 1];
    }
    out
}
pub fn low_rate(x: &[Complex64], f: &[f64], n: usize) -> Vec<f64> {
    let re: Vec<_> = x.iter().map(|x| x.re).collect();
    let im: Vec<_> = x.iter().map(|x| x.im).collect();
    let spec: Vec<_> = grid(2 * n, 600.)
        .iter()
        .map(|fl| {
            Complex64::new(interp(*fl, f, &re), interp(*fl, f, &im))
                * ((285. - fl) / 60.).clamp(0., 1.)
        })
        .collect();
    let mut out = fft::irfft(&spec, 2 * n);
    out.truncate(n);
    out
}
pub fn late_peaks(xs: &[Vec<f64>], count: usize) -> Vec<f64> {
    let mut power = vec![0.; 2049];
    for x in xs {
        let mut x = x.clone();
        let max = x.iter().map(|x| x.abs()).fold(0., f64::max);
        let k = x.iter().position(|x| x.abs() > 0.1 * max).unwrap_or(0);
        let end = (k + 90).min(x.len());
        x[..end].fill(0.);
        for (p, h) in power.iter_mut().zip(rfft(&x, 4096)) {
            *p += h.norm_sqr();
        }
    }
    let f = grid(4096, 600.);
    let start = f.partition_point(|f| *f < 20.);
    let end = f.partition_point(|f| *f <= 160.);
    let mut peaks = Vec::new();
    let mut i = start + 1;
    while i + 1 < end {
        if power[i] > power[i - 1] {
            let mut j = i;
            while j + 1 < end && power[j + 1] == power[i] {
                j += 1;
            }
            if j + 1 < end && power[j] > power[j + 1] {
                peaks.push((i + j) / 2);
            }
            i = j;
        }
        i += 1;
    }
    peaks.sort_by(|a, b| power[*b].total_cmp(&power[*a]));
    peaks.truncate(count);
    peaks.iter().map(|k| f[*k]).collect()
}
/// Fourth-order high-pass response used by low-rate support paths.
pub fn hp4(f: &[f64], fc: f64) -> Vec<Complex64> {
    f.iter()
        .map(|f| {
            let s = Complex64::new(0., f / fc);
            let h = s * s / (s * s + 2_f64.sqrt() * s + 1.);
            h * h
        })
        .collect()
}
/// Symmetric Hamming-window low-pass, normalized to unit DC gain.
pub fn firwin(taps: usize, cutoff: f64, fs: f64) -> Vec<f64> {
    let fc = cutoff / (fs / 2.);
    let mut h: Vec<_> = (0..taps)
        .map(|i| {
            let m = i as f64 - (taps - 1) as f64 / 2.;
            let x = fc * m;
            let sinc = if x == 0. {
                1.
            } else {
                (PI * x).sin() / (PI * x)
            };
            fc * sinc * (0.54 - 0.46 * (2. * PI * i as f64 / (taps - 1) as f64).cos())
        })
        .collect();
    let sum: f64 = h.iter().sum();
    for v in &mut h {
        *v /= sum;
    }
    h
}
/// Delay-removed, complex-smoothed group delay on an arbitrary frequency grid.
pub fn gd_curve(h: &[Complex64], f: &[f64], g: &[f64], frac: f64, fs: f64) -> Vec<f64> {
    let n = 2 * (h.len() - 1);
    let t = argmax(&fft::irfft(h, n).iter().map(|x| x.abs()).collect::<Vec<_>>()) as f64 / fs;
    let shifted: Vec<_> = h
        .iter()
        .zip(f)
        .map(|(h, f)| *h * Complex64::from_polar(1., 2. * PI * f * t))
        .collect();
    let s = smooth_complex(&shifted, f, &[frac]);
    let re: Vec<_> = s.iter().map(|x| x.re).collect();
    let im: Vec<_> = s.iter().map(|x| x.im).collect();
    let phase = unwrap(
        &g.iter()
            .map(|g| Complex64::new(interp(*g, f, &re), interp(*g, f, &im)).arg())
            .collect::<Vec<_>>(),
    );
    gradient(&phase, &g.iter().map(|g| 2. * PI * g).collect::<Vec<_>>())
        .iter()
        .map(|x| -x * 1000.)
        .collect()
}
pub fn mode_decay(x: &[f64], fc: f64, half: f64) -> Option<f64> {
    let sos = filters::butter_bandpass(2, (fc - half).max(1.) / 300., (fc + half).min(270.) / 300.);
    let mut input = vec![0.; 1200];
    input.extend_from_slice(x);
    input.reverse();
    let mut y = filters::sosfilt(&sos, &input);
    y.reverse();
    let e: Vec<_> = y.iter().map(|v| v * v).collect();
    let start = argmax(&e);
    let mut edc = vec![0.; e.len() - start];
    let mut sum = 0.;
    for i in (start..e.len()).rev() {
        sum += e[i];
        edc[i - start] = sum;
    }
    if edc[0] <= 0. {
        return None;
    }
    let db: Vec<_> = edc
        .iter()
        .map(|e| 10. * (e / edc[0]).max(1e-30).log10())
        .collect();
    let i5 = db.iter().position(|x| *x <= -5.).unwrap_or(0);
    let i25 = db.iter().position(|x| *x <= -25.).unwrap_or(0);
    if i25 <= i5 + 2 {
        return None;
    }
    let len = (i25 - i5) as f64;
    let tmean = (i5 + i25 - 1) as f64 / 1200.;
    let ymean = db[i5..i25].iter().sum::<f64>() / len;
    let mut num = 0.;
    let mut den = 0.;
    for (i, y) in db.iter().enumerate().take(i25).skip(i5) {
        let t = i as f64 / 600. - tmean;
        num += t * (y - ymean);
        den += t * t;
    }
    let slope = num / den;
    if slope < 0. { Some(-60. / slope) } else { None }
}
