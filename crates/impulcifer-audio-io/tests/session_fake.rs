#![forbid(unsafe_code)]

use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread::{self, ThreadId};
use std::time::{Duration, Instant};

use impulcifer_audio_io::policy::{
    open_input_with_policy, open_output_with_policy, open_output_with_preference,
};
use impulcifer_audio_io::session::{PlaybackBuffer, SessionEvent, SessionRequest, play_and_record};
use impulcifer_types::audio::*;

mod bench_support;

#[test]
fn bench_smoke_impulcifer_audio_io() {
    let backend = Fake::default();
    assert_eq!(bench_support::enumerate_devices(&backend).unwrap().len(), 1);
    // A fake/default device must never be mistaken for the required CABLE-A pair.
    assert!(bench_support::pair(&backend).is_err());
    let modes = bench_support::open_close_session(&backend, &endpoint(), &endpoint()).unwrap();
    assert_eq!(modes, (ShareMode::Exclusive, ShareMode::Exclusive));
    let mono = PlaybackBuffer {
        sample_rate: 1000,
        channels: 1,
        interleaved: vec![0.25; 2],
    };
    for segments in [1, 7] {
        let playback = bench_support::playback_set(&mono, segments);
        assert_eq!(playback.interleaved.len(), 4 * segments);
        let measured = bench_support::record(
            &backend,
            SessionRequest::new(endpoint(), endpoint(), playback, 2),
        )
        .unwrap();
        assert_eq!(measured.recording.capture.frames, 2 * segments);
        assert_eq!(
            measured.recording.playback.frames_drained,
            (2 * segments) as u64
        );
        assert!(bench_support::latency_ms(&measured.delivery).unwrap() >= 1.0);
        assert!(measured.delivery.first_frames > 0);
        assert!(measured.wall_ms >= measured.overhead_ms);
        bench_support::cpu_boundary(false, "BEGIN").unwrap();
        bench_support::cpu_boundary(false, "END").unwrap();
    }
    bench_support::measure("tiny_enumeration", 1, || {
        bench_support::enumerate_devices(&backend).map(|_| ())
    })
    .unwrap();
    let empty = bench_support::Delivery::default();
    assert!(bench_support::latency_ms(&empty).is_err());
    let failing = Fake {
        fault: Fault::Read,
        ..Fake::default()
    };
    assert!(bench_support::record(&failing, request()).is_err());
}

#[test]
fn bench_playback_set_preserves_transport_and_segment_routing() {
    let mono = PlaybackBuffer {
        sample_rate: 48000,
        channels: 1,
        interleaved: vec![0.25, -0.5],
    };
    assert_eq!(
        bench_support::playback_set(&mono, 1).interleaved,
        [0.25, 0.25, -0.5, -0.5]
    );
    let set = bench_support::playback_set(&mono, 7);
    assert_eq!(set.sample_rate, 48000);
    assert_eq!(set.channels, 2);
    for (index, segment) in set.interleaved.chunks_exact(4).enumerate() {
        assert_eq!(
            segment,
            if index % 2 == 0 {
                &[0.25, 0.0, -0.5, 0.0]
            } else {
                &[0.0, 0.25, 0.0, -0.5]
            }
        );
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Fault {
    #[default]
    None,
    Unsupported,
    Missing,
    InputOpen,
    OutputOpen,
    Start,
    Read,
    Stop,
    Play,
    ReadPanic,
    PlayPanic,
    OpenPanic,
    Count,
    Report,
}

#[derive(Default)]
struct State {
    history: Vec<&'static str>,
    attempts: Vec<(Direction, ShareMode)>,
    submitted: Vec<f32>,
    owners: Vec<ThreadId>,
}

#[derive(Default)]
struct Fake {
    state: Arc<Mutex<State>>,
    fault: Fault,
    duration: Duration,
    open_hook: Option<Arc<dyn Fn(Direction) + Send + Sync + std::panic::RefUnwindSafe>>,
}

struct Affinity {
    owner: ThreadId,
    _not_send: PhantomData<Rc<()>>,
}
impl Affinity {
    fn new() -> Self {
        Self {
            owner: thread::current().id(),
            _not_send: PhantomData,
        }
    }
    fn check(&self) {
        assert_eq!(self.owner, thread::current().id());
    }
}
impl Drop for Affinity {
    fn drop(&mut self) {
        self.check();
    }
}

impl Fake {
    fn attempt(&self, direction: Direction, mode: ShareMode) -> Result<(), AudioError> {
        self.state.lock().unwrap().attempts.push((direction, mode));
        if let Some(hook) = &self.open_hook {
            hook(direction);
        }
        if self.fault == Fault::Unsupported && mode == ShareMode::Exclusive {
            return Err(AudioError::UnsupportedFormat("exclusive".into()));
        }
        if self.fault == Fault::Missing {
            return Err(AudioError::DeviceNotFound("missing".into()));
        }
        if self.fault == Fault::OpenPanic {
            panic!("open panic");
        }
        if (direction == Direction::Input && self.fault == Fault::InputOpen)
            || (direction == Direction::Output && self.fault == Fault::OutputOpen)
        {
            return Err(AudioError::Backend(format!("{direction:?} open")));
        }
        self.state
            .lock()
            .unwrap()
            .owners
            .push(thread::current().id());
        Ok(())
    }
}
impl AudioBackend for Fake {
    fn name(&self) -> &'static str {
        "fake"
    }
    fn enumerate(&self) -> Result<Vec<Endpoint>, AudioError> {
        Ok(vec![endpoint()])
    }
    fn probe(
        &self,
        _: &Endpoint,
        _: Direction,
        spec: StreamSpec,
        mode: ShareMode,
    ) -> Result<ProbeResult, AudioError> {
        Ok(ProbeResult {
            supported: true,
            mode,
            native_sample_rate: spec.sample_rate,
            native_channels: spec.channels,
            detail: String::new(),
        })
    }
    fn open_output(
        &self,
        _: &Endpoint,
        spec: StreamSpec,
        mode: ShareMode,
    ) -> Result<Box<dyn OutputSession>, AudioError> {
        self.attempt(Direction::Output, mode)?;
        self.state.lock().unwrap().history.push("output open");
        Ok(Box::new(Output {
            state: self.state.clone(),
            fault: self.fault,
            spec,
            mode,
            duration: self.duration,
            affinity: Affinity::new(),
        }))
    }
    fn open_input(
        &self,
        _: &Endpoint,
        spec: StreamSpec,
        mode: ShareMode,
    ) -> Result<Box<dyn InputSession>, AudioError> {
        self.attempt(Direction::Input, mode)?;
        Ok(Box::new(Input {
            state: self.state.clone(),
            fault: self.fault,
            spec,
            cursor: 0,
            affinity: Affinity::new(),
        }))
    }
}

struct Output {
    state: Arc<Mutex<State>>,
    fault: Fault,
    spec: StreamSpec,
    mode: ShareMode,
    duration: Duration,
    affinity: Affinity,
}
impl Drop for Output {
    fn drop(&mut self) {
        self.affinity.check();
        self.state.lock().unwrap().history.push("output drop");
    }
}
impl OutputSession for Output {
    fn play_to_completion(
        &mut self,
        data: &[f32],
        cancel: &CancelToken,
    ) -> Result<PlaybackReport, AudioError> {
        self.affinity.check();
        self.state.lock().unwrap().history.push("output play");
        self.state.lock().unwrap().submitted = data.to_vec();
        if self.fault == Fault::PlayPanic {
            panic!("play panic");
        }
        if self.fault == Fault::Play {
            return Err(AudioError::Backend("play".into()));
        }
        let start = Instant::now();
        while start.elapsed() < self.duration {
            if cancel.is_cancelled() {
                self.state.lock().unwrap().history.push("output cancelled");
                return Err(AudioError::Cancelled);
            }
            thread::sleep(Duration::from_millis(1));
        }
        self.state.lock().unwrap().history.push("output drained");
        let frames = (data.len() / usize::from(self.spec.channels)) as u64;
        Ok(PlaybackReport {
            frames_submitted: frames,
            frames_drained: if self.fault == Fault::Report {
                0
            } else {
                frames
            },
            mode: self.mode,
            underruns: 0,
            cancelled: false,
        })
    }
}

struct Input {
    state: Arc<Mutex<State>>,
    fault: Fault,
    spec: StreamSpec,
    cursor: usize,
    affinity: Affinity,
}
impl Drop for Input {
    fn drop(&mut self) {
        self.affinity.check();
        self.state.lock().unwrap().history.push("input drop");
    }
}
impl InputSession for Input {
    fn start(&mut self) -> Result<(), AudioError> {
        self.affinity.check();
        if self.fault == Fault::Start {
            return Err(AudioError::Backend("start".into()));
        }
        self.state.lock().unwrap().history.push("input start");
        Ok(())
    }
    fn read_into(
        &mut self,
        dst: &mut [f32],
        cancel: &CancelToken,
    ) -> Result<CaptureRead, AudioError> {
        self.affinity.check();
        if self.fault == Fault::ReadPanic {
            panic!("read panic");
        }
        if self.fault == Fault::Read {
            return Err(AudioError::Backend("read".into()));
        }
        if self.fault == Fault::Count {
            return Ok(CaptureRead {
                frames: dst.len() + 1,
                silent: false,
                discontinuity: false,
            });
        }
        if cancel.is_cancelled() {
            return Err(AudioError::Cancelled);
        }
        // Partial chunks, with a known three-frame delay and per-channel signal.
        let channels = usize::from(self.spec.channels);
        let frames = (dst.len() / channels).min(7);
        for frame in 0..frames {
            for channel in 0..channels {
                dst[frame * channels + channel] = if self.cursor + frame < 3 {
                    0.0
                } else {
                    (self.cursor + frame - 3) as f32 + channel as f32 / 10.0
                };
            }
        }
        let silent = self.cursor == 0;
        self.cursor += frames;
        thread::sleep(Duration::from_millis(1));
        Ok(CaptureRead {
            frames,
            silent,
            discontinuity: self.cursor == 7,
        })
    }
    fn stop(&mut self) -> Result<(), AudioError> {
        self.affinity.check();
        self.state.lock().unwrap().history.push("input stop");
        if self.fault == Fault::Stop {
            Err(AudioError::Backend("stop".into()))
        } else {
            Ok(())
        }
    }
}

fn endpoint() -> Endpoint {
    Endpoint {
        id: "fake".into(),
        name: "fake".into(),
        host_api: "fake".into(),
        max_input_channels: 2,
        max_output_channels: 2,
        default_samplerate: 1000.0,
        is_default_input: true,
        is_default_output: true,
    }
}
fn request() -> SessionRequest {
    SessionRequest::new(
        endpoint(),
        endpoint(),
        PlaybackBuffer {
            sample_rate: 1000,
            channels: 2,
            interleaved: (0..40).map(|x| x as f32 / 40.0).collect(),
        },
        2,
    )
}

#[test]
fn fixed_share_preference_never_falls_back() {
    let fake = Fake {
        fault: Fault::Unsupported,
        ..Fake::default()
    };
    let spec = StreamSpec {
        sample_rate: 1000,
        channels: 2,
    };

    let (_, mode) = open_output_with_preference(&fake, &endpoint(), spec, SharePreference::Auto)
        .expect("auto falls back to shared");
    assert_eq!(mode, ShareMode::SharedAutoConvert);
    assert_eq!(
        fake.state.lock().unwrap().attempts,
        [
            (Direction::Output, ShareMode::Exclusive),
            (Direction::Output, ShareMode::SharedAutoConvert),
        ]
    );

    fake.state.lock().unwrap().attempts.clear();
    assert!(matches!(
        open_output_with_preference(&fake, &endpoint(), spec, SharePreference::Exclusive),
        Err(AudioError::UnsupportedFormat(reason)) if reason == "exclusive"
    ));
    assert_eq!(
        fake.state.lock().unwrap().attempts,
        [(Direction::Output, ShareMode::Exclusive)]
    );

    fake.state.lock().unwrap().attempts.clear();
    let (_, mode) = open_output_with_preference(&fake, &endpoint(), spec, SharePreference::Shared)
        .expect("fixed shared opens once");
    assert_eq!(mode, ShareMode::SharedAutoConvert);
    assert_eq!(
        fake.state.lock().unwrap().attempts,
        [(Direction::Output, ShareMode::SharedAutoConvert)]
    );
}

#[test]
fn session_request_carries_the_share_preference() {
    for (preference, expected) in [
        (SharePreference::Auto, ShareMode::Exclusive),
        (SharePreference::Exclusive, ShareMode::Exclusive),
        (SharePreference::Shared, ShareMode::SharedAutoConvert),
    ] {
        let fake = Fake::default();
        let mut req = request();
        req.share = preference;
        let recording = play_and_record(&fake, req, &CancelToken::new(), &mut |_| {}).unwrap();
        assert_eq!(recording.output_mode, expected);
        assert_eq!(recording.input_mode, expected);
    }

    let fake = Fake {
        fault: Fault::Unsupported,
        ..Fake::default()
    };
    let mut req = request();
    req.share = SharePreference::Exclusive;
    assert!(matches!(
        play_and_record(&fake, req, &CancelToken::new(), &mut |_| {}),
        Err(AudioError::UnsupportedFormat(reason)) if reason == "exclusive"
    ));
}

#[test]
fn session_starts_input_before_output() {
    let fake = Fake::default();
    let request = request();
    let expected = request.playback.interleaved.clone();
    play_and_record(&fake, request, &CancelToken::new(), &mut |_| {}).unwrap();
    let state = fake.state.lock().unwrap();
    let position = |s| state.history.iter().position(|x| *x == s).unwrap();
    assert!(position("input start") < position("output play"));
    assert!(position("output drained") < position("input stop"));
    assert_eq!(state.submitted, expected);
    assert_eq!(state.owners.len(), 2);
    assert_ne!(state.owners[0], state.owners[1]);
    assert!(state.owners.iter().all(|id| *id != thread::current().id()));
}

#[test]
fn session_reports_events_in_order() {
    let fake = Fake {
        duration: Duration::from_millis(240),
        ..Fake::default()
    };
    let mut events = Vec::new();
    play_and_record(&fake, request(), &CancelToken::new(), &mut |e| {
        events.push((Instant::now(), e))
    })
    .unwrap();
    assert!(matches!(events[0].1, SessionEvent::InputReady { .. }));
    assert!(matches!(events[1].1, SessionEvent::OutputStarted { .. }));
    assert!(matches!(
        events[events.len() - 2].1,
        SessionEvent::OutputDrained(_)
    ));
    assert!(matches!(
        events[events.len() - 1].1,
        SessionEvent::InputStopped { .. }
    ));
    let progress: Vec<_> = events
        .iter()
        .filter(|(_, e)| matches!(e, SessionEvent::Progress { .. }))
        .collect();
    assert!(!progress.is_empty());
    for pair in progress.windows(2) {
        assert!(pair[1].0.duration_since(pair[0].0) >= Duration::from_millis(100));
    }
}

#[test]
fn session_captures_playback_length_plus_tail() {
    let mut req = request();
    assert_eq!(req.tail_seconds, 0.0);
    req.tail_seconds = 0.010;
    let recording =
        play_and_record(&Fake::default(), req, &CancelToken::new(), &mut |_| {}).unwrap();
    assert_eq!(recording.sample_rate, 1000);
    assert_eq!(recording.channels, 2);
    assert_eq!(recording.capture.frames, 30);
    assert_eq!(recording.interleaved.len(), 60);
    assert_eq!(&recording.interleaved[..6], &[0.0; 6]);
    for frame in 3..30 {
        assert_eq!(recording.interleaved[frame * 2], (frame - 3) as f32);
        assert_eq!(
            recording.interleaved[frame * 2 + 1],
            (frame - 3) as f32 + 0.1
        );
    }
    assert!(recording.capture.discontinuity && recording.capture.silent);
    assert_eq!(recording.playback.frames_drained, 20);
}

#[test]
fn session_cancel_stops_both_sides() {
    let fake = Fake {
        duration: Duration::from_secs(1),
        ..Fake::default()
    };
    let token = CancelToken::new();
    let start = Instant::now();
    let result = play_and_record(&fake, request(), &token, &mut |event| {
        if matches!(event, SessionEvent::Progress { .. }) {
            token.cancel();
        }
    });
    assert!(matches!(result, Err(AudioError::Cancelled)));
    assert!(start.elapsed() < Duration::from_millis(500));
    let state = fake.state.lock().unwrap();
    assert!(state.history.contains(&"input stop"));
    assert!(state.history.contains(&"output cancelled"));
    assert!(state.history.contains(&"input drop") && state.history.contains(&"output drop"));
}

#[test]
fn session_propagates_input_open_error() {
    let fake = Fake {
        fault: Fault::InputOpen,
        ..Fake::default()
    };
    let result = play_and_record(&fake, request(), &CancelToken::new(), &mut |_| {});
    assert!(matches!(result, Err(AudioError::Backend(s)) if s == "Input open"));
    let state = fake.state.lock().unwrap();
    assert!(!state.history.contains(&"output play"));
    if state.history.contains(&"output open") {
        assert!(state.history.contains(&"output drop"));
    }
}

#[test]
fn policy_falls_back_to_shared_on_unsupported_format() {
    let fake = Fake {
        fault: Fault::Unsupported,
        ..Fake::default()
    };
    let spec = StreamSpec {
        sample_rate: 1000,
        channels: 2,
    };
    assert_eq!(
        open_output_with_policy(&fake, &endpoint(), spec).unwrap().1,
        ShareMode::SharedAutoConvert
    );
    assert_eq!(
        open_input_with_policy(&fake, &endpoint(), spec).unwrap().1,
        ShareMode::SharedAutoConvert
    );
    assert_eq!(
        fake.state.lock().unwrap().attempts,
        [
            (Direction::Output, ShareMode::Exclusive),
            (Direction::Output, ShareMode::SharedAutoConvert),
            (Direction::Input, ShareMode::Exclusive),
            (Direction::Input, ShareMode::SharedAutoConvert)
        ]
    );
}

#[test]
fn policy_does_not_retry_on_other_errors() {
    for fault in [Fault::Missing, Fault::InputOpen, Fault::OutputOpen] {
        let fake = Fake {
            fault,
            ..Fake::default()
        };
        let spec = StreamSpec {
            sample_rate: 1000,
            channels: 2,
        };
        let output = open_output_with_policy(&fake, &endpoint(), spec);
        let input = open_input_with_policy(&fake, &endpoint(), spec);
        if fault != Fault::InputOpen {
            assert!(output.is_err());
        }
        if fault != Fault::OutputOpen {
            assert!(input.is_err());
        }
        assert_eq!(fake.state.lock().unwrap().attempts.len(), 2);
        assert!(
            fake.state
                .lock()
                .unwrap()
                .attempts
                .iter()
                .all(|(_, mode)| *mode == ShareMode::Exclusive)
        );
    }
}

#[test]
fn session_errors_and_panics_stop_input_and_join_workers() {
    for fault in [
        Fault::OutputOpen,
        Fault::Start,
        Fault::Read,
        Fault::Stop,
        Fault::Play,
        Fault::ReadPanic,
        Fault::PlayPanic,
        Fault::Count,
        Fault::Report,
    ] {
        let fake = Fake {
            fault,
            ..Fake::default()
        };
        let result = play_and_record(&fake, request(), &CancelToken::new(), &mut |_| {});
        assert!(
            matches!(result, Err(AudioError::Backend(_))),
            "{fault:?}: {result:?}"
        );
        let state = fake.state.lock().unwrap();
        // Output initialization can fail before input starts opening. Every
        // input that did open must still stop and drop on its owning worker.
        if state.history.contains(&"input drop") {
            assert!(state.history.contains(&"input stop"), "{fault:?}");
        } else {
            assert_eq!(fault, Fault::OutputOpen);
        }
        if fault == Fault::Read {
            assert!(matches!(result, Err(AudioError::Backend(s)) if s == "read"));
        }
    }
    let fake = Fake {
        fault: Fault::OpenPanic,
        ..Fake::default()
    };
    assert!(matches!(
        play_and_record(&fake, request(), &CancelToken::new(), &mut |_| {}),
        Err(AudioError::Backend(_))
    ));
}

#[test]
fn session_validation_precedes_device_open() {
    let fake = Fake::default();
    let mut invalids = Vec::new();
    for tail in [-1.0, f64::NAN, f64::INFINITY, f64::MAX, usize::MAX as f64] {
        let mut req = request();
        req.tail_seconds = tail;
        invalids.push(req);
    }
    let mut req = request();
    req.playback.sample_rate = 0;
    invalids.push(req);
    let mut req = request();
    req.playback.channels = 0;
    invalids.push(req);
    let mut req = request();
    req.input_channels = 0;
    invalids.push(req);
    let mut req = request();
    req.input_channels = 3;
    invalids.push(req);
    let mut req = request();
    req.playback.channels = 3;
    invalids.push(req);
    let mut req = request();
    req.playback.interleaved.pop();
    invalids.push(req);
    let mut req = request();
    req.playback.interleaved[0] = f32::NAN;
    invalids.push(req);
    for req in invalids {
        assert!(matches!(
            play_and_record(&fake, req, &CancelToken::new(), &mut |_| {}),
            Err(AudioError::UnsupportedFormat(_))
        ));
    }
    assert!(fake.state.lock().unwrap().attempts.is_empty());
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(
        play_and_record(&fake, request(), &cancel, &mut |_| {}),
        Err(AudioError::Cancelled)
    ));
}

#[test]
fn session_event_panic_cancels_and_joins() {
    let fake = Fake {
        duration: Duration::from_secs(1),
        ..Fake::default()
    };
    let result = std::panic::catch_unwind(|| {
        play_and_record(&fake, request(), &CancelToken::new(), &mut |e| {
            if matches!(e, SessionEvent::OutputStarted { .. }) {
                panic!("observer panic");
            }
        })
    });
    assert!(result.is_err());
    let state = fake.state.lock().unwrap();
    assert!(state.history.contains(&"input stop"));
    assert!(state.history.contains(&"input drop") && state.history.contains(&"output drop"));
}

#[test]
fn concurrent_initialization_handshake_and_cleanup() {
    use std::sync::{Condvar, mpsc};
    for scenario in 0..4 {
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let (entered_tx, entered_rx) = mpsc::channel();
        let token = CancelToken::new();
        let fake = Fake {
            open_hook: Some(Arc::new({
                let gate = gate.clone();
                move |direction| {
                    entered_tx.send(direction).unwrap();
                    let (lock, cv) = &*gate;
                    let mut released = lock.lock().unwrap();
                    while !*released {
                        released = cv.wait(released).unwrap();
                    }
                    drop(released);
                    if scenario == 3 && direction == Direction::Output {
                        panic!("controlled output initialization panic");
                    }
                }
            })),
            fault: if scenario == 1 {
                Fault::InputOpen
            } else {
                Fault::None
            },
            ..Fake::default()
        };
        thread::scope(|scope| {
            let worker = scope.spawn(|| play_and_record(&fake, request(), &token, &mut |_| {}));
            // Both must enter open before either is permitted to finish it.
            // Timeout is only a deadlock watchdog, not a performance threshold;
            // always release the gate before asserting, even on regression.
            let first = entered_rx.recv_timeout(Duration::from_secs(5));
            let second = entered_rx.recv_timeout(Duration::from_secs(5));
            if scenario == 2 {
                token.cancel();
            }
            let (lock, cv) = &*gate;
            *lock.lock().unwrap() = true;
            cv.notify_all();
            let result = worker.join().unwrap();
            assert!(first.is_ok() && second.is_ok(), "opens did not overlap");
            assert_ne!(first.unwrap(), second.unwrap());
            assert_eq!(result.is_ok(), scenario == 0);
            if scenario == 2 {
                assert!(matches!(result, Err(AudioError::Cancelled)));
            }
        });
        let state = fake.state.lock().unwrap();
        if scenario != 3 {
            assert!(
                state.history.contains(&"output drop"),
                "scenario {scenario}"
            );
        }
        if scenario != 1 {
            assert!(state.history.contains(&"input stop"));
            assert!(state.history.contains(&"input drop"));
        }
        if scenario == 1 || scenario == 3 {
            assert!(!state.history.contains(&"output play"));
        }
    }
}

#[test]
fn session_zero_length_still_orders_start_drain_stop() {
    let mut req = request();
    req.playback.interleaved.clear();
    let mut events = Vec::new();
    let recording = play_and_record(&Fake::default(), req, &CancelToken::new(), &mut |e| {
        events.push(e)
    })
    .unwrap();
    assert_eq!(recording.capture.frames, 0);
    assert_eq!(events.len(), 4);
    assert!(matches!(events[2], SessionEvent::OutputDrained(_)));
    assert!(matches!(
        events[3],
        SessionEvent::InputStopped { frames: 0 }
    ));
}
