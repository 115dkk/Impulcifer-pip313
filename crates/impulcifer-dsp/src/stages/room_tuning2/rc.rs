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

fn long_modes(
    h: &[Vec<Complex64>],
    f: &[f64],
    band: &CorrectionBand,
    policy: &PositionPolicy,
) -> Vec<(f64, f64)> {
    let _timer = profile::timer("long_modes");
    let mut out = Vec::new();
    for i in chosen(policy, h.len()) {
        let x = low_rate(&h[i], f, 300);
        let modes: Vec<_> = late_mode_frequencies(std::slice::from_ref(&x), 8)
            .into_iter()
            .filter(|p| {
                *p <= 160.0
                    && band
                        .bands
                        .iter()
                        .any(|(low, high)| *p >= *low && *p <= high.min(160.0))
            })
            .filter_map(|p| mode_decay(&x, p, 1.5).filter(|t| *t > 0.).map(|t| (p, t)))
            .collect();
        if modes.len() < 2 {
            continue;
        }
        let cutoff = (1.3 * median(&modes.iter().map(|(_, t)| *t).collect::<Vec<_>>())).max(0.3);
        for (p, t) in modes {
            if t >= cutoff {
                out.push((p, t));
            }
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    out.dedup_by(|a, b| a == b);
    out
}
struct FirRun {
    fir: Vec<f64>,
    response: Vec<Complex64>,
    peq: peq::Bands,
    strength: f64,
    beta: f64,
    fits: Vec<peq::Bands>,
    limit_outcome: protect::LimitOutcome,
    low_limited: bool,
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
}
impl Realizer<'_> {
    fn run(&self, s: f64, m: &[Complex64]) -> (Vec<f64>, Vec<Complex64>, f64, f64) {
        let _timer = profile::timer("realization");
        let delta = self.f[1] - self.f[0];
        let mut sum = 0.;
        let spec: Vec<_> = m
            .iter()
            .zip(self.x)
            .map(|(m, x)| {
                let adv = x.clamp(-0.005, self.lambda * s);
                sum += self.lambda - adv;
                *m * Complex64::from_polar(1., -2. * PI * sum * delta)
            })
            .collect();
        let mut h = fft::irfft(&spec, self.n);
        h.truncate(self.taps);
        taper(&mut h, 0.1);
        if self.rolloff_hz.is_some() {
            protect::correct_fir_dc(&mut h);
        }
        let f = rfft(&h, self.n);
        let error: Vec<_> = self
            .f
            .iter()
            .enumerate()
            .filter(|(_, frequency)| {
                (**frequency >= 20_f64.max(self.ff) && **frequency <= self.fe)
                    || self.extras.iter().any(|(low, high)| {
                        **frequency >= *low
                            && **frequency <= (high * 2_f64.sqrt()).min(0.99 * self.fs / 2.0)
                    })
            })
            .map(|(i, _)| 20. * (f[i].norm() + 1e-12).log10() - 20. * (m[i].norm() + 1e-12).log10())
            .collect();
        let rms = (error.iter().map(|e| e * e).sum::<f64>() / error.len().max(1) as f64).sqrt();
        let max = error.iter().map(|e| e.abs()).fold(0., f64::max);
        (h, f, rms, max)
    }
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
    let mut v = vec![Complex64::new(0., 0.); f.len()];
    let res = resolution(f);
    for (h, w) in h.iter().zip(weights) {
        let mut r: Vec<_> = h
            .iter()
            .zip(&p1)
            .zip(&m)
            .map(|((h, p), m)| h * p * m)
            .collect();
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
    let x: Vec<_> = if opts.extra_bands.is_empty() {
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
    };
    let (mut fir, mut response, rms0, max0) = realizer.run(0., &m);
    if let Some(t) = capture.as_deref_mut() {
        t.firs.push(fir.clone());
    }
    let mut strength = 0.;
    for s in [0.9, 0.7, 0.5, 0.3] {
        let (h, f, rms, max) = realizer.run(s, &m);
        if let Some(t) = capture.as_deref_mut() {
            t.firs.push(h.clone());
        }
        if rms <= 0.5_f64.max(rms0 + 0.3) && max <= 2_f64.max(max0 + 1.) {
            fir = h;
            response = f;
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
    let mut p2 = peq::response(f, &b2, fs);
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
        let (h2, f2, _, _) = realizer.run(strength, &m2);
        if let Some(t) = capture.as_deref_mut() {
            t.firs.push(h2.clone());
        }
        if err(&f2) >= err(&response) {
            break;
        }
        fir = h2;
        response = f2;
        m = m2;
    }
    drop(touchup_timer);
    let low_limited = if let Some(rolloff_hz) = rolloff_hz {
        limit_outcome = protect::limit(&mut b2, &response, f, rolloff_hz, limit_outcome, fs);
        p2 = peq::response(f, &b2, fs);
        protect::limited(&b2, &response, f, rolloff_hz, fs)
    } else {
        true
    };
    for (r, p) in response.iter_mut().zip(p2) {
        *r *= p;
    }
    if let Some(t) = capture {
        t.fir = fir.clone();
        t.response = response.clone();
        t.peq = b2.clone();
        t.strength = strength;
        t.beta = beta;
    }
    Ok(FirRun {
        fir,
        response,
        peq: b2.clone(),
        strength,
        beta,
        fits: vec![b1, b2],
        limit_outcome,
        low_limited,
    })
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
    let p = power(&h, &opts.policy);
    let level_db = level(&p, &f, target);
    let res = resolution(&f);
    let s = smooth(&p, &f, &res);
    let s3 = smooth(&p, &f, &[3.]);
    let long_modes = long_modes(&h, &f, band, &opts.policy);
    let target = curve(target, &f.iter().map(|f| f.max(1.)).collect::<Vec<_>>());
    let rolloff_hz = if opts.low_protect {
        protect::rolloff(&p, &f).frequency
    } else {
        None
    };
    let mut dips = Vec::new();
    let u0: Vec<_> = if !opts.dip_limit && !opts.low_protect && opts.extra_bands.is_empty() {
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
    if let Some(t) = capture.as_deref_mut() {
        t.power = p.clone();
        t.smoothed_variable = s.clone();
        t.smoothed_third = s3.clone();
        t.design_db = u0.clone();
        t.long_mode_bins = f
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                long_modes.iter().any(|(p, _)| {
                    **f >= p * 2_f64.powf(-1. / 24.) && **f <= p * 2_f64.powf(1. / 24.)
                })
            })
            .map(|(k, _)| k)
            .collect();
    }
    let mut session = capture.as_ref().map(|_| RcSession::default());
    let params = opts.mode.params(points[0].fs);
    let (fir, response, peq, strength, beta, fits, limit_outcome, low_limited) =
        if opts.mode == Tuning2Mode::Zero {
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
                if score(&other, &h, &f, band, &opts.policy, fs)
                    < score(&run, &h, &f, band, &opts.policy, fs)
                {
                    run = other;
                }
            }
            (
                run.fir,
                run.response,
                run.peq,
                run.strength,
                run.beta,
                fits,
                run.limit_outcome,
                run.low_limited,
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
    Ok(SpeakerRc {
        points: points.len(),
        level_db,
        design_db: u0,
        peq,
        peq_fits: fits,
        fir,
        phase_strength: strength,
        phase_frac: beta,
        long_modes,
        response,
        lift_samples: (params.lift_ms * fs / 1000.).round_ties_even() as usize,
        rolloff_hz,
        limit_outcome,
        low_limited,
        limited_dips: dips,
        bands: band.bands.clone(),
    })
}
