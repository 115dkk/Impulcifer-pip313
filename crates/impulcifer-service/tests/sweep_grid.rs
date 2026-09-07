#![forbid(unsafe_code)]
mod brir_support;
#[test]
fn snap_sweep_samples_matches_python() {
    let values: serde_json::Value = serde_json::from_slice(
        &std::fs::read(brir_support::golden("p11_sweep_grid.json")).unwrap(),
    )
    .unwrap();
    for v in values.as_array().unwrap() {
        let (m, n, d) = impulcifer_service::brir::sweep_grid::snap_sweep_samples(
            v["estimate"].as_f64().unwrap(),
            v["fs"].as_u64().unwrap() as u32,
        );
        assert_eq!(m, v["m"].as_u64().unwrap() as usize);
        assert_eq!(n, v["n"].as_u64().unwrap() as usize);
        assert!((d - v["deviation"].as_f64().unwrap()).abs() < 1e-12);
    }
}
