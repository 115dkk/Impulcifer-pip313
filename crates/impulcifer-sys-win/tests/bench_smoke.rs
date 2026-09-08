#![forbid(unsafe_code)]
#![cfg(windows)]

mod bench_support;

#[test]
fn bench_smoke_impulcifer_sys_win() {
    let endpoints = bench_support::enumerate_devices().expect("WASAPI enumeration");
    // Device-less Windows CI is valid; errors are not silently converted to emptiness.
    for endpoint in endpoints {
        assert_eq!(endpoint.host_api, "Windows WASAPI");
        assert!(!endpoint.id.is_empty());
    }
}
