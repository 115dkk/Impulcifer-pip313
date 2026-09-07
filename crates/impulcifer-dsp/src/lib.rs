#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

//! Float64 DSP primitives with scipy-compatible semantics, plus the BRIR
//! pipeline stages. See docs/rust/ARCHITECTURE.md section 6 and
//! docs/research/rewrite-stack-2026-09/report-02-dsp.md section 2.4 for the
//! exact contracts each function must satisfy.

/// Invalid arguments for fallible DSP entry points.
#[derive(Debug, thiserror::Error)]
pub enum DspError {
    #[error("invalid DSP argument: {0}")]
    InvalidArgument(String),
}

pub mod channel_balance;
pub mod conv;
pub mod decay;
pub mod estimator;
pub mod fft;
pub mod filters;
pub mod fir;
pub mod fr;
pub mod hrir;
pub mod interp;
pub mod ir;
pub mod mic_deviation;
pub mod peaks;
pub mod pipeline;
pub mod resample;
pub mod smoothing;
pub mod spectrogram;
pub mod stages;
pub mod stats;
pub mod virtual_bass;
pub mod windows;
