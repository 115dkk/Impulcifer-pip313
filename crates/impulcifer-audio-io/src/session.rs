//! Thread-affine, blocking two-stream measurement. No session crosses a thread.
//!
//! Progress is time-estimated (at most once per 100 ms): OutputSession exposes no
//! cursor or start acknowledgement. OutputStarted means the blocking playback
//! call is about to begin, not that the first hardware sample has played. Only
//! OutputDrained's report is authoritative. Tail duration rounds to nearest frame.
//! External cancellation always returns Cancelled, never a partial Recording.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use impulcifer_types::audio::{
    AudioBackend, AudioError, CancelToken, CaptureRead, Endpoint, PlaybackReport, ShareMode,
    StreamSpec,
};

use crate::policy::{open_input_with_policy, open_output_with_policy};

const POLL: Duration = Duration::from_millis(5);

#[derive(Clone, Debug)]
pub struct PlaybackBuffer {
    pub sample_rate: u32,
    pub channels: u16,
    pub interleaved: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct SessionRequest {
    pub output: Endpoint,
    pub input: Endpoint,
    pub playback: PlaybackBuffer,
    pub input_channels: u16,
    pub tail_seconds: f64,
}

impl SessionRequest {
    pub fn new(
        output: Endpoint,
        input: Endpoint,
        playback: PlaybackBuffer,
        input_channels: u16,
    ) -> Self {
        Self {
            output,
            input,
            playback,
            input_channels,
            tail_seconds: 0.0,
        }
    }

    fn capture_samples(&self) -> Result<usize, AudioError> {
        let invalid = || {
            AudioError::UnsupportedFormat("invalid session rate, channels, buffer or tail".into())
        };
        let p = &self.playback;
        if p.sample_rate == 0
            || p.channels == 0
            || self.input_channels == 0
            || p.channels > self.output.max_output_channels
            || self.input_channels > self.input.max_input_channels
            || !p.interleaved.len().is_multiple_of(usize::from(p.channels))
            || !p.interleaved.iter().all(|x| x.is_finite())
            || !self.tail_seconds.is_finite()
            || self.tail_seconds < 0.0
        {
            return Err(invalid());
        }
        let tail = (self.tail_seconds * f64::from(p.sample_rate)).round();
        // Strict comparison also rejects the rounded floating representation of MAX.
        if !tail.is_finite() || tail >= usize::MAX as f64 {
            return Err(invalid());
        }
        let frames = (p.interleaved.len() / usize::from(p.channels))
            .checked_add(tail as usize)
            .ok_or_else(invalid)?;
        let samples = frames
            .checked_mul(usize::from(self.input_channels))
            .ok_or_else(invalid)?;
        if samples > isize::MAX as usize / size_of::<f32>() {
            return Err(invalid());
        }
        Ok(samples)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SessionEvent {
    InputReady {
        mode: ShareMode,
    },
    OutputStarted {
        mode: ShareMode,
    },
    Progress {
        frames_played: u64,
        frames_total: u64,
    },
    OutputDrained(PlaybackReport),
    InputStopped {
        frames: usize,
    },
}

#[derive(Debug)]
pub struct Recording {
    pub sample_rate: u32,
    pub channels: u16,
    pub interleaved: Vec<f32>,
    pub output_mode: ShareMode,
    pub input_mode: ShareMode,
    pub playback: PlaybackReport,
    pub capture: CaptureRead,
}

struct Captured {
    samples: Vec<f32>,
    report: CaptureRead,
    mode: ShareMode,
}

enum Message {
    Ready(ShareMode),
    Started(ShareMode),
    Output(Result<PlaybackReport, AudioError>),
    Input(Result<Captured, AudioError>),
}

fn guarded<T>(f: impl FnOnce() -> Result<T, AudioError>) -> Result<T, AudioError> {
    catch_unwind(AssertUnwindSafe(f))
        .unwrap_or_else(|_| Err(AudioError::Backend("audio worker panicked".into())))
}

struct CancelOnDrop(CancelToken);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

fn check(cancel: &CancelToken) -> Result<(), AudioError> {
    if cancel.is_cancelled() {
        Err(AudioError::Cancelled)
    } else {
        Ok(())
    }
}

fn remember(error: AudioError, first: &mut Option<AudioError>, cancel: &CancelToken) {
    cancel.cancel();
    if first.is_none() || matches!(first, Some(AudioError::Cancelled)) {
        *first = Some(error);
    }
}

pub fn play_and_record(
    backend: &(dyn AudioBackend + Sync),
    request: SessionRequest,
    cancel: &CancelToken,
    on_event: &mut dyn FnMut(SessionEvent),
) -> Result<Recording, AudioError> {
    check(cancel)?;
    let sample_count = request.capture_samples()?;
    // Allocate before starting hardware, with a recoverable allocation failure.
    let mut samples = Vec::new();
    samples
        .try_reserve_exact(sample_count)
        .map_err(|e| AudioError::Backend(format!("capture allocation: {e}")))?;
    samples.resize(sample_count, 0.0);
    let peer = CancelToken::new();
    let frames_total =
        (request.playback.interleaved.len() / usize::from(request.playback.channels)) as u64;
    let sample_rate = request.playback.sample_rate;
    let input_channels = request.input_channels;
    std::thread::scope(|scope| {
        // Also cancels workers if the user event callback unwinds.
        let _cancel_on_exit = CancelOnDrop(peer.clone());
        let (tx, rx) = mpsc::channel();
        let (permit_tx, permit_rx) = mpsc::channel();
        let (drained_tx, drained_rx) = mpsc::channel();
        let input_tx = tx.clone();
        let peer_ref = &peer;
        let input = scope.spawn(move || {
            let result = guarded(|| {
                check(peer_ref)?;
                let (mut session, mode) = open_input_with_policy(
                    backend,
                    &request.input,
                    StreamSpec {
                        sample_rate,
                        channels: input_channels,
                    },
                )?;
                // Catch read/start panics while the session still exists so stop
                // runs on this same worker on every exit, including failures.
                let captured = guarded(|| {
                    check(peer_ref)?;
                    session.start()?;
                    input_tx
                        .send(Message::Ready(mode))
                        .map_err(|_| AudioError::Cancelled)?;
                    let mut report = CaptureRead {
                        frames: 0,
                        discontinuity: false,
                        silent: false,
                    };
                    let channels = usize::from(input_channels);
                    while report.frames < sample_count / channels {
                        check(peer_ref)?;
                        let offset = report.frames * channels;
                        // Bound each request, including backends that fill the whole destination.
                        let end = sample_count.min(offset.saturating_add(1024 * channels));
                        let read = session.read_into(&mut samples[offset..end], peer_ref)?;
                        if read.frames > (end - offset) / channels {
                            return Err(AudioError::Backend(
                                "capture backend returned too many frames".into(),
                            ));
                        }
                        report.frames += read.frames;
                        report.discontinuity |= read.discontinuity;
                        report.silent |= read.silent;
                        check(peer_ref)?;
                        if read.frames == 0 {
                            std::thread::sleep(POLL);
                        }
                    }
                    loop {
                        check(peer_ref)?;
                        match drained_rx.recv_timeout(POLL) {
                            Ok(()) => break,
                            Err(mpsc::RecvTimeoutError::Timeout) => (),
                            Err(mpsc::RecvTimeoutError::Disconnected) => {
                                return Err(AudioError::Cancelled);
                            }
                        }
                    }
                    check(peer_ref)?;
                    Ok(report)
                });
                if captured.is_err() {
                    peer_ref.cancel();
                }
                let stopped = guarded(|| session.stop());
                let report = captured?;
                stopped?;
                Ok(Captured {
                    samples,
                    report,
                    mode,
                })
            });
            if result.is_err() {
                peer_ref.cancel();
            }
            let _ = input_tx.send(Message::Input(result));
        });
        let output = scope.spawn(move || {
            let result = guarded(|| {
                check(peer_ref)?;
                // Initialize concurrently, but retain the capture-start permit.
                // The thread-affine session is created and destroyed here.
                let (mut session, mode) = open_output_with_policy(
                    backend,
                    &request.output,
                    StreamSpec {
                        sample_rate,
                        channels: request.playback.channels,
                    },
                )?;
                loop {
                    check(peer_ref)?;
                    match permit_rx.recv_timeout(POLL) {
                        Ok(()) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => (),
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            return Err(AudioError::Cancelled);
                        }
                    }
                }
                check(peer_ref)?;
                tx.send(Message::Started(mode))
                    .map_err(|_| AudioError::Cancelled)?;
                let report = session.play_to_completion(&request.playback.interleaved, peer_ref)?;
                if report.cancelled {
                    return Err(AudioError::Cancelled);
                }
                check(peer_ref)?;
                if report.frames_submitted != frames_total
                    || report.frames_drained != frames_total
                    || report.mode != mode
                {
                    return Err(AudioError::Backend(
                        "invalid playback completion report".into(),
                    ));
                }
                Ok(report)
            });
            if result.is_err() {
                peer_ref.cancel();
            }
            let _ = tx.send(Message::Output(result));
        });
        let mut input_done = false;
        let mut output_done = false;
        let mut captured = None;
        let mut playback = None;
        let mut error = None;
        let mut started = None;
        let mut last_progress = Instant::now();
        while !input_done || !output_done {
            if cancel.is_cancelled() {
                peer.cancel();
            }
            match rx.recv_timeout(POLL) {
                Ok(Message::Ready(mode)) => {
                    on_event(SessionEvent::InputReady { mode });
                    let _ = permit_tx.send(());
                }
                Ok(Message::Started(mode)) => {
                    on_event(SessionEvent::OutputStarted { mode });
                    started = Some(Instant::now());
                    last_progress = Instant::now();
                }
                Ok(Message::Output(result)) => {
                    output_done = true;
                    match result {
                        Ok(report) => {
                            on_event(SessionEvent::OutputDrained(report.clone()));
                            playback = Some(report);
                            // Release input only AFTER delivering OutputDrained.
                            let _ = drained_tx.send(());
                        }
                        Err(e) => remember(e, &mut error, &peer),
                    }
                }
                Ok(Message::Input(result)) => {
                    input_done = true;
                    match result {
                        Ok(value) => {
                            on_event(SessionEvent::InputStopped {
                                frames: value.report.frames,
                            });
                            captured = Some(value);
                        }
                        Err(e) => remember(e, &mut error, &peer),
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => (),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    remember(
                        AudioError::Backend("audio workers disconnected".into()),
                        &mut error,
                        &peer,
                    );
                    break;
                }
            }
            if !output_done
                && !peer.is_cancelled()
                && last_progress.elapsed() >= Duration::from_millis(100)
                && let Some(start) = started
            {
                on_event(SessionEvent::Progress {
                    frames_played: ((start.elapsed().as_secs_f64() * f64::from(sample_rate))
                        as u64)
                        .min(frames_total),
                    frames_total,
                });
                last_progress = Instant::now();
            }
        }
        let input_join = input.join();
        let output_join = output.join();
        if input_join.is_err() || output_join.is_err() {
            remember(
                AudioError::Backend("audio worker join failed".into()),
                &mut error,
                &peer,
            );
        }
        if cancel.is_cancelled() {
            return Err(AudioError::Cancelled);
        }
        if let Some(e) = error {
            return Err(e);
        }
        let captured =
            captured.ok_or_else(|| AudioError::Backend("missing capture report".into()))?;
        let playback =
            playback.ok_or_else(|| AudioError::Backend("missing playback report".into()))?;
        Ok(Recording {
            sample_rate,
            channels: input_channels,
            interleaved: captured.samples,
            output_mode: playback.mode,
            input_mode: captured.mode,
            playback,
            capture: captured.report,
        })
    })
}
