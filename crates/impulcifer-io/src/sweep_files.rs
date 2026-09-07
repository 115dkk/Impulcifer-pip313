//! Sweep filename formatting/parsing and pure filename-based segment inference.

use impulcifer_types::constants::SPEAKER_NAMES;

#[derive(Debug, Clone, PartialEq)]
pub struct SweepFileInfo {
    pub speakers: Vec<String>,
    pub layout: String,
    pub duration_s: f64,
    pub fs: u32,
    pub bits: u16,
    pub f_lo: f64,
    pub f_hi: f64,
}

/// Uses the estimator's .2f duration/low frequency and .0f high frequency.
pub fn sweep_file_name(
    speakers: &[&str],
    layout: &str,
    duration_s: f64,
    fs: u32,
    bits: u16,
    f_lo: f64,
    f_hi: f64,
) -> String {
    format!(
        "sweep-seg-{}-{layout}-{duration_s:.2}s-{fs}Hz-{bits}bit-{f_lo:.2}Hz-{f_hi:.0}Hz.wav",
        speakers.join(",")
    )
}
fn decimal(value: &str) -> Option<f64> {
    let mut parts = value.split('.');
    let integer = parts.next()?;
    if integer.is_empty() || !integer.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if let Some(fraction) = parts.next()
        && (fraction.is_empty() || !fraction.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    if parts.next().is_some() {
        return None;
    }
    value.parse::<f64>().ok().filter(|n| n.is_finite())
}
fn speakers(value: &str) -> Option<Vec<String>> {
    value
        .split(',')
        .map(|s| {
            let name = s.to_ascii_uppercase();
            SPEAKER_NAMES.contains(&name.as_str()).then_some(name)
        })
        .collect()
}
fn basename(name: &str) -> &str {
    name.rsplit(['/', '\\']).next().unwrap_or(name)
}

/// Parses segmented names and the unsegmented bundled `sweep-...wav` name.
/// Unsegmented files have no speaker list or layout in their name.
pub fn parse_sweep_file_name(name: &str) -> Option<SweepFileInfo> {
    let name = basename(name).to_ascii_lowercase();
    let stem = name.strip_suffix(".wav")?;
    let (speakers, layout, suffix) = if let Some(seg) = stem.strip_prefix("sweep-seg-") {
        let mut parts = seg.splitn(3, '-');
        let speakers = speakers(parts.next()?)?;
        let layout = parts.next()?.to_string();
        if layout.is_empty() {
            return None;
        }
        (speakers, layout, parts.next()?)
    } else {
        (Vec::new(), String::new(), stem.strip_prefix("sweep-")?)
    };
    let parts: Vec<_> = suffix.split('-').collect();
    if parts.len() != 5 {
        return None;
    }
    let duration_s = decimal(parts[0].strip_suffix('s')?)?;
    let fs = parts[1].strip_suffix("hz")?.parse::<u32>().ok()?;
    let bits = parts[2].strip_suffix("bit")?.parse::<u16>().ok()?;
    let f_lo = decimal(parts[3].strip_suffix("hz")?)?;
    let f_hi = decimal(parts[4].strip_suffix("hz")?)?;
    if duration_s <= 0.0 || fs == 0 || !matches!(bits, 16 | 24 | 32) || f_hi < f_lo {
        return None;
    }
    Some(SweepFileInfo {
        speakers,
        layout,
        duration_s,
        fs,
        bits,
        f_lo,
        f_hi,
    })
}

#[derive(Debug, Clone, PartialEq)]
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

/// Pure port of recording_progress.infer_sweep_segments: a two-second lead,
/// then each sweep followed by a two-second gap. Accepts the same partial
/// filename/display-name match; no estimator or audio decoding is needed.
pub fn infer_sweep_segments(play_file: &str, total_duration: f64) -> Vec<SweepSegment> {
    let file = basename(play_file).to_ascii_lowercase();
    for (offset, _) in file.match_indices("sweep-seg-") {
        let mut fields = file[offset + 10..].splitn(4, '-');
        let Some(speakers) = fields.next().and_then(speakers) else {
            continue;
        };
        let Some(layout) = fields.next().filter(|s| !s.is_empty()) else {
            continue;
        };
        let _ = layout;
        let Some(mut duration) = fields
            .next()
            .and_then(|s| s.strip_suffix('s'))
            .and_then(decimal)
        else {
            continue;
        };
        if fields.next().is_none() {
            continue;
        }
        let total = speakers.len();
        if duration <= 0.0 {
            duration = ((total_duration - 2.0 * (total + 1) as f64) / total as f64).max(0.0);
        }
        return speakers
            .into_iter()
            .enumerate()
            .filter_map(|(index, speaker)| {
                let start = 2.0 + index as f64 * (duration + 2.0);
                let end = total_duration.min(start + duration);
                (end > start).then_some(SweepSegment {
                    speaker,
                    index: index + 1,
                    total,
                    start,
                    end,
                })
            })
            .collect();
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sweep_file_name_round_trips() {
        let name = sweep_file_name(&["FL", "FR"], "stereo", 6.15, 48000, 32, 2.93, 24000.0);
        assert_eq!(
            name,
            "sweep-seg-FL,FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"
        );
        let info = parse_sweep_file_name(&name).unwrap();
        assert_eq!(info.speakers, ["FL", "FR"]);
        assert_eq!(info.layout, "stereo");
        assert_eq!(
            (info.duration_s, info.fs, info.bits, info.f_lo, info.f_hi),
            (6.15, 48000, 32, 2.93, 24000.0)
        );
        assert!(
            parse_sweep_file_name("sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")
                .unwrap()
                .speakers
                .is_empty()
        );
        assert!(parse_sweep_file_name(&name.replace("FL,FR", "UNKNOWN")).is_none());
        assert!(parse_sweep_file_name(&name.replace("48000Hz", "0Hz")).is_none());
        assert_eq!(
            sweep_file_name(&["FL"], "mono", 6.125, 48000, 24, 2.125, 24000.5),
            "sweep-seg-FL-mono-6.12s-48000Hz-24bit-2.12Hz-24000Hz.wav"
        );
    }
    #[test]
    fn segments_match_pure_oracle_boundaries() {
        let seg = infer_sweep_segments("prefix-sweep-seg-fl,fr-stereo-6.15s-any suffix", 11.0);
        assert_eq!(seg.len(), 2);
        assert_eq!(
            (&*seg[0].speaker, seg[0].start, seg[0].end),
            ("FL", 2.0, 8.15)
        );
        assert_eq!(
            (seg[1].index, seg[1].total, seg[1].start, seg[1].end),
            (2, 2, 10.15, 11.0)
        );
        assert!(seg[0].contains(2.0));
        assert!(!seg[0].contains(8.15));
        let fallback = infer_sweep_segments("sweep-seg-FL,FR-stereo-0s-tail", 10.0);
        assert_eq!(
            (
                fallback[0].start,
                fallback[0].end,
                fallback[1].start,
                fallback[1].end
            ),
            (2.0, 4.0, 6.0, 8.0)
        );
        assert!(infer_sweep_segments("sweep-seg-FL-mono-1s-", 1.0).is_empty());
        assert!(infer_sweep_segments("sweep-seg-X-mono-1s-", 20.0).is_empty());
    }
}
