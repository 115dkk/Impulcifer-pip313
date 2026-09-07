#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

//! Float64 DSP primitives with scipy-compatible semantics, plus the BRIR
//! pipeline stages. See docs/rust/ARCHITECTURE.md section 6 and
//! docs/research/rewrite-stack-2026-09/report-02-dsp.md section 2.4 for the
//! exact contracts each function must satisfy.

pub mod conv;
pub mod fft;
pub mod filters;
pub mod fir;
pub mod interp;
pub mod peaks;
pub mod pipeline;
pub mod resample;
pub mod smoothing;
pub mod spectrogram;
pub mod stats;
pub mod windows;
