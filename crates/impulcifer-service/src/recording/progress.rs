//! Flat ten-key recorder payloads, with exact generated or inferred segments.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SweepSegment {
    pub speaker: String,
    pub index: usize,
    pub total: usize,
    pub start: f64,
    pub end: f64,
}
impl SweepSegment {
    pub fn contains(&self, elapsed: f64) -> bool {
        self.start <= elapsed && elapsed < self.end
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RecorderProgressEvent {
    pub phase: String,
    pub elapsed: f64,
    pub duration: f64,
    pub progress: f64,
    pub speaker: Option<String>,
    pub segment_index: Option<usize>,
    pub segment_total: usize,
    pub segment_progress: Option<f64>,
    pub speakers: Vec<String>,
    pub message: String,
}
pub fn infer_sweep_segments(play_file: &str, total_duration: f64) -> Vec<SweepSegment> {
    impulcifer_io::sweep_files::infer_sweep_segments(play_file, total_duration)
        .into_iter()
        .map(|s| SweepSegment {
            speaker: s.speaker,
            index: s.index,
            total: s.total,
            start: s.start,
            end: s.end,
        })
        .collect()
}
pub fn event_for_elapsed(
    elapsed: f64,
    duration: f64,
    segments: &[SweepSegment],
) -> RecorderProgressEvent {
    let elapsed = elapsed.max(0.0);
    let duration = duration.max(0.0);
    let mut event = RecorderProgressEvent {
        phase: "recording".into(),
        elapsed,
        duration,
        progress: if duration > 0.0 {
            (elapsed / duration).min(0.98)
        } else {
            0.0
        },
        segment_total: segments.len(),
        speakers: segments.iter().map(|s| s.speaker.clone()).collect(),
        ..Default::default()
    };
    if let Some(s) = segments.iter().find(|s| s.contains(elapsed)) {
        event.speaker = Some(s.speaker.clone());
        event.segment_index = Some(s.index);
        event.segment_total = s.total;
        event.segment_progress =
            Some(((elapsed - s.start) / (s.end - s.start).max(0.001)).clamp(0.0, 1.0));
    }
    event
}
