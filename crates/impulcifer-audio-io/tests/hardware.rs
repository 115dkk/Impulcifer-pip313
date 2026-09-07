#![forbid(unsafe_code)]

//! Explicit opt-in only: never select a default device or a physical speaker.
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use impulcifer_audio_io::cpal_backend::{CpalBackend, CpalOutputSession, DrainDiagnostics};
use impulcifer_audio_io::session::{PlaybackBuffer, SessionRequest, play_and_record};
use impulcifer_types::audio::*;

// Both tests live in this binary and hold the same lock through teardown.
static CABLE: Mutex<()> = Mutex::new(());

struct MeasuredCpal {
    diagnostics: Arc<Mutex<Option<DrainDiagnostics>>>,
}
struct MeasuredOutput {
    inner: CpalOutputSession,
    diagnostics: Arc<Mutex<Option<DrainDiagnostics>>>,
}
impl OutputSession for MeasuredOutput {
    fn play_to_completion(
        &mut self,
        data: &[f32],
        cancel: &CancelToken,
    ) -> Result<PlaybackReport, AudioError> {
        let report = self.inner.play_to_completion(data, cancel)?;
        *self.diagnostics.lock().unwrap() = self.inner.drain_diagnostics();
        Ok(report)
    }
}
impl AudioBackend for MeasuredCpal {
    fn name(&self) -> &'static str {
        "cpal measured"
    }
    fn enumerate(&self) -> Result<Vec<Endpoint>, AudioError> {
        CpalBackend.enumerate()
    }
    fn probe(
        &self,
        ep: &Endpoint,
        dir: Direction,
        spec: StreamSpec,
        mode: ShareMode,
    ) -> Result<ProbeResult, AudioError> {
        CpalBackend.probe(ep, dir, spec, mode)
    }
    fn open_input(
        &self,
        ep: &Endpoint,
        spec: StreamSpec,
        mode: ShareMode,
    ) -> Result<Box<dyn InputSession>, AudioError> {
        CpalBackend.open_input(ep, spec, mode)
    }
    fn open_output(
        &self,
        ep: &Endpoint,
        spec: StreamSpec,
        mode: ShareMode,
    ) -> Result<Box<dyn OutputSession>, AudioError> {
        Ok(Box::new(MeasuredOutput {
            inner: CpalBackend.open_output_session(ep, spec, mode)?,
            diagnostics: self.diagnostics.clone(),
        }))
    }
}

fn measure(backend: &dyn AudioBackend) {
    let endpoints = backend.enumerate().expect("enumerate hardware");
    let output = endpoints
        .iter()
        .find(|e| e.name.contains("CABLE-A Input") && e.max_output_channels >= 2)
        .expect("explicit hardware test requires CABLE-A Input; no speaker fallback")
        .clone();
    let input = endpoints
        .iter()
        .find(|e| e.name.contains("CABLE-A Output") && e.max_input_channels >= 2)
        .expect("explicit hardware test requires CABLE-A Output")
        .clone();
    let channels = output.max_output_channels;
    println!(
        "backend={} output={output:?} input={input:?}",
        backend.name()
    );
    let mut data = vec![0.0; 48000 * usize::from(channels)];
    for frame in 0..48000 {
        data[frame * usize::from(channels) + 1] =
            (0.1 * (std::f64::consts::TAU * 1000.0 * frame as f64 / 48000.0).sin()) as f32;
    }
    let mut req = SessionRequest::new(
        output,
        input,
        PlaybackBuffer {
            sample_rate: 48000,
            channels,
            interleaved: data,
        },
        2,
    );
    req.tail_seconds = 1.0;
    let start = Instant::now();
    let mut events = Vec::new();
    // Watchdog is scoped, cancellable, and joined before this test returns.
    // A backend that stops delivering callbacks must never hang an explicit test.
    let cancel = CancelToken::new();
    let finished = CancelToken::new();
    let recording = std::thread::scope(|scope| {
        scope.spawn(|| {
            while !finished.is_cancelled() && start.elapsed() < Duration::from_secs(15) {
                std::thread::sleep(Duration::from_millis(5));
            }
            if !finished.is_cancelled() {
                cancel.cancel();
            }
        });
        let result = play_and_record(backend, req, &cancel, &mut |event| {
            events.push((start.elapsed(), event));
        });
        finished.cancel();
        result
    })
    .expect("virtual cable session");
    let rms = |channel: usize| {
        let mean = recording
            .interleaved
            .chunks_exact(2)
            .map(|f| f64::from(f[channel]).powi(2))
            .sum::<f64>()
            / recording.capture.frames as f64;
        10.0 * mean.log10()
    };
    println!(
        "elapsed={:?} left_rms_dbfs={} right_rms_dbfs={} playback={:?} capture={:?}",
        start.elapsed(),
        rms(0),
        rms(1),
        recording.playback,
        recording.capture
    );
    for (elapsed, event) in events {
        println!("event_at={elapsed:?} {event:?}");
    }
    assert_eq!(recording.capture.frames, 96000);
    assert_eq!(recording.interleaved.len(), 192000);
    assert_eq!(recording.playback.frames_submitted, 48000);
    assert_eq!(recording.playback.frames_drained, 48000);
    assert!(
        (rms(1) - (-26.02)).abs() <= 1.0,
        "right RMS {} dBFS",
        rms(1)
    );
    assert!(rms(0) < -60.0, "unexpected left signal {} dBFS", rms(0));
}

#[test]
#[ignore = "requires CABLE-A virtual cable; emits a -20 dBFS peak tone only into CABLE-A"]
fn cpal_play_and_capture_virtual_cable() {
    let _lock = CABLE.lock().unwrap_or_else(|e| e.into_inner());
    let backend = MeasuredCpal {
        diagnostics: Arc::new(Mutex::new(None)),
    };
    measure(&backend);
    let diagnostics = backend
        .diagnostics
        .lock()
        .unwrap()
        .expect("measured final callback");
    println!("CPAL drain heuristic: {diagnostics:?}");
    assert!(diagnostics.waited_after_final >= diagnostics.final_callback_period * 2);
}

#[cfg(windows)]
#[test]
#[ignore = "requires CABLE-A virtual cable; emits a -20 dBFS peak tone only into CABLE-A"]
fn wasapi_session_virtual_cable() {
    let _lock = CABLE.lock().unwrap_or_else(|e| e.into_inner());
    measure(impulcifer_audio_io::default_backend().as_ref());
}
