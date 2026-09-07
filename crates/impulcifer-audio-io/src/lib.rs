#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

//! Backend selection and the measurement session (port of
//! `core/recorder.py::play_and_record`): open input first, acknowledge
//! readiness, play the whole sweep buffer to completion and drain, stop input,
//! join both threads. Windows uses `impulcifer-sys-win`, other platforms cpal.

pub mod cpal_backend;
pub mod policy;
pub mod session;

use impulcifer_types::audio::AudioBackend;

/// The platform default backend.
pub fn default_backend() -> Box<dyn AudioBackend> {
    #[cfg(windows)]
    {
        Box::new(impulcifer_sys_win::WasapiBackend::new())
    }
    #[cfg(not(windows))]
    {
        Box::new(cpal_backend::CpalBackend::new())
    }
}
