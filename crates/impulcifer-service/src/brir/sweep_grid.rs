//! core/sweep_detection.py, including edited-recording envelope fallback.
use super::{
    BrirError,
    discovery::{listing, recording_speakers},
};
use impulcifer_io::read_wav;
use serde_json::{Value, json};
use std::path::Path;

pub fn snap_sweep_samples(estimate: f64, fs: u32) -> (usize, usize, f64) {
    let p = (fs as f64 / 10.0).log2().ceil();
    let power = 2.0_f64.powf(p);
    let unit = 2.0 * power.ln() * power;
    let m = (estimate / unit).round_ties_even().max(1.0) as usize;
    (
        m,
        (m as f64 * unit).round_ties_even() as usize,
        (estimate - m as f64 * unit).abs() / unit,
    )
}
#[derive(Clone, Debug)]
pub struct SweepDetection {
    pub fs: u32,
    pub m: usize,
    pub sweep_samples: usize,
    pub n_segments: usize,
    pub speakers: Vec<String>,
    pub confidence: &'static str,
    pub deviation: f64,
    pub source_files: Vec<String>,
}
impl SweepDetection {
    pub fn duration(&self) -> f64 {
        self.sweep_samples as f64 / self.fs as f64
    }
    pub fn is_default(&self) -> bool {
        self.fs == 48000 && self.m == 2
    }
    pub fn generate_spec(&self) -> String {
        format!("generate:{:.2}s@{}", self.duration(), self.fs)
    }
    pub fn payload(&self, sidecar: bool) -> Value {
        json!({"found":true,"sidecar":sidecar,"fs":self.fs,"duration_seconds":(self.duration()*10000.0).round_ties_even()/10000.0,
            "n_segments":self.n_segments,"speakers":self.speakers,"confidence":self.confidence,"is_default":self.is_default(),"generate_spec":self.generate_spec(),"source_files":self.source_files})
    }
}
fn envelope_estimate(tracks: &[Vec<f64>], fs: u32) -> Option<(f64, usize)> {
    let n = tracks.first()?.len();
    if n == 0 {
        return None;
    }
    let kernel = ((0.05 * fs as f64) as usize).max(1);
    let values: Vec<_> = (0..n)
        .map(|i| tracks.iter().map(|t| t[i].abs()).fold(0.0, f64::max))
        .collect();
    let mut prefix = Vec::with_capacity(n + 1);
    prefix.push(0.0);
    for x in values {
        prefix.push(prefix.last().unwrap() + x);
    }
    let smoothed: Vec<_> = (0..n)
        .map(|i| {
            let start = i.saturating_sub(kernel / 2);
            let end = (i + kernel.div_ceil(2)).min(n);
            (prefix[end] - prefix[start]) / kernel as f64
        })
        .collect();
    let peak = smoothed.iter().copied().fold(0.0, f64::max);
    if peak <= 0.0 {
        return None;
    }
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut start = None;
    for (i, sample) in smoothed
        .iter()
        .copied()
        .chain(std::iter::once(0.0))
        .enumerate()
    {
        let active = i < n && sample > peak * 0.01;
        if active && start.is_none() {
            start = Some(i);
        }
        if !active && let Some(s) = start.take() {
            if let Some(last) = runs.last_mut()
                && ((s - last.1) as f64) < 0.3 * fs as f64
            {
                last.1 = i;
            } else {
                runs.push((s, i));
            }
        }
    }
    runs.retain(|(s, e)| (e - s) as f64 >= 0.5 * fs as f64);
    if runs.len() >= 2 {
        let mut intervals: Vec<_> = runs.windows(2).map(|w| (w[1].0 - w[0].0) as f64).collect();
        intervals.sort_by(f64::total_cmp);
        let k = intervals.len();
        Some((
            (intervals[(k - 1) / 2] + intervals[k / 2]) / 2.0 - 2.0 * fs as f64,
            runs.len(),
        ))
    } else {
        runs.first().map(|(s, e)| ((e - s) as f64, 1))
    }
}
pub fn detect_sweep_parameters(dir: &Path) -> Result<Option<SweepDetection>, BrirError> {
    let files: Vec<_> = listing(dir)?
        .into_iter()
        .filter(|n| recording_speakers(n).is_some())
        .collect();
    let mut speakers = Vec::new();
    let mut results = Vec::new();
    for file in &files {
        let names = recording_speakers(file).unwrap();
        let count = names.len();
        speakers.extend(names);
        let Ok(wav) = read_wav(&dir.join(file)) else {
            continue;
        };
        let fs = wav.sample_rate;
        let estimate =
            (wav.tracks[0].len() as f64 - 2.0 * fs as f64 * (count + 1) as f64) / count as f64;
        let (mut m, mut n, mut deviation) = snap_sweep_samples(estimate, fs);
        let mut segments = count;
        if estimate <= 0.0 || deviation > 0.15 {
            if let Some((estimate, count)) = envelope_estimate(&wav.tracks, fs) {
                (m, n, deviation) = snap_sweep_samples(estimate, fs);
                segments = count;
            } else if estimate <= 0.0 {
                continue;
            }
        }
        results.push((fs, m, n, deviation, segments));
    }
    let Some(&(fs, m, n, _, _)) = results.first() else {
        return Ok(None);
    };
    let deviation = results.iter().map(|r| r.3).fold(0.0, f64::max);
    let high = deviation <= 0.15 && results.iter().all(|r| r.0 == fs && r.1 == m);
    Ok(Some(SweepDetection {
        fs,
        m,
        sweep_samples: n,
        n_segments: results.iter().map(|r| r.4).sum(),
        speakers,
        confidence: if high { "high" } else { "low" },
        deviation,
        source_files: files,
    }))
}
