//! Numeric README data, with rendering left to the service.
use crate::hrir::{Hrir, ReflectionLevels};
use impulcifer_types::constants::{SPEAKER_NAMES, Side, speaker_side};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReverbKind {
    Rt60,
    Rt30,
    Rt20,
    Edt,
    Rtxx,
}
#[derive(Clone, Debug)]
pub struct ReadmeRow {
    pub speaker: String,
    pub side: Side,
    pub pnr_db: f64,
    pub itd_us: f64,
    pub length_ms: Option<f64>,
    pub reverb: Option<(ReverbKind, f64)>,
}
#[derive(Clone, Debug)]
pub struct ReadmeData {
    pub rows: Vec<ReadmeRow>,
    pub reverb_header: ReverbKind,
    pub reflections: Vec<(String, Side, ReflectionLevels)>,
    pub applied_gain_db: f64,
    pub fs: u32,
}
/// Python write_readme, core/pipeline_stages.py:525-687; p10_readme.
pub fn readme_data(hrir: &Hrir, fs: u32, applied_gain: f64) -> ReadmeData {
    let mut speakers: Vec<_> = hrir.speakers.iter().collect();
    speakers.sort_by_key(|s| {
        SPEAKER_NAMES
            .iter()
            .position(|n| *n == s.speaker)
            .unwrap_or(usize::MAX)
    });
    let mut rows = Vec::new();
    let mut counts: Vec<(ReverbKind, usize)> = Vec::new();
    for s in speakers {
        let itd = if let (Some(l), Some(r)) = (&s.left, &s.right) {
            l.peak_index(0, None, 0.12589)
                .abs_diff(r.peak_index(0, None, 0.12589)) as f64
                / hrir.fs as f64
                * 1e6
        } else {
            0.0
        };
        for (side, ir) in [(Side::Left, &s.left), (Side::Right, &s.right)] {
            if let Some(ir) = ir {
                let peak = ir.peak_index(0, None, 0.12589);
                let params = ir.decay_params();
                let times = ir.decay_times(Some(&params));
                let reverb = [
                    (ReverbKind::Rt60, times.rt60),
                    (ReverbKind::Rt30, times.rt30),
                    (ReverbKind::Rt20, times.rt20),
                    (ReverbKind::Edt, times.edt),
                ]
                .into_iter()
                .find_map(|(k, v)| v.filter(|v| !v.is_nan()).map(|v| (k, v * 1000.0)));
                if let Some((k, _)) = reverb {
                    if let Some((_, n)) = counts.iter_mut().find(|(v, _)| *v == k) {
                        *n += 1;
                    } else {
                        counts.push((k, 1));
                    }
                }
                let contra = matches!(
                    (speaker_side(&s.speaker), side),
                    (Side::Left, Side::Right) | (Side::Right, Side::Left)
                );
                rows.push(ReadmeRow {
                    speaker: s.speaker.clone(),
                    side,
                    pnr_db: ir.data.get(peak).map_or(f64::NAN, |v| {
                        20.0 * (v.abs() + 1e-9).log10() - params.noise_floor_db
                    }),
                    itd_us: if contra { itd } else { 0.0 },
                    length_ms: (params.knee_index > peak)
                        .then(|| (params.knee_index - peak) as f64 / ir.fs as f64 * 1000.0),
                    reverb,
                });
            }
        }
    }
    let mut reverb_header = ReverbKind::Rtxx;
    let mut max = 0;
    for (k, n) in counts {
        if n > max {
            max = n;
            reverb_header = k;
        }
    }
    let mut reflections = hrir.calculate_reflection_levels(2.0, 20.0, 50.0, 50.0, 150.0, 1e-12);
    reflections.sort_by_key(|(s, _, _)| {
        SPEAKER_NAMES
            .iter()
            .position(|n| *n == s)
            .unwrap_or(usize::MAX)
    });
    ReadmeData {
        rows,
        reverb_header,
        reflections,
        applied_gain_db: applied_gain,
        fs,
    }
}
