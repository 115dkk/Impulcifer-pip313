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
