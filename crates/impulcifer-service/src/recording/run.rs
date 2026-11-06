use super::{
    RecordingError,
    channel_order::ChannelOrder,
    checks::{RecordingCheck, check_recording},
    devices::{output_plan, resolve_devices},
    progress::{RecorderProgressEvent, SweepSegment, event_for_elapsed, infer_sweep_segments},
    request::{Mode, ValidatedRecording},
    sweep::{build_sweep_playback, write_sidecar},
};
use impulcifer_audio_io::session::{PlaybackBuffer, SessionEvent, SessionRequest, play_and_record};
use impulcifer_io::{Wav, ffmpeg, read_wav, write_wav};
use impulcifer_jobs::registry::{JobContext, JobFailure};
use impulcifer_types::{
    audio::{AudioBackend, AudioError, ShareMode},
    ipc::ErrorCode,
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, Instant},
};

fn emit(ctx: &JobContext, event: RecorderProgressEvent) {
    ctx.progress(
        event.progress,
        serde_json::to_value(event).expect("finite recorder event"),
    );
}
pub fn analyze_recording(path: &Path) -> Option<Value> {
    let wav = read_wav(path).ok()?;
    let frames = wav.tracks.first()?.len();
    if frames == 0 || wav.sample_rate == 0 {
        return None;
    }
    let peaks: Vec<_> = wav
        .tracks
        .iter()
        .map(|t| t.iter().map(|x| x.abs()).fold(0.0, f64::max))
        .collect();
    let peak = peaks.iter().copied().fold(0.0, f64::max);
    Some(
        json!({"sample_rate":wav.sample_rate,"channels":wav.tracks.len(),"duration":frames as f64/wav.sample_rate as f64,"peak_db":20.0*peak.max(1e-10).log10(),"active_channels":peaks.iter().filter(|&&p|p>1e-6).count()}),
    )
}
/// Python appends channels, padding whichever recording has fewer samples.
pub fn save_recording(
    path: &Path,
    fs: u32,
    mut tracks: Vec<Vec<f64>>,
    append: bool,
) -> Result<(), RecordingError> {
    if append && path.is_file() {
        let mut previous = read_wav(path).map_err(RecordingError::internal)?.tracks;
        let n = previous
            .first()
            .map_or(0, Vec::len)
            .max(tracks.first().map_or(0, Vec::len));
        for t in previous.iter_mut().chain(tracks.iter_mut()) {
            t.resize(n, 0.0);
        }
        previous.extend(tracks);
        tracks = previous;
    }
    write_wav(path, fs, &tracks, 32).map_err(|_| output_missing(path))?;
    if !path.is_file() {
        return Err(output_missing(path));
    }
    Ok(())
}
fn output_missing(path: &Path) -> RecordingError {
    RecordingError {
        code: ErrorCode::OutputMissing,
        message: "Recording finished without producing the expected file.".into(),
        details: json!({"record_path":path}),
        retryable: false,
    }
}
struct DecodeTemp(PathBuf);
impl DecodeTemp {
    fn new() -> Result<Self, RecordingError> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        loop {
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "impulcifer-record-decode-{}-{n}",
                std::process::id()
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(RecordingError::internal(e)),
            }
        }
    }
}
impl Drop for DecodeTemp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
/// The playback, and for a TrueHD file the `<record stem>_channels.txt` to write
/// once the recording is saved (a cancelled or failed recording leaves no file).
fn load_playback(
    path: &Path,
    record: &Path,
) -> Result<(Wav, Option<(PathBuf, String)>), RecordingError> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "mlp" | "thd" | "truehd") {
        return Ok((read_wav(path).map_err(RecordingError::internal)?, None));
    }
    let tools = ffmpeg::discover(false)
        .map_err(RecordingError::internal)?
        .ok_or_else(|| RecordingError::internal("FFmpeg is required for TrueHD/MLP input"))?;
    let temp = DecodeTemp::new()?;
    let decoded = temp.0.join("decoded.wav");
    ffmpeg::decode_to_wav(&tools, path, &decoded).map_err(RecordingError::internal)?;
    let wav = read_wav(&decoded).map_err(RecordingError::internal)?;
    let mut channel_names = None;
    if let Some(names) = ffmpeg::truehd_channel_names(wav.tracks.len()) {
        let info = record.with_file_name(format!(
            "{}_channels.txt",
            record.file_stem().unwrap_or_default().to_string_lossy()
        ));
        channel_names = Some((info, names.join(",")));
    } else if ffmpeg::is_truehd_atmos_object_master(&tools, path)
        .map_err(RecordingError::internal)?
    {
        return Err(RecordingError::internal(format!(
            "TrueHD/MLP file '{}' is a Dolby Atmos object master; FFmpeg decodes only its {}-channel bed and the height/wide objects cannot be played as discrete outputs through a normal audio interface. Please use a multi-channel sweep WAV generated by Impulcifer instead.",
            path.file_name().unwrap_or_default().to_string_lossy(),
            wav.tracks.len()
        )));
    }
    Ok((wav, channel_names))
}
enum Clock {
    Start,
    Position(f64),
    Stop,
}
fn monitor(rx: mpsc::Receiver<Clock>, ctx: &JobContext, duration: f64, segments: &[SweepSegment]) {
    let mut start = None;
    let mut position = None;
    let mut next = Instant::now();
    loop {
        let timeout = if start.is_some() {
            next.saturating_duration_since(Instant::now())
        } else {
            Duration::from_millis(250)
        };
        match rx.recv_timeout(timeout) {
            Ok(Clock::Start) => {
                start = Some(Instant::now());
                next = Instant::now() + Duration::from_millis(250);
                emit(ctx, event_for_elapsed(0.0, duration, segments));
            }
            Ok(Clock::Position(p)) => position = Some(p),
            Ok(Clock::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(start) = start {
                    emit(
                        ctx,
                        event_for_elapsed(
                            position.unwrap_or_else(|| start.elapsed().as_secs_f64()),
                            duration,
                            segments,
                        ),
                    );
                    next = Instant::now() + Duration::from_millis(250);
                }
            }
        }
    }
}
enum RunError {
    Recording(RecordingError),
    Cancelled,
}
impl From<RecordingError> for RunError {
    fn from(error: RecordingError) -> Self {
        Self::Recording(error)
    }
}
fn cancelled(ctx: &JobContext, duration: f64, speakers: Vec<String>) -> RunError {
    emit(
        ctx,
        RecorderProgressEvent {
            phase: "cancelled".into(),
            duration,
            speakers,
            ..Default::default()
        },
    );
    RunError::Cancelled
}
pub fn run_recording(
    validated: &ValidatedRecording,
    backend: &(dyn AudioBackend + Sync),
    ctx: &JobContext,
) -> Result<Value, JobFailure> {
    if ctx.cancel.is_cancelled() {
        return Err(match cancelled(ctx, 0.0, Vec::new()) {
            RunError::Cancelled => JobFailure {
                error: Value::Null,
                cancelled: true,
            },
            RunError::Recording(_) => unreachable!(),
        });
    }
    match run(validated, backend, ctx) {
        Ok(result) => Ok(result),
        Err(RunError::Recording(error)) => Err(RecordingError::failure(error)),
        Err(RunError::Cancelled) => Err(JobFailure {
            error: Value::Null,
            cancelled: true,
        }),
    }
}
fn run(
    v: &ValidatedRecording,
    backend: &(dyn AudioBackend + Sync),
    ctx: &JobContext,
) -> Result<Value, RunError> {
    let record = Path::new(&v.record_path);
    if let Some(dir) = record.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|_| output_missing(record))?;
    }
    let mut generated = v
        .sweep_spec
        .as_ref()
        .map(build_sweep_playback)
        .transpose()?;
    let label = generated
        .as_ref()
        .map(|p| p.display_name.clone())
        .unwrap_or_else(|| v.play_path.clone().unwrap_or_default());
    emit(
        ctx,
        RecorderProgressEvent {
            phase: "loading".into(),
            message: label,
            ..Default::default()
        },
    );
    let mut channel_names = None;
    let (fs, mut tracks, segments) = if let Some(p) = generated.as_mut() {
        (p.spec.fs, std::mem::take(&mut p.tracks), p.segments.clone())
    } else {
        let play = v.play_path.as_deref().unwrap_or("");
        let (mut wav, names) = load_playback(Path::new(play), record)?;
        channel_names = names;
        if v.mode == Mode::Headphones && wav.tracks.len() == 1 {
            wav.tracks.push(wav.tracks[0].clone());
        }
        let duration = wav.tracks.first().map_or(0, Vec::len) as f64 / wav.sample_rate as f64;
        (
            wav.sample_rate,
            wav.tracks,
            infer_sweep_segments(play, duration),
        )
    };
    let frames = tracks.first().map_or(0, Vec::len);
    let duration = frames as f64 / fs as f64;
    let speakers: Vec<_> = segments.iter().map(|s| s.speaker.clone()).collect();
    let playback_channels = u16::try_from(tracks.len()).map_err(RecordingError::internal)?;
    let (input, output) = resolve_devices(
        backend,
        v.input_device.as_deref(),
        v.output_device.as_deref(),
        v.host_api.as_deref(),
        playback_channels,
    )?;
    let output_plan = output_plan(v, &output, &mut tracks);
    let effective_channel_order = output_plan.channel_order;
    if let (Some(order), Some(slots)) = (effective_channel_order, output_plan.slots.as_ref()) {
        let channels = slots
            .iter()
            .enumerate()
            .map(|(index, slot)| format!("{} {slot}", index + 1))
            .collect::<Vec<_>>()
            .join(", ");
        let (key, message) = if order == ChannelOrder::Hdmi {
            (
                "recording_channel_order_hdmi",
                format!("HDMI channel order: {channels}"),
            )
        } else {
            (
                "recording_channel_order_standard",
                format!("Device channel order: {channels}"),
            )
        };
        ctx.log(json!({
            "level":"info",
            "key":key,
            "message":message,
            "channels":channels,
            "auto":v.channel_order == ChannelOrder::Auto,
        }));
    }
    emit(
        ctx,
        RecorderProgressEvent {
            phase: "devices".into(),
            duration,
            speakers: speakers.clone(),
            message: format!(
                "{} {} → {} {}",
                input.name, input.host_api, output.name, output.host_api
            ),
            ..Default::default()
        },
    );
    let mut warnings = Vec::new();
    let output_channels = output_plan.shortage.map(|shortage| {
        let message = shortage.message(&output.name);
        ctx.log(json!({
            "level":"warning",
            "key":"recording_output_channels_short",
            "message":message,
            "device":output.name,
            "available":shortage.available,
            "required":shortage.required,
            "first":shortage.first,
            "last":shortage.last,
            "missing":shortage.missing_text(),
        }));
        warnings.push(message);
        json!({
            "available":shortage.available,
            "required":shortage.required,
            "missing":shortage.missing,
        })
    });
    tracks.truncate(usize::from(output.max_output_channels));
    let interleaved = (0..frames)
        .flat_map(|i| tracks.iter().map(move |t| t[i] as f32))
        .collect();
    let mut request = SessionRequest::new(
        output,
        input,
        PlaybackBuffer {
            sample_rate: fs,
            channels: tracks.len() as u16,
            interleaved,
        },
        v.channels,
    );
    request.share = v.share;
    drop(tracks);
    let (result, streams_open) = std::thread::scope(|scope| {
        let (tx, rx) = mpsc::channel();
        let timer = scope.spawn(|| monitor(rx, ctx, duration, &segments));
        // OutputStarted follows the input ready acknowledgement and the output
        // open, so once it arrives both streams are open and a later failure is
        // an ordinary device error, never a share-mode refusal.
        let mut streams_open = false;
        let result = play_and_record(backend, request, &ctx.cancel, &mut |event| {
            let message = match event {
                SessionEvent::OutputStarted { .. } => {
                    streams_open = true;
                    Some(Clock::Start)
                }
                SessionEvent::Progress { frames_played, .. } => {
                    Some(Clock::Position(frames_played as f64 / fs as f64))
                }
                _ => None,
            };
            if let Some(message) = message {
                let _ = tx.send(message);
            }
        });
        let _ = tx.send(Clock::Stop);
        timer.join().expect("recording progress timer");
        (result, streams_open)
    });
    if matches!(result, Err(AudioError::Cancelled)) || ctx.cancel.is_cancelled() {
        return Err(cancelled(ctx, duration, speakers));
    }
    let captured = result.map_err(|e| {
        let reason = e.to_string();
        let error = if v.share.fixed_mode().is_some() && !streams_open {
            RecordingError {
                code: ErrorCode::DeviceError,
                message: format!(
                    "The device did not open in {} mode: {reason}",
                    v.share.as_str()
                ),
                details: json!({"kind":"share_mode_refused","share_mode":v.share.as_str(),"reason":reason}),
                retryable: true,
            }
        } else {
            RecordingError::device(reason)
        };
        emit(
            ctx,
            RecorderProgressEvent {
                phase: "error".into(),
                duration,
                message: error.message.clone(),
                ..Default::default()
            },
        );
        error
    })?;
    let share = json!({
        "requested": v.share.as_str(),
        "output": captured.output_mode,
        "input": captured.input_mode,
    });
    let display = |mode| match mode {
        ShareMode::Exclusive => "exclusive",
        ShareMode::SharedAutoConvert => "shared (auto-convert)",
    };
    ctx.log(json!({
        "level":"info",
        "key":"recording_share_mode_opened",
        "message":format!("Output: {} · Input: {}", display(captured.output_mode), display(captured.input_mode)),
        "share":share,
    }));
    emit(
        ctx,
        RecorderProgressEvent {
            phase: "saving".into(),
            elapsed: duration,
            duration,
            progress: 0.99,
            speakers: speakers.clone(),
            ..Default::default()
        },
    );
    let tracks: Vec<Vec<f64>> = (0..usize::from(v.channels))
        .map(|ch| {
            captured
                .interleaved
                .chunks_exact(usize::from(v.channels))
                .map(|f| f[ch] as f64)
                .collect()
        })
        .collect();
    let input_silent = tracks.iter().any(|track| !track.is_empty())
        && tracks.iter().flatten().all(|sample| *sample == 0.0);
    // The last point a cancellation is honoured: past it the recording is
    // written (and an appended file changed), so the job completes.
    if ctx.cancel.is_cancelled() {
        return Err(cancelled(ctx, duration, speakers));
    }
    save_recording(record, fs, tracks.clone(), v.append)?;
    if let Some((path, names)) = channel_names {
        std::fs::write(path, names).map_err(RecordingError::internal)?;
    }
    let check_results = Some(check_recording(
        &tracks,
        fs,
        &segments,
        generated.as_ref().map(|playback| &playback.estimator),
        v.expected_azimuths.as_ref(),
        v.reference_measurements.as_deref(),
        v.mode == Mode::Headphones,
    ));
    let silent_check = input_silent.then(|| {
        let key = if cfg!(target_os = "macos") {
            "record_check_input_silent_macos"
        } else {
            "record_check_input_silent"
        };
        ctx.log(json!({
            "level":"error",
            "key":key,
            "message":"The input recorded only digital silence."
        }));
        RecordingCheck {
            key,
            speaker: String::new(),
            severity: "error",
            values: serde_json::Map::new(),
        }
    });
    emit(
        ctx,
        RecorderProgressEvent {
            phase: "complete".into(),
            elapsed: duration,
            duration,
            progress: 1.0,
            speakers,
            ..Default::default()
        },
    );
    let sidecar = if let Some(p) = &generated
        && v.mode == Mode::Speakers
        && !p.spec.is_default_signal()
    {
        Some(write_sidecar(
            record.parent().unwrap_or(Path::new(".")),
            &p.estimator,
        )?)
    } else {
        None
    };
    let mut result = json!({"mode":v.mode,"record_path":v.record_path,"summary":analyze_recording(record),"sweep":generated.as_ref().map(|p|&p.display_name),"sidecar_path":sidecar,"share":share});
    if let Some(order) = effective_channel_order {
        result["channel_order"] = json!(order.as_str());
    }
    if let Some(check_results) = check_results {
        result["measurements"] = json!(check_results.measurements);
        result["checks"] = if let Some(check) = silent_check.as_ref() {
            json!([check])
        } else {
            json!(check_results.checks)
        };
    } else if let Some(check) = silent_check.as_ref() {
        result["checks"] = json!([check]);
    }
    if let Some(output_channels) = output_channels {
        result["output_channels"] = output_channels;
    }
    if !warnings.is_empty() {
        result["warnings"] = json!(warnings);
    }
    Ok(result)
}
