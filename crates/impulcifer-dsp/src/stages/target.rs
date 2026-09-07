//! Pipeline bass/tilt target.
use crate::fr::{self, FrequencyResponse, generate_frequencies};
/// Python create_target, core/pipeline_stages.py:485-501; p10_target.
pub fn create_target(
    fs: u32,
    bass_boost_gain: f64,
    bass_boost_fc: f64,
    bass_boost_q: f64,
    tilt: f64,
) -> FrequencyResponse {
    let f = generate_frequencies(10.0, fs as f64 / 2.0, 1.01);
    let raw = fr::create_target(&f, bass_boost_gain, bass_boost_fc, bass_boost_q, Some(tilt));
    FrequencyResponse::new("bass_and_tilt", Some(f), Some(raw)).expect("valid target grid")
}
