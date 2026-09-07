#![forbid(unsafe_code)]
//! Foreground release timing of the uncached nnresample design, including FFT.
use impulcifer_dsp::resample::nnresample_design;
use std::{hint::black_box, time::Instant};

/// Times nnresample.compute_filt's full design, pinned by p06_design_*.json.
/// Run cargo run -p impulcifer-dsp --release --example p06_design_timing.
/// Includes planning/allocation, both 32001-tap designs, and the 2^19 real FFT.
fn main() {
    if cfg!(debug_assertions) {
        eprintln!("timing requires --release");
        std::process::exit(2);
    }
    for (up, down) in [
        (48000, 44100),
        (44100, 48000),
        (96000, 48000),
        (48000, 96000),
    ] {
        for run in 1..=3 {
            let start = Instant::now();
            let design = black_box(nnresample_design(black_box(up), black_box(down)).unwrap());
            let elapsed = start.elapsed();
            println!(
                "TIMING design {up}/{down} run={run}: {:.6} ms; taps={}; cutoff={:.17e}",
                elapsed.as_secs_f64() * 1000.0,
                design.taps.len(),
                design.cutoff
            );
            assert!(elapsed.as_secs_f64() < 0.2, "design+FFT exceeds 200ms");
        }
    }
}
