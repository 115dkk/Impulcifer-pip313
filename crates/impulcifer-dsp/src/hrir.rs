//! Insertion-ordered, file-free binaural responses, core/hrir.py.
use crate::{DspError, conv, decay, estimator::SweepEstimator, fft, ir::ImpulseResponse, windows};
use impulcifer_types::constants::{
    SPEAKER_DELAYS, SPEAKER_NAMES, Side, base_channel_count, track_name,
};

#[derive(Clone, Debug)]
pub struct SpeakerIrs {
    pub speaker: String,
    pub left: Option<ImpulseResponse>,
    pub right: Option<ImpulseResponse>,
}
#[derive(Clone, Debug)]
pub struct Hrir {
    pub fs: u32,
    pub speakers: Vec<SpeakerIrs>,
}
#[derive(Clone, Debug)]
pub struct ReflectionLevels {
    pub early_db: f64,
    pub late_db: f64,
}

/// Python _ingest_recording, core/hrir.py:77-364; p08_demo_open.json.
/// Side-specific tracks deliberately retain Python's hard-coded i // 2 mapping.
pub fn ingest_recording(
    estimator: &SweepEstimator,
    expected_fs: u32,
    fs: u32,
    tracks: &[Vec<f64>],
    speakers: &[&str],
    side: Option<Side>,
    silence_length: f64,
) -> Result<Vec<SpeakerIrs>, DspError> {
    if fs != expected_fs {
        return Err(DspError::InvalidArgument(
            "Sampling rate of recording must match sampling rate of test signal.".into(),
        ));
    }
    let silence = silence_length * fs as f64;
    if !silence.is_finite() || silence != silence.trunc() {
        return Err(DspError::InvalidArgument(
            "Silence length must produce full samples with given sampling rate.".into(),
        ));
    }
    let k = if side.is_none() { 2 } else { 1 };
    if tracks.len() < k
        || tracks.iter().any(|t| t.len() != tracks[0].len())
        || silence < 0.0
        || side == Some(Side::Center)
    {
        return Err(DspError::InvalidArgument(
            "invalid recording track shape or side".into(),
        ));
    }
    let silence = silence as usize;
    let lead = silence.min(tracks[0].len());
    let available = tracks[0].len() - lead;
    let sweep = estimator.test_signal.len();
    let mut n = decay::py_round(speakers.len() as f64 / (tracks.len() / k) as f64) as usize;
    let mut size = silence + sweep;
    if size > available {
        if n <= 1 {
            size = available;
            n = 1;
        } else {
            size = available / n;
        }
    }
    let mut columns = Vec::new();
    for i in 0..n {
        let start = i * size;
        let end = ((i + 1) * size).min(available);
        if end > start && end - start >= sweep {
            columns.push((lead + start, lead + end));
        }
    }
    if columns.is_empty() {
        if silence > 0 && available >= sweep + (fs as f64 * 0.5) as usize {
            let adjusted = available - sweep;
            for i in 0..n {
                let start = i * sweep;
                let end = ((i + 1) * sweep).min(available - adjusted);
                if end > start && end - start >= sweep {
                    columns.push((lead + adjusted + start, lead + adjusted + end));
                }
            }
        }
        if columns.is_empty() && available as f64 > sweep as f64 * 0.8 {
            if n == 1 {
                columns.push((lead, lead + available));
            } else if let Some(column_size) = available.checked_div(n) {
                size = column_size;
                for i in 0..n {
                    if size > 0 {
                        columns.push((lead + i * size, lead + (i + 1) * size));
                    }
                }
            }
        }
        if columns.is_empty() {
            return Err(DspError::InvalidArgument(format!(
                "No valid columns could be extracted even with fallback methods.\nRecording length ({available} samples, {:.2}s) is too short for the required estimator length ({sweep} samples, {:.2}s).\nSolutions:\n  1. Re-record with longer duration (minimum {:.1}s)\n  2. Use a shorter test signal\n  3. Check if the correct test signal file was used for recording",
                available as f64 / fs as f64,
                sweep as f64 / fs as f64,
                (sweep + silence) as f64 / fs as f64
            )));
        }
    }
    let mut out = Hrir {
        fs,
        speakers: Vec::new(),
    };
    for i in (0..tracks.len()).step_by(k) {
        for (j, &(start, end)) in columns.iter().enumerate() {
            let Some(&speaker) = speakers.get(i / 2 * columns.len() + j) else {
                continue;
            };
            if !SPEAKER_NAMES.contains(&speaker) {
                continue;
            }
            if out.get(speaker).is_none() {
                out.speakers.push(SpeakerIrs {
                    speaker: speaker.into(),
                    left: None,
                    right: None,
                });
            }
            let pair = out.get_mut(speaker).unwrap();
            let make = |index: usize| {
                let recording = tracks[index][start..end].to_vec();
                ImpulseResponse {
                    data: estimator.estimate(&recording),
                    fs,
                    recording: Some(recording),
                }
            };
            match side {
                None if i + 1 < tracks.len() => {
                    pair.left = Some(make(i));
                    pair.right = Some(make(i + 1));
                }
                Some(Side::Left) => pair.left = Some(make(i)),
                Some(Side::Right) => pair.right = Some(make(i)),
                _ => (),
            }
        }
    }
    Ok(out.speakers)
}

/// Python compact_tracks, core/brir_layout.py:38-44; p08_stack.json.
/// The fixed tuple signature represents an all-silent result with empty vectors.
pub fn compact_tracks(tracks: &[Vec<f64>], order: &[&str]) -> (Vec<Vec<f64>>, Vec<String>) {
    tracks
        .iter()
        .zip(order)
        .filter(|(row, _)| row.iter().any(|x| *x != 0.0))
        .map(|(row, name)| (row.clone(), (*name).into()))
        .unzip()
}

impl Hrir {
    /// Python dict.get, core/hrir.py:383-395; p08_demo_*.json.
    pub fn get(&self, speaker: &str) -> Option<&SpeakerIrs> {
        self.speakers.iter().find(|s| s.speaker == speaker)
    }
    /// Python dict.get/update, core/hrir.py:424-425; p08_demo_*.json.
    pub fn get_mut(&mut self, speaker: &str) -> Option<&mut SpeakerIrs> {
        self.speakers.iter_mut().find(|s| s.speaker == speaker)
    }
    /// Python pair indexing, core/hrir.py:582-584; p08_demo_*.json.
    pub fn pair(&self, speaker: &str) -> Option<(&ImpulseResponse, &ImpulseResponse)> {
        let s = self.get(speaker)?;
        Some((s.left.as_ref()?, s.right.as_ref()?))
    }
    /// Python IR iteration, core/hrir.py:561-563; p08_demo_*.json.
    pub fn for_each_ir(&mut self, mut f: impl FnMut(&mut ImpulseResponse)) {
        for s in &mut self.speakers {
            for ir in s.left.iter_mut().chain(s.right.iter_mut()) {
                f(ir);
            }
        }
    }
    /// Python subset(copy_irs=True), core/hrir.py:383-395; P08 properties.
    pub fn subset(&self, speakers: &[&str]) -> Hrir {
        let mut out = Hrir {
            fs: self.fs,
            speakers: Vec::new(),
        };
        for name in speakers {
            if out.get(name).is_none()
                && let Some(s) = self.get(name)
            {
                out.speakers.push(s.clone());
            }
        }
        out
    }
    /// Python open_recording merge, core/hrir.py:397-425; p08_demo_open.json.
    pub fn open_recording_samples(
        &mut self,
        estimator: &SweepEstimator,
        fs: u32,
        tracks: &[Vec<f64>],
        speakers: &[&str],
        side: Option<Side>,
        silence_length: f64,
    ) -> Result<(), DspError> {
        if self.fs != estimator.fs {
            return Err(DspError::InvalidArgument("Refusing to open recording because HRIR's sampling rate doesn't match impulse response estimator's sampling rate.".into()));
        }
        for incoming in ingest_recording(
            estimator,
            self.fs,
            fs,
            tracks,
            speakers,
            side,
            silence_length,
        )? {
            if let Some(existing) = self.get_mut(&incoming.speaker) {
                if incoming.left.is_some() {
                    existing.left = incoming.left;
                }
                if incoming.right.is_some() {
                    existing.right = incoming.right;
                }
            } else {
                self.speakers.push(incoming);
            }
        }
        Ok(())
    }
    /// Python write_wav pure stacking, core/hrir.py:427-474; p08_stack.json.
    pub fn stack_tracks(
        &self,
        order: &[&str],
        trim_extensions: bool,
    ) -> Result<Vec<Vec<f64>>, DspError> {
        let rows: Vec<_> = self
            .speakers
            .iter()
            .flat_map(|s| {
                s.left
                    .iter()
                    .map(|ir| (track_name(&s.speaker, "left"), ir))
                    .chain(
                        s.right
                            .iter()
                            .map(|ir| (track_name(&s.speaker, "right"), ir)),
                    )
            })
            .collect();
        let reference = rows
            .first()
            .ok_or_else(|| {
                DspError::InvalidArgument("No impulse responses available for WAV output.".into())
            })?
            .1
            .len();
        let mut data: Vec<_> = order
            .iter()
            .map(|name| {
                rows.iter()
                    .find(|(key, _)| key == name)
                    .map(|(_, ir)| ir.data.clone())
                    .unwrap_or_else(|| vec![0.0; reference])
            })
            .collect();
        let selected_length = data
            .first()
            .ok_or_else(|| {
                DspError::InvalidArgument("at least one track is required for stacking".into())
            })?
            .len();
        if data.iter().any(|row| row.len() != selected_length) {
            return Err(DspError::InvalidArgument(
                "selected tracks must have equal lengths for stacking".into(),
            ));
        }
        if trim_extensions {
            let minimum = base_channel_count(order);
            while data.len() > minimum
                && data[data.len() - 2..]
                    .iter()
                    .all(|row| row.iter().all(|x| *x == 0.0))
            {
                data.truncate(data.len() - 2);
            }
        }
        Ok(data)
    }
    /// Python normalize, core/hrir.py:476-565; p08_demo_normalize.json.
    pub fn normalize(
        &mut self,
        peak_target: Option<f64>,
        avg_target: Option<f64>,
    ) -> Result<f64, DspError> {
        if peak_target.is_some() == avg_target.is_some() {
            return Err(DspError::InvalidArgument("One and only one of the parameters \"peak_target\" and \"avg_target\" must be given!".into()));
        }
        let mut magnitudes = Vec::new();
        for left in [true, false] {
            let rows: Vec<_> = self
                .speakers
                .iter()
                .filter_map(|s| {
                    if left {
                        s.left.as_ref()
                    } else {
                        s.right.as_ref()
                    }
                })
                .filter(|ir| !ir.is_empty())
                .collect();
            let n=rows.iter().map(|ir|ir.len()).max().ok_or_else(||DspError::InvalidArgument("No valid impulse response data found for normalization. All channels appear to be empty.".into()))?;
            let mut sum = vec![0.0; n];
            for ir in rows {
                for (x, y) in sum.iter_mut().zip(&ir.data) {
                    *x += y;
                }
            }
            let db = fft::magnitude_response(&sum);
            magnitudes.extend(db.into_iter().enumerate().filter_map(|(i, x)| {
                let f = i as f64 * (self.fs as f64 / n as f64);
                if peak_target.is_some() || (f > 80.0 && f < 6000.0) {
                    Some(x)
                } else {
                    None
                }
            }));
        }
        let gain = if let Some(target) = peak_target {
            target - magnitudes.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        } else {
            avg_target.unwrap() - magnitudes.iter().sum::<f64>() / magnitudes.len() as f64
        };
        let scalar = 10.0_f64.powf(gain / 20.0);
        self.for_each_ir(|ir| ir.data.iter_mut().for_each(|x| *x *= scalar));
        Ok(gain)
    }
    /// Python crop_heads, core/hrir.py:567-631; p08_demo_crop_heads.json.
    /// Estimator-rate validation belongs to callers: this fixed type owns no estimator.
    pub fn crop_heads(&mut self, head_ms: f64) -> Result<(), DspError> {
        if !head_ms.is_finite() || head_ms < 0.0 {
            return Err(DspError::InvalidArgument(
                "head duration must be nonnegative".into(),
            ));
        }
        let head = (head_ms * self.fs as f64 / 1000.0) as usize;
        // Built lazily: Python creates the Hann only after both ears are known
        // to hold at least `head` samples, so a huge head_ms on short IRs
        // must not allocate.
        let mut window: Option<Vec<f64>> = None;
        for s in &mut self.speakers {
            let (Some(left), Some(right)) = (&mut s.left, &mut s.right) else {
                return Err(DspError::InvalidArgument(
                    "both ears required for head cropping".into(),
                ));
            };
            let index = SPEAKER_NAMES
                .iter()
                .position(|name| *name == s.speaker)
                .ok_or_else(|| DspError::InvalidArgument("unknown speaker delay".into()))?;
            let delay = decay::py_round(SPEAKER_DELAYS[index] * self.fs as f64) as usize + head;
            let start = left
                .peak_index(0, None, 0.12589)
                .min(right.peak_index(0, None, 0.12589))
                .saturating_sub(delay);
            left.data = left.data[start.min(left.len())..].to_vec();
            right.data = right.data[start.min(right.len())..].to_vec();
            if left.len() >= head && right.len() >= head {
                let window = window.get_or_insert_with(|| windows::hann(head * 2, true));
                for ir in [left, right] {
                    for (x, w) in ir.data[..head].iter_mut().zip(window.iter()) {
                        *x *= w;
                    }
                }
            }
        }
        Ok(())
    }
    /// Python crop_tails, core/hrir.py:633-672; p08_demo_crop_tails.json.
    pub fn crop_tails(&mut self, estimator: &SweepEstimator) -> Result<usize, DspError> {
        if self.fs != estimator.fs {
            return Err(DspError::InvalidArgument("Refusing to crop tails because HRIR sampling rate doesn't match estimator sampling rate.".into()));
        }
        let mut tails = Vec::new();
        let mut lengths = Vec::new();
        self.for_each_ir(|ir| {
            tails.push(ir.decay_params().knee_index);
            lengths.push(ir.len());
        });
        let Some(max) = tails.into_iter().max() else {
            return Ok(0);
        };
        let seconds =
            estimator.test_signal.len() as f64 / estimator.fs as f64 / estimator.n_octaves;
        let half = (self.fs as f64 * seconds * (1.0 / 24.0)) as usize;
        let window = windows::hann(half * 2, true);
        let tail = lengths
            .into_iter()
            .min()
            .unwrap()
            .min(fft::next_fast_len_legacy(max));
        if tail < half {
            return Err(DspError::InvalidArgument(
                "tail shorter than fade window".into(),
            ));
        }
        self.for_each_ir(|ir| {
            ir.data.truncate(tail);
            for (x, w) in ir.data[tail - half..].iter_mut().zip(&window[half..]) {
                *x *= w;
            }
        });
        Ok(tail)
    }
    /// Python equalize, core/hrir.py:877-907; P08 properties, p08_ir.json.
    pub fn equalize(&mut self, left_fir: &[f64], right_fir: &[f64]) {
        for s in &mut self.speakers {
            if let Some(ir) = &mut s.left {
                ir.equalize(left_fir);
            }
            if let Some(ir) = &mut s.right {
                ir.equalize(right_fir);
            }
        }
    }
    /// Python equalize mono tiling, core/hrir.py:901-907; p08_ir.json.
    pub fn equalize_same(&mut self, fir: &[f64]) {
        self.equalize(fir, fir);
    }
    /// Python resample, core/hrir.py:909-938; p06_poly_* and P08 properties.
    pub fn resample(&mut self, fs: u32) -> Result<(), DspError> {
        for s in &mut self.speakers {
            for ir in s.left.iter_mut().chain(s.right.iter_mut()) {
                ir.resample(fs)?;
            }
        }
        self.fs = fs;
        Ok(())
    }
    /// Python align_ipsilateral_all, core/hrir.py:940-974; p08_demo_ipsilateral.json.
    pub fn align_ipsilateral_all(&mut self, pairs: &[(&str, &str)], segment_ms: f64) {
        let segment = (self.fs as f64 * segment_ms / 1000.0) as usize;
        for &(a, b) in pairs {
            let (Some(first), Some(second)) = (
                self.get(a).and_then(|s| s.left.as_ref()),
                self.get(b).and_then(|s| s.right.as_ref()),
            ) else {
                continue;
            };
            let x = &first.data[..segment.min(first.len())];
            let y = &second.data[..segment.min(second.len())];
            if x.is_empty() || y.is_empty() {
                continue;
            }
            let corr = conv::correlate(x, y, conv::Mode::Full);
            let mut index = 0;
            for i in 1..corr.len() {
                if corr[i] > corr[index] {
                    index = i;
                }
            }
            let lag = index as i64 - x.len() as i64 + 1;
            if a == b {
                let s = self.get_mut(a).unwrap();
                if lag > 0 {
                    s.right.as_mut().unwrap().shift(lag);
                } else if lag < 0 {
                    s.left.as_mut().unwrap().shift(-lag);
                }
            } else if lag != 0 {
                let s = self.get_mut(if lag > 0 { b } else { a }).unwrap();
                for ir in s.left.iter_mut().chain(s.right.iter_mut()) {
                    ir.shift(lag.abs());
                }
            }
        }
    }
    /// Python align_onset_groups_peak_leftref, core/hrir.py:976-1020; p08_demo_onset.json.
    pub fn align_onset_groups_peak_leftref(
        &mut self,
        groups: Option<&[&[&str]]>,
    ) -> Result<(), DspError> {
        let defaults: &[&[&str]] = &[
            &["FL", "FR"],
            &["SL", "SR"],
            &["BL", "BR"],
            &["WL", "WR"],
            &["TFL", "TFR"],
            &["TSL", "TSR"],
            &["TBL", "TBR"],
            &["FC"],
        ];
        let reference = self
            .get("FL")
            .and_then(|s| s.left.as_ref())
            .ok_or_else(|| {
                DspError::InvalidArgument(
                    "Cannot find FL left channel reference for onset alignment.".into(),
                )
            })?
            .peak_index(0, None, 0.12589) as i64;
        for group in groups.unwrap_or(defaults) {
            if *group == ["FL", "FR"] {
                continue;
            }
            let Some(ir) = group
                .first()
                .and_then(|name| self.get(name))
                .and_then(|s| s.left.as_ref())
            else {
                continue;
            };
            let shift = reference - ir.peak_index(0, None, 0.12589) as i64;
            for name in *group {
                if let Some(s) = self.get_mut(name) {
                    for ir in s.left.iter_mut().chain(s.right.iter_mut()) {
                        ir.shift(shift);
                    }
                }
            }
        }
        Ok(())
    }
    /// Python calculate_reflection_levels, core/hrir.py:1022-1109; p08_reflections.json.
    pub fn calculate_reflection_levels(
        &self,
        direct_sound_duration_ms: f64,
        early_ref_start_ms: f64,
        early_ref_end_ms: f64,
        late_ref_start_ms: f64,
        late_ref_end_ms: f64,
        epsilon: f64,
    ) -> Vec<(String, Side, ReflectionLevels)> {
        let mut out = Vec::new();
        for s in &self.speakers {
            for (side, ir) in [(Side::Left, &s.left), (Side::Right, &s.right)] {
                if let Some(ir) = ir {
                    let peak = ir.peak_index(0, None, 0.12589);
                    let rms = |start: f64, end: f64, empty: f64| {
                        let start =
                            (peak + (start * self.fs as f64 / 1000.0) as usize).min(ir.len());
                        let end = (peak + (end * self.fs as f64 / 1000.0) as usize).min(ir.len());
                        if end <= start {
                            empty
                        } else {
                            (ir.data[start..end].iter().map(|x| x * x).sum::<f64>()
                                / (end - start) as f64)
                                .sqrt()
                        }
                    };
                    let direct = rms(0.0, direct_sound_duration_ms, epsilon).max(epsilon);
                    out.push((
                        s.speaker.clone(),
                        side,
                        ReflectionLevels {
                            early_db: 20.0
                                * (rms(early_ref_start_ms, early_ref_end_ms, 0.0) / direct
                                    + epsilon)
                                    .log10(),
                            late_db: 20.0
                                * (rms(late_ref_start_ms, late_ref_end_ms, 0.0) / direct + epsilon)
                                    .log10(),
                        },
                    ));
                }
            }
        }
        out
    }
}
