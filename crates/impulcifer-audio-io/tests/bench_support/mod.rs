//! PA05 bench-only observers around the unchanged production session.
use std::hint::black_box;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use impulcifer_audio_io::policy::{open_input_with_policy, open_output_with_policy};
use impulcifer_audio_io::session::{PlaybackBuffer, Recording, SessionRequest, play_and_record};
use impulcifer_types::audio::*;

pub const OUTPUT: &str = "CABLE-A Input (VB-Audio Cable A)";
pub const INPUT: &str = "CABLE-A Output (VB-Audio Cable A)";
pub const HOST: &str = "Windows WASAPI";

pub fn enumerate_devices(backend: &dyn AudioBackend) -> Result<Vec<Endpoint>, AudioError> {
    backend.enumerate()
}

pub fn pair(backend: &dyn AudioBackend) -> Result<(Endpoint, Endpoint), AudioError> {
    let devices = enumerate_devices(backend)?;
    let select = |name: &str, input: bool| {
        let matches: Vec<_> = devices
            .iter()
            .filter(|e| {
                // Windows localizes the space before parentheses away on this machine.
                // Only this whitespace difference is accepted, never a substring/default.
                e.name.replace(" (", "(") == name.replace(" (", "(")
                    && e.host_api == HOST
                    && if input {
                        e.max_input_channels >= 2
                    } else {
                        e.max_output_channels >= 2
                    }
            })
            .collect();
        if matches.len() != 1 {
            return Err(AudioError::DeviceNotFound(format!(
                "expected exactly one {HOST} {name}, got {}",
                matches.len()
            )));
        }
        Ok(matches[0].clone())
    };
    Ok((select(INPUT, true)?, select(OUTPUT, false)?))
}

pub fn open_close_session(
    backend: &dyn AudioBackend,
    input: &Endpoint,
    output: &Endpoint,
) -> Result<(ShareMode, ShareMode), AudioError> {
    let spec = StreamSpec {
        sample_rate: 48000,
        channels: 2,
    };
    std::thread::scope(|scope| {
        let capture = scope.spawn(|| {
            let (session, mode) = open_input_with_policy(backend, input, spec)?;
            drop(session);
            Ok::<_, AudioError>(mode)
        });
        let render = scope.spawn(|| {
            let (session, mode) = open_output_with_policy(backend, output, spec)?;
            drop(session);
            Ok::<_, AudioError>(mode)
        });
        // Join both before propagating either error. No initialized session is
        // sent, retained or destroyed outside this timed pair.
        let input = capture.join();
        let output = render.join();
        Ok((
            input.map_err(|_| AudioError::Backend("capture open panicked".into()))??,
            output.map_err(|_| AudioError::Backend("render open panicked".into()))??,
        ))
    })
}

#[derive(Clone, Debug, Default)]
pub struct Delivery {
    pub start: Option<Instant>,
    pub first: Option<Instant>,
    pub first_frames: usize,
    pub reads: usize,
    pub empty_reads: usize,
    pub exclusive_failures: Vec<String>,
}

struct ObservedBackend<'a> {
    inner: &'a dyn AudioBackend,
    delivery: Arc<Mutex<Delivery>>,
    latency_chunks: bool,
}
struct ObservedInput {
    inner: Box<dyn InputSession>,
    delivery: Arc<Mutex<Delivery>>,
    chunk_samples: usize,
}
impl AudioBackend for ObservedBackend<'_> {
    fn name(&self) -> &'static str {
        self.inner.name()
    }
    fn enumerate(&self) -> Result<Vec<Endpoint>, AudioError> {
        self.inner.enumerate()
    }
    fn probe(
        &self,
        endpoint: &Endpoint,
        direction: Direction,
        spec: StreamSpec,
        mode: ShareMode,
    ) -> Result<ProbeResult, AudioError> {
        self.inner.probe(endpoint, direction, spec, mode)
    }
    fn open_output(
        &self,
        endpoint: &Endpoint,
        spec: StreamSpec,
        mode: ShareMode,
    ) -> Result<Box<dyn OutputSession>, AudioError> {
        let result = self.inner.open_output(endpoint, spec, mode);
        if mode == ShareMode::Exclusive
            && let Err(e) = &result
        {
            self.delivery
                .lock()
                .unwrap()
                .exclusive_failures
                .push(format!("output: {e}"));
        }
        result
    }
    fn open_input(
        &self,
        endpoint: &Endpoint,
        spec: StreamSpec,
        mode: ShareMode,
    ) -> Result<Box<dyn InputSession>, AudioError> {
        let result = self.inner.open_input(endpoint, spec, mode);
        if mode == ShareMode::Exclusive
            && let Err(e) = &result
        {
            self.delivery
                .lock()
                .unwrap()
                .exclusive_failures
                .push(format!("input: {e}"));
        }
        Ok(Box::new(ObservedInput {
            inner: result?,
            delivery: self.delivery.clone(),
            chunk_samples: if self.latency_chunks
                && std::env::var("IMPULCIFER_PA05_LATENCY_1024").as_deref() != Ok("1")
            {
                480 * usize::from(spec.channels)
            } else {
                usize::MAX
            },
        }))
    }
}
impl InputSession for ObservedInput {
    fn start(&mut self) -> Result<(), AudioError> {
        self.delivery.lock().unwrap().start = Some(Instant::now());
        self.inner.start()
    }
    fn read_into(
        &mut self,
        dst: &mut [f32],
        cancel: &CancelToken,
    ) -> Result<CaptureRead, AudioError> {
        let count = dst.len().min(self.chunk_samples);
        let read = self.inner.read_into(&mut dst[..count], cancel)?;
        // Timestamp only after actual nonempty delivery, NOT entry to read_into.
        let now = Instant::now();
        let mut delivery = self.delivery.lock().unwrap();
        delivery.reads += 1;
        delivery.empty_reads += usize::from(read.frames == 0);
        if read.frames > 0 && delivery.first.is_none() {
            delivery.first = Some(now);
            delivery.first_frames = read.frames;
        }
        Ok(read)
    }
    fn stop(&mut self) -> Result<(), AudioError> {
        self.inner.stop()
    }
}

pub struct Measurement {
    pub recording: Recording,
    pub delivery: Delivery,
    pub wall_ms: f64,
    pub overhead_ms: f64,
}

pub fn record(
    backend: &dyn AudioBackend,
    request: SessionRequest,
) -> Result<Measurement, AudioError> {
    let duration_ms = request.playback.interleaved.len() as f64
        / f64::from(request.playback.channels)
        / f64::from(request.playback.sample_rate)
        * 1000.0;
    let delivery = Arc::new(Mutex::new(Delivery::default()));
    let observed = ObservedBackend {
        inner: backend,
        delivery: delivery.clone(),
        latency_chunks: request.playback.sample_rate == 48000
            && request.playback.interleaved.len() == 4800 * 2,
    };
    let start = Instant::now();
    let recording = play_and_record(&observed, request, &CancelToken::new(), &mut |_| {})?;
    let wall_ms = start.elapsed().as_secs_f64() * 1000.0;
    let delivery = delivery.lock().unwrap().clone();
    Ok(Measurement {
        recording,
        delivery,
        wall_ms,
        overhead_ms: wall_ms - duration_ms,
    })
}

pub fn latency_ms(delivery: &Delivery) -> Result<f64, AudioError> {
    match (delivery.start, delivery.first) {
        (Some(start), Some(first)) => Ok(first.duration_since(start).as_secs_f64() * 1000.0),
        _ => Err(AudioError::Backend(
            "no nonempty capture delivery observed".into(),
        )),
    }
}

// Uses the existing f32 transport unchanged; no DSP or new numeric representation.
pub fn playback_set(mono: &PlaybackBuffer, segments: usize) -> PlaybackBuffer {
    assert_eq!(mono.channels, 1);
    let mut interleaved = Vec::with_capacity(mono.interleaved.len() * segments * 2);
    for segment in 0..segments {
        for &sample in &mono.interleaved {
            if segments == 1 {
                interleaved.extend_from_slice(&[sample, sample]);
            } else if segment % 2 == 0 {
                interleaved.extend_from_slice(&[sample, 0.0]);
            } else {
                interleaved.extend_from_slice(&[0.0, sample]);
            }
        }
    }
    PlaybackBuffer {
        sample_rate: mono.sample_rate,
        channels: 2,
        interleaved,
    }
}

pub fn print_row(name: &str, size: &str, samples: &[f64]) {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    let median = (sorted[(n - 1) / 2] + sorted[n / 2]) / 2.0;
    println!("| {name} | {size} | {median:.6} | {:.6} |", sorted[0]);
}

pub fn measure(
    name: &str,
    calls: usize,
    mut operation: impl FnMut() -> Result<(), AudioError>,
) -> Result<(), AudioError> {
    for _ in 0..3 {
        for _ in 0..calls {
            operation()?;
        }
    }
    let mut elapsed = Vec::new();
    for batch in 0..5 {
        let start = Instant::now();
        for _ in 0..calls {
            operation()?;
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        println!("raw op={name} batch={batch} calls={calls} wall_ms={ms:.6}");
        elapsed.push(ms);
    }
    print_row(name, &format!("{calls} calls/batch"), &elapsed);
    Ok(())
}

// The synchronous observer acknowledges each boundary. No spawned/detached task
// here: the foreground Python parent reads process CPU while this thread waits.
pub fn cpu_boundary(enabled: bool, boundary: &str) -> Result<(), AudioError> {
    if enabled {
        println!("PA05_CPU_{boundary} {}", std::process::id());
        io::stdout()
            .flush()
            .map_err(|e| AudioError::Backend(e.to_string()))?;
        let mut ack = String::new();
        io::stdin()
            .read_line(&mut ack)
            .map_err(|e| AudioError::Backend(e.to_string()))?;
        if ack.trim() != "ACK" {
            return Err(AudioError::Backend("missing CPU observer ACK".into()));
        }
    }
    Ok(())
}

#[allow(dead_code)]
pub fn run(backend: &dyn AudioBackend) -> Result<(), String> {
    if !cfg!(windows) {
        return Err("PA05 requires Windows WASAPI".into());
    }
    let op = std::env::var("IMPULCIFER_PA05_OP").unwrap_or_else(|_| "all".into());
    let run = || -> Result<(), AudioError> {
        println!("| op | size | rust median ms | rust min ms |");
        if op == "all" || op == "enumerate_backend" {
            let cold = Instant::now();
            black_box(enumerate_devices(backend)?);
            println!(
                "cold_enumeration_ms={:.6}",
                cold.elapsed().as_secs_f64() * 1000.0
            );
            measure("enumerate_backend", 20, || {
                black_box(enumerate_devices(backend)?);
                Ok(())
            })?;
        }
        if op == "enumerate_backend" {
            return Ok(());
        }
        let (input, output) = pair(backend)?;
        println!("input={input:?}\noutput={output:?}");
        if op == "profile_open" {
            let spec = StreamSpec {
                sample_rate: 48000,
                channels: 2,
            };
            measure("profile_shared_pair", 20, || {
                std::thread::scope(|scope| {
                    let capture = scope.spawn(|| {
                        backend
                            .open_input(&input, spec, ShareMode::SharedAutoConvert)
                            .map(drop)
                    });
                    let render = scope.spawn(|| {
                        backend
                            .open_output(&output, spec, ShareMode::SharedAutoConvert)
                            .map(drop)
                    });
                    let input = capture.join();
                    let output = render.join();
                    input.map_err(|_| AudioError::Backend("capture open panicked".into()))??;
                    output.map_err(|_| AudioError::Backend("render open panicked".into()))??;
                    Ok(())
                })
            })?;
            measure("profile_exclusive_rejections", 20, || {
                for result in [
                    backend
                        .open_input(&input, spec, ShareMode::Exclusive)
                        .map(|_| ()),
                    backend
                        .open_output(&output, spec, ShareMode::Exclusive)
                        .map(|_| ()),
                ] {
                    match result {
                        Err(AudioError::UnsupportedFormat(_)) => (),
                        Err(e) => return Err(e),
                        Ok(()) => {
                            return Err(AudioError::Backend(
                                "expected rejection did not occur".into(),
                            ));
                        }
                    }
                }
                Ok(())
            })?;
            return Ok(());
        }
        if op == "all" || op == "open_close_session" {
            println!(
                "open modes={:?}",
                open_close_session(backend, &input, &output)?
            );
            measure("open_close_session", 20, || {
                black_box(open_close_session(backend, &input, &output)?);
                Ok(())
            })?;
        }
        if op == "open_close_session" {
            return Ok(());
        }
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav");
        let wav = impulcifer_io::read_wav(&path).map_err(|e| AudioError::Backend(e.to_string()))?;
        assert_eq!(wav.sample_rate, 48000);
        assert_eq!(wav.tracks.len(), 1);
        assert_eq!(wav.tracks[0].len(), 295270);
        let mono = PlaybackBuffer {
            sample_rate: wav.sample_rate,
            channels: 1,
            interleaved: wav.tracks[0].iter().map(|&s| s as f32).collect(),
        };
        for (name, segments, repetitions) in [
            ("play_record_headphones_sweep", 1, 5),
            ("play_record_7_speaker_set", 7, 3),
            ("first_sample_latency", 1, 10),
        ] {
            if op != "all"
                && op != name
                && !(op == "capture_loop_cpu" && segments == 1 && repetitions == 5)
            {
                continue;
            }
            let mut playback = playback_set(&mono, segments);
            if name == "first_sample_latency" {
                playback.interleaved.truncate(4800 * 2);
            }
            let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/bench_support")
                .join(format!("sixth-{name}.wav"));
            let file = if name == "play_record_headphones_sweep" {
                path.clone()
            } else {
                let tracks: Vec<Vec<f64>> = (0..2)
                    .map(|ch| {
                        playback
                            .interleaved
                            .chunks_exact(2)
                            .map(|f| f[ch] as f64)
                            .collect()
                    })
                    .collect();
                impulcifer_io::write_wav(&fixture, 48000, &tracks, 32)
                    .map_err(|e| AudioError::Backend(e.to_string()))?;
                fixture
            };
            let frames = playback.interleaved.len() / 2;
            let hash = playback
                .interleaved
                .iter()
                .flat_map(|s| s.to_le_bytes())
                .fold(14695981039346656037_u64, |h, b| {
                    (h ^ u64::from(b)).wrapping_mul(1099511628211)
                });
            println!(
                "workload op={name} frames={frames} channels=2 rate=48000 duration_s={:.12} transport_fnv1a={hash:016x}",
                frames as f64 / 48000.0
            );
            let mut walls = Vec::new();
            let mut overheads = Vec::new();
            let mut latencies = Vec::new();
            // Integrity-only runs include a tail and save every capture. They are
            // deliberately separate from the benchmark warmups and timing rows.
            let diagnostic = std::env::var("IMPULCIFER_PA05_INTEGRITY").as_deref() == Ok("1");
            let runs = if diagnostic {
                std::env::var("IMPULCIFER_PA05_INTEGRITY_RUNS")
                    .ok()
                    .map(|value| value.parse::<usize>().expect("integrity run count"))
                    .unwrap_or(10)
            } else {
                3 + repetitions
            };
            for index in 0..runs {
                let cpu = std::env::var("IMPULCIFER_PA05_CPU_OBSERVER").as_deref() == Ok("1")
                    && index >= 3
                    && name == "play_record_headphones_sweep";
                cpu_boundary(cpu, "BEGIN")?;
                let start = Instant::now();
                let loaded = impulcifer_io::read_wav(&file)
                    .map_err(|e| AudioError::Backend(e.to_string()))?;
                let (input, output) = pair(backend)?;
                let interleaved = (0..loaded.tracks[0].len())
                    .flat_map(|i| {
                        [
                            loaded.tracks[0][i] as f32,
                            loaded.tracks[loaded.tracks.len() - 1][i] as f32,
                        ]
                    })
                    .collect();
                let mut request = SessionRequest::new(
                    output,
                    input,
                    PlaybackBuffer {
                        sample_rate: 48000,
                        channels: 2,
                        interleaved,
                    },
                    2,
                );
                if std::env::var("IMPULCIFER_PA05_TAIL").as_deref() == Ok("1") {
                    request.tail_seconds = 0.25;
                }
                drop(loaded);
                let measured = record(backend, request);
                let wall_ms = start.elapsed().as_secs_f64() * 1000.0;
                cpu_boundary(cpu, "END")?;
                let mut measured = measured?;
                measured.wall_ms = wall_ms;
                measured.overhead_ms = wall_ms - frames as f64 / 48.0;
                let tail_frames = if std::env::var("IMPULCIFER_PA05_TAIL").as_deref() == Ok("1") {
                    12000
                } else {
                    0
                };
                assert_eq!(measured.recording.capture.frames, frames + tail_frames);
                assert_eq!(measured.recording.playback.frames_drained as usize, frames);
                let latency = latency_ms(&measured.delivery)?;
                println!(
                    "raw op={name} run={index} warmup={} wall_ms={:.6} overhead_ms={:.6} first_delivery_ms={latency:.6} first_frames={} reads={} empty_reads={} captured_frames={} drained_frames={} input_mode={:?} output_mode={:?} underruns={} discontinuity={} silent={} exclusive_errors={:?}",
                    !diagnostic && index < 3,
                    measured.wall_ms,
                    measured.overhead_ms,
                    measured.delivery.first_frames,
                    measured.delivery.reads,
                    measured.delivery.empty_reads,
                    measured.recording.capture.frames,
                    measured.recording.playback.frames_drained,
                    measured.recording.input_mode,
                    measured.recording.output_mode,
                    measured.recording.playback.underruns,
                    measured.recording.capture.discontinuity,
                    measured.recording.capture.silent,
                    measured.delivery.exclusive_failures
                );
                println!(
                    "transport diagnostics: opt-in sys-win CSV contains packet/write metadata; public session trait does not expose it. first_delivery_ms measures input session start call to first nonempty application read return, not driver packet arrival"
                );
                if (diagnostic || index == 3)
                    && let Ok(prefix) = std::env::var("IMPULCIFER_PA05_CAPTURE_PREFIX")
                {
                    let bytes: Vec<_> = measured
                        .recording
                        .interleaved
                        .iter()
                        .flat_map(|x| x.to_le_bytes())
                        .collect();
                    std::fs::write(format!("{prefix}-{name}-{index}.f32"), bytes)
                        .map_err(|e| AudioError::Backend(e.to_string()))?;
                }
                if index >= 3 {
                    walls.push(measured.wall_ms);
                    overheads.push(measured.overhead_ms);
                    latencies.push(latency);
                }
            }
            if diagnostic {
                continue;
            }
            if name == "first_sample_latency" {
                print_row(
                    name,
                    "input session start call to first nonempty application read return; 10 runs",
                    &latencies,
                );
            } else {
                print_row(
                    name,
                    &format!("{frames} frames; {repetitions} runs"),
                    &walls,
                );
                print_row(
                    &format!("{name}_overhead"),
                    "wall minus playback duration",
                    &overheads,
                );
            }
        }
        Ok(())
    };
    run().map_err(|e| e.to_string())
}
