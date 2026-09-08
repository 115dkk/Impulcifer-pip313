#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]
// The WASAPI backend below is `cfg(windows)`; the platform-independent helpers
// (format validation, byte conversion, packet handling) are exercised by the
// unit tests on every OS but only reached by the backend on Windows.
#![cfg_attr(not(windows), allow(dead_code))]

//! WASAPI-only Windows audio backend.
//!
//! WASAPI/COM objects in this crate are thread-affine. Each operation initializes
//! MTA on its calling thread, and an opened session keeps that apartment alive
//! until all of its WASAPI objects have been dropped. `OutputSession` and
//! `InputSession` are deliberately `!Send`; callers must create, use, and drop a
//! session on one OS thread. This crate never uses `WaveFormat::parse` or
//! `Device::from_raw`.

use std::collections::VecDeque;

use impulcifer_types::audio::{
    AudioBackend, AudioError, CancelToken, Direction, Endpoint, InputSession, OutputSession,
    PlaybackReport, ProbeResult, ShareMode, StreamSpec,
};

const HOST_API: &str = "Windows WASAPI";

/// WASAPI backend. The value itself owns no COM state and may be shared.
#[derive(Clone, Copy, Debug, Default)]
pub struct WasapiBackend;

impl WasapiBackend {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaptureStats {
    pub discontinuity_packets: u64,
    pub silent_packets: u64,
}

fn validate_spec(spec: StreamSpec) -> Result<(), AudioError> {
    if spec.sample_rate == 0 {
        return Err(AudioError::UnsupportedFormat(
            "sample rate must be greater than zero".into(),
        ));
    }
    if spec.channels == 0 {
        return Err(AudioError::UnsupportedFormat(
            "channel count must be greater than zero".into(),
        ));
    }
    Ok(())
}

fn validate_shared_channels(requested: u16, mix_channels: u16) -> Result<(), AudioError> {
    if requested > mix_channels {
        return Err(AudioError::UnsupportedFormat(format!(
            "shared mode requested {requested} channels, but the endpoint mix format has {mix_channels} channels"
        )));
    }
    Ok(())
}

fn validate_frame_aligned(len: usize, channels: u16) -> Result<(), AudioError> {
    if channels == 0 || !len.is_multiple_of(usize::from(channels)) {
        return Err(AudioError::UnsupportedFormat(format!(
            "sample buffer length {len} is not aligned to {channels} channels"
        )));
    }
    Ok(())
}

fn endpoint_from_format(
    id: String,
    name: String,
    direction: Direction,
    sample_rate: u32,
    channels: u16,
    is_default: bool,
) -> Endpoint {
    Endpoint {
        id,
        name,
        host_api: HOST_API.into(),
        max_input_channels: u16::from(direction == Direction::Input) * channels,
        max_output_channels: u16::from(direction == Direction::Output) * channels,
        default_samplerate: f64::from(sample_rate),
        is_default_input: direction == Direction::Input && is_default,
        is_default_output: direction == Direction::Output && is_default,
    }
}

fn probe_result_from_format(
    mode: ShareMode,
    sample_rate: u32,
    channels: u16,
    supported: bool,
    detail: String,
) -> ProbeResult {
    ProbeResult {
        supported,
        mode,
        native_sample_rate: sample_rate,
        native_channels: channels,
        detail,
    }
}

#[cfg(test)]
fn f32_to_bytes(samples: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(samples));
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
fn bytes_to_f32(bytes: &[u8]) -> Result<Vec<f32>, AudioError> {
    if !bytes.len().is_multiple_of(size_of::<f32>()) {
        return Err(AudioError::UnsupportedFormat(format!(
            "byte buffer length {} is not a multiple of four",
            bytes.len()
        )));
    }
    Ok(bytes
        .chunks_exact(size_of::<f32>())
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

#[derive(Clone, Copy, Debug, Default)]
struct PacketFlags {
    silent: bool,
    discontinuity: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct PacketHeader {
    frames: usize,
    flags: PacketFlags,
    index: u64,
    timestamp: u64,
}

#[derive(Clone, Copy, Debug, Default)]
struct AppendedPacket {
    header: PacketHeader,
    raw_zero_runs: usize,
    raw_zero_frames: usize,
    silent_fill_frames: usize,
}

trait PacketSource {
    fn read_packet(&mut self, scratch: &mut [u8]) -> Result<PacketHeader, AudioError>;
}

fn append_packet<S: PacketSource>(
    source: &mut S,
    scratch: &mut [u8],
    pending: &mut VecDeque<f32>,
    channels: u16,
    diagnose_zeros: bool,
) -> Result<AppendedPacket, AudioError> {
    let header = source.read_packet(scratch)?;
    let sample_count = header
        .frames
        .checked_mul(usize::from(channels))
        .ok_or_else(|| AudioError::Backend("capture packet size overflow".into()))?;
    let byte_count = sample_count
        .checked_mul(size_of::<f32>())
        .ok_or_else(|| AudioError::Backend("capture packet size overflow".into()))?;
    if byte_count > scratch.len() {
        return Err(AudioError::Backend(format!(
            "capture packet reported {byte_count} bytes for a {}-byte buffer",
            scratch.len()
        )));
    }
    let (raw_zero_runs, raw_zero_frames, silent_fill_frames) = if header.flags.silent {
        pending.extend(std::iter::repeat_n(0.0, sample_count));
        (0, 0, header.frames)
    } else {
        let mut zero_runs = 0usize;
        let mut zero_frames = 0usize;
        if diagnose_zeros {
            let mut in_zero_run = false;
            for frame in
                scratch[..byte_count].chunks_exact(size_of::<f32>() * usize::from(channels))
            {
                let all_zero = frame.chunks_exact(4).all(|b| {
                    f32::from_le_bytes([b[0], b[1], b[2], b[3]]).to_bits() & 0x7fff_ffff == 0
                });
                if all_zero {
                    zero_frames += 1;
                    if !in_zero_run {
                        zero_runs += 1;
                        in_zero_run = true;
                    }
                } else {
                    in_zero_run = false;
                }
            }
        }
        pending.extend(
            scratch[..byte_count]
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        );
        (zero_runs, zero_frames, 0)
    };
    Ok(AppendedPacket {
        header,
        raw_zero_runs,
        raw_zero_frames,
        silent_fill_frames,
    })
}

/// Samples read from the device but not yet handed to the caller, together
/// with the packet flags that apply to them. Keeping the flags next to the
/// queued samples lets a later `read_into` still report SILENT/discontinuity
/// for frames that were captured during an earlier call.
#[derive(Default)]
struct CaptureQueue {
    pending: VecDeque<f32>,
    flags: PacketFlags,
}

impl CaptureQueue {
    /// Move queued samples into `dst[*written..]`. Returns the flags that
    /// applied to the samples handed out (all false when nothing was queued)
    /// and clears the stored flags once the queue runs empty.
    fn drain_into(&mut self, dst: &mut [f32], written: &mut usize) -> PacketFlags {
        if self.pending.is_empty() {
            return PacketFlags::default();
        }
        let count = self.pending.len().min(dst.len() - *written);
        if count == 0 {
            return PacketFlags::default();
        }
        let (a, b) = self.pending.as_slices();
        let first = count.min(a.len());
        dst[*written..*written + first].copy_from_slice(&a[..first]);
        dst[*written + first..*written + count].copy_from_slice(&b[..count - first]);
        self.pending.drain(..count);
        *written += count;
        let flags = self.flags;
        if self.pending.is_empty() {
            self.flags = PacketFlags::default();
        }
        flags
    }

    /// Read one packet from `source` into the queue and remember its flags.
    fn push_packet<S: PacketSource>(
        &mut self,
        source: &mut S,
        scratch: &mut [u8],
        channels: u16,
        diagnose_zeros: bool,
    ) -> Result<AppendedPacket, AudioError> {
        let packet = append_packet(source, scratch, &mut self.pending, channels, diagnose_zeros)?;
        self.flags.silent |= packet.header.flags.silent;
        self.flags.discontinuity |= packet.header.flags.discontinuity;
        Ok(packet)
    }
}

trait SampleSink {
    fn wait(&self) -> Result<(), AudioError> {
        std::thread::sleep(std::time::Duration::from_millis(1));
        Ok(())
    }
    fn capacity_frames(&self) -> Result<usize, AudioError>;
    fn padding_frames(&self) -> Result<usize, AudioError>;
    fn buffer_snapshot(&self) -> Result<(usize, usize), AudioError> {
        let free = self.capacity_frames()?;
        Ok((free, 0))
    }
    fn write_frames(&mut self, samples: &[f32], snapshot: (usize, usize))
    -> Result<(), AudioError>;
    fn start(&mut self) -> Result<(), AudioError>;
    fn stop(&mut self) -> Result<(), AudioError>;
}

fn play_with_sink<S: SampleSink>(
    sink: &mut S,
    interleaved: &[f32],
    channels: u16,
    mode: ShareMode,
    cancel: &CancelToken,
) -> Result<PlaybackReport, AudioError> {
    validate_frame_aligned(interleaved.len(), channels)?;
    let channels = usize::from(channels);
    let total_frames = interleaved.len() / channels;
    let mut submitted = 0usize;
    let mut underruns = 0u32;
    let mut started = false;

    while submitted < total_frames {
        if cancel.is_cancelled() {
            let padding = if started { sink.padding_frames()? } else { 0 };
            if started {
                sink.stop()?;
            }
            return Ok(PlaybackReport {
                frames_submitted: submitted as u64,
                frames_drained: submitted.saturating_sub(padding) as u64,
                mode,
                underruns,
                cancelled: true,
            });
        }

        let (capacity, padding) = sink.buffer_snapshot()?;
        if started && capacity > 0 && padding == 0 {
            underruns = underruns.saturating_add(1);
        }
        if capacity == 0 {
            sink.wait()?;
            continue;
        }
        let frames = capacity.min(total_frames - submitted);
        let first = submitted * channels;
        let last = first + frames * channels;
        sink.write_frames(&interleaved[first..last], (capacity, padding))?;
        submitted += frames;
        if !started {
            sink.start()?;
            started = true;
        }
        // After priming, wait for the engine before querying fresh padding.
        // In particular, do not refill a tiny fragment immediately after Start.
        if submitted < total_frames {
            sink.wait()?;
        }
    }

    while started && sink.padding_frames()? > 0 {
        if cancel.is_cancelled() {
            let padding = sink.padding_frames()?;
            sink.stop()?;
            return Ok(PlaybackReport {
                frames_submitted: submitted as u64,
                frames_drained: submitted.saturating_sub(padding) as u64,
                mode,
                underruns,
                cancelled: true,
            });
        }
        sink.wait()?;
    }
    if started {
        sink.stop()?;
    }
    Ok(PlaybackReport {
        frames_submitted: submitted as u64,
        frames_drained: submitted as u64,
        mode,
        underruns,
        cancelled: false,
    })
}

#[cfg(windows)]
mod windows_backend {
    use std::fs::File;
    use std::io::{BufWriter, Write};
    use std::marker::PhantomData;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    use impulcifer_types::audio::CaptureRead;
    use wasapi::{
        AudioCaptureClient, AudioClient, AudioRenderClient, Device, DeviceEnumerator,
        Direction as WasapiDirection, SampleType, ShareMode as WasapiShareMode, StreamMode,
        WaveFormat,
    };

    use super::*;

    // PA05 paired ten-run trials at three and two periods reported underruns.
    // Retain four periods; a successful render submission alone cannot prove
    // that the shared engine/cable delivered every source frame.
    const SHARED_BUFFER_PERIODS: i64 = 4;
    const AUDCLNT_E_UNSUPPORTED_FORMAT: u32 = 0x8889_0008;
    const TRACE_ENV: &str = "IMPULCIFER_PA05_TRACE_PREFIX";
    static TRACE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[derive(Default)]
    struct TraceRecord {
        elapsed_us: u128,
        category: &'static str,
        event: &'static str,
        duration_us: u128,
        ordinal: Option<u64>,
        frames: Option<usize>,
        frame_start: Option<u64>,
        frame_end: Option<u64>,
        buffer_index: Option<u64>,
        buffer_timestamp: Option<u64>,
        silent: Option<bool>,
        discontinuity: Option<bool>,
        initial_discontinuity: Option<bool>,
        raw_zero_runs: Option<usize>,
        raw_zero_frames: Option<usize>,
        silent_fill_frames: Option<usize>,
        queue_before: Option<usize>,
        queue_after: Option<usize>,
        delivery_start: Option<u64>,
        delivery_end: Option<u64>,
        buffer_size: Option<usize>,
        padding: Option<usize>,
        free: Option<usize>,
        detail: String,
    }

    struct TraceState {
        started: Instant,
        path: PathBuf,
        records: Vec<TraceRecord>,
        flushed: bool,
    }

    #[derive(Clone)]
    struct TraceHandle(Rc<std::cell::RefCell<TraceState>>);

    struct TraceOwner(Option<TraceHandle>);

    impl TraceHandle {
        fn from_env(kind: &str) -> Result<Option<(Self, TraceOwner)>, AudioError> {
            let Some(prefix) = std::env::var_os(TRACE_ENV) else {
                return Ok(None);
            };
            let prefix = PathBuf::from(prefix);
            if !prefix.is_absolute() {
                return Err(AudioError::Backend(format!(
                    "{TRACE_ENV} must be an absolute path"
                )));
            }
            let sequence = TRACE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = PathBuf::from(format!(
                "{}-{kind}-{}-{sequence}.csv",
                prefix.display(),
                std::process::id()
            ));
            let state = Self(Rc::new(std::cell::RefCell::new(TraceState {
                started: Instant::now(),
                path,
                records: Vec::with_capacity(16_384),
                flushed: false,
            })));
            Ok(Some((state.clone(), TraceOwner(Some(state)))))
        }

        fn record(&self, mut record: TraceRecord) {
            let mut state = self.0.borrow_mut();
            record.elapsed_us = state.started.elapsed().as_micros();
            state.records.push(record);
        }

        fn stage<T>(
            &self,
            event: &'static str,
            operation: impl FnOnce() -> Result<T, AudioError>,
        ) -> Result<T, AudioError> {
            let started = Instant::now();
            let result = operation();
            self.record(TraceRecord {
                category: "open",
                event,
                duration_us: started.elapsed().as_micros(),
                detail: if result.is_ok() { "ok" } else { "error" }.into(),
                ..TraceRecord::default()
            });
            result
        }

        fn flush(&self) -> Result<(), AudioError> {
            let mut state = self.0.borrow_mut();
            if state.flushed {
                return Ok(());
            }
            let file = File::create(&state.path).map_err(|error| {
                AudioError::Backend(format!(
                    "create PA05 trace {}: {error}",
                    state.path.display()
                ))
            })?;
            let mut writer = BufWriter::new(file);
            writeln!(writer, "elapsed_us,category,event,duration_us,ordinal,frames,frame_start,frame_end,buffer_index,buffer_timestamp,silent,discontinuity,initial_discontinuity,raw_zero_runs,raw_zero_frames,silent_fill_frames,queue_before,queue_after,delivery_start,delivery_end,buffer_size,padding,free,detail")
                .map_err(|error| AudioError::Backend(format!("write PA05 trace header: {error}")))?;
            for record in &state.records {
                writeln!(
                    writer,
                    "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                    record.elapsed_us,
                    record.category,
                    record.event,
                    record.duration_us,
                    csv_option(record.ordinal),
                    csv_option(record.frames),
                    csv_option(record.frame_start),
                    csv_option(record.frame_end),
                    csv_option(record.buffer_index),
                    csv_option(record.buffer_timestamp),
                    csv_option(record.silent),
                    csv_option(record.discontinuity),
                    csv_option(record.initial_discontinuity),
                    csv_option(record.raw_zero_runs),
                    csv_option(record.raw_zero_frames),
                    csv_option(record.silent_fill_frames),
                    csv_option(record.queue_before),
                    csv_option(record.queue_after),
                    csv_option(record.delivery_start),
                    csv_option(record.delivery_end),
                    csv_option(record.buffer_size),
                    csv_option(record.padding),
                    csv_option(record.free),
                    record.detail.replace(',', ";")
                )
                .map_err(|error| AudioError::Backend(format!("write PA05 trace row: {error}")))?;
            }
            writer
                .flush()
                .map_err(|error| AudioError::Backend(format!("flush PA05 trace: {error}")))?;
            state.flushed = true;
            Ok(())
        }
    }

    fn csv_option<T: std::fmt::Display>(value: Option<T>) -> String {
        value.map(|item| item.to_string()).unwrap_or_default()
    }

    fn trace_stage<T>(
        trace: Option<&TraceHandle>,
        event: &'static str,
        operation: impl FnOnce() -> Result<T, AudioError>,
    ) -> Result<T, AudioError> {
        if let Some(trace) = trace {
            trace.stage(event, operation)
        } else {
            operation()
        }
    }

    struct TracedDrop<T> {
        value: Option<T>,
        event: &'static str,
        trace: Option<TraceHandle>,
    }

    impl<T> TracedDrop<T> {
        fn new(value: T, event: &'static str, trace: Option<&TraceHandle>) -> Self {
            Self {
                value: Some(value),
                event,
                trace: trace.cloned(),
            }
        }
    }

    impl<T> std::ops::Deref for TracedDrop<T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            self.value.as_ref().expect("traced value already dropped")
        }
    }

    impl<T> Drop for TracedDrop<T> {
        fn drop(&mut self) {
            let started = Instant::now();
            drop(self.value.take());
            if let Some(trace) = &self.trace {
                trace.record(TraceRecord {
                    category: "drop",
                    event: self.event,
                    duration_us: started.elapsed().as_micros(),
                    detail: "ok".into(),
                    ..TraceRecord::default()
                });
            }
        }
    }

    impl Drop for TraceOwner {
        fn drop(&mut self) {
            if let Some(trace) = &self.0 {
                let _ = trace.flush();
            }
        }
    }

    fn shared_buffer_duration(
        client: &AudioClient,
        trace: Option<&TraceHandle>,
    ) -> Result<i64, AudioError> {
        let (period, _) = trace_stage(trace, "get_device_period", || {
            client
                .get_device_period()
                .map_err(|err| backend_error("failed to query shared device period", err))
        })?;
        if period <= 0 {
            return Err(AudioError::Backend("invalid shared device period".into()));
        }
        let duration = period.saturating_mul(SHARED_BUFFER_PERIODS);
        if let Some(trace) = trace {
            trace.record(TraceRecord {
                category: "open",
                event: "shared_period",
                detail: format!(
                    "device_period_hns={period} requested_periods={SHARED_BUFFER_PERIODS} requested_duration_hns={duration}"
                ),
                ..TraceRecord::default()
            });
        }
        Ok(duration)
    }

    fn wait_event(event: &wasapi::Handle) -> Result<(), AudioError> {
        match event.wait_for_event(10) {
            Ok(()) | Err(wasapi::WasapiError::EventTimeout) => Ok(()),
            Err(e) => Err(backend_error("WASAPI event wait", e)),
        }
    }

    struct ComGuard {
        trace: Option<TraceHandle>,
        _not_send: PhantomData<Rc<()>>,
    }

    impl ComGuard {
        fn initialize(trace: Option<&TraceHandle>) -> Result<Self, AudioError> {
            trace_stage(trace, "com_init", || {
                wasapi::initialize_mta().ok().map_err(|err| {
                    AudioError::Backend(format!("failed to initialize COM MTA: {err}"))
                })
            })?;
            Ok(Self {
                trace: trace.cloned(),
                _not_send: PhantomData,
            })
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            let started = Instant::now();
            wasapi::deinitialize();
            if let Some(trace) = &self.trace {
                trace.record(TraceRecord {
                    category: "drop",
                    event: "com_uninit",
                    duration_us: started.elapsed().as_micros(),
                    detail: "ok".into(),
                    ..TraceRecord::default()
                });
            }
        }
    }

    struct EnumeratorState {
        enumerator: TracedDrop<DeviceEnumerator>,
        leases: usize,
    }

    thread_local! {
        static ENUMERATOR: std::cell::RefCell<Option<EnumeratorState>> = const {
            std::cell::RefCell::new(None)
        };
    }

    struct EnumeratorLease {
        _not_send: PhantomData<Rc<()>>,
    }

    impl EnumeratorLease {
        fn acquire(trace: Option<&TraceHandle>) -> Result<Self, AudioError> {
            ENUMERATOR.with(|slot| {
                let mut slot = slot.borrow_mut();
                if let Some(state) = slot.as_mut() {
                    state.leases += 1;
                    if let Some(trace) = trace {
                        trace.record(TraceRecord {
                            category: "open",
                            event: "enumerator",
                            detail: "thread_local_reuse".into(),
                            ..TraceRecord::default()
                        });
                    }
                } else {
                    let enumerator = trace_stage(trace, "enumerator", || {
                        DeviceEnumerator::new().map_err(|err| {
                            backend_error("failed to create WASAPI device enumerator", err)
                        })
                    })?;
                    *slot = Some(EnumeratorState {
                        enumerator: TracedDrop::new(enumerator, "enumerator_release", trace),
                        leases: 1,
                    });
                }
                Ok(Self {
                    _not_send: PhantomData,
                })
            })
        }

        fn get_device(
            &self,
            endpoint: &Endpoint,
            direction: Direction,
            trace: Option<&TraceHandle>,
        ) -> Result<Device, AudioError> {
            ENUMERATOR.with(|slot| {
                let slot = slot.borrow();
                let state = slot.as_ref().ok_or_else(|| {
                    AudioError::Backend("WASAPI enumerator lease has no state".into())
                })?;
                let device = trace_stage(trace, "device_lookup", || {
                    state
                        .enumerator
                        .get_device(&endpoint.id)
                        .map_err(|_| AudioError::DeviceNotFound(endpoint.id.clone()))
                })?;
                validate_device_direction(device, endpoint, direction)
            })
        }
    }

    impl Drop for EnumeratorLease {
        fn drop(&mut self) {
            ENUMERATOR.with(|slot| {
                let mut slot = slot.borrow_mut();
                if let Some(state) = slot.as_mut() {
                    state.leases -= 1;
                    if state.leases == 0 {
                        drop(slot.take());
                    }
                }
            });
        }
    }

    fn backend_error(context: &str, err: impl std::fmt::Display) -> AudioError {
        AudioError::Backend(format!("{context}: {err}"))
    }

    fn unsupported(context: &str, err: impl std::fmt::Display) -> AudioError {
        AudioError::UnsupportedFormat(format!("{context}: {err}"))
    }

    fn wasapi_direction(direction: Direction) -> WasapiDirection {
        match direction {
            Direction::Input => WasapiDirection::Capture,
            Direction::Output => WasapiDirection::Render,
        }
    }

    fn requested_format(spec: StreamSpec) -> WaveFormat {
        WaveFormat::new(
            32,
            32,
            &SampleType::Float,
            spec.sample_rate as usize,
            usize::from(spec.channels),
            None,
        )
    }

    fn validate_device_direction(
        device: Device,
        endpoint: &Endpoint,
        direction: Direction,
    ) -> Result<Device, AudioError> {
        if device.get_direction() != wasapi_direction(direction) {
            return Err(AudioError::DeviceNotFound(format!(
                "{} is not a {:?} endpoint",
                endpoint.id, direction
            )));
        }
        Ok(device)
    }

    fn get_device(
        endpoint: &Endpoint,
        direction: Direction,
        trace: Option<&TraceHandle>,
    ) -> Result<Device, AudioError> {
        let enumerator = trace_stage(trace, "enumerator", || {
            DeviceEnumerator::new()
                .map_err(|err| backend_error("failed to create WASAPI device enumerator", err))
        })?;
        let device = trace_stage(trace, "device_lookup", || {
            enumerator
                .get_device(&endpoint.id)
                .map_err(|_| AudioError::DeviceNotFound(endpoint.id.clone()))
        })?;
        validate_device_direction(device, endpoint, direction)
    }

    fn mix_format(device: &Device) -> Result<WaveFormat, AudioError> {
        device
            .get_iaudioclient()
            .and_then(|client| client.get_mixformat())
            .map_err(|err| backend_error("failed to read endpoint mix format", err))
    }

    fn stream_mode(
        client: &AudioClient,
        format: &WaveFormat,
        mode: ShareMode,
        trace: Option<&TraceHandle>,
    ) -> Result<StreamMode, AudioError> {
        match mode {
            ShareMode::SharedAutoConvert => Ok(StreamMode::EventsShared {
                autoconvert: true,
                buffer_duration_hns: shared_buffer_duration(client, trace)?,
            }),
            ShareMode::Exclusive => {
                let (default_period, _) = client
                    .get_device_period()
                    .map_err(|err| backend_error("failed to query device period", err))?;
                let period = client
                    .calculate_aligned_period_near(default_period, Some(128), format)
                    .map_err(|err| backend_error("failed to calculate exclusive period", err))?;
                Ok(StreamMode::PollingExclusive {
                    period_hns: period,
                    buffer_duration_hns: period.saturating_mul(4),
                })
            }
        }
    }

    fn accepted_format(
        client: &AudioClient,
        requested: &WaveFormat,
        mode: ShareMode,
    ) -> Result<WaveFormat, AudioError> {
        match mode {
            ShareMode::Exclusive => {
                // Match wasapi's safe quirks candidates, but preserve failures
                // from EVERY query. Its helper collapses transient HRESULTs into
                // UnsupportedFormat, which must not become a permanent refusal.
                let query = |format: &WaveFormat| match client
                    .is_supported(format, &WasapiShareMode::Exclusive)
                {
                    Ok(_) => Ok(true),
                    Err(wasapi::WasapiError::Windows(e))
                        if e.code().0 as u32 == AUDCLNT_E_UNSUPPORTED_FORMAT =>
                    {
                        Ok(false)
                    }
                    Err(wasapi::WasapiError::UnsupportedFormat) => Ok(false),
                    Err(e) => Err(backend_error("exclusive format query", e)),
                };
                if query(requested)? {
                    return Ok(requested.clone());
                }
                if requested.get_nchannels() <= 2 {
                    let plain = requested
                        .to_waveformatex()
                        .map_err(|e| backend_error("exclusive format conversion", e))?;
                    if query(&plain)? {
                        return Ok(plain);
                    }
                }
                for mask in wasapi::make_channelmasks(requested.get_nchannels() as usize) {
                    if mask == requested.get_dwchannelmask() {
                        continue;
                    }
                    let candidate = WaveFormat::new(
                        32,
                        32,
                        &SampleType::Float,
                        requested.get_samplespersec() as usize,
                        requested.get_nchannels() as usize,
                        Some(mask),
                    );
                    if query(&candidate)? {
                        return Ok(candidate);
                    }
                }
                Err(unsupported(
                    "exclusive 32-bit float format rejected",
                    wasapi::WasapiError::UnsupportedFormat,
                ))
            }
            // Initialize is authoritative for AUTOCONVERTPCM. An ignored
            // IsFormatSupported query adds a COM round trip to every open.
            ShareMode::SharedAutoConvert => Ok(requested.clone()),
        }
    }

    fn initialize_audio_client(
        device: &Device,
        direction: Direction,
        spec: StreamSpec,
        mode: ShareMode,
        trace: Option<&TraceHandle>,
    ) -> Result<(AudioClient, WaveFormat), AudioError> {
        validate_spec(spec)?;
        let mut client = trace_stage(trace, "get_iaudioclient", || {
            device
                .get_iaudioclient()
                .map_err(|err| backend_error("failed to create WASAPI audio client", err))
        })?;
        if mode == ShareMode::SharedAutoConvert {
            trace_stage(trace, "mixformat_validation", || {
                let mix = client
                    .get_mixformat()
                    .map_err(|err| backend_error("failed to read endpoint mix format", err))?;
                validate_shared_channels(spec.channels, mix.get_nchannels())
            })?;
        }
        let requested = requested_format(spec);
        let accepted = accepted_format(&client, &requested, mode)?;
        if accepted.get_samplespersec() != spec.sample_rate
            || accepted.get_nchannels() != spec.channels
            || accepted.get_subformat().ok() != Some(SampleType::Float)
        {
            return Err(AudioError::UnsupportedFormat(format!(
                "device did not accept requested {} Hz, {} channel, float32 transport verbatim",
                spec.sample_rate, spec.channels
            )));
        }
        let stream_mode = stream_mode(&client, &accepted, mode, trace)?;
        trace_stage(trace, "initialize_client", || {
            client
                .initialize_client(&accepted, &wasapi_direction(direction), &stream_mode)
                .map_err(|err| match err {
                    wasapi::WasapiError::Windows(ref e)
                        if e.code().0 as u32 == AUDCLNT_E_UNSUPPORTED_FORMAT =>
                    {
                        unsupported("WASAPI stream initialization rejected the format", err)
                    }
                    wasapi::WasapiError::UnsupportedFormat => {
                        unsupported("WASAPI stream initialization rejected the format", err)
                    }
                    _ => backend_error("WASAPI stream initialization", err),
                })
        })?;
        if let Some(trace) = trace {
            let frames = client
                .get_buffer_size()
                .map_err(|err| backend_error("negotiated buffer size", err))?;
            trace.record(TraceRecord {
                category: "open",
                event: "negotiated_buffer",
                buffer_size: Some(frames as usize),
                detail: format!(
                    "direction={direction:?} rate={} channels={} format=float32 mode={mode:?}",
                    spec.sample_rate, spec.channels
                ),
                ..TraceRecord::default()
            });
        }
        Ok((client, accepted))
    }

    fn enumerate_direction(
        enumerator: &DeviceEnumerator,
        direction: Direction,
    ) -> Result<Vec<Endpoint>, AudioError> {
        let wasapi_dir = wasapi_direction(direction);
        let default_id = enumerator
            .get_default_device(&wasapi_dir)
            .and_then(|device| device.get_id())
            .ok();
        let collection = enumerator
            .get_device_collection(&wasapi_dir)
            .map_err(|err| backend_error("failed to enumerate active WASAPI endpoints", err))?;
        let count = collection
            .get_nbr_devices()
            .map_err(|err| backend_error("failed to count WASAPI endpoints", err))?;
        let mut endpoints = Vec::with_capacity(count as usize);
        for index in 0..count {
            let device = collection
                .get_device_at_index(index)
                .map_err(|err| backend_error("failed to access WASAPI endpoint", err))?;
            let id = device
                .get_id()
                .map_err(|err| backend_error("failed to read WASAPI endpoint id", err))?;
            let name = device
                .get_friendlyname()
                .map_err(|err| backend_error("failed to read WASAPI endpoint name", err))?;
            let mix = mix_format(&device)?;
            endpoints.push(endpoint_from_format(
                id.clone(),
                name,
                direction,
                mix.get_samplespersec(),
                mix.get_nchannels(),
                default_id.as_deref() == Some(id.as_str()),
            ));
        }
        Ok(endpoints)
    }

    impl AudioBackend for WasapiBackend {
        fn name(&self) -> &'static str {
            "wasapi"
        }

        fn enumerate(&self) -> Result<Vec<Endpoint>, AudioError> {
            let _com = ComGuard::initialize(None)?;
            let enumerator = DeviceEnumerator::new()
                .map_err(|err| backend_error("failed to create WASAPI device enumerator", err))?;
            let mut endpoints = enumerate_direction(&enumerator, Direction::Output)?;
            endpoints.extend(enumerate_direction(&enumerator, Direction::Input)?);
            Ok(endpoints)
        }

        fn probe(
            &self,
            endpoint: &Endpoint,
            direction: Direction,
            spec: StreamSpec,
            mode: ShareMode,
        ) -> Result<ProbeResult, AudioError> {
            validate_spec(spec)?;
            let _com = ComGuard::initialize(None)?;
            let device = get_device(endpoint, direction, None)?;
            let mix = mix_format(&device)?;
            let native_rate = mix.get_samplespersec();
            let native_channels = mix.get_nchannels();
            if mode == ShareMode::SharedAutoConvert
                && let Err(err) = validate_shared_channels(spec.channels, native_channels)
            {
                return Ok(probe_result_from_format(
                    mode,
                    native_rate,
                    native_channels,
                    false,
                    err.to_string(),
                ));
            }
            let client = device
                .get_iaudioclient()
                .map_err(|err| backend_error("failed to create WASAPI audio client", err))?;
            let format = requested_format(spec);
            let (supported, detail) = match mode {
                ShareMode::Exclusive => match client.is_supported_exclusive_with_quirks(&format) {
                    Ok(_) => (
                        true,
                        "exclusive float32 format accepted verbatim (possibly with a driver-compatible channel mask)"
                            .to_string(),
                    ),
                    Err(err) => (false, format!("exclusive float32 format rejected: {err}")),
                },
                ShareMode::SharedAutoConvert => {
                    match client.is_supported(&format, &WasapiShareMode::Shared) {
                        Ok(None) => (
                            true,
                            "shared float32 format is native; auto-convert enabled".into(),
                        ),
                        Ok(Some(nearest)) => (
                            true,
                            format!(
                                "shared auto-convert enabled; engine mix format is {} Hz/{} channels (IsFormatSupported nearest: {} Hz/{} channels)",
                                native_rate,
                                native_channels,
                                nearest.get_samplespersec(),
                                nearest.get_nchannels()
                            ),
                        ),
                        Err(query_err) => {
                            // IsFormatSupported does not model AUTOCONVERTPCM, so a
                            // rejected query is not conclusive. A short authoritative
                            // initialization decides; the trial client is dropped
                            // immediately and no stream is started.
                            let mut trial = device.get_iaudioclient().map_err(|err| {
                                backend_error("failed to create WASAPI audio client", err)
                            })?;
                            let stream_mode = StreamMode::PollingShared {
                                autoconvert: true,
                                buffer_duration_hns: shared_buffer_duration(&trial, None)?,
                            };
                            match trial.initialize_client(
                                &format,
                                &wasapi_direction(direction),
                                &stream_mode,
                            ) {
                                Ok(()) => (
                                    true,
                                    format!(
                                        "shared auto-convert accepted by initialization (IsFormatSupported said: {query_err})"
                                    ),
                                ),
                                Err(init_err) => (
                                    false,
                                    format!(
                                        "shared auto-convert rejected: IsFormatSupported {query_err}; Initialize {init_err}"
                                    ),
                                ),
                            }
                        }
                    }
                }
            };
            Ok(probe_result_from_format(
                mode,
                native_rate,
                native_channels,
                supported,
                detail,
            ))
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
            mode: ShareMode,
        ) -> Result<Box<dyn InputSession>, AudioError> {
            Ok(Box::new(self.open_input_session(endpoint, spec, mode)?))
        }
    }

    impl WasapiBackend {
        pub fn open_output_session(
            &self,
            endpoint: &Endpoint,
            spec: StreamSpec,
            mode: ShareMode,
        ) -> Result<WasapiOutputSession, AudioError> {
            let trace_pair = TraceHandle::from_env("render")?;
            let (trace, trace_owner) = match trace_pair {
                Some((trace, owner)) => (Some(trace), owner),
                None => (None, TraceOwner(None)),
            };
            let com = ComGuard::initialize(trace.as_ref())?;
            let enumerator = EnumeratorLease::acquire(trace.as_ref())?;
            let device = enumerator.get_device(endpoint, Direction::Output, trace.as_ref())?;
            let (client, _) =
                initialize_audio_client(&device, Direction::Output, spec, mode, trace.as_ref())?;
            trace_stage(trace.as_ref(), "device_release", || {
                drop(device);
                Ok(())
            })?;
            let render = trace_stage(trace.as_ref(), "service_acquisition", || {
                client
                    .get_audiorenderclient()
                    .map_err(|err| backend_error("failed to get WASAPI render client", err))
            })?;
            let buffer_frames = trace_stage(trace.as_ref(), "get_buffer_size", || {
                client
                    .get_buffer_size()
                    .map(|frames| frames as usize)
                    .map_err(|err| backend_error("render buffer size", err))
            })?;
            let event = if mode == ShareMode::SharedAutoConvert {
                Some(trace_stage(trace.as_ref(), "event_creation", || {
                    client
                        .set_get_eventhandle()
                        .map_err(|e| backend_error("render event", e))
                })?)
            } else {
                None
            };
            let scratch = trace_stage(trace.as_ref(), "allocation", || {
                Ok(Vec::with_capacity(
                    buffer_frames * usize::from(spec.channels) * 4,
                ))
            })?;
            Ok(WasapiOutputSession {
                event,
                scratch,
                render: TracedDrop::new(render, "render_service_release", trace.as_ref()),
                client: TracedDrop::new(client, "audio_client_release", trace.as_ref()),
                buffer_frames,
                spec,
                mode,
                trace,
                _enumerator: enumerator,
                _com: com,
                _trace_owner: trace_owner,
            })
        }

        pub fn open_input_session(
            &self,
            endpoint: &Endpoint,
            spec: StreamSpec,
            mode: ShareMode,
        ) -> Result<WasapiInputSession, AudioError> {
            let trace_pair = TraceHandle::from_env("capture")?;
            let (trace, trace_owner) = match trace_pair {
                Some((trace, owner)) => (Some(trace), owner),
                None => (None, TraceOwner(None)),
            };
            let com = ComGuard::initialize(trace.as_ref())?;
            let enumerator = EnumeratorLease::acquire(trace.as_ref())?;
            let device = enumerator.get_device(endpoint, Direction::Input, trace.as_ref())?;
            let (client, _) =
                initialize_audio_client(&device, Direction::Input, spec, mode, trace.as_ref())?;
            trace_stage(trace.as_ref(), "device_release", || {
                drop(device);
                Ok(())
            })?;
            let capture = trace_stage(trace.as_ref(), "service_acquisition", || {
                client
                    .get_audiocaptureclient()
                    .map_err(|err| backend_error("failed to get WASAPI capture client", err))
            })?;
            let buffer_frames = trace_stage(trace.as_ref(), "get_buffer_size", || {
                client
                    .get_buffer_size()
                    .map(|frames| frames as usize)
                    .map_err(|err| backend_error("failed to get WASAPI capture buffer size", err))
            })?;
            let scratch_len = buffer_frames
                .checked_mul(usize::from(spec.channels))
                .and_then(|samples| samples.checked_mul(size_of::<f32>()))
                .ok_or_else(|| AudioError::Backend("capture buffer size overflow".into()))?;
            let event = if mode == ShareMode::SharedAutoConvert {
                Some(trace_stage(trace.as_ref(), "event_creation", || {
                    client
                        .set_get_eventhandle()
                        .map_err(|e| backend_error("capture event", e))
                })?)
            } else {
                None
            };
            let (scratch, queue) = trace_stage(trace.as_ref(), "allocation", || {
                Ok((
                    vec![0; scratch_len],
                    CaptureQueue {
                        pending: VecDeque::with_capacity(
                            buffer_frames * usize::from(spec.channels),
                        ),
                        flags: PacketFlags::default(),
                    },
                ))
            })?;
            Ok(WasapiInputSession {
                event,
                capture: TracedDrop::new(capture, "capture_service_release", trace.as_ref()),
                client: TracedDrop::new(client, "audio_client_release", trace.as_ref()),
                spec,
                scratch,
                queue,
                started: false,
                stats: CaptureStats::default(),
                packet_ordinal: 0,
                captured_frames: 0,
                delivered_frames: 0,
                saw_discontinuity: false,
                trace,
                _enumerator: enumerator,
                _com: com,
                _trace_owner: trace_owner,
            })
        }
    }

    pub struct WasapiOutputSession {
        event: Option<wasapi::Handle>,
        scratch: Vec<u8>,
        render: TracedDrop<AudioRenderClient>,
        client: TracedDrop<AudioClient>,
        buffer_frames: usize,
        spec: StreamSpec,
        mode: ShareMode,
        trace: Option<TraceHandle>,
        _enumerator: EnumeratorLease,
        _com: ComGuard,
        _trace_owner: TraceOwner,
    }

    struct WasapiSink<'a> {
        event: Option<&'a wasapi::Handle>,
        scratch: &'a mut Vec<u8>,
        render: &'a AudioRenderClient,
        client: &'a AudioClient,
        buffer_frames: usize,
        channels: usize,
        submitted_frames: u64,
        write_ordinal: u64,
        trace: Option<&'a TraceHandle>,
    }

    impl SampleSink for WasapiSink<'_> {
        fn wait(&self) -> Result<(), AudioError> {
            let started = Instant::now();
            let (outcome, result) = if let Some(event) = self.event {
                match event.wait_for_event(10) {
                    Ok(()) => ("signaled", Ok(())),
                    Err(wasapi::WasapiError::EventTimeout) => ("timeout", Ok(())),
                    Err(error) => ("error", Err(backend_error("WASAPI event wait", error))),
                }
            } else {
                std::thread::sleep(std::time::Duration::from_millis(1));
                ("poll_sleep", Ok(()))
            };
            if let Some(trace) = self.trace {
                trace.record(TraceRecord {
                    category: "render",
                    event: "wait",
                    duration_us: started.elapsed().as_micros(),
                    detail: outcome.into(),
                    ..TraceRecord::default()
                });
            }
            result
        }
        fn capacity_frames(&self) -> Result<usize, AudioError> {
            let (free, _) = self.buffer_snapshot()?;
            Ok(free)
        }

        fn padding_frames(&self) -> Result<usize, AudioError> {
            self.client
                .get_current_padding()
                .map(|frames| frames as usize)
                .map_err(|err| backend_error("failed to query WASAPI render padding", err))
        }

        fn buffer_snapshot(&self) -> Result<(usize, usize), AudioError> {
            let padding = self.padding_frames()?;
            let free = self.buffer_frames.checked_sub(padding).ok_or_else(|| {
                AudioError::Backend(format!(
                    "render padding {padding} exceeds buffer {}",
                    self.buffer_frames
                ))
            })?;
            Ok((free, padding))
        }

        fn write_frames(
            &mut self,
            samples: &[f32],
            (free, padding): (usize, usize),
        ) -> Result<(), AudioError> {
            let frames = samples.len() / self.channels;
            let verified_free = self.buffer_frames.checked_sub(padding).ok_or_else(|| {
                AudioError::Backend(format!(
                    "render padding {padding} exceeds buffer {}",
                    self.buffer_frames
                ))
            })?;
            if free != verified_free {
                return Err(AudioError::Backend(
                    "inconsistent render buffer snapshot".into(),
                ));
            }
            if frames > free {
                return Err(AudioError::Backend(format!(
                    "render write {frames} exceeds free space {free}"
                )));
            }
            self.scratch.clear();
            self.scratch
                .extend(samples.iter().flat_map(|s| s.to_le_bytes()));
            // Fingerprint the exact scratch slice, not the source or a reconstructed
            // payload. The safe wrapper copies and releases all requested frames on
            // Ok; on Err it does not expose which COM call failed or an actual count.
            let checksum = self.trace.map(|_| {
                self.scratch.iter().fold(14695981039346656037_u64, |h, b| {
                    (h ^ u64::from(*b)).wrapping_mul(1099511628211)
                })
            });
            let epoch = self.trace.map(|trace| trace.0.borrow().started);
            let before_us = epoch.map(|start| start.elapsed().as_micros());
            let result = self.render.write_to_device(frames, self.scratch, None);
            let after_us = epoch.map(|start| start.elapsed().as_micros());
            let after_cursor =
                self.submitted_frames + if result.is_ok() { frames as u64 } else { 0 };
            if let Some(trace) = self.trace {
                trace.record(TraceRecord {
                    category: "render",
                    event: "write",
                    ordinal: Some(self.write_ordinal),
                    frames: Some(frames),
                    frame_start: Some(self.submitted_frames),
                    frame_end: Some(after_cursor),
                    buffer_size: Some(self.buffer_frames),
                    padding: Some(padding),
                    free: Some(free),
                    duration_us: after_us.unwrap() - before_us.unwrap(),
                    detail: format!(
                        "before_us={} after_us={} requested={} written={} cursor_before={} cursor_after={} payload_fnv1a={:016x} bytes={} outcome={}",
                        before_us.unwrap(), after_us.unwrap(), frames,
                        if result.is_ok() { frames.to_string() } else { "unknown".into() },
                        self.submitted_frames, after_cursor, checksum.unwrap(), self.scratch.len(),
                        match &result {
                            Ok(()) => "release_ok_not_hardware_consumption".into(),
                            Err(error) => format!("error:{error}"),
                        }
                    ),
                    ..TraceRecord::default()
                });
            }
            result.map_err(|err| backend_error("failed to write WASAPI render buffer", err))?;
            self.submitted_frames = after_cursor;
            self.write_ordinal += 1;
            Ok(())
        }

        fn start(&mut self) -> Result<(), AudioError> {
            let result = self
                .client
                .start_stream()
                .map_err(|err| backend_error("failed to start WASAPI render stream", err));
            if let Some(trace) = self.trace {
                trace.record(TraceRecord {
                    category: "render",
                    event: "start",
                    buffer_size: Some(self.buffer_frames),
                    detail: if result.is_ok() { "ok" } else { "error" }.into(),
                    ..TraceRecord::default()
                });
            }
            result
        }

        fn stop(&mut self) -> Result<(), AudioError> {
            self.client
                .stop_stream()
                .map_err(|err| backend_error("failed to stop WASAPI render stream", err))
        }
    }

    impl OutputSession for WasapiOutputSession {
        fn play_to_completion(
            &mut self,
            interleaved: &[f32],
            cancel: &CancelToken,
        ) -> Result<PlaybackReport, AudioError> {
            let mut sink = WasapiSink {
                event: self.event.as_ref(),
                scratch: &mut self.scratch,
                render: &self.render,
                client: &self.client,
                buffer_frames: self.buffer_frames,
                channels: usize::from(self.spec.channels),
                submitted_frames: 0,
                write_ordinal: 0,
                trace: self.trace.as_ref(),
            };
            play_with_sink(
                &mut sink,
                interleaved,
                self.spec.channels,
                self.mode,
                cancel,
            )
        }
    }

    struct WasapiPacketSource<'a> {
        capture: &'a AudioCaptureClient,
    }

    impl PacketSource for WasapiPacketSource<'_> {
        fn read_packet(&mut self, scratch: &mut [u8]) -> Result<PacketHeader, AudioError> {
            let (frames, info) = self
                .capture
                .read_from_device(scratch)
                .map_err(|err| backend_error("failed to read WASAPI capture packet", err))?;
            Ok(PacketHeader {
                frames: frames as usize,
                flags: PacketFlags {
                    silent: info.flags.silent,
                    discontinuity: info.flags.data_discontinuity,
                },
                index: info.index,
                timestamp: info.timestamp,
            })
        }
    }

    pub struct WasapiInputSession {
        event: Option<wasapi::Handle>,
        capture: TracedDrop<AudioCaptureClient>,
        client: TracedDrop<AudioClient>,
        spec: StreamSpec,
        scratch: Vec<u8>,
        queue: CaptureQueue,
        started: bool,
        stats: CaptureStats,
        packet_ordinal: u64,
        captured_frames: u64,
        delivered_frames: u64,
        saw_discontinuity: bool,
        trace: Option<TraceHandle>,
        _enumerator: EnumeratorLease,
        _com: ComGuard,
        _trace_owner: TraceOwner,
    }

    impl WasapiInputSession {
        pub fn capture_stats(&self) -> CaptureStats {
            self.stats
        }

        fn packet_ready(&self) -> Result<bool, AudioError> {
            match self
                .capture
                .get_next_packet_size()
                .map_err(|err| backend_error("failed to query WASAPI capture packet", err))?
            {
                Some(frames) => Ok(frames > 0),
                None => self
                    .client
                    .get_current_padding()
                    .map(|frames| frames > 0)
                    .map_err(|err| backend_error("failed to query WASAPI capture padding", err)),
            }
        }
    }

    impl InputSession for WasapiInputSession {
        fn start(&mut self) -> Result<(), AudioError> {
            if !self.started {
                self.client
                    .start_stream()
                    .map_err(|err| backend_error("failed to start WASAPI capture stream", err))?;
                self.started = true;
            }
            Ok(())
        }

        fn read_into(
            &mut self,
            dst: &mut [f32],
            cancel: &CancelToken,
        ) -> Result<CaptureRead, AudioError> {
            validate_frame_aligned(dst.len(), self.spec.channels)?;
            if !self.started {
                return Err(AudioError::Backend(
                    "capture stream must be started before reading".into(),
                ));
            }
            let mut written = 0usize;
            let mut any_silent = false;
            let mut any_discontinuity = false;
            while !dst.is_empty() {
                let delivery_start = self.delivered_frames;
                let queue_before = self.queue.pending.len();
                let before_written = written;
                let drained = self.queue.drain_into(dst, &mut written);
                let delivered = (written - before_written) / usize::from(self.spec.channels);
                self.delivered_frames += delivered as u64;
                if delivered > 0
                    && let Some(trace) = &self.trace
                {
                    trace.record(TraceRecord {
                        category: "capture",
                        event: "delivery",
                        frames: Some(delivered),
                        queue_before: Some(queue_before),
                        queue_after: Some(self.queue.pending.len()),
                        delivery_start: Some(delivery_start),
                        delivery_end: Some(self.delivered_frames),
                        silent: Some(drained.silent),
                        discontinuity: Some(drained.discontinuity),
                        detail: "queue_to_caller".into(),
                        ..TraceRecord::default()
                    });
                }
                any_silent |= drained.silent;
                any_discontinuity |= drained.discontinuity;
                if cancel.is_cancelled() {
                    break;
                }
                if !self.packet_ready()? {
                    // A wake may represent several packets. Return a full dst
                    // only after every available packet has been released.
                    if written == dst.len() {
                        break;
                    }
                    if let Some(event) = &self.event {
                        wait_event(event)?;
                    } else {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    continue;
                }
                // Bound backlog to two endpoint buffers. A caller that cannot
                // keep up gets an explicit failure, never silently dropped data.
                if self.queue.pending.len() > self.scratch.len() / size_of::<f32>() {
                    return Err(AudioError::Backend(
                        "capture backlog exceeded endpoint buffer".into(),
                    ));
                }
                let mut source = WasapiPacketSource {
                    capture: &self.capture,
                };
                let queue_before = self.queue.pending.len();
                let packet = self.queue.push_packet(
                    &mut source,
                    &mut self.scratch,
                    self.spec.channels,
                    self.trace.is_some(),
                )?;
                let initial_discontinuity = packet.header.flags.discontinuity
                    && !self.saw_discontinuity
                    && self.packet_ordinal == 0;
                self.saw_discontinuity |= packet.header.flags.discontinuity;
                let frame_start = self.captured_frames;
                self.captured_frames += packet.header.frames as u64;
                if let Some(trace) = &self.trace {
                    trace.record(TraceRecord {
                        category: "capture",
                        event: "packet",
                        ordinal: Some(self.packet_ordinal),
                        frames: Some(packet.header.frames),
                        frame_start: Some(frame_start),
                        frame_end: Some(self.captured_frames),
                        buffer_index: Some(packet.header.index),
                        buffer_timestamp: Some(packet.header.timestamp),
                        silent: Some(packet.header.flags.silent),
                        discontinuity: Some(packet.header.flags.discontinuity),
                        initial_discontinuity: Some(initial_discontinuity),
                        raw_zero_runs: Some(packet.raw_zero_runs),
                        raw_zero_frames: Some(packet.raw_zero_frames),
                        silent_fill_frames: Some(packet.silent_fill_frames),
                        queue_before: Some(queue_before),
                        queue_after: Some(self.queue.pending.len()),
                        detail: "device_to_queue".into(),
                        ..TraceRecord::default()
                    });
                }
                self.packet_ordinal += 1;
                if packet.header.flags.silent {
                    self.stats.silent_packets = self.stats.silent_packets.saturating_add(1);
                }
                if packet.header.flags.discontinuity {
                    self.stats.discontinuity_packets =
                        self.stats.discontinuity_packets.saturating_add(1);
                }
            }
            Ok(CaptureRead {
                frames: written / usize::from(self.spec.channels),
                discontinuity: any_discontinuity,
                silent: any_silent,
            })
        }

        fn stop(&mut self) -> Result<(), AudioError> {
            if self.started {
                self.client
                    .stop_stream()
                    .map_err(|err| backend_error("failed to stop WASAPI capture stream", err))?;
                self.started = false;
            }
            Ok(())
        }
    }
}

#[cfg(windows)]
pub use windows_backend::{WasapiInputSession, WasapiOutputSession};

#[cfg(not(windows))]
impl AudioBackend for WasapiBackend {
    fn name(&self) -> &'static str {
        "wasapi"
    }

    fn enumerate(&self) -> Result<Vec<Endpoint>, AudioError> {
        Err(AudioError::Backend(
            "WASAPI is available only on Windows".into(),
        ))
    }

    fn probe(
        &self,
        _endpoint: &Endpoint,
        _direction: Direction,
        _spec: StreamSpec,
        _mode: ShareMode,
    ) -> Result<ProbeResult, AudioError> {
        Err(AudioError::Backend(
            "WASAPI is available only on Windows".into(),
        ))
    }

    fn open_output(
        &self,
        _endpoint: &Endpoint,
        _spec: StreamSpec,
        _mode: ShareMode,
    ) -> Result<Box<dyn OutputSession>, AudioError> {
        Err(AudioError::Backend(
            "WASAPI is available only on Windows".into(),
        ))
    }

    fn open_input(
        &self,
        _endpoint: &Endpoint,
        _spec: StreamSpec,
        _mode: ShareMode,
    ) -> Result<Box<dyn InputSession>, AudioError> {
        Err(AudioError::Backend(
            "WASAPI is available only on Windows".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_from_mix_format_fields() {
        let endpoint = endpoint_from_format(
            "id".into(),
            "name".into(),
            Direction::Output,
            96_000,
            16,
            true,
        );
        assert_eq!(endpoint.default_samplerate, 96_000.0);
        assert_eq!(endpoint.max_output_channels, 16);
        assert_eq!(endpoint.max_input_channels, 0);
        assert!(endpoint.is_default_output);

        let probe =
            probe_result_from_format(ShareMode::SharedAutoConvert, 96_000, 16, true, "ok".into());
        assert_eq!(probe.native_sample_rate, 96_000);
        assert_eq!(probe.native_channels, 16);
    }

    #[test]
    fn bytes_to_f32_round_trip() {
        let expected = [0.0, -0.0, 0.25, -1.0, f32::MIN_POSITIVE, f32::INFINITY];
        let bytes = f32_to_bytes(&expected);
        let actual = bytes_to_f32(&bytes).unwrap();
        assert_eq!(
            actual
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        );
        assert!(bytes_to_f32(&bytes[..bytes.len() - 1]).is_err());
    }

    struct FakePacket {
        bytes: Vec<u8>,
        flags: PacketFlags,
    }

    impl PacketSource for FakePacket {
        fn read_packet(&mut self, scratch: &mut [u8]) -> Result<PacketHeader, AudioError> {
            scratch[..self.bytes.len()].copy_from_slice(&self.bytes);
            Ok(PacketHeader {
                frames: self.bytes.len() / size_of::<f32>(),
                flags: self.flags,
                index: 0,
                timestamp: 0,
            })
        }
    }

    #[test]
    fn queue_slice_drain_preserves_wrapped_transport_bits_and_capacity() {
        let values = [
            0.0,
            -0.0,
            f32::MIN_POSITIVE,
            -1.0,
            f32::from_bits(0x7fc01234),
            f32::INFINITY,
        ];
        let mut queue = CaptureQueue {
            pending: VecDeque::with_capacity(8),
            flags: PacketFlags {
                discontinuity: true,
                silent: false,
            },
        };
        queue.pending.extend([1.0; 6]);
        queue.pending.drain(..6);
        queue.pending.extend(values);
        let capacity = queue.pending.capacity();
        let mut output = [0.0; 6];
        let mut written = 0;
        assert!(
            queue
                .drain_into(&mut output[..2], &mut written)
                .discontinuity
        );
        let mut written = 2;
        assert!(queue.drain_into(&mut output, &mut written).discontinuity);
        assert_eq!(written, 6);
        assert_eq!(output.map(f32::to_bits), values.map(f32::to_bits));
        assert_eq!(queue.pending.capacity(), capacity);
        assert!(!queue.flags.discontinuity);
        let mut source = FakePacket {
            bytes: f32_to_bytes(&values),
            flags: PacketFlags::default(),
        };
        let mut scratch = [0; 24];
        queue
            .push_packet(&mut source, &mut scratch, 1, true)
            .unwrap();
        assert_eq!(queue.pending.capacity(), capacity);
        assert_eq!(
            queue
                .pending
                .iter()
                .map(|x| x.to_bits())
                .collect::<Vec<_>>(),
            values.map(f32::to_bits)
        );
    }

    #[test]
    fn full_destination_does_not_consume_queued_flags() {
        let mut queue = CaptureQueue {
            pending: VecDeque::from([0.25, -0.5]),
            flags: PacketFlags {
                silent: false,
                discontinuity: true,
            },
        };
        let mut dst = [0.0; 2];
        let mut written = 2;
        assert!(!queue.drain_into(&mut dst, &mut written).discontinuity);
        assert_eq!(queue.pending.len(), 2);
        written = 0;
        assert!(queue.drain_into(&mut dst, &mut written).discontinuity);
        assert_eq!(dst, [0.25, -0.5]);
    }

    #[test]
    fn render_waits_after_priming_before_refill() {
        struct Sink {
            writes: usize,
            waited: std::cell::Cell<bool>,
        }
        impl SampleSink for Sink {
            fn wait(&self) -> Result<(), AudioError> {
                self.waited.set(true);
                Ok(())
            }
            fn capacity_frames(&self) -> Result<usize, AudioError> {
                assert!(self.writes == 0 || self.waited.get());
                Ok(2)
            }
            fn padding_frames(&self) -> Result<usize, AudioError> {
                Ok(0)
            }
            fn write_frames(&mut self, _: &[f32], _: (usize, usize)) -> Result<(), AudioError> {
                self.writes += 1;
                self.waited.set(false);
                Ok(())
            }
            fn start(&mut self) -> Result<(), AudioError> {
                Ok(())
            }
            fn stop(&mut self) -> Result<(), AudioError> {
                Ok(())
            }
        }
        let mut sink = Sink {
            writes: 0,
            waited: std::cell::Cell::new(false),
        };
        let report = play_with_sink(
            &mut sink,
            &[0.25; 8],
            2,
            ShareMode::SharedAutoConvert,
            &CancelToken::new(),
        )
        .unwrap();
        assert_eq!(report.frames_drained, 4);
        assert_eq!(sink.writes, 2);
    }

    #[test]
    fn silent_packet_is_zero_filled() {
        let mut source = FakePacket {
            bytes: f32_to_bytes(&[0.75, -0.5, 1.0, 0.25]),
            flags: PacketFlags {
                silent: true,
                discontinuity: false,
            },
        };
        let mut scratch = vec![0; source.bytes.len()];
        let mut pending = VecDeque::new();
        let report = append_packet(&mut source, &mut scratch, &mut pending, 1, true).unwrap();
        assert!(report.header.flags.silent);
        assert_eq!(pending, VecDeque::from(vec![0.0; 4]));
    }

    #[test]
    fn discontinuity_flag_is_reported() {
        let mut source = FakePacket {
            bytes: f32_to_bytes(&[0.0, 0.0]),
            flags: PacketFlags {
                silent: false,
                discontinuity: true,
            },
        };
        let mut scratch = vec![0; source.bytes.len()];
        let mut pending = VecDeque::new();
        let report = append_packet(&mut source, &mut scratch, &mut pending, 1, true).unwrap();
        assert!(report.header.flags.discontinuity);
    }

    #[test]
    fn pending_packet_keeps_silent_flag_across_reads() {
        let mut source = FakePacket {
            bytes: f32_to_bytes(&[0.1, 0.2, 0.3, 0.4]),
            flags: PacketFlags {
                silent: true,
                discontinuity: false,
            },
        };
        let mut scratch = vec![0; source.bytes.len()];
        let mut queue = CaptureQueue::default();
        queue
            .push_packet(&mut source, &mut scratch, 1, true)
            .unwrap();

        let mut dst = [1.0f32; 2];
        let mut written = 0;
        let first = queue.drain_into(&mut dst, &mut written);
        assert!(first.silent);
        assert_eq!(written, 2);
        assert_eq!(dst, [0.0, 0.0]);

        let mut written = 0;
        let second = queue.drain_into(&mut dst, &mut written);
        assert!(
            second.silent,
            "flags must survive across reads while samples remain queued"
        );
        assert_eq!(written, 2);

        let mut written = 0;
        let third = queue.drain_into(&mut dst, &mut written);
        assert!(!third.silent, "flags reset once the queue is empty");
        assert_eq!(written, 0);
    }

    #[test]
    fn shared_mode_rejects_more_channels_than_mix_format() {
        assert!(validate_shared_channels(9, 8).is_err());
        assert!(validate_shared_channels(8, 8).is_ok());
    }

    #[test]
    fn appended_packet_keeps_header_and_counts_raw_zeros() {
        let mut source = FakePacket {
            bytes: f32_to_bytes(&[0.0, 0.25, 0.0]),
            flags: PacketFlags {
                silent: false,
                discontinuity: true,
            },
        };
        let mut scratch = vec![0; source.bytes.len()];
        let mut pending = VecDeque::new();
        let packet = append_packet(&mut source, &mut scratch, &mut pending, 1, true).unwrap();
        assert_eq!(packet.header.frames, 3);
        assert!(packet.header.flags.discontinuity);
        assert_eq!(packet.raw_zero_runs, 2);
        assert_eq!(packet.raw_zero_frames, 2);
        assert_eq!(packet.silent_fill_frames, 0);
        assert_eq!(pending.len(), 3);
    }

    #[test]
    fn silent_diagnostic_distinguishes_fill_from_raw_zero() {
        let mut source = FakePacket {
            bytes: f32_to_bytes(&[0.5, -0.5]),
            flags: PacketFlags {
                silent: true,
                discontinuity: false,
            },
        };
        let mut scratch = vec![0; source.bytes.len()];
        let mut pending = VecDeque::new();
        let packet = append_packet(&mut source, &mut scratch, &mut pending, 1, true).unwrap();
        assert_eq!(packet.raw_zero_runs, 0);
        assert_eq!(packet.raw_zero_frames, 0);
        assert_eq!(packet.silent_fill_frames, 2);
        assert!(pending.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn dst_length_must_be_frame_aligned() {
        assert!(validate_frame_aligned(7, 2).is_err());
        assert!(validate_frame_aligned(8, 2).is_ok());
    }

    struct CancellingSink {
        cancel: CancelToken,
        capacity: usize,
        padding: usize,
        started: bool,
    }

    impl SampleSink for CancellingSink {
        fn capacity_frames(&self) -> Result<usize, AudioError> {
            Ok(self.capacity)
        }

        fn padding_frames(&self) -> Result<usize, AudioError> {
            Ok(self.padding)
        }

        fn write_frames(&mut self, samples: &[f32], _: (usize, usize)) -> Result<(), AudioError> {
            self.padding = samples.len() / 2;
            self.cancel.cancel();
            Ok(())
        }

        fn start(&mut self) -> Result<(), AudioError> {
            self.started = true;
            Ok(())
        }

        fn stop(&mut self) -> Result<(), AudioError> {
            self.started = false;
            Ok(())
        }
    }

    #[test]
    fn play_reports_cancelled_when_token_set() {
        let cancel = CancelToken::new();
        let mut sink = CancellingSink {
            cancel: cancel.clone(),
            capacity: 2,
            padding: 0,
            started: false,
        };
        let report =
            play_with_sink(&mut sink, &[0.0; 16], 2, ShareMode::Exclusive, &cancel).unwrap();
        assert!(report.cancelled);
        assert_eq!(report.frames_submitted, 2);
        assert!(!sink.started);
    }
}
