#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

//! Track-major WAV I/O and external ffmpeg/ffprobe processes. PCM output
//! follows the 2.x soundfile oracle; no memory mapping or installers.

use std::{path::PathBuf, process::ExitStatus};

/// I/O, process and format failures retain their original diagnostic cause.
#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("I/O failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid audio format: {0}")]
    Format(String),
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("{executable} exited with {status}: {stderr}")]
    Process {
        executable: PathBuf,
        status: ExitStatus,
        stderr: String,
    },
}

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod wav_tests;

pub mod ffmpeg;
pub mod sweep_files;
pub mod wav;

pub use wav::{Wav, pcm32_round_trip, read_wav, write_wav};
