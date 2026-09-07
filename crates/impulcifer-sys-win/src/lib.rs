#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

//! WASAPI backend (Windows only) built on the `wasapi` crate. This is the
//! only crate that may ever hold handwritten `unsafe`, and it starts with an
//! allowance of zero (see unsafe-budget.toml). Banned upstream APIs, because
//! they are unsound behind safe signatures: `wasapi::WaveFormat::parse`,
//! `wasapi::Device::from_raw`. Capture must zero-fill SILENT packets.
//! Policy: exclusive mode first, shared + auto-convert fallback; open with the
//! endpoint's mix-format channel count; ASIO is out of scope.

use impulcifer_types::audio::*;

pub struct WasapiBackend;

impl WasapiBackend {
    pub fn new() -> Self {
        WasapiBackend
    }
}

impl Default for WasapiBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioBackend for WasapiBackend {
    fn name(&self) -> &'static str {
        "wasapi"
    }
    fn enumerate(&self) -> Result<Vec<Endpoint>, AudioError> {
        Err(AudioError::Backend("not implemented".into()))
    }
    fn probe(
        &self,
        _endpoint: &Endpoint,
        _direction: Direction,
        _spec: StreamSpec,
        _mode: ShareMode,
    ) -> Result<ProbeResult, AudioError> {
        Err(AudioError::Backend("not implemented".into()))
    }
    fn open_output(
        &self,
        _endpoint: &Endpoint,
        _spec: StreamSpec,
        _mode: ShareMode,
    ) -> Result<Box<dyn OutputSession>, AudioError> {
        Err(AudioError::Backend("not implemented".into()))
    }
    fn open_input(
        &self,
        _endpoint: &Endpoint,
        _spec: StreamSpec,
        _mode: ShareMode,
    ) -> Result<Box<dyn InputSession>, AudioError> {
        Err(AudioError::Backend("not implemented".into()))
    }
}
