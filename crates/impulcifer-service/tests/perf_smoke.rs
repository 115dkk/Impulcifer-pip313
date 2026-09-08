#![forbid(unsafe_code)]
#[path = "../examples/demo_brir.rs"]
mod demo;
#[path = "bench_support/service.rs"]
#[allow(dead_code)]
mod service;

#[test]
fn bench_smoke_impulcifer_service() {
    let fixture = service::Fixture::new();
    for (op, _) in service::OPS {
        assert!(fixture.sample(op, 1, true) > 0.0);
    }
}

#[test]
fn bench_smoke_pipeline_demo() {
    for vbass in [false, true] {
        let dir = demo::DemoCopy::new(&demo::root().join("data/demo"), true).unwrap();
        assert!(dir.0.join("FL,FR.wav").is_file());
        assert!(!dir.0.join("FC.wav").exists());
        assert!(!dir.0.join("BL,SL.wav").exists());
        assert!(!dir.0.join("SR,BR.wav").exists());
        let result = demo::run(&dir.0, vbass).unwrap();
        assert!(result["seconds"].as_f64().unwrap() > 0.0);
        let wav = impulcifer_io::read_wav(&dir.0.join("hesuvi.wav")).unwrap();
        assert_eq!(wav.sample_rate, 48000);
        assert_eq!(wav.tracks.len(), 14);
        assert!(wav.tracks[0].len() > 24000);
    }
}
