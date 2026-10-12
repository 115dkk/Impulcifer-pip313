use super::*;
use crate::{conv, fft, hrir::Hrir, stages::room_tuning as v1};
use impulcifer_types::constants::Side;
use rayon::prelude::*;

#[derive(Clone, Debug)]
pub struct Tuning2Options {
    pub rc: RcOptions,
    pub level_match: bool,
    pub mimo: Option<MimoOptions>,
    pub pairs: Vec<v1::PairMeasurement>,
}
#[derive(Clone, Debug)]
pub struct PlannedSpeaker {
    pub speaker: String,
    pub omni: Vec<(Side, ImpulseResponse)>,
    pub rc: SpeakerRc,
}
#[derive(Clone, Debug)]
pub struct Tuning2Plan {
    pub fs: u32,
    pub band: Option<CorrectionBand>,
    pub speakers: Vec<PlannedSpeaker>,
    pub options: Tuning2Options,
}
#[derive(Clone, Debug)]
pub struct Speaker2Report {
    pub speaker: String,
    pub points: usize,
    pub level_db: f64,
    pub trim_db: f64,
    pub trim_source: &'static str,
    pub level_difference_db: f64,
    pub peq_bands: usize,
    pub phase_strength: f64,
    pub phase_frac: f64,
    pub long_modes: Vec<(f64, f64)>,
    pub pre_echo_channels: Vec<v1::PreEcho>,
    pub representation_rms_db: Vec<(Side, f64)>,
    pub weak: bool,
    pub mismatch: bool,
    pub lf_edt_before_ms: f64,
    pub lf_edt_after_ms: f64,
    pub excess_median_before_ms: f64,
    pub excess_median_after_ms: f64,
}
#[derive(Clone, Debug)]
pub struct Tuning2Report {
    pub fs: u32,
    pub mode: Tuning2Mode,
    pub latency_samples: usize,
    pub band: Option<CorrectionBand>,
    pub speakers: Vec<Speaker2Report>,
    pub mimo: Option<MimoDesign>,
    pub pair_checks: Vec<v1::PairCheck>,
}
pub fn prepare_tuning2(
    omni: &Hrir,
    options: &Tuning2Options,
    calibration: Option<&FrequencyResponse>,
    target: Option<&FrequencyResponse>,
) -> Result<Tuning2Plan, DspError> {
    let mut options = options.clone();
    options.rc.policy = PositionPolicy::Symmetric;
    let points: Vec<Vec<_>> = omni
        .speakers
        .iter()
        .map(|s| s.left.iter().chain(&s.right).cloned().collect())
        .collect();
    let nonempty: Vec<_> = points
        .iter()
        .filter(|p| !p.is_empty())
        .map(Vec::as_slice)
        .collect();
    let band = if nonempty.is_empty() {
        None
    } else {
        Some(correction_band(&nonempty, &options.rc)?)
    };
    let speakers: Result<Vec<_>, _> = omni
        .speakers
        .par_iter()
        .zip(&points)
        .filter(|(_, p)| !p.is_empty())
        .map(|(s, p)| {
            let rc = design_rc(p, &options.rc, band.as_ref().unwrap(), target, calibration)?;
            let omni = s
                .left
                .iter()
                .map(|p| (Side::Left, p.clone()))
                .chain(s.right.iter().map(|p| (Side::Right, p.clone())))
                .collect();
            Ok(PlannedSpeaker {
                speaker: s.speaker.clone(),
                omni,
                rc,
            })
        })
        .collect();
    Ok(Tuning2Plan {
        fs: omni.fs,
        band,
        speakers: speakers?,
        options,
    })
}
pub fn apply_tuning2(hrir: &mut Hrir, plan: &Tuning2Plan) -> Result<Tuning2Report, DspError> {
    apply_impl(hrir, plan, false)
}
/// Full convolution boundary before output-length cropping and the terminal fade.
/// Useful for checking the support sum without windowing leakage.
pub fn apply_tuning2_full(hrir: &mut Hrir, plan: &Tuning2Plan) -> Result<Tuning2Report, DspError> {
    apply_impl(hrir, plan, true)
}
fn apply_impl(hrir: &mut Hrir, plan: &Tuning2Plan, full: bool) -> Result<Tuning2Report, DspError> {
    if hrir.fs != plan.fs {
        return Err(DspError::InvalidArgument(
            "tuning2 sample rate differs".into(),
        ));
    }
    let measured: Vec<_> = plan
        .speakers
        .iter()
        .filter(|s| hrir.get(&s.speaker).is_some())
        .collect();
    let delay = measured.first().map_or(0, |s| s.rc.lift_samples);
    let mut report = Tuning2Report {
        fs: hrir.fs,
        mode: plan.options.rc.mode,
        latency_samples: delay,
        band: plan.band.clone(),
        speakers: vec![],
        mimo: None,
        pair_checks: vec![],
    };
    if measured.is_empty() {
        return Ok(report);
    }
    let refs: Vec<_> = measured
        .iter()
        .map(|s| 10_f64.powf(s.rc.level_db / 20.))
        .collect();
    let ear_levels: Vec<_> = measured
        .iter()
        .map(|s| {
            let ears = hrir.get(&s.speaker).unwrap();
            ears.left
                .iter()
                .chain(&ears.right)
                .map(|ir| {
                    v1::spectrum(&ir.data, hrir.fs as usize)[100..501]
                        .iter()
                        .map(|v| v.norm_sqr())
                        .sum::<f64>()
                })
                .sum::<f64>()
                .sqrt()
        })
        .collect();
    let checks = v1::level_trims(&refs, &ear_levels);
    let minimum = measured
        .iter()
        .zip(&checks)
        .filter(|(_, c)| !c.1)
        .map(|(s, _)| s.rc.level_db)
        .fold(f64::INFINITY, f64::min);
    for (s, (_, skip, diff)) in measured.iter().zip(&checks) {
        let trim = if plan.options.level_match && !skip {
            minimum - s.rc.level_db
        } else {
            0.
        };
        report.speakers.push(Speaker2Report {
            speaker: s.speaker.clone(),
            points: s.rc.points,
            level_db: s.rc.level_db,
            trim_db: trim,
            trim_source: if !plan.options.level_match {
                "off"
            } else if *skip {
                "skipped"
            } else {
                "omni"
            },
            level_difference_db: if plan.options.level_match { *diff } else { 0. },
            peq_bands: s.rc.peq.len(),
            phase_strength: s.rc.phase_strength,
            phase_frac: s.rc.phase_frac,
            long_modes: s.rc.long_modes.clone(),
            pre_echo_channels: vec![],
            representation_rms_db: vec![],
            weak: false,
            mismatch: false,
            lf_edt_before_ms: 0.,
            lf_edt_after_ms: 0.,
            excess_median_before_ms: 0.,
            excess_median_after_ms: 0.,
        });
    }
    report.pair_checks = plan
        .options
        .pairs
        .iter()
        .filter_map(v1::geometry_check)
        .collect();
    if let Some(opts) = &plan.options.mimo {
        let inputs: Vec<_> = measured
            .iter()
            .zip(&report.speakers)
            .filter(|(s, _)| {
                let e = hrir.get(&s.speaker).unwrap();
                e.left.is_some() && e.right.is_some()
            })
            .map(|(s, r)| MimoInput {
                speaker: &s.speaker,
                gain_db: r.trim_db,
                response: &s.rc.response,
            })
            .collect();
        report.mimo = Some(design_mimo(hrir, &inputs, opts)?);
    }
    let original = hrir.clone();
    let fs = hrir.fs;
    let n = analysis_len(fs);
    let updates: Result<Vec<_>, DspError> = report
        .speakers
        .par_iter()
        .zip(&measured)
        .map(|(r, s)| {
            let mut r = r.clone();
            let mut ears = original.get(&r.speaker).unwrap().clone();
            let filter = fft::irfft(&s.rc.response, n);
            let supports: Vec<_> = report
                .mimo
                .as_ref()
                .and_then(|m| m.inputs.iter().find(|i| i.speaker == r.speaker))
                .map(|i| {
                    i.supports
                        .iter()
                        .map(|support| {
                            Ok((
                                support.speaker.clone(),
                                fft::irfft(&support_response(support, fs, n)?, n),
                            ))
                        })
                        .collect::<Result<Vec<_>, DspError>>()
                })
                .transpose()?
                .unwrap_or_default();
            let mut echoes = vec![];
            let mut before = [0.; 7];
            let mut after = [0.; 7];
            let mut count = 0.;
            for (side, ir) in [(Side::Left, &mut ears.left), (Side::Right, &mut ears.right)] {
                let Some(ir) = ir else {
                    continue;
                };
                count += 1.;
                let old = ir.clone();
                r.excess_median_before_ms += v1::excess_group_delay_median(&old);
                for (a, b) in before.iter_mut().zip(v1::lf_decay_bands(&old)) {
                    *a += b;
                }
                if let Some((_, omni)) = s.omni.iter().find(|(e, _)| *e == side) {
                    let mag = |ir: &ImpulseResponse| {
                        v1::smooth_real(
                            &v1::spectrum(&ir.data, fs as usize)
                                .iter()
                                .map(|v| 20. * v.norm().max(1e-12).log10())
                                .collect::<Vec<_>>(),
                            1. / 6.,
                        )
                    };
                    let mut diagnostic_omni = omni.clone();
                    diagnostic_omni.crop_head(1.0);
                    let a = mag(&diagnostic_omni);
                    let b = mag(&old);
                    let diff: Vec<_> = a.iter().zip(b).map(|(a, b)| a - b).collect();
                    let center = diff[100..300].iter().sum::<f64>() / 200.;
                    let rms = (diff[30..301]
                        .iter()
                        .map(|v| (v - center).powi(2))
                        .sum::<f64>()
                        / 271.)
                        .sqrt();
                    r.representation_rms_db.push((side, rms));
                }
                let gain = 10_f64.powf(r.trim_db / 20.);
                let mut data = conv::convolve(&old.data, &filter, conv::Mode::Full);
                if !full {
                    data.resize(old.len() + delay, 0.);
                }
                for v in &mut data {
                    *v *= gain;
                }
                for (name, q) in &supports {
                    let e = original.get(name).unwrap();
                    let support = if side == Side::Left {
                        &e.left
                    } else {
                        &e.right
                    };
                    if let Some(support) = support {
                        for (v, add) in
                            data.iter_mut()
                                .zip(conv::convolve(&support.data, q, conv::Mode::Full))
                        {
                            *v += add;
                        }
                    }
                }
                let len = (0.005 * fs as f64).round() as usize;
                let start = data.len().saturating_sub(len);
                if !full {
                    for (i, v) in data[start..].iter_mut().enumerate() {
                        *v *= 0.5 * (1. + (PI * i as f64 / len as f64).cos());
                    }
                }
                ir.data = data;
                r.excess_median_after_ms += v1::excess_group_delay_median(ir);
                for (a, b) in after.iter_mut().zip(v1::lf_decay_bands(ir)) {
                    *a += b;
                }
                echoes.push((side, v1::pre_echo_parts(&old), v1::pre_echo_parts(ir)));
            }
            let reference_before = echoes.iter().map(|e| e.1.1).fold(0., f64::max);
            let reference_after = echoes.iter().map(|e| e.2.1).fold(0., f64::max);
            r.pre_echo_channels = echoes
                .iter()
                .map(|(side, a, b)| v1::PreEcho {
                    side: *side,
                    before_db: v1::level_db(a.0, reference_before),
                    after_db: v1::level_db(b.0, reference_after),
                })
                .collect();
            if count > 0. {
                r.lf_edt_before_ms = median(&before.map(|v| v / count));
                r.lf_edt_after_ms = median(&after.map(|v| v / count));
                r.excess_median_before_ms /= count;
                r.excess_median_after_ms /= count;
            }
            r.weak = !r.representation_rms_db.is_empty()
                && r.representation_rms_db.iter().map(|(_, v)| v).sum::<f64>()
                    / r.representation_rms_db.len() as f64
                    > 4.;
            r.mismatch = report
                .pair_checks
                .iter()
                .any(|p| p.mismatch && p.speakers.contains(&r.speaker));
            Ok((r, ears))
        })
        .collect();
    for speaker in &mut hrir.speakers {
        if !measured.iter().any(|s| s.speaker == speaker.speaker) && delay > 0 {
            for ir in speaker.left.iter_mut().chain(&mut speaker.right) {
                let mut data = vec![0.; delay];
                data.extend_from_slice(&ir.data);
                ir.data = data;
            }
        }
    }
    for (r, ears) in updates? {
        let index = hrir
            .speakers
            .iter()
            .position(|s| s.speaker == r.speaker)
            .unwrap();
        hrir.speakers[index] = ears;
        let index = report
            .speakers
            .iter()
            .position(|s| s.speaker == r.speaker)
            .unwrap();
        report.speakers[index] = r;
    }
    Ok(report)
}
