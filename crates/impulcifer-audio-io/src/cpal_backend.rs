//! cpal backend for macOS (CoreAudio) and Linux (ALSA). Implemented by the Daybreak packet.

use impulcifer_types::audio::*;

pub struct CpalBackend;

impl CpalBackend {
    pub fn new() -> Self {
        CpalBackend
    }
}

impl Default for CpalBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioBackend for CpalBackend {
    fn name(&self) -> &'static str {
        "cpal"
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
