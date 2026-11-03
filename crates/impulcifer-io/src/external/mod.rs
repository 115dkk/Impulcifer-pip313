use std::{fs, path::Path};

pub mod assign;
mod dirac;
mod java_stream;
mod rew;

const MAX_FILE_SIZE: u64 = 1 << 30;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalFormat {
    Rew,
    Dirac,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternalFile {
    pub format: ExternalFormat,
    pub title: Option<String>,
    pub sample_rate: u32,
    pub microphone: Option<String>,
    pub mic_calibration: Option<MicCalibration>,
    pub speakers: Vec<ExternalSpeaker>,
    pub positions: Vec<usize>,
    pub measurements: Vec<ExternalMeasurement>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternalSpeaker {
    pub index: usize,
    pub name: String,
    pub subwoofer: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MicCalibration {
    pub frequency: Vec<f64>,
    pub gain_db: Vec<f64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Skip {
    NoImpulseResponse,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternalMeasurement {
    pub id: String,
    pub name: String,
    pub speaker: Option<usize>,
    pub position: Option<usize>,
    pub start_time: f64,
    pub samples: Vec<f64>,
    pub skipped: Option<Skip>,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct ExternalError {
    message: String,
}

impl ExternalError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub fn read_external(path: &Path) -> Result<ExternalFile, ExternalError> {
    let metadata = fs::metadata(path)
        .map_err(|error| ExternalError::new(format!("could not read measurement file: {error}")))?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err(ExternalError::new("measurement file exceeds 1 GiB limit"));
    }
    let bytes = fs::read(path)
        .map_err(|error| ExternalError::new(format!("could not read measurement file: {error}")))?;
    read_external_bytes(&bytes)
}

pub fn read_external_bytes(bytes: &[u8]) -> Result<ExternalFile, ExternalError> {
    if bytes.len() as u64 > MAX_FILE_SIZE {
        return Err(ExternalError::new("measurement file exceeds 1 GiB limit"));
    }
    if bytes.starts_with(&[0xac, 0xed, 0, 5]) {
        rew::read(bytes)
    } else {
        dirac::read(bytes)
    }
}
