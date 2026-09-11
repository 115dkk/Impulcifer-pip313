//! Per-backend endpoint snapshots and exclusive format refusal policy memory.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use impulcifer_types::audio::{
    AudioBackend, AudioError, Direction, Endpoint, InputSession, OutputSession, ProbeResult,
    ShareMode, StreamSpec,
};

const TTL: Duration = Duration::from_secs(2);
type Clock = Box<dyn Fn() -> Duration + Send + Sync>;

struct Snapshot {
    taken: Duration,
    endpoints: Vec<Endpoint>,
}
#[derive(PartialEq, Eq)]
struct RefusalKey {
    id: String,
    host: String,
    direction: Direction,
    spec: StreamSpec,
}

/// Adds a two-second endpoint snapshot and instance-local exclusive format memory.
/// Only DTOs are retained: sessions and COM objects remain on their calling thread.
/// Explicit exclusive requests still return the original refusal, never shared sessions.
pub struct CachedBackend {
    inner: Box<dyn AudioBackend>,
    snapshot: Mutex<Option<Snapshot>>,
    refusals: Mutex<Vec<(RefusalKey, String)>>,
    clock: Clock,
}
impl CachedBackend {
    pub fn new(inner: Box<dyn AudioBackend>) -> Self {
        let epoch = Instant::now();
        Self::with_clock(inner, Box::new(move || epoch.elapsed()))
    }
    fn with_clock(inner: Box<dyn AudioBackend>, clock: Clock) -> Self {
        Self {
            inner,
            snapshot: Mutex::new(None),
            refusals: Mutex::new(Vec::new()),
            clock,
        }
    }
    fn open<T>(
        &self,
        endpoint: &Endpoint,
        direction: Direction,
        spec: StreamSpec,
        mode: ShareMode,
        operation: impl FnOnce() -> Result<T, AudioError>,
    ) -> Result<T, AudioError> {
        if mode != ShareMode::Exclusive {
            return operation();
        }
        let key = RefusalKey {
            id: endpoint.id.clone(),
            host: endpoint.host_api.clone(),
            direction,
            spec,
        };
        {
            let refusals = self
                .refusals
                .lock()
                .map_err(|_| AudioError::Backend("refusal cache poisoned".into()))?;
            if let Some((_, detail)) = refusals.iter().find(|(k, _)| *k == key) {
                return Err(AudioError::UnsupportedFormat(detail.clone()));
            }
        }
        // Do not hold a shared lock while opening thread-affine sessions.
        let result = operation();
        if let Err(AudioError::UnsupportedFormat(detail)) = &result {
            let mut refusals = self
                .refusals
                .lock()
                .map_err(|_| AudioError::Backend("refusal cache poisoned".into()))?;
            if !refusals.iter().any(|(k, _)| *k == key) {
                refusals.push((key, detail.clone()));
            }
        }
        result
    }
}
impl AudioBackend for CachedBackend {
    fn name(&self) -> &'static str {
        self.inner.name()
    }
    fn selectable_share_modes(&self) -> &'static [ShareMode] {
        self.inner.selectable_share_modes()
    }
    fn enumerate(&self) -> Result<Vec<Endpoint>, AudioError> {
        let mut cache = self
            .snapshot
            .lock()
            .map_err(|_| AudioError::Backend("endpoint cache poisoned".into()))?;
        let now = (self.clock)();
        if let Some(snapshot) = cache.as_ref()
            && now.saturating_sub(snapshot.taken) < TTL
        {
            return Ok(snapshot.endpoints.clone());
        }
        // Invalidate before refresh, so failures cannot perpetuate stale devices.
        *cache = None;
        let endpoints = self.inner.enumerate()?;
        // Age from refresh entry, not completion, to keep the hard freshness bound.
        *cache = Some(Snapshot {
            taken: now,
            endpoints: endpoints.clone(),
        });
        Ok(endpoints)
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
        self.open(endpoint, Direction::Output, spec, mode, || {
            self.inner.open_output(endpoint, spec, mode)
        })
    }
    fn open_input(
        &self,
        endpoint: &Endpoint,
        spec: StreamSpec,
        mode: ShareMode,
    ) -> Result<Box<dyn InputSession>, AudioError> {
        self.open(endpoint, Direction::Input, spec, mode, || {
            self.inner.open_input(endpoint, spec, mode)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

    #[derive(Default)]
    struct Fake {
        enumerations: AtomicUsize,
        opens: AtomicUsize,
        fail: AtomicBool,
    }
    fn endpoint() -> Endpoint {
        Endpoint {
            id: "one".into(),
            name: "one".into(),
            host_api: "fake".into(),
            max_input_channels: 2,
            max_output_channels: 2,
            default_samplerate: 48000.0,
            is_default_input: false,
            is_default_output: false,
        }
    }
    struct FakeBackend(Arc<Fake>);
    impl std::ops::Deref for FakeBackend {
        type Target = Fake;
        fn deref(&self) -> &Fake {
            &self.0
        }
    }
    impl AudioBackend for FakeBackend {
        fn name(&self) -> &'static str {
            "fake"
        }
        fn enumerate(&self) -> Result<Vec<Endpoint>, AudioError> {
            let n = self.enumerations.fetch_add(1, Ordering::SeqCst);
            if self.fail.load(Ordering::SeqCst) {
                return Err(AudioError::Backend("transient".into()));
            }
            let mut e = endpoint();
            e.name = n.to_string();
            Ok(vec![e])
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
        fn open_input(
            &self,
            _: &Endpoint,
            _: StreamSpec,
            mode: ShareMode,
        ) -> Result<Box<dyn InputSession>, AudioError> {
            self.opens.fetch_add(1, Ordering::SeqCst);
            if self.fail.load(Ordering::SeqCst) {
                Err(AudioError::Backend("transient".into()))
            } else if mode == ShareMode::Exclusive {
                Err(AudioError::UnsupportedFormat("exact refusal".into()))
            } else {
                Ok(Box::new(Input))
            }
        }
        fn open_output(
            &self,
            _: &Endpoint,
            _: StreamSpec,
            _: ShareMode,
        ) -> Result<Box<dyn OutputSession>, AudioError> {
            self.opens.fetch_add(1, Ordering::SeqCst);
            Err(AudioError::UnsupportedFormat("output refusal".into()))
        }
    }
    struct Input;
    impl InputSession for Input {
        fn start(&mut self) -> Result<(), AudioError> {
            Ok(())
        }
        fn stop(&mut self) -> Result<(), AudioError> {
            Ok(())
        }
        fn read_into(
            &mut self,
            _: &mut [f32],
            _: &impulcifer_types::audio::CancelToken,
        ) -> Result<impulcifer_types::audio::CaptureRead, AudioError> {
            unreachable!()
        }
    }
    fn cached(fake: Arc<Fake>, time: Arc<AtomicU64>) -> CachedBackend {
        CachedBackend::with_clock(
            Box::new(FakeBackend(fake)),
            Box::new(move || Duration::from_millis(time.load(Ordering::SeqCst))),
        )
    }
    #[test]
    fn ttl_exact_boundary_freshness_failure_and_cloned_snapshot() {
        let fake = Arc::new(Fake::default());
        let time = Arc::new(AtomicU64::new(0));
        let backend = cached(fake.clone(), time.clone());
        let mut first = backend.enumerate().unwrap();
        first[0].name = "mutated".into();
        time.store(1999, Ordering::SeqCst);
        assert_eq!(backend.enumerate().unwrap()[0].name, "0");
        time.store(2000, Ordering::SeqCst);
        assert_eq!(backend.enumerate().unwrap()[0].name, "1");
        time.store(4000, Ordering::SeqCst);
        fake.fail.store(true, Ordering::SeqCst);
        assert!(backend.enumerate().is_err());
        assert!(backend.enumerate().is_err());
        fake.fail.store(false, Ordering::SeqCst);
        assert_eq!(backend.enumerate().unwrap()[0].name, "4");
    }
    #[test]
    fn caches_and_refusals_are_instance_local() {
        let fake = Arc::new(Fake::default());
        let time = Arc::new(AtomicU64::new(0));
        let a = cached(fake.clone(), time.clone());
        let b = cached(fake.clone(), time);
        assert_ne!(a.enumerate().unwrap(), b.enumerate().unwrap());
        let spec = StreamSpec {
            sample_rate: 48000,
            channels: 2,
        };
        assert!(
            a.open_input(&endpoint(), spec, ShareMode::Exclusive)
                .is_err()
        );
        assert!(
            b.open_input(&endpoint(), spec, ShareMode::Exclusive)
                .is_err()
        );
        assert_eq!(fake.opens.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn refusal_keys_include_endpoint_host_direction_rate_channels_and_policy_metadata() {
        let fake = Arc::new(Fake::default());
        let backend = cached(fake.clone(), Arc::new(AtomicU64::new(0)));
        let e = endpoint();
        let spec = StreamSpec {
            sample_rate: 48000,
            channels: 2,
        };
        for _ in 0..2 {
            assert!(
                matches!(backend.open_input(&e, spec, ShareMode::Exclusive), Err(AudioError::UnsupportedFormat(s)) if s == "exact refusal")
            );
            let (_, mode) = crate::policy::open_input_with_policy(&backend, &e, spec).unwrap();
            assert_eq!(mode, ShareMode::SharedAutoConvert);
        }
        assert_eq!(fake.opens.load(Ordering::SeqCst), 3);
        let mut other = e.clone();
        other.id = "two".into();
        assert!(
            backend
                .open_input(&other, spec, ShareMode::Exclusive)
                .is_err()
        );
        other = e.clone();
        other.host_api = "other".into();
        assert!(
            backend
                .open_input(&other, spec, ShareMode::Exclusive)
                .is_err()
        );
        assert!(backend.open_output(&e, spec, ShareMode::Exclusive).is_err());
        assert!(
            backend
                .open_input(
                    &e,
                    StreamSpec {
                        sample_rate: 96000,
                        ..spec
                    },
                    ShareMode::Exclusive
                )
                .is_err()
        );
        assert!(
            backend
                .open_input(
                    &e,
                    StreamSpec {
                        channels: 1,
                        ..spec
                    },
                    ShareMode::Exclusive
                )
                .is_err()
        );
        assert_eq!(fake.opens.load(Ordering::SeqCst), 8);
        assert!(
            backend
                .probe(&e, Direction::Input, spec, ShareMode::Exclusive)
                .unwrap()
                .supported
        );
    }
    #[test]
    fn transient_errors_are_never_memoized() {
        let fake = Arc::new(Fake::default());
        fake.fail.store(true, Ordering::SeqCst);
        let backend = cached(fake.clone(), Arc::new(AtomicU64::new(0)));
        let spec = StreamSpec {
            sample_rate: 48000,
            channels: 2,
        };
        for _ in 0..2 {
            assert!(matches!(
                backend.open_input(&endpoint(), spec, ShareMode::Exclusive),
                Err(AudioError::Backend(_))
            ));
        }
        assert_eq!(fake.opens.load(Ordering::SeqCst), 2);
        fake.fail.store(false, Ordering::SeqCst);
        assert_eq!(
            crate::policy::open_input_with_policy(&backend, &endpoint(), spec)
                .unwrap()
                .1,
            ShareMode::SharedAutoConvert
        );
    }
}
