use super::{
    RecordingError,
    naming::{normalize_speakers, record_filename_for_speakers},
    progress::SweepSegment,
};
use impulcifer_dsp::estimator::SweepEstimator;
use impulcifer_types::constants::{SEQUENCE_TRACK_ORDERS, SWEEP_TRACK_LAYOUTS};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SweepSpec {
    pub fs: u32,
    pub duration: f64,
    pub speakers: Vec<String>,
    pub tracks: String,
}
impl Default for SweepSpec {
    fn default() -> Self {
        Self {
            fs: 48000,
            duration: 5.0,
            speakers: vec!["FL".into(), "FR".into()],
            tracks: "stereo".into(),
        }
    }
}
impl SweepSpec {
    pub fn is_default_signal(&self) -> bool {
        self.fs == 48000 && self.duration == 5.0
    }
}
pub fn validate_sweep_spec(mut spec: SweepSpec) -> Result<SweepSpec, String> {
    spec.speakers = normalize_speakers(&spec.speakers)?;
    if !SWEEP_TRACK_LAYOUTS.contains(&spec.tracks.as_str()) {
        return Err(format!(
            "Unsupported track configuration \"{}\". Supported: {}.",
            spec.tracks,
            SWEEP_TRACK_LAYOUTS.join(", ")
        ));
    }
    if spec.tracks == "stereo" && spec.speakers.len() > 2 {
        return Err("\"stereo\" track configuration requires one or two speakers.".into());
    }
    if let Some((_, order)) = SEQUENCE_TRACK_ORDERS
        .iter()
        .find(|(layout, _)| *layout == spec.tracks)
    {
        for speaker in &spec.speakers {
            if !order.contains(&speaker.as_str()) {
                return Err(format!(
                    "Speaker \"{speaker}\" is not available in the \"{}\" layout.",
                    spec.tracks
                ));
            }
        }
    }
    if !(8000..=384000).contains(&spec.fs) {
        return Err("Sampling rate must be between 8000 and 384000 Hz.".into());
    }
    if !(0.1..=60.0).contains(&spec.duration) {
        return Err("Sweep duration must be between 0.1 and 60 seconds.".into());
    }
    Ok(spec)
}
pub struct SweepPlayback {
    pub estimator: SweepEstimator,
    pub spec: SweepSpec,
    pub tracks: Vec<Vec<f64>>,
    pub segments: Vec<SweepSegment>,
    pub record_filename: String,
    pub display_name: String,
}
pub fn build_sweep_playback(spec: &SweepSpec) -> Result<SweepPlayback, RecordingError> {
    let mut spec = validate_sweep_spec(spec.clone()).map_err(RecordingError::internal)?;
    let estimator =
        SweepEstimator::new(spec.duration, spec.fs).map_err(RecordingError::internal)?;
    let names: Vec<_> = spec.speakers.iter().map(String::as_str).collect();
    let tracks = impulcifer_io::wav::pcm32_round_trip(
        &estimator
            .sweep_sequence(&names, &spec.tracks)
            .map_err(RecordingError::internal)?,
    );
    if spec.tracks == "mono" {
        spec.speakers = vec!["FL".into()];
    }
    let segments = spec
        .speakers
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let start = 2.0 + i as f64 * (estimator.duration + 2.0);
            SweepSegment {
                speaker: s.clone(),
                index: i + 1,
                total: spec.speakers.len(),
                start,
                end: start + estimator.duration,
            }
        })
        .collect();
    let display_name = format!(
        "sweep-seg-{}-{}-{} (generated)",
        spec.speakers.join(","),
        spec.tracks,
        estimator.file_name(32)
    );
    let record_filename =
        record_filename_for_speakers(&spec.speakers).map_err(RecordingError::internal)?;
    Ok(SweepPlayback {
        estimator,
        spec,
        tracks,
        segments,
        record_filename,
        display_name,
    })
}
pub fn write_sidecar(dir: &Path, estimator: &SweepEstimator) -> Result<String, RecordingError> {
    let path = dir.join("test.wav");
    impulcifer_io::write_wav(
        &path,
        estimator.fs,
        std::slice::from_ref(&estimator.test_signal),
        32,
    )
    .map_err(RecordingError::internal)?;
    Ok(path.to_string_lossy().into_owned())
}
