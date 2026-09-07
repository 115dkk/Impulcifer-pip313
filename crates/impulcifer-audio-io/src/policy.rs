//! Exclusive first; only format rejection permits shared auto-convert fallback.

use impulcifer_types::audio::{
    AudioBackend, AudioError, Endpoint, InputSession, OutputSession, ShareMode, StreamSpec,
};

pub fn open_output_with_policy(
    backend: &dyn AudioBackend,
    endpoint: &Endpoint,
    spec: StreamSpec,
) -> Result<(Box<dyn OutputSession>, ShareMode), AudioError> {
    match backend.open_output(endpoint, spec, ShareMode::Exclusive) {
        Ok(session) => Ok((session, ShareMode::Exclusive)),
        Err(AudioError::UnsupportedFormat(_)) => backend
            .open_output(endpoint, spec, ShareMode::SharedAutoConvert)
            .map(|session| (session, ShareMode::SharedAutoConvert)),
        Err(error) => Err(error),
    }
}

pub fn open_input_with_policy(
    backend: &dyn AudioBackend,
    endpoint: &Endpoint,
    spec: StreamSpec,
) -> Result<(Box<dyn InputSession>, ShareMode), AudioError> {
    match backend.open_input(endpoint, spec, ShareMode::Exclusive) {
        Ok(session) => Ok((session, ShareMode::Exclusive)),
        Err(AudioError::UnsupportedFormat(_)) => backend
            .open_input(endpoint, spec, ShareMode::SharedAutoConvert)
            .map(|session| (session, ShareMode::SharedAutoConvert)),
        Err(error) => Err(error),
    }
}
