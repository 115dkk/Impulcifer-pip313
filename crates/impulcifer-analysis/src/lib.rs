#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

//! Analysis arrays (frequency response, impulse response, ILD/IPD/IACC/EDC,
//! spectrogram, waterfall) computed once, then rendered to PNG (plotters) and
//! to offline HTML (vendored Plotly.js). Rendering never feeds back into DSP.

pub mod html;
pub mod model;
pub mod png;
