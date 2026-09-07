#![forbid(unsafe_code)]
//! Compile and execute the actual PA02 benchmark operations at smoke sizes.
#[path = "../benches/perf.rs"]
mod perf;

#[test]
fn bench_smoke_impulcifer_dsp() {
    perf::smoke();
}
