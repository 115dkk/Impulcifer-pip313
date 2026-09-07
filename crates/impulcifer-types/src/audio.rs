//! Audio backend contract shared by `impulcifer-sys-win` (WASAPI) and
//! `impulcifer-audio-io` (cpal + orchestration). Sessions are deliberately
//! not `Send`: they are created, used and dropped on one dedicated OS thread.
//! Threads exchange endpoint ids, specs, owned buffers and reports only.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Input,
    Output,
}

/// Windows share mode. The cpal backend ignores it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShareMode {
    Exclusive,
    SharedAutoConvert,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Endpoint {
    /// Stable backend identifier (WASAPI endpoint id string, cpal device name on other platforms).
    pub id: String,
    pub name: String,
    pub host_api: String,
    pub max_input_channels: u16,
    pub max_output_channels: u16,
    pub default_samplerate: f64,
    pub is_default_input: bool,
    pub is_default_output: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamSpec {
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProbeResult {
    pub supported: bool,
    pub mode: ShareMode,
    /// The format the backend would actually open (may differ from the request in shared mode).
    pub native_sample_rate: u32,
    pub native_channels: u16,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlaybackReport {
    pub frames_submitted: u64,
    pub frames_drained: u64,
    pub mode: ShareMode,
    pub underruns: u32,
    pub cancelled: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaptureRead {
    pub frames: usize,
    pub discontinuity: bool,
    pub silent: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("device not found: {0}")]
    DeviceNotFound(String),
    #[error("format not supported: {0}")]
    UnsupportedFormat(String),
    #[error("backend error: {0}")]
    Backend(String),
    #[error("cancelled")]
    Cancelled,
}

/// Cooperative cancellation flag shared between the coordinator and workers.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

pub trait AudioBackend: Send + Sync {
    fn name(&self) -> &'static str;
    fn enumerate(&self) -> Result<Vec<Endpoint>, AudioError>;
    fn probe(
        &self,
        endpoint: &Endpoint,
        direction: Direction,
        spec: StreamSpec,
        mode: ShareMode,
    ) -> Result<ProbeResult, AudioError>;
    fn open_output(
        &self,
        endpoint: &Endpoint,
        spec: StreamSpec,
        mode: ShareMode,
    ) -> Result<Box<dyn OutputSession>, AudioError>;
    fn open_input(
        &self,
        endpoint: &Endpoint,
        spec: StreamSpec,
        mode: ShareMode,
    ) -> Result<Box<dyn InputSession>, AudioError>;
}

/// Plays one interleaved f32 buffer to completion (including the backend's drain) and returns.
pub trait OutputSession {
    fn play_to_completion(
        &mut self,
        interleaved: &[f32],
        cancel: &CancelToken,
    ) -> Result<PlaybackReport, AudioError>;
}

/// Blocking capture into caller-owned interleaved f32 storage.
pub trait InputSession {
    fn start(&mut self) -> Result<(), AudioError>;
    fn read_into(
        &mut self,
        dst: &mut [f32],
        cancel: &CancelToken,
    ) -> Result<CaptureRead, AudioError>;
    fn stop(&mut self) -> Result<(), AudioError>;
}
