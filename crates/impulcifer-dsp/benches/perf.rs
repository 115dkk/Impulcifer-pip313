#![forbid(unsafe_code)]
//! PA02 single-thread primitive timings; shared verbatim with the smoke test.
use impulcifer_dsp::{
    conv, fft, filters, fir, interp, peaks, resample, smoothing, spectrogram, stats, windows,
};
use std::{f64::consts::PI, hint::black_box, time::Instant};

pub struct Case<'a> {
    pub name: &'static str,
    pub size: String,
    run: Box<dyn Fn() + 'a>,
}

/// Seed-7 wrapping 32-bit LCG, independently reproduced in the Python benchmark.
pub fn noise(n: usize) -> Vec<f64> {
    let mut state = 7_u32;
    (0..n)
        .map(|_| {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            state as f64 / 2147483648.0 - 1.0
        })
        .collect()
}
fn linspace(end: f64, n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            if i == n - 1 {
                end
            } else {
                i as f64 * (end / (n - 1) as f64)
            }
        })
        .collect()
}
fn sinc_fir(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            let z = 0.1 * (i as f64 - (n - 1) as f64 / 2.0);
            let sinc = if z == 0.0 {
                1.0
            } else {
                (PI * z).sin() / (PI * z)
            };
            0.1 * sinc * (0.54 + 0.46 * (-PI + i as f64 * (2.0 * PI / (n - 1) as f64)).cos())
        })
        .collect()
}

pub struct Inputs {
    x: Vec<f64>,
    rec: Vec<f64>,
    inv: Vec<f64>,
    corr: Vec<f64>,
    short: Vec<f64>,
    taps: Vec<f64>,
    h: Vec<f64>,
    freq: Vec<f64>,
    gain: Vec<f64>,
    knots: Vec<f64>,
    queries: Vec<f64>,
    regression_x: Vec<f64>,
    regression_y: Vec<f64>,
    poly_taps: Vec<f64>,
    nperseg: usize,
    overlap: usize,
    window_n: usize,
}
impl Inputs {
    pub fn new(tiny: bool) -> Self {
        let pick = |full, small| if tiny { small } else { full };
        let n = pick(96000, 128);
        let m = pick(783, 95);
        let mesh = pick(9600, 32);
        let freq = linspace(24000.0, mesh);
        let mut gain: Vec<_> = freq
            .iter()
            .map(|f| 1.0 + 0.2 * (f / 24000.0 * PI * 4.0).cos())
            .collect();
        gain[mesh - 1] = 0.0;
        let rec = noise(pick(391000, 256));
        let inv = noise(pick(295000, 128));
        let params = spectrogram::spectrogram_params(inv.len(), 48000, 10.0, 200).unwrap();
        let x = noise(n);
        // Explicitly warm the composed resampler before any timed calls.
        black_box(resample::nnresample(&x, 96000, 48000).unwrap());
        Self {
            x,
            rec,
            inv,
            corr: noise(pick(1440, 32)),
            short: noise(m),
            taps: sinc_fir(pick(9600, 32)),
            h: sinc_fir(pick(19200, 64)),
            freq,
            gain,
            // Already-logarithmic axis: construction and evaluation only, in both languages.
            knots: linspace(3.38, m).into_iter().map(|v| v + 1.0).collect(),
            queries: linspace(3.38, pick(4800, 128))
                .into_iter()
                .map(|v| v + 1.0)
                .collect(),
            regression_x: linspace(1.0, pick(1000, 32)),
            regression_y: noise(pick(1000, 32)),
            poly_taps: resample::nnresample_design(147, 160).unwrap().taps,
            nperseg: params.nfft,
            overlap: params.noverlap,
            window_n: pick(32001, 65),
        }
    }
    pub fn cases(&self) -> Vec<Case<'_>> {
        let mut cases = Vec::new();
        macro_rules! case {
            ($name:literal, $size:expr, $body:expr) => {
                cases.push(Case {
                    name: $name,
                    size: $size.to_string(),
                    run: Box::new(|| {
                        black_box($body);
                    }),
                });
            };
        }
        case!(
            "convolve_full_ir_fir",
            format!("{} x {}", self.x.len(), self.taps.len()),
            conv::convolve(black_box(&self.x), &self.taps, conv::Mode::Full)
        );
        case!(
            "convolve_same_estimate",
            format!("{} x {}", self.rec.len(), self.inv.len()),
            conv::convolve(&self.rec, &self.inv, conv::Mode::Same)
        );
        case!(
            "correlate_full_30ms",
            format!("8 x {} x {}", self.corr.len(), self.corr.len()),
            {
                for _ in 0..8 {
                    black_box(conv::correlate(&self.corr, &self.corr, conv::Mode::Full));
                }
            }
        );
        case!(
            "rfft_irfft_96000",
            self.x.len(),
            fft::irfft(&fft::rfft(&self.x), self.x.len())
        );
        case!("magnitude_response_96000", self.x.len(), {
            let db = fft::magnitude_response(&self.x);
            let f: Vec<_> = (0..db.len())
                .map(|i| i as f64 * (48000.0 / self.x.len() as f64))
                .collect();
            (f, db)
        });
        case!("butter8_sosfilt_96000", self.x.len(), {
            let mut sos = filters::butter(4, 250.0 / 24000.0, filters::BType::Highpass);
            sos.0
                .extend(filters::butter(4, 250.0 / 24000.0, filters::BType::Highpass).0);
            filters::sosfilt(&sos, &self.x)
        });
        case!(
            "firwin2_19200",
            format!("{} taps / {} mesh", self.h.len(), self.freq.len()),
            fir::firwin2(self.h.len(), &self.freq, &self.gain, None, 48000.0).unwrap()
        );
        case!(
            "minimum_phase_19200",
            self.h.len(),
            fir::minimum_phase(&self.h, self.h.len(), true).unwrap()
        );
        case!("savgol_heavy_light_783", self.short.len(), {
            for w in [13, 23, 23, 91, 23, 23] {
                black_box(smoothing::savgol_filter(&self.short, w, 2).unwrap());
            }
        });
        case!("find_peaks_96000", self.x.len(), {
            let pos = peaks::find_peaks(&self.x, Some(0.12589));
            let neg: Vec<_> = self.x.iter().map(|v| -v).collect();
            (pos, peaks::find_peaks(&neg, Some(0.12589)))
        });
        case!(
            "first_peak_index_96000",
            self.x.len(),
            peaks::first_peak_index(&self.x, 0, None, 0.12589)
        );
        case!(
            "spline_k1_783_to_4800",
            format!("{} to {}", self.knots.len(), self.queries.len()),
            interp::Spline::new(&self.knots, &self.short, 1)
                .unwrap()
                .eval(&self.queries)
        );
        case!(
            "spline_k2_783",
            self.knots.len(),
            interp::Spline::new(&self.knots, &self.short, 2)
                .unwrap()
                .eval(&self.knots)
        );
        case!(
            "spline_k3_783",
            self.knots.len(),
            interp::Spline::new(&self.knots, &self.short, 3)
                .unwrap()
                .eval(&self.knots)
        );
        case!(
            "linregress_1000",
            self.regression_x.len(),
            stats::linregress(&self.regression_x, &self.regression_y)
        );
        case!(
            "nnresample_design_147_160",
            "32001 / FFT 524288",
            resample::nnresample_design(black_box(147), black_box(160)).unwrap()
        );
        case!(
            "resample_poly_96000_48k_44k1",
            self.x.len(),
            resample::resample_poly(&self.x, 147, 160, &self.poly_taps).unwrap()
        );
        case!(
            "nnresample_96000_48k_96k",
            self.x.len(),
            resample::nnresample(&self.x, 96000, 48000).unwrap()
        );
        case!(
            "spectrogram_295000_4800",
            format!(
                "{} / {} / overlap {}",
                self.inv.len(),
                self.nperseg,
                self.overlap
            ),
            spectrogram::spectrogram(&self.inv, 48000, self.nperseg, self.overlap).unwrap()
        );
        case!(
            "windows_32001",
            format!("{} / {} / {}", self.window_n, self.h.len(), self.h.len()),
            (
                windows::kaiser(self.window_n, 5.65326, true),
                windows::hann(self.h.len(), true),
                windows::hamming(self.h.len(), true)
            )
        );
        case!(
            "expit_783",
            self.short.len(),
            self.short
                .iter()
                .map(|&v| stats::expit(v))
                .collect::<Vec<_>>()
        );
        case!(
            "next_fast_len_1e6",
            "1000000 (real + legacy)",
            (
                fft::next_fast_len(black_box(1000000)),
                fft::next_fast_len_legacy(black_box(1000000))
            )
        );
        cases
    }
}

pub fn smoke() {
    let inputs = Inputs::new(true);
    let cases = inputs.cases();
    assert_eq!(cases.len(), 22);
    for case in cases {
        (case.run)();
    }
    use impulcifer_dsp::{ir::ImpulseResponse, stages::room_tuning2::*};
    let mut data = vec![0.; 4800];
    data[100] = 1.;
    let points = [ImpulseResponse {
        fs: 48000,
        data,
        recording: None,
    }];
    for mode in [
        Tuning2Mode::Zero,
        Tuning2Mode::Mixed,
        Tuning2Mode::Linear,
        Tuning2Mode::Long,
        Tuning2Mode::Ultra(60.),
    ] {
        let opts = RcOptions {
            mode,
            ..Default::default()
        };
        let band = correction_band(&[&points], &opts).unwrap();
        let result = design_rc(&points, &opts, &band, None, None).unwrap();
        assert_eq!(result.fir.len(), mode.params(48000).taps);
    }
    let support = MimoSupport {
        speaker: "FL".into(),
        low: 50.,
        fir: primitives::firwin(210, 150., 600.),
    };
    assert_eq!(
        support_response(&support, 48000, analysis_len(48000))
            .unwrap()
            .len(),
        65537
    );
}

fn room_tuning2_bench() {
    use impulcifer_dsp::{
        estimator::SweepEstimator,
        hrir::Hrir,
        stages::{room, room_tuning2::*},
    };
    use impulcifer_io::read_wav;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let wav = read_wav(&root.join("data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")).unwrap();
    let estimator = SweepEstimator::from_samples(wav.sample_rate, &wav.tracks[0]).unwrap();
    let mut points = Vec::new();
    for side in ["left", "right"] {
        let file = format!("room-FL,FR-{side}.wav");
        let names = room::parse_room_measurement_name(&file).unwrap();
        let wav = read_wav(&root.join("data/demo").join(file)).unwrap();
        let mut h = Hrir {
            fs: wav.sample_rate,
            speakers: vec![],
        };
        h.open_recording_samples(
            &estimator,
            wav.sample_rate,
            &wav.tracks,
            &["FL", "FR"],
            names.side,
            2.,
        )
        .unwrap();
        let s = h.get("FL").unwrap();
        points.push(s.left.as_ref().or(s.right.as_ref()).unwrap().clone());
    }
    let logarithms: Vec<_> = (1..=700).map(|i| i as f64 / 100.).collect();
    let start = Instant::now();
    for _ in 0..10000 {
        for x in &logarithms {
            black_box(black_box(*x).log10());
        }
    }
    let log_seconds = start.elapsed().as_secs_f64() / 7_000_000.;
    println!(
        "LOG10 ns/point={:.6} threads={}",
        log_seconds * 1e9,
        rayon::current_num_threads()
    );
    for (name, mode, reference_seconds) in [
        ("zero", Tuning2Mode::Zero, 1.55),
        ("mixed", Tuning2Mode::Mixed, 0.88),
        ("linear", Tuning2Mode::Linear, 2.22),
        ("long", Tuning2Mode::Long, 0.73),
        ("ultra-60", Tuning2Mode::Ultra(60.), 1.91),
    ] {
        let opts = RcOptions {
            mode,
            policy: PositionPolicy::Listening {
                index: 0,
                weight: 1.,
            },
            ..Default::default()
        };
        let band = correction_band(&[&points], &opts).unwrap();
        black_box(design_rc(&points, &opts, &band, None, None).unwrap());
        profile::start();
        black_box(design_rc(&points, &opts, &band, None, None).unwrap());
        let timings = profile::finish();
        for (step, (elapsed, count)) in &timings {
            println!(
                "PROFILE {name}/{step}: ms={:.6} count={count} us/call={:.6}",
                elapsed.as_secs_f64() * 1000.,
                elapsed.as_secs_f64() * 1e6 / *count as f64
            );
        }
        let residuals = timings.get("residual").unwrap().1;
        let grid_points = if mode == Tuning2Mode::Zero { 900 } else { 700 };
        let svd = timings.get("svd").unwrap();
        println!(
            "FLOOR {name}: {residuals} evaluations * {grid_points} points * {:.6}ns log10 + {} svd * {:.6}ms = {:.6}s",
            log_seconds * 1e9,
            svd.1,
            svd.0.as_secs_f64() * 1000. / svd.1 as f64,
            residuals as f64 * grid_points as f64 * log_seconds + svd.0.as_secs_f64()
        );
        let mut times = Vec::new();
        for _ in 0..3 {
            let start = Instant::now();
            black_box(design_rc(&points, &opts, &band, None, None).unwrap());
            times.push(start.elapsed().as_secs_f64());
        }
        times.sort_by(f64::total_cmp);
        println!(
            "room_tuning2/{name} FL two positions: median {:.6}s min {:.6}s SECS {reference_seconds}s speedup {:.3}x target=4x",
            times[1],
            times[0],
            reference_seconds / times[1],
        );
    }
}

fn mimo_scalar_probe(label: &str) {
    let start = Instant::now();
    for _ in 0..10000 {
        for i in 1..=700 {
            black_box(black_box(i as f64 / 100.).log10());
        }
    }
    println!(
        "MIMO_SCALAR {label} ns/point={:.6}",
        start.elapsed().as_secs_f64() * 1e9 / 7_000_000.
    );
}
fn mimo_solver_compare(s: &impulcifer_dsp::stages::room_tuning2::mimo::MimoStages) {
    use faer::{
        Mat, Par,
        dyn_stack::{MemBuffer, MemStack},
        linalg::cholesky::llt,
    };
    use impulcifer_dsp::stages::room_tuning2::mimo::bisect;
    let start = Instant::now();
    let (logs, ratios, kept, u) = bisect(&s.normal, &s.rhs, &s.lp, s.direct).unwrap();
    let evd_seconds = start.elapsed().as_secs_f64();
    let m = s.rhs.len();
    let sigma = (0..m).map(|i| s.normal[(i, i)]).sum::<f64>() / m as f64;
    let start = Instant::now();
    let (mut lo, mut hi) = (-8., 4.);
    let mut chosen = 0;
    let mut best = vec![];
    let mut memory = MemBuffer::new(llt::factor::cholesky_in_place_scratch::<f64>(
        m,
        Par::Seq,
        Default::default(),
    ));
    for trial in 0..19 {
        let log = if trial == 0 { 4. } else { (lo + hi) / 2. };
        assert_eq!(log, logs[trial]);
        let mut a = s.normal.clone();
        for i in 0..m {
            a[(i, i)] += sigma * 10f64.powf(log);
        }
        llt::factor::cholesky_in_place(
            a.as_mut(),
            Default::default(),
            Par::Seq,
            MemStack::new(&mut memory),
            Default::default(),
        )
        .unwrap();
        let mut b = Mat::from_fn(m, 1, |i, _| s.rhs[i]);
        llt::solve::solve_in_place(a.as_ref(), b.as_mut(), Par::Seq, MemStack::new(&mut memory));
        let values: Vec<_> = (0..m).map(|i| b[(i, 0)]).collect();
        let ratio = values
            .chunks(180)
            .map(|u| {
                let mut q = vec![0.; 210];
                for (i, a) in u.iter().enumerate() {
                    for (j, b) in s.lp.iter().enumerate() {
                        q[i + j] += a * b;
                    }
                }
                q.iter().map(|v| v * v).sum::<f64>()
            })
            .sum::<f64>()
            / s.direct;
        assert_eq!(ratio > 10f64.powf(-1.2), ratios[trial] > 10f64.powf(-1.2));
        if trial == 0 || ratio <= 10f64.powf(-1.2) {
            best = values;
            chosen = trial;
            if trial > 0 {
                hi = log;
            }
        } else {
            lo = log;
        }
    }
    let chol_seconds = start.elapsed().as_secs_f64();
    let rel = (u
        .iter()
        .zip(&best)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
        / best.iter().map(|v| v * v).sum::<f64>())
    .sqrt();
    assert_eq!(kept, chosen);
    assert!(rel < 1e-9);
    println!(
        "SOLVER m={m} evd={evd_seconds:.6}s cholesky19={chol_seconds:.6}s decisions=19/19 kept={kept}/{chosen} relative_u={rel:e}"
    );
}
fn mimo_bench() {
    use impulcifer_dsp::{estimator::SweepEstimator, hrir::Hrir, stages::room_tuning2::*};
    use impulcifer_io::read_wav;
    use impulcifer_types::constants::IPSILATERAL_PAIRS;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let wav = read_wav(&root.join("data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")).unwrap();
    let estimator = SweepEstimator::from_samples(wav.sample_rate, &wav.tracks[0]).unwrap();
    let mut h = Hrir {
        fs: 48000,
        speakers: vec![],
    };
    for file in ["FL,FR.wav", "FC.wav", "BL,SL.wav", "SR,BR.wav"] {
        let wav = read_wav(&root.join("data/demo").join(file)).unwrap();
        let names: Vec<_> = file.strip_suffix(".wav").unwrap().split(',').collect();
        h.open_recording_samples(&estimator, wav.sample_rate, &wav.tracks, &names, None, 2.)
            .unwrap();
    }
    h.crop_heads(1.).unwrap();
    h.align_ipsilateral_all(&IPSILATERAL_PAIRS, 30.);
    h.align_onset_groups_peak_leftref(None).unwrap();
    h.crop_tails(&estimator).unwrap();
    let opts = Tuning2Options {
        rc: RcOptions::default(),
        level_match: true,
        mimo: Some(MimoOptions {
            fsiso: 150.,
            low: None,
        }),
        pairs: vec![],
    };
    let plan = prepare_tuning2(&h, &opts, None, None).unwrap();
    let inputs: Vec<_> = plan
        .speakers
        .iter()
        .map(|s| MimoInput {
            speaker: &s.speaker,
            gain_db: 0.,
            response: &s.rc.response,
        })
        .collect();
    mimo_scalar_probe("before-demo");
    let start = Instant::now();
    black_box(design_mimo(&h, &inputs, opts.mimo.as_ref().unwrap()).unwrap());
    let elapsed = start.elapsed().as_secs_f64();
    mimo_scalar_probe("after-demo");
    println!(
        "mimo/demo total={elapsed:.6}s per_input={:.6}s",
        elapsed / 7.
    );
    let mut omni = Hrir {
        fs: 48000,
        speakers: vec![],
    };
    for file in [
        "room-FL,FR-left.wav",
        "room-FL,FR-right.wav",
        "room-FC-left.wav",
        "room-FC-right.wav",
        "room-BL,SL-left.wav",
        "room-BL,SL-right.wav",
        "room-SR,BR-left.wav",
        "room-SR,BR-right.wav",
    ] {
        let wav = read_wav(&root.join("data/demo").join(file)).unwrap();
        let parsed = impulcifer_dsp::stages::room::parse_room_measurement_name(file).unwrap();
        let names: Vec<_> = parsed.speakers.iter().map(String::as_str).collect();
        omni.open_recording_samples(
            &estimator,
            wav.sample_rate,
            &wav.tracks,
            &names,
            parsed.side,
            2.,
        )
        .unwrap();
    }
    for enabled in [false, true] {
        let mut opts = opts.clone();
        if !enabled {
            opts.mimo = None;
        }
        let start = Instant::now();
        let plan = prepare_tuning2(&omni, &opts, None, None).unwrap();
        black_box(apply_tuning2(&mut h.clone(), &plan).unwrap());
        println!(
            "mimo/whole_prepare_apply enabled={enabled} seconds={:.6}",
            start.elapsed().as_secs_f64()
        );
    }
    let mut large = Hrir {
        fs: 48000,
        speakers: vec![],
    };
    for i in 0..24 {
        let mut s = h.speakers[i % 7].clone();
        s.speaker = format!("S{i}");
        for ir in s.left.iter_mut().chain(&mut s.right) {
            let mut data = vec![0.; i * 11];
            data.extend_from_slice(&ir.data);
            ir.data = data;
        }
        large.speakers.push(s);
    }
    let input = [MimoInput {
        speaker: "S0",
        gain_db: 0.,
        response: &plan.speakers[0].rc.response,
    }];
    let start = Instant::now();
    black_box(design_mimo(&large, &input, opts.mimo.as_ref().unwrap()).unwrap());
    println!(
        "mimo/24_speakers per_input={:.6}s R=23 m=4140",
        start.elapsed().as_secs_f64()
    );
    mimo_scalar_probe("after-24");
    mimo::design_mimo_observed(&large, &input, opts.mimo.as_ref().unwrap(), &|_, s| {
        mimo_solver_compare(s)
    })
    .unwrap();
}

#[allow(dead_code)]
fn main() {
    if std::env::args().any(|s| s == "mimo") {
        mimo_bench();
        return;
    }
    if std::env::args().any(|s| s.contains("room_tuning2"))
        || std::env::var("PA02_FILTER").is_ok_and(|s| s.contains("room_tuning2"))
    {
        room_tuning2_bench();
        return;
    }
    let inputs = Inputs::new(false);
    let filter = std::env::var("PA02_FILTER").unwrap_or_default();
    println!("| op | size | rust median ms | rust min ms |");
    println!("|---|---|---:|---:|");
    for case in inputs.cases() {
        if !filter.is_empty() && !case.name.contains(&filter) {
            continue;
        }
        for _ in 0..3 {
            (case.run)();
        }
        let mut times = Vec::new();
        // Amortize the Windows clock resolution for the scalar length helper.
        // The reported time remains per pair of calls, as on the Python side.
        let batch = if case.name == "next_fast_len_1e6" {
            1000
        } else {
            1
        };
        for _ in 0..11 {
            let start = Instant::now();
            for _ in 0..batch {
                (case.run)();
            }
            times.push(start.elapsed().as_secs_f64() * 1000.0 / batch as f64);
        }
        times.sort_by(f64::total_cmp);
        println!(
            "| {} | {} | {:.6} | {:.6} |",
            case.name, case.size, times[5], times[0]
        );
    }
}
