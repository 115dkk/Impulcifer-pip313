//! Opt-in, per-thread timings; no clocks or allocations in the disabled path.
use std::{
    cell::RefCell,
    collections::BTreeMap,
    time::{Duration, Instant},
};
thread_local! {
    static TIMES: RefCell<Option<BTreeMap<&'static str, (Duration, usize)>>> = const { RefCell::new(None) };
}
pub fn start() {
    TIMES.with(|t| *t.borrow_mut() = Some(BTreeMap::new()));
}
pub fn finish() -> BTreeMap<&'static str, (Duration, usize)> {
    TIMES.with(|t| t.borrow_mut().take().unwrap_or_default())
}
pub fn enabled() -> bool {
    TIMES.with(|t| t.borrow().is_some())
}
type Timings = BTreeMap<&'static str, (Duration, usize)>;
/// Save thread-local state: a one-thread join may execute inline.
pub fn capture<T>(enabled: bool, f: impl FnOnce() -> T) -> (T, Timings) {
    if !enabled {
        return (f(), BTreeMap::new());
    }
    struct Restore(Option<Timings>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TIMES.with(|t| *t.borrow_mut() = self.0.take());
        }
    }
    let previous = Restore(TIMES.with(|t| t.borrow_mut().replace(BTreeMap::new())));
    let result = f();
    let times = finish();
    drop(previous);
    (result, times)
}
pub fn merge(times: Timings) {
    TIMES.with(|t| {
        if let Some(all) = t.borrow_mut().as_mut() {
            for (key, (duration, count)) in times {
                let entry = all.entry(key).or_default();
                entry.0 += duration;
                entry.1 += count;
            }
        }
    });
}
pub struct Timer {
    name: &'static str,
    start: Option<Instant>,
}
pub fn timer(name: &'static str) -> Timer {
    Timer {
        name,
        start: TIMES.with(|t| t.borrow().as_ref().map(|_| Instant::now())),
    }
}
impl Drop for Timer {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            TIMES.with(|t| {
                if let Some(times) = t.borrow_mut().as_mut() {
                    let v = times.entry(self.name).or_default();
                    v.0 += start.elapsed();
                    v.1 += 1;
                }
            });
        }
    }
}
