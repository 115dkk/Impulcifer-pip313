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
}

#[allow(dead_code)]
fn main() {
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
