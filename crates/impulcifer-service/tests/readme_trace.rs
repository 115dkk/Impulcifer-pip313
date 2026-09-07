#![forbid(unsafe_code)]
//! Diagnostic trace, not a substitute for the strict README byte gates.
mod brir_support;
use brir_support::*;
use impulcifer_dsp::{
    DspError,
    ir::ImpulseResponse,
    pipeline::{StageObserver, StageProgress, run_pipeline},
};
use impulcifer_service::brir::{
    BrirError, BrirEvents, discovery::discover, estimator::open_estimator, inputs::load_inputs,
};
use impulcifer_types::config::ProcessingConfig;
use serde_json::Value;
struct Quiet;
impl BrirEvents for Quiet {
    fn step(&mut self, _: &str, _: Value) -> Result<(), BrirError> {
        Ok(())
    }
    fn log(&mut self, _: &str, _: &str, _: Value) {}
    fn check_cancelled(&self) -> Result<(), BrirError> {
        Ok(())
    }
}
impl StageObserver for Quiet {
    fn on_stage(&mut self, _: StageProgress) {}
    fn check_cancelled(&self) -> Result<(), DspError> {
        Ok(())
    }
}
#[test]
fn readme_numerical_trace_on_identical_oracle_samples() {
    for scenario in ["default", "vbass"] {
        let temp = Temp::demo();
        let config = ProcessingConfig {
            dir_path: Some(temp.0.to_string_lossy().into_owned()),
            vbass: scenario == "vbass",
            ..Default::default()
        };
        let dir = discover(&temp.0, &config).unwrap();
        let auto = open_estimator(&dir, None).unwrap();
        let explicit = open_estimator(
            &dir,
            Some(
                root()
                    .join("data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")
                    .to_str()
                    .unwrap(),
            ),
        )
        .unwrap();
        assert_eq!(auto.test_signal, explicit.test_signal);
        assert_eq!(auto.inverse_filter, explicit.inverse_filter);
        let inputs = load_inputs(&dir, &explicit, &config, &mut Quiet).unwrap();
        println!(
            "TRACE {scenario} recordings={:?} fs={} sweep_samples={} room={} headphone={} eq_left={} eq_right={}",
            dir.recordings
                .iter()
                .map(|(p, _)| p.file_name().unwrap())
                .collect::<Vec<_>>(),
            explicit.fs,
            explicit.test_signal.len(),
            inputs.room.is_some(),
            inputs.headphone.is_some(),
            inputs.eq_left.is_some(),
            inputs.eq_right.is_some()
        );
        let mut staged = inputs.hrir.clone();
        staged.crop_heads(config.head_ms).unwrap();
        staged.align_ipsilateral_all(&impulcifer_types::constants::IPSILATERAL_PAIRS, 30.0);
        staged.align_onset_groups_peak_leftref(None).unwrap();
        staged.crop_tails(&explicit).unwrap();
        let compare = |name: &str, hrir: &impulcifer_dsp::hrir::Hrir| {
            let bytes =
                std::fs::read(golden(&format!("p11_{scenario}_{name}_FR-left.f64"))).unwrap();
            let reference: Vec<_> = bytes
                .chunks_exact(8)
                .map(|b| f64::from_le_bytes(b.try_into().unwrap()))
                .collect();
            let values = &hrir.get("FR").unwrap().left.as_ref().unwrap().data;
            assert_eq!(values.len(), reference.len());
            let max = values
                .iter()
                .zip(&reference)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f64::max);
            println!(
                "TRACE {scenario} stage={name} FR-left samples={} max_error={max:.12e}",
                values.len()
            );
        };
        compare("crop_and_align", &staged);
        if config.vbass {
            impulcifer_dsp::virtual_bass::apply_virtual_bass(
                &mut staged,
                &impulcifer_dsp::virtual_bass::VirtualBassOptions {
                    crossover_freq: config.vbass_freq,
                    head_ms: config.head_ms,
                    hp_freq: config.vbass_hp,
                    invert_polarity: None,
                },
            )
            .unwrap();
            compare("virtual_bass", &staged);
        }
        let target = impulcifer_dsp::stages::target::create_target(
            explicit.fs,
            config.bass_boost_gain,
            config.bass_boost_fc,
            config.bass_boost_q,
            config.tilt,
        );
        let source_oracle: Value = serde_json::from_slice(
            &std::fs::read(golden(&format!("p11_{scenario}.json"))).unwrap(),
        )
        .unwrap();
        let room_fr = &inputs
            .room
            .as_ref()
            .unwrap()
            .frs
            .0
            .iter()
            .find(|(s, side, _)| s == "FR" && *side == impulcifer_types::constants::Side::Left)
            .unwrap()
            .2;
        for (name, values) in [
            ("room", &room_fr.error),
            ("headphone", &inputs.headphone.as_ref().unwrap().left.error),
            ("target", &target.raw),
        ] {
            let expected = source_oracle["eq_sources"][name].as_array().unwrap();
            assert_eq!(values.len(), expected.len());
            let error = values
                .iter()
                .zip(expected)
                .map(|(a, b)| (a - b.as_f64().unwrap()).abs())
                .fold(0.0, f64::max);
            println!("TRACE {scenario} EQ source={name} max_error={error:.12e}");
        }
        impulcifer_dsp::stages::equalize::equalize_hrir(
            &mut staged,
            &impulcifer_dsp::stages::equalize::EqInputs {
                room_frs: inputs.room.as_ref().map(|r| &r.frs),
                hp: inputs.headphone.as_ref(),
                eq_left: inputs.eq_left.as_ref(),
                eq_right: inputs.eq_right.as_ref(),
                target: &target,
                fs: explicit.fs,
            },
        )
        .unwrap();
        compare("equalize", &staged);
        let output = run_pipeline(&config, inputs, &mut Quiet).unwrap();
        let oracle: Value = serde_json::from_slice(
            &std::fs::read(golden(&format!("p11_{scenario}.json"))).unwrap(),
        )
        .unwrap();
        for row in oracle["readme_stats"].as_array().unwrap() {
            let speaker = row["speaker"].as_str().unwrap();
            let pair = output.hrir.get(speaker).unwrap();
            let ir = if row["side"] == "left" {
                pair.left.as_ref()
            } else {
                pair.right.as_ref()
            }
            .unwrap();
            let p = ir.decay_params();
            let peak = ir.peak_index(0, None, 0.12589);
            println!(
                "TRACE {scenario} {speaker}-{} Rust peak={peak} knee={} window={} noise={:.12} peak_value={:.12e} pnr={:.12}; Python {row}",
                row["side"],
                p.knee_index,
                p.window_size,
                p.noise_floor_db,
                ir.data[peak],
                20.0 * (ir.data[peak].abs() + 1e-9).log10() - p.noise_floor_db
            );
        }
        let bytes = std::fs::read(golden(&format!("p11_{scenario}_readme_FR-left.f64"))).unwrap();
        let ir = ImpulseResponse {
            fs: 48000,
            recording: None,
            data: bytes
                .chunks_exact(8)
                .map(|b| f64::from_le_bytes(b.try_into().unwrap()))
                .collect(),
        };
        let p = ir.decay_params();
        let peak = ir.peak_index(0, None, 0.12589);
        let expected = oracle["readme_stats"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["speaker"] == "FR" && r["side"] == "left")
            .unwrap();
        assert_eq!(peak as u64, expected["peak"].as_u64().unwrap());
        assert_eq!(p.knee_index as u64, expected["knee"].as_u64().unwrap());
        assert_eq!(p.window_size as u64, expected["window"].as_u64().unwrap());
        assert!((p.noise_floor_db - expected["noise"].as_f64().unwrap()).abs() < 1e-9);
        let rust = output.hrir.get("FR").unwrap().left.as_ref().unwrap();
        let error = rust
            .data
            .iter()
            .zip(&ir.data)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        println!(
            "TRACE {scenario} identical Python FR-left samples -> Rust peak={peak} knee={} window={} noise={:.12} pnr={:.12}; pipeline sample max_error={error:.12e}",
            p.knee_index,
            p.window_size,
            p.noise_floor_db,
            20.0 * (ir.data[peak].abs() + 1e-9).log10() - p.noise_floor_db
        );
    }
}
