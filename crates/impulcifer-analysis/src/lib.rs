#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

//! Analysis arrays computed once from a borrowed HRIR snapshot (`model`: the
//! 2.x `core/plotting/analysis.py` metrics and the data half of the Bokeh
//! generators), rendered to a self-contained offline HTML report (`html`).
//! Static PNG charts live in `impulcifer-plots`. Rendering never feeds back
//! into DSP.

pub mod html;
pub mod model;
pub mod png;
