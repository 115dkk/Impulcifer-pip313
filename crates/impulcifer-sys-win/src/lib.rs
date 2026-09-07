#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

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
    AudioBackend, AudioError, CancelToken, CaptureRead, Direction, Endpoint, InputSession,
    OutputSession, PlaybackReport, ProbeResult, ShareMode, StreamSpec,
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

fn f32_to_bytes(samples: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(samples));
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

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

trait PacketSource {
    fn read_packet(&mut self, scratch: &mut [u8]) -> Result<(usize, PacketFlags), AudioError>;
}

fn append_packet<S: PacketSource>(
    source: &mut S,
    scratch: &mut [u8],
    pending: &mut VecDeque<f32>,
    channels: u16,
) -> Result<CaptureRead, AudioError> {
    let (frames, flags) = source.read_packet(scratch)?;
    let sample_count = frames
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
    if flags.silent {
        pending.extend(std::iter::repeat_n(0.0, sample_count));
    } else {
        pending.extend(bytes_to_f32(&scratch[..byte_count])?);
    }
    Ok(CaptureRead {
        frames,
        discontinuity: flags.discontinuity,
        silent: flags.silent,
    })
}

trait SampleSink {
    fn capacity_frames(&self) -> Result<usize, AudioError>;
    fn padding_frames(&self) -> Result<usize, AudioError>;
    fn write_frames(&mut self, samples: &[f32]) -> Result<(), AudioError>;
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

        let capacity = sink.capacity_frames()?;
        if started && capacity > 0 && sink.padding_frames()? == 0 {
            underruns = underruns.saturating_add(1);
        }
        if capacity == 0 {
            std::thread::sleep(std::time::Duration::from_millis(1));
            continue;
        }
        let frames = capacity.min(total_frames - submitted);
        let first = submitted * channels;
        let last = first + frames * channels;
        sink.write_frames(&interleaved[first..last])?;
        submitted += frames;
        if !started {
            sink.start()?;
            started = true;
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
        std::thread::sleep(std::time::Duration::from_millis(1));
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
    use std::marker::PhantomData;
    use std::rc::Rc;
    use std::time::Duration;

    use wasapi::{
        AudioCaptureClient, AudioClient, AudioRenderClient, Device, DeviceEnumerator,
        Direction as WasapiDirection, SampleType, ShareMode as WasapiShareMode, StreamMode,
        WaveFormat,
    };

    use super::*;

    const SHARED_BUFFER_HNS: i64 = 200_000;

    struct ComGuard {
        _not_send: PhantomData<Rc<()>>,
    }

    impl ComGuard {
        fn initialize() -> Result<Self, AudioError> {
            wasapi::initialize_mta().ok().map_err(|err| {
                AudioError::Backend(format!("failed to initialize COM MTA: {err}"))
            })?;
            Ok(Self {
                _not_send: PhantomData,
            })
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            wasapi::deinitialize();
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

    fn get_device(endpoint: &Endpoint, direction: Direction) -> Result<Device, AudioError> {
        let enumerator = DeviceEnumerator::new()
            .map_err(|err| backend_error("failed to create WASAPI device enumerator", err))?;
        let device = enumerator
            .get_device(&endpoint.id)
            .map_err(|_| AudioError::DeviceNotFound(endpoint.id.clone()))?;
        if device.get_direction() != wasapi_direction(direction) {
            return Err(AudioError::DeviceNotFound(format!(
                "{} is not a {:?} endpoint",
                endpoint.id, direction
            )));
        }
        Ok(device)
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
    ) -> Result<StreamMode, AudioError> {
        match mode {
            ShareMode::SharedAutoConvert => Ok(StreamMode::PollingShared {
                autoconvert: true,
                buffer_duration_hns: SHARED_BUFFER_HNS,
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
            ShareMode::Exclusive => client
                .is_supported_exclusive_with_quirks(requested)
                .map_err(|err| unsupported("exclusive 32-bit float format rejected", err)),
            ShareMode::SharedAutoConvert => {
                // IsFormatSupported does not account for AUTOCONVERTPCM rate conversion.
                // Calling it is still useful for diagnostics; Initialize is authoritative.
                let _ = client.is_supported(requested, &WasapiShareMode::Shared);
                Ok(requested.clone())
            }
        }
    }

    fn initialize_audio_client(
        device: &Device,
        direction: Direction,
        spec: StreamSpec,
        mode: ShareMode,
    ) -> Result<(AudioClient, WaveFormat), AudioError> {
        validate_spec(spec)?;
        let mix = mix_format(device)?;
        if mode == ShareMode::SharedAutoConvert {
            validate_shared_channels(spec.channels, mix.get_nchannels())?;
        }
        let requested = requested_format(spec);
        let mut client = device
            .get_iaudioclient()
            .map_err(|err| backend_error("failed to create WASAPI audio client", err))?;
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
        let stream_mode = stream_mode(&client, &accepted, mode)?;
        client
            .initialize_client(&accepted, &wasapi_direction(direction), &stream_mode)
            .map_err(|err| unsupported("WASAPI stream initialization rejected the format", err))?;
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
            let _com = ComGuard::initialize()?;
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
            let _com = ComGuard::initialize()?;
            let device = get_device(endpoint, direction)?;
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
            let query = match mode {
                ShareMode::Exclusive => client
                    .is_supported_exclusive_with_quirks(&format)
                    .map(|_| {
                        "exclusive float32 format accepted verbatim (possibly with a driver-compatible channel mask)"
                            .to_string()
                    }),
                ShareMode::SharedAutoConvert => {
                    match client.is_supported(&format, &WasapiShareMode::Shared) {
                        Ok(None) => Ok("shared float32 format is native; auto-convert enabled".into()),
                        Ok(Some(nearest)) => Ok(format!(
                            "shared auto-convert enabled; engine mix format is {} Hz/{} channels (IsFormatSupported nearest: {} Hz/{} channels)",
                            native_rate,
                            native_channels,
                            nearest.get_samplespersec(),
                            nearest.get_nchannels()
                        )),
                        Err(err) => Ok(format!(
                            "shared auto-convert enabled; IsFormatSupported returned {err}, but AUTOCONVERTPCM initialization may accept rate conversion"
                        )),
                    }
                }
            };
            Ok(match query {
                Ok(detail) => {
                    probe_result_from_format(mode, native_rate, native_channels, true, detail)
                }
                Err(err) => probe_result_from_format(
                    mode,
                    native_rate,
                    native_channels,
                    false,
                    format!("exclusive float32 format rejected: {err}"),
                ),
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
            let com = ComGuard::initialize()?;
            let device = get_device(endpoint, Direction::Output)?;
            let (client, _) = initialize_audio_client(&device, Direction::Output, spec, mode)?;
            let render = client
                .get_audiorenderclient()
                .map_err(|err| backend_error("failed to get WASAPI render client", err))?;
            Ok(WasapiOutputSession {
                render,
                client,
                spec,
                mode,
                _com: com,
            })
        }

        pub fn open_input_session(
            &self,
            endpoint: &Endpoint,
            spec: StreamSpec,
            mode: ShareMode,
        ) -> Result<WasapiInputSession, AudioError> {
            let com = ComGuard::initialize()?;
            let device = get_device(endpoint, Direction::Input)?;
            let (client, _) = initialize_audio_client(&device, Direction::Input, spec, mode)?;
            let capture = client
                .get_audiocaptureclient()
                .map_err(|err| backend_error("failed to get WASAPI capture client", err))?;
            let buffer_frames = client
                .get_buffer_size()
                .map_err(|err| backend_error("failed to get WASAPI capture buffer size", err))?
                as usize;
            let scratch_len = buffer_frames
                .checked_mul(usize::from(spec.channels))
                .and_then(|samples| samples.checked_mul(size_of::<f32>()))
                .ok_or_else(|| AudioError::Backend("capture buffer size overflow".into()))?;
            Ok(WasapiInputSession {
                capture,
                client,
                spec,
                scratch: vec![0; scratch_len],
                pending: VecDeque::new(),
                started: false,
                stats: CaptureStats::default(),
                _com: com,
            })
        }
    }

    pub struct WasapiOutputSession {
        render: AudioRenderClient,
        client: AudioClient,
        spec: StreamSpec,
        mode: ShareMode,
        _com: ComGuard,
    }

    struct WasapiSink<'a> {
        render: &'a AudioRenderClient,
        client: &'a AudioClient,
        channels: usize,
    }

    impl SampleSink for WasapiSink<'_> {
        fn capacity_frames(&self) -> Result<usize, AudioError> {
            self.client
                .get_available_space_in_frames()
                .map(|frames| frames as usize)
                .map_err(|err| backend_error("failed to query WASAPI render capacity", err))
        }

        fn padding_frames(&self) -> Result<usize, AudioError> {
            self.client
                .get_current_padding()
                .map(|frames| frames as usize)
                .map_err(|err| backend_error("failed to query WASAPI render padding", err))
        }

        fn write_frames(&mut self, samples: &[f32]) -> Result<(), AudioError> {
            let frames = samples.len() / self.channels;
            let bytes = f32_to_bytes(samples);
            self.render
                .write_to_device(frames, &bytes, None)
                .map_err(|err| backend_error("failed to write WASAPI render buffer", err))
        }

        fn start(&mut self) -> Result<(), AudioError> {
            self.client
                .start_stream()
                .map_err(|err| backend_error("failed to start WASAPI render stream", err))
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
                render: &self.render,
                client: &self.client,
                channels: usize::from(self.spec.channels),
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
        fn read_packet(&mut self, scratch: &mut [u8]) -> Result<(usize, PacketFlags), AudioError> {
            let (frames, info) = self
                .capture
                .read_from_device(scratch)
                .map_err(|err| backend_error("failed to read WASAPI capture packet", err))?;
            Ok((
                frames as usize,
                PacketFlags {
                    silent: info.flags.silent,
                    discontinuity: info.flags.data_discontinuity,
                },
            ))
        }
    }

    pub struct WasapiInputSession {
        capture: AudioCaptureClient,
        client: AudioClient,
        spec: StreamSpec,
        scratch: Vec<u8>,
        pending: VecDeque<f32>,
        started: bool,
        stats: CaptureStats,
        _com: ComGuard,
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
            while written < dst.len() {
                while written < dst.len() {
                    let Some(sample) = self.pending.pop_front() else {
                        break;
                    };
                    dst[written] = sample;
                    written += 1;
                }
                if written == dst.len() || cancel.is_cancelled() {
                    break;
                }
                if !self.packet_ready()? {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                let mut source = WasapiPacketSource {
                    capture: &self.capture,
                };
                let packet = append_packet(
                    &mut source,
                    &mut self.scratch,
                    &mut self.pending,
                    self.spec.channels,
                )?;
                if packet.silent {
                    self.stats.silent_packets = self.stats.silent_packets.saturating_add(1);
                    any_silent = true;
                }
                if packet.discontinuity {
                    self.stats.discontinuity_packets =
                        self.stats.discontinuity_packets.saturating_add(1);
                    any_discontinuity = true;
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
        fn read_packet(&mut self, scratch: &mut [u8]) -> Result<(usize, PacketFlags), AudioError> {
            scratch[..self.bytes.len()].copy_from_slice(&self.bytes);
            Ok((self.bytes.len() / size_of::<f32>(), self.flags))
        }
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
        let report = append_packet(&mut source, &mut scratch, &mut pending, 1).unwrap();
        assert!(report.silent);
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
        let report = append_packet(&mut source, &mut scratch, &mut pending, 1).unwrap();
        assert!(report.discontinuity);
    }

    #[test]
    fn shared_mode_rejects_more_channels_than_mix_format() {
        assert!(validate_shared_channels(9, 8).is_err());
        assert!(validate_shared_channels(8, 8).is_ok());
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

        fn write_frames(&mut self, samples: &[f32]) -> Result<(), AudioError> {
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
