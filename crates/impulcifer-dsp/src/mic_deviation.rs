#![forbid(unsafe_code)]
//! MicrophoneMatchingCorrector, core/microphone_deviation_correction.py:57-454.
use crate::{
    DspError, fft,
    fr::{FrequencyResponse, generate_frequencies},
    hrir::Hrir,
    windows,
};
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
    Auto,
    Frontal,
    Diffuse,
}
#[derive(Clone, Debug)]
pub struct MicDeviationOptions {
    pub correction_strength: f64,
    pub max_correction_db: f64,
    pub smoothing_octave: f64,
    pub f_min: f64,
    pub f_max: f64,
    pub window_ms: f64,
    pub pre_ms: f64,
    pub anchor: Anchor,
}
impl Default for MicDeviationOptions {
    /// Python __init__, core/microphone_deviation_correction.py:57-65; p10_mic_deviation.
    fn default() -> Self {
        Self {
            correction_strength: 0.7,
            max_correction_db: 6.0,
            smoothing_octave: 1.0 / 6.0,
            f_min: 200.0,
            f_max: 16000.0,
            window_ms: 5.0,
            pre_ms: 0.5,
            anchor: Anchor::Auto,
        }
    }
}
#[derive(Clone, Debug)]
pub struct MicDeviationSummary {
    pub method: String,
    pub anchor: String,
    pub avg_error_db: f64,
    pub max_error_db: f64,
    pub speakers_analyzed: Vec<String>,
    pub correction_strength: f64,
    pub speakers_processed: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct MicMatching {
    pub fs: u32,
    pub options: MicDeviationOptions,
    pub frequency: Vec<f64>,
    pub speaker_power: Vec<(String, [Vec<f64>; 2])>,
    pub mismatch_db: Option<Vec<f64>>,
    pub anchor_used: String,
    win_samples: usize,
    pre_samples: usize,
}
impl MicMatching {
    /// Python __init__, core/microphone_deviation_correction.py:57-104; p10_mic_deviation.
    pub fn new(fs: u32, options: &MicDeviationOptions) -> Self {
        let mut options = options.clone();
        let nyq = fs as f64 / 2.0;
        options.correction_strength = options.correction_strength.clamp(0.0, 1.0);
        options.f_min = options.f_min.max(1.0).min(nyq * 0.5);
        options.f_max = options.f_max.max(options.f_min * 2.0).min(nyq * 0.98);
        let win_samples =
            ((options.window_ms * fs as f64 / 1000.0).round_ties_even() as usize).max(32);
        let pre_samples = (options.pre_ms * fs as f64 / 1000.0)
            .round_ties_even()
            .max(0.0) as usize;
        Self {
            fs,
            options,
            frequency: generate_frequencies(20.0, nyq, 1.01),
            speaker_power: Vec::new(),
            mismatch_db: None,
            anchor_used: "none".into(),
            win_samples,
            pre_samples,
        }
    }
    /// Python _windowed_power, core/microphone_deviation_correction.py:106-140; p10_mic_deviation.
    pub fn windowed_power(&self, ir: &[f64], peak: Option<usize>) -> Vec<f64> {
        if ir.is_empty() {
            return vec![0.0; self.frequency.len()];
        }
        let peak = peak
            .unwrap_or_else(|| {
                ir.iter()
                    .enumerate()
                    .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()).then_with(|| b.0.cmp(&a.0)))
                    .unwrap()
                    .0
            })
            .min(ir.len() - 1);
        let mut seg = ir
            [peak.saturating_sub(self.pre_samples)..(peak + self.win_samples).min(ir.len())]
            .to_vec();
        if seg.len() < 8 {
            return vec![0.0; self.frequency.len()];
        }
        let fade = self.pre_samples.min(seg.len() / 4);
        if fade > 1 {
            for (v, w) in seg[..fade].iter_mut().zip(windows::hann(2 * fade, true)) {
                *v *= w;
            }
        }
        let fade = (seg.len() / 4).max(1);
        let start = seg.len() - fade;
        if fade > 1 {
            for (v, w) in seg[start..]
                .iter_mut()
                .zip(&windows::hann(2 * fade, true)[fade..])
            {
                *v *= w;
            }
        }
        // scipy.fft.next_fast_len defaults to complex transforms (235711).
        let mut n = seg.len().max(8192);
        loop {
            let mut r = n;
            for p in [2, 3, 5, 7, 11] {
                while r.is_multiple_of(p) {
                    r /= p;
                }
            }
            if r == 1 {
                break;
            }
            n += 1;
        }
        seg.resize(n, 0.0);
        let spec = fft::rfft(&seg);
        let mag: Vec<_> = spec.iter().map(|z| z.norm()).collect();
        self.frequency
            .iter()
            .map(|f| {
                let x = f * n as f64 / self.fs as f64;
                let i = (x.floor() as usize).min(mag.len() - 1);
                let j = (i + 1).min(mag.len() - 1);
                let m = mag[i] + (mag[j] - mag[i]) * (x - i as f64);
                m * m
            })
            .collect()
    }
    /// Python collect_speaker, core/microphone_deviation_correction.py:142-151; p10_mic_deviation.
    pub fn collect_speaker(
        &mut self,
        speaker: &str,
        left: &[f64],
        right: &[f64],
        left_peak: Option<usize>,
        right_peak: Option<usize>,
    ) {
        let power = [
            self.windowed_power(left, left_peak),
            self.windowed_power(right, right_peak),
        ];
        if let Some((_, p)) = self.speaker_power.iter_mut().find(|(s, _)| s == speaker) {
            *p = power;
        } else {
            self.speaker_power.push((speaker.into(), power));
        }
    }
    /// Python _band_weight, core/microphone_deviation_correction.py:169-193; p10_mic_deviation.
    pub fn band_weight(&self) -> Vec<f64> {
        let lo2 = self.options.f_min.log10();
        let lo1 = (self.options.f_min / 2.0).max(1.0).log10();
        let hi1 = self.options.f_max.log10();
        let hi2 = (self.options.f_max * 2.0)
            .min(self.fs as f64 / 2.0 * 0.999)
            .log10();
        self.frequency
            .iter()
            .map(|f| {
                let f = f.log10();
                let mut w = 1.0;
                if f < lo1 {
                    w = 0.0;
                } else if f < lo2 {
                    w = 0.5
                        - 0.5 * (std::f64::consts::PI * (f - lo1) / (lo2 - lo1).max(1e-9)).cos();
                }
                if f > hi2 {
                    w = 0.0;
                } else if f > hi1 {
                    w = 0.5
                        + 0.5 * (std::f64::consts::PI * (f - hi1) / (hi2 - hi1).max(1e-9)).cos();
                }
                w
            })
            .collect()
    }
    /// Python estimate_interaural_mismatch, core/microphone_deviation_correction.py:195-238; p10_mic_deviation.
    pub fn estimate_interaural_mismatch(&mut self) -> Result<&[f64], DspError> {
        // Python's matching anchors differ from the general speaker-side catalogue.
        let centers = ["FC", "TFC", "BC"];
        let frontal = self.options.anchor != Anchor::Diffuse
            && self
                .speaker_power
                .iter()
                .any(|(s, _)| centers.contains(&s.as_str()));
        self.anchor_used = if self.speaker_power.is_empty() {
            "none"
        } else if frontal {
            "frontal"
        } else {
            "diffuse"
        }
        .into();
        let selected: Vec<_> = self
            .speaker_power
            .iter()
            .filter(|(s, _)| !frontal || centers.contains(&s.as_str()))
            .collect();
        let mut delta = vec![0.0; self.frequency.len()];
        if !selected.is_empty() {
            for (i, d) in delta.iter_mut().enumerate() {
                let l = selected.iter().map(|(_, p)| p[0][i]).sum::<f64>() / selected.len() as f64;
                let r = selected.iter().map(|(_, p)| p[1][i]).sum::<f64>() / selected.len() as f64;
                *d = 10.0 * ((l + 1e-20) / (r + 1e-20)).log10();
            }
        }
        let mut fr = FrequencyResponse::new(
            "interaural_mismatch",
            Some(self.frequency.clone()),
            Some(delta.clone()),
        )?;
        if fr
            .smoothen(
                self.options.smoothing_octave,
                self.options.smoothing_octave,
                100.0,
                10000.0,
            )
            .is_ok()
            && !fr.smoothed.is_empty()
        {
            delta = fr.smoothed;
        }
        for (d, w) in delta.iter_mut().zip(self.band_weight()) {
            *d = (*d * w).clamp(
                -2.0 * self.options.max_correction_db,
                2.0 * self.options.max_correction_db,
            );
        }
        self.mismatch_db = Some(delta);
        Ok(self.mismatch_db.as_deref().unwrap())
    }
    /// Python _fir_from_curve, core/microphone_deviation_correction.py:264-276; p10_mic_deviation.
    pub fn fir_from_curve(&self, curve: &[f64], name: &str) -> Result<Vec<f64>, DspError> {
        let mut fr =
            FrequencyResponse::new(name, Some(self.frequency.clone()), Some(curve.to_vec()))?;
        fr.equalization = curve.to_vec();
        let mut fir = fr
            .minimum_phase_impulse_response(self.fs, 10.0, false)
            .unwrap_or_else(|_| vec![1.0]);
        fir.truncate(2048.min(self.fs as usize / 10));
        Ok(fir)
    }
    /// Python design_correction_filters, core/microphone_deviation_correction.py:247-262; p10_mic_deviation.
    pub fn design_correction_filters(&mut self) -> Result<[Vec<f64>; 2], DspError> {
        if self.mismatch_db.is_none() {
            self.estimate_interaural_mismatch()?;
        }
        let half: Vec<_> = self
            .mismatch_db
            .as_ref()
            .unwrap()
            .iter()
            .map(|d| {
                (d * self.options.correction_strength / 2.0).clamp(
                    -self.options.max_correction_db,
                    self.options.max_correction_db,
                )
            })
            .collect();
        Ok([
            self.fir_from_curve(
                &half.iter().map(|d| -d).collect::<Vec<_>>(),
                "left_mic_correction",
            )?,
            self.fir_from_curve(&half, "right_mic_correction")?,
        ])
    }
    /// Python get_analysis_summary, core/microphone_deviation_correction.py:278-292; p10_mic_deviation.
    pub fn analysis_summary(&self) -> Result<MicDeviationSummary, DspError> {
        let delta = self
            .mismatch_db
            .as_ref()
            .ok_or_else(|| DspError::InvalidArgument("analysis incomplete".into()))?;
        let nz: Vec<_> = delta
            .iter()
            .zip(self.band_weight())
            .filter(|(_, w)| *w > 0.0)
            .map(|(d, _)| {
                (d * self.options.correction_strength / 2.0)
                    .clamp(
                        -self.options.max_correction_db,
                        self.options.max_correction_db,
                    )
                    .abs()
            })
            .collect();
        Ok(MicDeviationSummary {
            method: "interaural_v4".into(),
            anchor: self.anchor_used.clone(),
            avg_error_db: if nz.is_empty() {
                0.0
            } else {
                nz.iter().sum::<f64>() / nz.len() as f64
            },
            max_error_db: nz.iter().copied().fold(0.0, f64::max),
            speakers_analyzed: self.speaker_power.iter().map(|(s, _)| s.clone()).collect(),
            correction_strength: self.options.correction_strength,
            speakers_processed: Vec::new(),
        })
    }
}
/// Python apply_microphone_deviation_correction_to_hrir,
/// core/microphone_deviation_correction.py:387-454; p10_mic_deviation.
pub fn apply_mic_deviation_correction(
    hrir: &mut Hrir,
    options: &MicDeviationOptions,
) -> Result<MicDeviationSummary, DspError> {
    let mut m = MicMatching::new(hrir.fs, options);
    for s in &hrir.speakers {
        if let (Some(l), Some(r)) = (&s.left, &s.right) {
            m.collect_speaker(
                &s.speaker,
                &l.data,
                &r.data,
                Some(l.peak_index(0, None, 0.12589)),
                Some(r.peak_index(0, None, 0.12589)),
            );
        }
    }
    if m.speaker_power.is_empty() {
        return Err(DspError::InvalidArgument("no speaker data".into()));
    }
    m.estimate_interaural_mismatch()?;
    let mut summary = m.analysis_summary()?;
    if summary.max_error_db < 0.05 {
        return Ok(summary);
    }
    let firs = m.design_correction_filters()?;
    hrir.equalize(&firs[0], &firs[1]);
    summary.speakers_processed = summary.speakers_analyzed.clone();
    Ok(summary)
}
