//! CPAL transport (also available on Windows for explicit backend comparison).
//!
//! ShareMode is ignored by CPAL, but retained as request/report metadata. The
//! requested rate must be native; no resampling happens. The channel count may
//! be lower than the device's: playback silences extra channels and capture keeps
//! the first ones. CPAL's CoreAudio host exposes only the device's current total
//! channel count. Device ids are names, scoped by host_api; duplicate names within
//! one host are therefore inherently ambiguous in the shared Endpoint contract.
//!
//! CPAL has no drain API. Completion waits two full final callback periods after
//! the callback that consumes the last sample (one for that buffer, one extra).
//! The period is max(buffer frames / rate, observed callback interval). This is
//! a heuristic, not proof of DAC delivery. See DrainDiagnostics for measurements.

use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use impulcifer_types::audio::{
    AudioBackend, AudioError, CancelToken, CaptureRead, Direction, Endpoint, InputSession,
    OutputSession, PlaybackReport, ProbeResult, ShareMode, StreamSpec,
};

const POLL: Duration = Duration::from_millis(5);
const STALL: Duration = Duration::from_secs(2);
const PACKET_SAMPLES: usize = 1024;
const QUEUE_PACKETS: usize = 256;

#[derive(Clone, Copy, Debug, Default)]
pub struct CpalBackend;

fn backend_error(e: impl std::fmt::Display) -> AudioError {
    AudioError::Backend(format!("CPAL: {e}"))
}

fn validate(spec: StreamSpec) -> Result<(), AudioError> {
    if spec.sample_rate == 0 || spec.channels == 0 {
        return Err(AudioError::UnsupportedFormat(
            "zero rate or channels".into(),
        ));
    }
    Ok(())
}

fn aligned(len: usize, channels: u16) -> Result<(), AudioError> {
    if channels == 0 || !len.is_multiple_of(usize::from(channels)) {
        return Err(AudioError::UnsupportedFormat(
            "unaligned sample buffer".into(),
        ));
    }
    Ok(())
}

fn device_name(device: &cpal::Device) -> String {
    device.to_string()
}

fn find_device(endpoint: &Endpoint) -> Result<cpal::Device, AudioError> {
    for id in cpal::available_hosts() {
        if id.name() != endpoint.host_api {
            continue;
        }
        let host = cpal::host_from_id(id).map_err(backend_error)?;
        for device in host.devices().map_err(backend_error)? {
            if device_name(&device) == endpoint.id {
                return Ok(device);
            }
        }
    }
    Err(AudioError::DeviceNotFound(format!(
        "{} / {}",
        endpoint.host_api, endpoint.id
    )))
}

fn ranges(
    device: &cpal::Device,
    direction: Direction,
) -> Result<Vec<cpal::SupportedStreamConfigRange>, AudioError> {
    match direction {
        Direction::Input => device
            .supported_input_configs()
            .map(|x| x.collect())
            .map_err(backend_error),
        Direction::Output => device
            .supported_output_configs()
            .map(|x| x.collect())
            .map_err(backend_error),
    }
}

fn pcm(format: SampleFormat) -> bool {
    matches!(
        format,
        SampleFormat::I8
            | SampleFormat::I16
            | SampleFormat::I24
            | SampleFormat::I32
            | SampleFormat::I64
            | SampleFormat::U8
            | SampleFormat::U16
            | SampleFormat::U24
            | SampleFormat::U32
            | SampleFormat::U64
            | SampleFormat::F32
            | SampleFormat::F64
    )
}

fn select_config(
    device: &cpal::Device,
    direction: Direction,
    spec: StreamSpec,
) -> Result<cpal::SupportedStreamConfig, AudioError> {
    validate(spec)?;
    choose_config(ranges(device, direction)?, direction, spec)
}

fn choose_config(
    supported: Vec<cpal::SupportedStreamConfigRange>,
    direction: Direction,
    spec: StreamSpec,
) -> Result<cpal::SupportedStreamConfig, AudioError> {
    validate(spec)?;
    supported
        .into_iter()
        .filter(|r| {
            r.channels() >= spec.channels
                && r.min_sample_rate() <= spec.sample_rate
                && spec.sample_rate <= r.max_sample_rate()
                && pcm(r.sample_format())
        })
        // Prefer the closest count, then float to avoid integer quantization.
        .min_by_key(|r| (r.channels(), r.sample_format() != SampleFormat::F32))
        .map(|r| r.with_sample_rate(spec.sample_rate))
        .ok_or_else(|| {
            AudioError::UnsupportedFormat(format!(
                "native {:?} {} Hz / {} channels unavailable",
                direction, spec.sample_rate, spec.channels
            ))
        })
}

// All branches use CPAL/dasp's native equilibrium and numeric conversions.
macro_rules! with_sample_type {
    ($format:expr, $function:ident $(, $arg:expr)*) => {
        match $format {
            SampleFormat::I8 => $function::<i8>($($arg),*),
            SampleFormat::I16 => $function::<i16>($($arg),*),
            SampleFormat::I24 => $function::<cpal::I24>($($arg),*),
            SampleFormat::I32 => $function::<i32>($($arg),*),
            SampleFormat::I64 => $function::<i64>($($arg),*),
            SampleFormat::U8 => $function::<u8>($($arg),*),
            SampleFormat::U16 => $function::<u16>($($arg),*),
            SampleFormat::U24 => $function::<cpal::U24>($($arg),*),
            SampleFormat::U32 => $function::<u32>($($arg),*),
            SampleFormat::U64 => $function::<u64>($($arg),*),
            SampleFormat::F32 => $function::<f32>($($arg),*),
            SampleFormat::F64 => $function::<f64>($($arg),*),
            _ => Err(AudioError::UnsupportedFormat("non-PCM transport".into())),
        }
    };
}

impl CpalBackend {
    pub fn new() -> Self {
        Self
    }

    pub fn open_output_session(
        &self,
        endpoint: &Endpoint,
        spec: StreamSpec,
        mode: ShareMode,
    ) -> Result<CpalOutputSession, AudioError> {
        let device = find_device(endpoint)?;
        let config = select_config(&device, Direction::Output, spec)?;
        Ok(CpalOutputSession {
            device,
            config,
            spec,
            mode,
            diagnostics: None,
            _not_send: PhantomData,
        })
    }
}

impl AudioBackend for CpalBackend {
    fn name(&self) -> &'static str {
        "cpal"
    }

    fn enumerate(&self) -> Result<Vec<Endpoint>, AudioError> {
        let mut endpoints = Vec::new();
        for id in cpal::available_hosts() {
            let host = cpal::host_from_id(id).map_err(backend_error)?;
            let default_input = host.default_input_device().map(|d| device_name(&d));
            let default_output = host.default_output_device().map(|d| device_name(&d));
            for device in host.devices().map_err(backend_error)? {
                // Unsupported directions legitimately have no default/ranges.
                let inputs = device
                    .supported_input_configs()
                    .map(|r| r.collect::<Vec<_>>())
                    .unwrap_or_default();
                let outputs = device
                    .supported_output_configs()
                    .map(|r| r.collect::<Vec<_>>())
                    .unwrap_or_default();
                let max_input = inputs.iter().map(|r| r.channels()).max().unwrap_or(0);
                let max_output = outputs.iter().map(|r| r.channels()).max().unwrap_or(0);
                if max_input == 0 && max_output == 0 {
                    continue;
                }
                let default = device
                    .default_output_config()
                    .or_else(|_| device.default_input_config())
                    .map_err(backend_error)?;
                let name = device_name(&device);
                endpoints.push(Endpoint {
                    id: name.clone(),
                    name: name.clone(),
                    host_api: id.name().into(),
                    max_input_channels: max_input,
                    max_output_channels: max_output,
                    default_samplerate: f64::from(default.sample_rate()),
                    is_default_input: max_input > 0 && default_input.as_ref() == Some(&name),
                    is_default_output: max_output > 0 && default_output.as_ref() == Some(&name),
                });
            }
        }
        Ok(endpoints)
    }

    fn probe(
        &self,
        endpoint: &Endpoint,
        direction: Direction,
        spec: StreamSpec,
        mode: ShareMode,
    ) -> Result<ProbeResult, AudioError> {
        let device = find_device(endpoint)?;
        let (supported, native_channels, detail) = match select_config(&device, direction, spec) {
            Ok(config) => {
                let channels = config.channels();
                let detail = if channels == spec.channels {
                    "native tuple; CPAL ignores ShareMode".into()
                } else {
                    match direction {
                        Direction::Output => format!(
                            "native rate; device has {channels} channels, {} above the requested {} stay silent; CPAL ignores ShareMode",
                            channels - spec.channels,
                            spec.channels
                        ),
                        Direction::Input => format!(
                            "native rate; {} of {channels} input channels are kept; CPAL ignores ShareMode",
                            spec.channels
                        ),
                    }
                };
                (true, channels, detail)
            }
            Err(AudioError::UnsupportedFormat(detail)) => (false, spec.channels, detail),
            Err(e) => return Err(e),
        };
        Ok(ProbeResult {
            supported,
            mode,
            native_sample_rate: spec.sample_rate,
            native_channels,
            detail,
        })
    }

    fn open_output(
        &self,
        endpoint: &Endpoint,
        spec: StreamSpec,
        mode: ShareMode,
    ) -> Result<Box<dyn OutputSession>, AudioError> {
        Ok(Box::new(self.open_output_session(endpoint, spec, mode)?))
    }

    fn open_input(
        &self,
        endpoint: &Endpoint,
        spec: StreamSpec,
        _mode: ShareMode,
    ) -> Result<Box<dyn InputSession>, AudioError> {
        validate(spec)?;
        if usize::from(spec.channels) > PACKET_SAMPLES {
            return Err(AudioError::UnsupportedFormat(
                "capture channel count exceeds packet capacity".into(),
            ));
        }
        let device = find_device(endpoint)?;
        let config = select_config(&device, Direction::Input, spec)?;
        let state = Arc::new(CallbackState::new());
        let (tx, rx) = mpsc::sync_channel(QUEUE_PACKETS);
        let stream = with_sample_type!(
            config.sample_format(),
            build_input,
            &device,
            config.config(),
            tx,
            state.clone(),
            spec.channels
        )?;
        Ok(Box::new(CpalInputSession {
            stream: Some(stream),
            spec,
            rx,
            state,
            pending: None,
            pending_offset: 0,
            started: false,
            _not_send: PhantomData,
        }))
    }
}

struct CallbackState {
    epoch: Instant,
    failed: AtomicBool,
    error_kind: AtomicUsize,
    xruns: AtomicUsize,
    discontinuity: AtomicBool,
    last_callback_ns: AtomicU64,
    cursor: AtomicUsize,
    final_ns: AtomicU64,
    final_period_ns: AtomicU64,
}

impl CallbackState {
    fn new() -> Self {
        Self {
            epoch: Instant::now(),
            failed: AtomicBool::new(false),
            error_kind: AtomicUsize::new(0),
            xruns: AtomicUsize::new(0),
            discontinuity: AtomicBool::new(false),
            last_callback_ns: AtomicU64::new(0),
            cursor: AtomicUsize::new(0),
            final_ns: AtomicU64::new(0),
            final_period_ns: AtomicU64::new(0),
        }
    }
    fn stream_error(&self, error: cpal::Error) {
        // WASAPI capture continues after reporting a discontinuity as Xrun.
        // Preserve that diagnostic, as the direct WASAPI backend does, rather
        // than treating a recoverable packet discontinuity as stream failure.
        if error.kind() == cpal::ErrorKind::Xrun {
            self.xruns.fetch_add(1, Ordering::Relaxed);
            self.discontinuity.store(true, Ordering::Release);
            return;
        }
        // Keep error callbacks allocation-free; format the diagnostic on caller.
        let kind = match error.kind() {
            cpal::ErrorKind::RealtimeDenied => 2,
            cpal::ErrorKind::DeviceChanged => 3,
            cpal::ErrorKind::StreamInvalidated => 4,
            cpal::ErrorKind::DeviceNotAvailable => 5,
            _ => 6,
        };
        self.error_kind.store(kind, Ordering::Relaxed);
        self.failed.store(true, Ordering::Release);
    }
    fn failure(&self) -> AudioError {
        let kind = match self.error_kind.load(Ordering::Relaxed) {
            1 => "Xrun",
            2 => "RealtimeDenied",
            3 => "DeviceChanged",
            4 => "StreamInvalidated",
            5 => "DeviceNotAvailable",
            6 => "other backend error",
            _ => "invalid callback data or disconnected queue",
        };
        backend_error(format!("asynchronous stream error: {kind}"))
    }
    fn now_ns(&self) -> u64 {
        (self.epoch.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64).saturating_add(1)
    }
    fn check(&self, cancel: &CancelToken) -> Result<(), AudioError> {
        if cancel.is_cancelled() {
            return Err(AudioError::Cancelled);
        }
        if self.failed.load(Ordering::Acquire) {
            return Err(self.failure());
        }
        let last = self.last_callback_ns.load(Ordering::Acquire);
        if self.now_ns().saturating_sub(last) > STALL.as_nanos() as u64 {
            return Err(backend_error("audio callbacks stalled for two seconds"));
        }
        Ok(())
    }
}

/// Timing from the latest successful playback, relative to stream construction.
#[derive(Clone, Copy, Debug)]
pub struct DrainDiagnostics {
    pub final_callback_at: Duration,
    pub final_callback_period: Duration,
    pub waited_after_final: Duration,
}

pub struct CpalOutputSession {
    device: cpal::Device,
    config: cpal::SupportedStreamConfig,
    spec: StreamSpec,
    mode: ShareMode,
    diagnostics: Option<DrainDiagnostics>,
    _not_send: PhantomData<Rc<()>>,
}

impl CpalOutputSession {
    pub fn drain_diagnostics(&self) -> Option<DrainDiagnostics> {
        self.diagnostics
    }
}

// This helper only copies/converts samples and updates atomics. The source Arc
// remains owned by the caller until after the stream is dropped.
fn render<T: Sample + FromSample<f32>>(
    dst: &mut [T],
    source: &[f32],
    state: &CallbackState,
    spec: StreamSpec,
    device_channels: usize,
) {
    let now = state.now_ns();
    let previous = state.last_callback_ns.swap(now, Ordering::AcqRel);
    let cursor = state.cursor.load(Ordering::Relaxed);
    let channels = usize::from(spec.channels);
    dst.fill(T::EQUILIBRIUM);
    let frames = dst.len() / device_channels;
    let count = (frames * channels).min(source.len().saturating_sub(cursor));
    for (out, frame) in dst
        .chunks_exact_mut(device_channels)
        .zip(source[cursor..cursor + count].chunks_exact(channels))
    {
        for (out, &sample) in out[..channels].iter_mut().zip(frame) {
            *out = T::from_sample(sample.clamp(-1.0, 1.0));
        }
    }
    if cursor < source.len() && cursor + count == source.len() {
        let nominal = Duration::from_secs_f64(frames as f64 / f64::from(spec.sample_rate));
        let observed = if previous == 0 {
            0
        } else {
            now.saturating_sub(previous)
        };
        state
            .final_period_ns
            .store((nominal.as_nanos() as u64).max(observed), Ordering::Relaxed);
        state.final_ns.store(now, Ordering::Release);
    }
    state.cursor.store(cursor + count, Ordering::Release);
}

fn build_output<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    source: Arc<[f32]>,
    state: Arc<CallbackState>,
    spec: StreamSpec,
) -> Result<cpal::Stream, AudioError>
where
    T: SizedSample + FromSample<f32>,
{
    let errors = state.clone();
    let device_channels = usize::from(config.channels);
    device
        .build_output_stream(
            config,
            move |dst: &mut [T], _| render(dst, &source, &state, spec, device_channels),
            move |error| errors.stream_error(error),
            Some(STALL),
        )
        .map_err(backend_error)
}

impl OutputSession for CpalOutputSession {
    fn play_to_completion(
        &mut self,
        interleaved: &[f32],
        cancel: &CancelToken,
    ) -> Result<PlaybackReport, AudioError> {
        aligned(interleaved.len(), self.spec.channels)?;
        self.diagnostics = None;
        if cancel.is_cancelled() {
            return Err(AudioError::Cancelled);
        }
        if !interleaved.iter().all(|s| s.is_finite()) {
            return Err(AudioError::UnsupportedFormat("nonfinite playback".into()));
        }
        let frames = (interleaved.len() / usize::from(self.spec.channels)) as u64;
        if interleaved.is_empty() {
            return Ok(PlaybackReport {
                frames_submitted: 0,
                frames_drained: 0,
                mode: self.mode,
                underruns: 0,
                cancelled: false,
            });
        }
        let source: Arc<[f32]> = interleaved.into();
        let state = Arc::new(CallbackState::new());
        let stream = with_sample_type!(
            self.config.sample_format(),
            build_output,
            &self.device,
            self.config.config(),
            source.clone(),
            state.clone(),
            self.spec
        )?;
        stream.play().map_err(backend_error)?;
        let result = (|| {
            loop {
                state.check(cancel)?;
                let final_ns = state.final_ns.load(Ordering::Acquire);
                let period = state.final_period_ns.load(Ordering::Relaxed);
                if state.cursor.load(Ordering::Acquire) == source.len()
                    && final_ns > 0
                    && state.now_ns().saturating_sub(final_ns) >= period.saturating_mul(2)
                {
                    self.diagnostics = Some(DrainDiagnostics {
                        final_callback_at: Duration::from_nanos(final_ns),
                        final_callback_period: Duration::from_nanos(period),
                        waited_after_final: Duration::from_nanos(
                            state.now_ns().saturating_sub(final_ns),
                        ),
                    });
                    return Ok(());
                }
                std::thread::sleep(POLL);
            }
        })();
        // Drop is the portable stop operation (pause is unsupported by some hosts).
        drop(stream);
        result?;
        Ok(PlaybackReport {
            frames_submitted: frames,
            frames_drained: frames,
            mode: self.mode,
            underruns: state.xruns.load(Ordering::Relaxed).min(u32::MAX as usize) as u32,
            cancelled: false,
        })
    }
}

#[derive(Clone, Copy)]
struct Packet {
    samples: [f32; PACKET_SAMPLES],
    len: usize,
    silent: bool,
}

fn capture<T: Sample>(
    source: &[T],
    tx: &mpsc::SyncSender<Packet>,
    state: &CallbackState,
    channels: usize,
    device_channels: usize,
) where
    f32: FromSample<T>,
{
    state
        .last_callback_ns
        .store(state.now_ns(), Ordering::Release);
    if channels == 0
        || channels > device_channels
        || channels > PACKET_SAMPLES
        || !source.len().is_multiple_of(device_channels)
    {
        state.failed.store(true, Ordering::Release);
        return;
    }
    // Drop whole frames on overflow: otherwise e.g. three-channel packets would
    // shift the channel positions in subsequent packets. Fixed Copy packets do
    // not allocate, destroy heap storage, or use blocking send in the callback.
    let packet_frames = PACKET_SAMPLES / channels;
    for chunk in source.chunks(packet_frames * device_channels) {
        let mut packet = Packet {
            samples: [0.0; PACKET_SAMPLES],
            len: chunk.len() / device_channels * channels,
            silent: true,
        };
        for (dst, frame) in packet.samples[..packet.len]
            .chunks_exact_mut(channels)
            .zip(chunk.chunks_exact(device_channels))
        {
            for (dst, &sample) in dst.iter_mut().zip(&frame[..channels]) {
                *dst = f32::from_sample(sample);
                packet.silent &= *dst == 0.0;
            }
        }
        match tx.try_send(packet) {
            Ok(()) => (),
            Err(mpsc::TrySendError::Full(_)) => {
                state.discontinuity.store(true, Ordering::Release);
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                state.failed.store(true, Ordering::Release);
                return;
            }
        }
    }
}

fn build_input<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    tx: mpsc::SyncSender<Packet>,
    state: Arc<CallbackState>,
    channels: u16,
) -> Result<cpal::Stream, AudioError>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let errors = state.clone();
    let device_channels = usize::from(config.channels);
    let channels = usize::from(channels);
    device
        .build_input_stream(
            config,
            move |src: &[T], _| capture(src, &tx, &state, channels, device_channels),
            move |error| errors.stream_error(error),
            Some(STALL),
        )
        .map_err(backend_error)
}

struct CpalInputSession {
    stream: Option<cpal::Stream>,
    spec: StreamSpec,
    rx: mpsc::Receiver<Packet>,
    state: Arc<CallbackState>,
    pending: Option<Packet>,
    pending_offset: usize,
    started: bool,
    _not_send: PhantomData<Rc<()>>,
}

impl InputSession for CpalInputSession {
    fn start(&mut self) -> Result<(), AudioError> {
        if !self.started {
            // Restart timeout accounting at start, not at open.
            self.state
                .last_callback_ns
                .store(self.state.now_ns(), Ordering::Release);
            self.stream
                .as_ref()
                .ok_or_else(|| backend_error("input already stopped"))?
                .play()
                .map_err(backend_error)?;
            self.started = true;
        }
        Ok(())
    }

    fn read_into(
        &mut self,
        dst: &mut [f32],
        cancel: &CancelToken,
    ) -> Result<CaptureRead, AudioError> {
        aligned(dst.len(), self.spec.channels)?;
        if !self.started {
            return Err(backend_error("input not started"));
        }
        let mut written = 0;
        let mut silent = false;
        self.state.check(cancel)?;
        while written < dst.len() {
            self.state.check(cancel)?;
            if self.pending.is_none() {
                match self.rx.recv_timeout(POLL) {
                    Ok(packet) => {
                        self.pending = Some(packet);
                        self.pending_offset = 0;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        return Err(backend_error("input callback disconnected"));
                    }
                }
            }
            if let Some(packet) = &self.pending {
                let count = (packet.len - self.pending_offset).min(dst.len() - written);
                dst[written..written + count].copy_from_slice(
                    &packet.samples[self.pending_offset..self.pending_offset + count],
                );
                silent |= packet.silent;
                self.pending_offset += count;
                written += count;
                if self.pending_offset == packet.len {
                    self.pending = None;
                }
            }
        }
        Ok(CaptureRead {
            frames: written / usize::from(self.spec.channels),
            silent,
            discontinuity: self.state.discontinuity.swap(false, Ordering::AcqRel),
        })
    }

    fn stop(&mut self) -> Result<(), AudioError> {
        // Consuming the stream is portable; some hosts do not implement pause.
        // Stop is terminal and idempotent. Destruction remains on this worker.
        drop(self.stream.take());
        self.started = false;
        if self.state.failed.load(Ordering::Acquire) {
            return Err(self.state.failure());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(channels: u16, format: SampleFormat) -> cpal::SupportedStreamConfigRange {
        cpal::SupportedStreamConfigRange::new(
            channels,
            44100,
            48000,
            cpal::SupportedBufferSize::Unknown,
            format,
        )
    }

    #[test]
    fn config_exact_channels_win_over_larger_float() {
        let config = choose_config(
            vec![range(16, SampleFormat::F32), range(2, SampleFormat::I16)],
            Direction::Output,
            StreamSpec {
                sample_rate: 48000,
                channels: 2,
            },
        )
        .unwrap();
        assert_eq!(config.channels(), 2);
        assert_eq!(config.sample_format(), SampleFormat::I16);
    }

    #[test]
    fn config_twelve_channels_open_sixteen_channel_device() {
        let config = choose_config(
            vec![range(16, SampleFormat::F32)],
            Direction::Output,
            StreamSpec {
                sample_rate: 48000,
                channels: 12,
            },
        )
        .unwrap();
        assert_eq!(config.channels(), 16);
        assert_eq!(config.sample_rate(), 48000);
    }

    #[test]
    fn config_smallest_sufficient_channel_count_wins() {
        let config = choose_config(
            vec![range(16, SampleFormat::F32), range(8, SampleFormat::I16)],
            Direction::Output,
            StreamSpec {
                sample_rate: 44100,
                channels: 2,
            },
        )
        .unwrap();
        assert_eq!(config.channels(), 8);
        assert_eq!(config.sample_rate(), 44100);
    }

    #[test]
    fn config_float_is_preferred_within_channel_count() {
        for channels in [2, 18] {
            let config = choose_config(
                vec![
                    range(channels, SampleFormat::I16),
                    range(channels, SampleFormat::F32),
                ],
                Direction::Input,
                StreamSpec {
                    sample_rate: 48000,
                    channels: 2,
                },
            )
            .unwrap();
            assert_eq!(config.channels(), channels);
            assert_eq!(config.sample_format(), SampleFormat::F32);
        }
    }

    #[test]
    fn config_fewer_channels_are_rejected() {
        let error = choose_config(
            vec![range(2, SampleFormat::F32), range(8, SampleFormat::F32)],
            Direction::Output,
            StreamSpec {
                sample_rate: 48000,
                channels: 12,
            },
        )
        .unwrap_err();
        assert!(matches!(error, AudioError::UnsupportedFormat(detail)
            if detail == "native Output 48000 Hz / 12 channels unavailable"));
    }

    #[test]
    fn config_non_native_rate_is_rejected() {
        let error = choose_config(
            vec![range(2, SampleFormat::F32), range(16, SampleFormat::F32)],
            Direction::Input,
            StreamSpec {
                sample_rate: 96000,
                channels: 2,
            },
        )
        .unwrap_err();
        assert!(matches!(error, AudioError::UnsupportedFormat(detail)
            if detail == "native Input 96000 Hz / 2 channels unavailable"));
    }

    #[test]
    fn callback_output_pads_device_channels_and_counts_source_samples() {
        let state = CallbackState::new();
        let spec = StreamSpec {
            sample_rate: 1000,
            channels: 2,
        };
        let source = [0.25, -0.25, 0.5, -0.5, 0.75, -0.75];
        let mut dst = [1.0; 8];
        render(&mut dst, &source, &state, spec, 4);
        assert_eq!(dst, [0.25, -0.25, 0.0, 0.0, 0.5, -0.5, 0.0, 0.0]);
        assert_eq!(state.cursor.load(Ordering::Acquire), 4);
        assert_eq!(state.final_ns.load(Ordering::Acquire), 0);
        // Isolate the nominal period from scheduler delays between callbacks.
        state.last_callback_ns.store(0, Ordering::Release);
        render(&mut dst, &source, &state, spec, 4);
        assert_eq!(dst, [0.75, -0.75, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(state.cursor.load(Ordering::Acquire), source.len());
        assert_eq!(state.final_period_ns.load(Ordering::Acquire), 2_000_000);
        let final_ns = state.final_ns.load(Ordering::Acquire);
        assert_ne!(final_ns, 0);
        render(&mut dst, &source, &state, spec, 4);
        assert_eq!(dst, [0.0; 8]);
        assert_eq!(state.final_ns.load(Ordering::Acquire), final_ns);
    }

    #[test]
    fn callback_output_non_frame_tail_uses_equilibrium() {
        let state = CallbackState::new();
        let spec = StreamSpec {
            sample_rate: 1000,
            channels: 2,
        };
        let mut dst = [0_u16; 11];
        render(&mut dst, &[-1.0, 0.5, 0.0, -0.5], &state, spec, 4);
        assert_eq!(
            dst,
            [
                0, 49152, 32768, 32768, 32768, 16384, 32768, 32768, 32768, 32768, 32768
            ]
        );
        assert_eq!(state.cursor.load(Ordering::Acquire), 4);
        assert_eq!(state.final_period_ns.load(Ordering::Acquire), 2_000_000);
    }

    #[test]
    fn callback_output_short_device_frame_does_not_consume_source() {
        let state = CallbackState::new();
        let mut dst = [1.0; 3];
        render(
            &mut dst,
            &[0.25, 0.5],
            &state,
            StreamSpec {
                sample_rate: 48000,
                channels: 2,
            },
            4,
        );
        assert_eq!(dst, [0.0; 3]);
        assert_eq!(state.cursor.load(Ordering::Acquire), 0);
        assert_eq!(state.final_ns.load(Ordering::Acquire), 0);
    }

    #[test]
    fn callback_capture_keeps_first_channels_of_each_device_frame() {
        let state = CallbackState::new();
        let (tx, rx) = mpsc::sync_channel(1);
        capture(
            &[0.25_f32, -0.25, 0.75, 1.0, 0.5, -0.5, 0.75, 1.0],
            &tx,
            &state,
            2,
            4,
        );
        let packet = rx.try_recv().unwrap();
        assert_eq!(packet.len, 4);
        assert_eq!(&packet.samples[..packet.len], &[0.25, -0.25, 0.5, -0.5]);
        assert!(!packet.silent);
        assert!(!state.failed.load(Ordering::Acquire));
    }

    #[test]
    fn callback_capture_adapted_overflow_drops_whole_kept_frames() {
        let state = CallbackState::new();
        let (tx, rx) = mpsc::sync_channel(1);
        let samples = [0.25_f32, -0.25, 0.75, 1.0].repeat(PACKET_SAMPLES / 2 + 3);
        capture(&samples, &tx, &state, 2, 4);
        let packet = rx.try_recv().unwrap();
        assert_eq!(packet.len, PACKET_SAMPLES);
        assert!(
            packet
                .samples
                .as_chunks::<2>()
                .0
                .iter()
                .all(|frame| *frame == [0.25, -0.25])
        );
        assert!(state.discontinuity.load(Ordering::Acquire));
        assert!(rx.try_recv().is_err());
        capture(&[0.5_f32, -0.5, 1.0, 1.0], &tx, &state, 2, 4);
        let packet = rx.try_recv().unwrap();
        assert_eq!(packet.len, 2);
        assert_eq!(&packet.samples[..2], &[0.5, -0.5]);
        assert!(!state.failed.load(Ordering::Acquire));
    }

    #[test]
    fn callback_capture_silence_ignores_discarded_channels() {
        let state = CallbackState::new();
        let (tx, rx) = mpsc::sync_channel(1);
        capture(&[0.0_f32, 0.0, 1.0, -1.0], &tx, &state, 2, 4);
        assert!(rx.try_recv().unwrap().silent);
        capture(&[0.0_f32, 0.25, 0.0, 0.0], &tx, &state, 2, 4);
        assert!(!rx.try_recv().unwrap().silent);
    }

    #[test]
    fn callback_capture_validates_device_alignment_and_kept_capacity() {
        for (channels, device_channels, len) in [
            (0, 4, 4),
            (2, 0, 4),
            (4, 2, 4),
            (2, 4, 6),
            (1025, 1025, 1025),
        ] {
            let state = CallbackState::new();
            let (tx, rx) = mpsc::sync_channel(1);
            capture(&vec![0.0_f32; len], &tx, &state, channels, device_channels);
            assert!(state.failed.load(Ordering::Acquire));
            assert!(rx.try_recv().is_err());
        }
        let state = CallbackState::new();
        let (tx, rx) = mpsc::sync_channel(1);
        // Native frames may exceed packet capacity when only a few channels are kept.
        capture(
            &[0.0_f32; PACKET_SAMPLES + 1],
            &tx,
            &state,
            2,
            PACKET_SAMPLES + 1,
        );
        assert_eq!(rx.try_recv().unwrap().len, 2);
        assert!(!state.failed.load(Ordering::Acquire));
    }

    #[test]
    fn callback_output_zero_fills_and_measures_final_period() {
        let state = CallbackState::new();
        let mut dst = [1.0; 8];
        let spec = StreamSpec {
            sample_rate: 1000,
            channels: 2,
        };
        render(&mut dst, &[0.25, -0.25], &state, spec, 2);
        assert_eq!(dst, [0.25, -0.25, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(state.cursor.load(Ordering::Acquire), 2);
        assert_eq!(state.final_period_ns.load(Ordering::Acquire), 4_000_000);
        let final_ns = state.final_ns.load(Ordering::Acquire);
        render(&mut dst, &[0.25, -0.25], &state, spec, 2);
        assert_eq!(dst, [0.0; 8]);
        assert_eq!(state.final_ns.load(Ordering::Acquire), final_ns);
    }

    #[test]
    fn callback_capture_is_bounded_and_reports_overflow() {
        let state = CallbackState::new();
        let (tx, rx) = mpsc::sync_channel(1);
        capture(&[0_i16; PACKET_SAMPLES + 7], &tx, &state, 1, 1);
        let packet = rx.try_recv().unwrap();
        assert_eq!(packet.len, PACKET_SAMPLES);
        assert!(packet.silent);
        assert!(state.discontinuity.load(Ordering::Acquire));
        assert!(rx.try_recv().is_err());
        drop(rx);
        capture(&[1_i16], &tx, &state, 1, 1);
        assert!(state.failed.load(Ordering::Acquire));
    }

    #[test]
    fn callback_native_sample_conversion() {
        let state = CallbackState::new();
        let mut dst = [0_u16; 4];
        render(
            &mut dst,
            &[-1.0, 0.0, 0.5],
            &state,
            StreamSpec {
                sample_rate: 48000,
                channels: 1,
            },
            1,
        );
        assert_eq!(dst, [0, 32768, 49152, 32768]);
        let (tx, rx) = mpsc::sync_channel(1);
        capture(&dst, &tx, &state, 1, 1);
        assert_eq!(&rx.recv().unwrap().samples[..4], &[-1.0, 0.0, 0.5, 0.0]);
    }

    #[test]
    fn callback_overflow_preserves_three_channel_alignment() {
        let state = CallbackState::new();
        let (tx, rx) = mpsc::sync_channel(1);
        let samples: Vec<f32> = (0..2052).map(|i| (i % 3) as f32 / 4.0).collect();
        capture(&samples, &tx, &state, 3, 3);
        let packet = rx.recv().unwrap();
        assert_eq!(packet.len, 1023);
        assert!(state.discontinuity.load(Ordering::Acquire));
        capture(&samples[..6], &tx, &state, 3, 3);
        assert_eq!(
            &rx.recv().unwrap().samples[..6],
            &[0.0, 0.25, 0.5, 0.0, 0.25, 0.5]
        );
    }

    #[test]
    fn capture_pending_partial_reads_flags_and_lifecycle() {
        let state = Arc::new(CallbackState::new());
        let (tx, rx) = mpsc::sync_channel(2);
        let mut session = CpalInputSession {
            stream: None,
            spec: StreamSpec {
                sample_rate: 48000,
                channels: 2,
            },
            rx,
            state: state.clone(),
            pending: None,
            pending_offset: 0,
            started: true,
            _not_send: PhantomData,
        };
        capture(&[0.0_f32; 6], &tx, &state, 2, 2);
        capture(&[0.25_f32, 0.5], &tx, &state, 2, 2);
        state.discontinuity.store(true, Ordering::Release);
        let cancel = CancelToken::new();
        let mut first = [1.0; 4];
        let report = session.read_into(&mut first, &cancel).unwrap();
        assert_eq!(report.frames, 2);
        assert!(report.silent && report.discontinuity);
        assert_eq!(first, [0.0; 4]);
        let mut second = [1.0; 4];
        let report = session.read_into(&mut second, &cancel).unwrap();
        assert_eq!(second, [0.0, 0.0, 0.25, 0.5]);
        assert!(report.silent);
        assert!(!report.discontinuity);
        assert!(session.read_into(&mut [0.0; 3], &cancel).is_err());
        cancel.cancel();
        assert!(matches!(
            session.read_into(&mut first, &cancel),
            Err(AudioError::Cancelled)
        ));
        session.stop().unwrap();
        session.stop().unwrap();
        assert!(session.start().is_err());
        assert!(session.read_into(&mut first, &CancelToken::new()).is_err());
    }

    #[test]
    fn callback_xrun_records_discontinuity_without_stopping() {
        let state = CallbackState::new();
        state.stream_error(cpal::ErrorKind::Xrun.into());
        state.check(&CancelToken::new()).unwrap();
        assert_eq!(state.xruns.load(Ordering::Relaxed), 1);
        assert!(state.discontinuity.load(Ordering::Acquire));
        state.stream_error(cpal::ErrorKind::StreamInvalidated.into());
        assert!(matches!(
            state.check(&CancelToken::new()),
            Err(AudioError::Backend(_))
        ));
    }

    #[test]
    fn callback_failure_and_stall_are_errors() {
        let state = CallbackState::new();
        state.failed.store(true, Ordering::Release);
        assert!(matches!(
            state.check(&CancelToken::new()),
            Err(AudioError::Backend(_))
        ));
        let mut state = CallbackState::new();
        state.epoch = Instant::now() - STALL - POLL;
        assert!(matches!(
            state.check(&CancelToken::new()),
            Err(AudioError::Backend(_))
        ));
        let cancel = CancelToken::new();
        cancel.cancel();
        assert!(matches!(state.check(&cancel), Err(AudioError::Cancelled)));
    }
}
