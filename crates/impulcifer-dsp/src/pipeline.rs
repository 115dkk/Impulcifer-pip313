#![forbid(unsafe_code)]
//! In-memory stage driver; core/pipeline.py:424-972.
use crate::{
    DspError,
    channel_balance::{ChannelBalance, correct_channel_balance},
    estimator::SweepEstimator,
    fr::FrequencyResponse,
    hrir::{Hrir, compact_tracks},
    mic_deviation::{MicDeviationOptions, apply_mic_deviation_correction},
    stages::{
        equalize::{EqInputs, equalize_hrir},
        headphone::HeadphoneCompensation,
        readme::{ReadmeData, readme_data},
        room::RoomCorrection,
    },
    virtual_bass::{VirtualBassOptions, apply_virtual_bass},
};
use impulcifer_types::{
    config::{DecaySpec, ProcessingConfig},
    constants::{
        HESUVI_TRACK_ORDER, HEXADECAGONAL_TRACK_ORDER, IPSILATERAL_PAIRS, SPEAKER_NAMES,
        TRUEHD_11CH_ORDER, TRUEHD_13CH_ORDER,
    },
    stages::StageKey,
};
#[derive(Clone, Debug)]
pub struct PipelineInputs {
    pub estimator: SweepEstimator,
    pub hrir: Hrir,
    pub room: Option<RoomCorrection>,
    pub headphone: Option<HeadphoneCompensation>,
    pub eq_left: Option<FrequencyResponse>,
    pub eq_right: Option<FrequencyResponse>,
}
#[derive(Clone, Debug)]
pub struct StageProgress {
    pub key: StageKey,
    pub step: usize,
    pub total: usize,
}
pub trait StageObserver {
    /// Python logger.step, core/pipeline.py:516-972; p10_stage_table.
    fn on_stage(&mut self, progress: StageProgress);
    /// Python check_cancelled, core/pipeline.py:505-509; p10_stage_table.
    fn check_cancelled(&self) -> Result<(), DspError>;
}
#[derive(Clone, Debug)]
pub struct PipelineOutputs {
    pub hrir: Hrir,
    pub applied_gain_db: f64,
    pub readme: ReadmeData,
    pub hrir_tracks: Vec<Vec<f64>>,
    pub hesuvi_tracks: Vec<Vec<f64>>,
    pub responses_tracks: Vec<Vec<f64>>,
    pub truehd: Vec<(String, Vec<String>, Vec<Vec<f64>>)>,
    pub jamesdsp: Option<Vec<Vec<f64>>>,
    pub hangloose: Vec<(String, Vec<Vec<f64>>)>,
}
/// Python bool(cfg.decay), core/pipeline.py:457; p10_stage_table.
fn has_decay(c: &ProcessingConfig) -> bool {
    match &c.decay {
        None => false,
        Some(DecaySpec::Uniform(v)) => *v != 0.0,
        Some(DecaySpec::PerChannel(m)) => !m.is_empty(),
    }
}
/// Python _stage_table, core/pipeline.py:424-470; p10_stage_table.
pub fn stage_table(c: &ProcessingConfig) -> Vec<(StageKey, bool, usize)> {
    use StageKey::*;
    vec![
        (Estimator, true, 1),
        (RoomCorrection, c.do_room_correction, 1),
        (HeadphoneCompensation, c.do_headphone_compensation, 1),
        (EqualizationFiles, c.do_equalization, 1),
        (Target, true, 1),
        (OpenMeasurements, true, 1),
        (PlotPre, c.plot, 1),
        (CropAndAlign, true, 1),
        (VirtualBass, c.vbass, 1),
        (
            MicDeviationSkipped,
            c.microphone_deviation_correction && c.do_headphone_compensation,
            0,
        ),
        (
            MicDeviation,
            c.microphone_deviation_correction && !c.do_headphone_compensation,
            1,
        ),
        (WriteResponses, true, 0),
        (
            Equalize,
            c.do_room_correction || c.do_headphone_compensation || c.do_equalization,
            1,
        ),
        (Decay, has_decay(c), 1),
        (ChannelBalance, c.channel_balance.is_some(), 1),
        (Normalize, true, 1),
        (WriteReadme, true, 0),
        (PlotPost, c.plot, 1),
        (PlotResults, true, 1),
        (PlotAdditional, c.plot, 1),
        (InteractivePlots, c.interactive_plots, 1),
        (Resample, c.fs.is_some(), 1),
        (WriteBrirs, true, 1),
        (TruehdLayouts, c.output_truehd_layouts, 1),
        (Jamesdsp, c.jamesdsp, 1),
        (Hangloose, c.hangloose, 1),
    ]
}
/// Python run total_steps, core/pipeline.py:487-489; p10_stage_table.
pub fn total_steps(c: &ProcessingConfig) -> usize {
    stage_table(c)
        .iter()
        .filter(|(_, enabled, _)| *enabled)
        .map(|(_, _, n)| n)
        .sum()
}
/// Python normalize and discarded second gain, core/pipeline.py:733-743,859-871; p10_resample.
fn normalize(hrir: &mut Hrir, c: &ProcessingConfig) -> Result<f64, DspError> {
    hrir.normalize(
        if c.target_level.is_none() {
            Some(-0.1)
        } else {
            None
        },
        c.target_level,
    )
}
/// Python _stage_write_brirs, core/pipeline.py:873-895; p10_default_final.
fn output_tracks(hrir: &Hrir, order: &[&str], compact: bool) -> Result<Vec<Vec<f64>>, DspError> {
    let tracks = hrir.stack_tracks(order, !compact)?;
    if compact {
        let (tracks, _) = compact_tracks(&tracks, order);
        if tracks.is_empty() {
            return Err(DspError::InvalidArgument(
                "all output channels are silent".into(),
            ));
        }
        Ok(tracks)
    } else {
        Ok(tracks)
    }
}
/// Python run and DSP stage bodies, core/pipeline.py:424-972; p10_default,
/// p10_vbass, p10_decay, p10_resample. File/plot rows only emit progress.
pub fn run_pipeline(
    config: &ProcessingConfig,
    inputs: PipelineInputs,
    observer: &mut dyn StageObserver,
) -> Result<PipelineOutputs, DspError> {
    let PipelineInputs {
        estimator,
        mut hrir,
        room,
        headphone,
        eq_left,
        eq_right,
    } = inputs;
    let total = total_steps(config);
    let mut step = 0;
    let mut target = None;
    let mut applied_gain_db = 0.0;
    let mut readme = None;
    let mut responses_tracks = Vec::new();
    let mut hrir_tracks = Vec::new();
    let mut hesuvi_tracks = Vec::new();
    let mut truehd = Vec::new();
    let mut jamesdsp = None;
    let mut hangloose = Vec::new();
    for (key, enabled, n) in stage_table(config) {
        if !enabled {
            continue;
        }
        observer.check_cancelled()?;
        step += n;
        observer.on_stage(StageProgress { key, step, total });
        match key {
            StageKey::Target => {
                target = Some(crate::stages::target::create_target(
                    estimator.fs,
                    config.bass_boost_gain,
                    config.bass_boost_fc,
                    config.bass_boost_q,
                    config.tilt,
                ))
            }
            StageKey::CropAndAlign => {
                hrir.crop_heads(config.head_ms)?;
                hrir.align_ipsilateral_all(&IPSILATERAL_PAIRS, 30.0);
                hrir.align_onset_groups_peak_leftref(None)?;
                hrir.crop_tails(&estimator)?;
            }
            StageKey::VirtualBass => apply_virtual_bass(
                &mut hrir,
                &VirtualBassOptions {
                    crossover_freq: config.vbass_freq,
                    head_ms: config.head_ms,
                    hp_freq: config.vbass_hp,
                    invert_polarity: match config.vbass_polarity.as_str() {
                        "normal" => Some(false),
                        "invert" => Some(true),
                        _ => None,
                    },
                },
            )?,
            StageKey::MicDeviation => {
                apply_mic_deviation_correction(
                    &mut hrir,
                    &MicDeviationOptions {
                        correction_strength: config.mic_deviation_strength,
                        ..Default::default()
                    },
                )?;
            }
            StageKey::WriteResponses => {
                responses_tracks = hrir.stack_tracks(&HEXADECAGONAL_TRACK_ORDER, false)?
            }
            StageKey::Equalize => equalize_hrir(
                &mut hrir,
                &EqInputs {
                    room_frs: room
                        .as_ref()
                        .filter(|_| config.do_room_correction)
                        .map(|r| &r.frs),
                    hp: headphone
                        .as_ref()
                        .filter(|_| config.do_headphone_compensation),
                    eq_left: eq_left.as_ref().filter(|_| config.do_equalization),
                    eq_right: eq_right.as_ref().filter(|_| config.do_equalization),
                    target: target.as_ref().unwrap(),
                    fs: estimator.fs,
                },
            )?,
            StageKey::Decay => {
                let targets: Vec<(String, f64)> = match config.decay.as_ref().unwrap() {
                    DecaySpec::Uniform(v) => {
                        SPEAKER_NAMES.iter().map(|s| ((*s).into(), *v)).collect()
                    }
                    DecaySpec::PerChannel(m) => m.iter().map(|(s, v)| (s.clone(), *v)).collect(),
                };
                crate::stages::decay::adjust_decay(&mut hrir, &targets)?;
            }
            StageKey::ChannelBalance => correct_channel_balance(
                &mut hrir,
                ChannelBalance::parse(config.channel_balance.as_ref().unwrap())?,
            )?,
            StageKey::Normalize => applied_gain_db = normalize(&mut hrir, config)?,
            StageKey::WriteReadme => {
                readme = Some(readme_data(
                    &hrir,
                    config.fs.unwrap_or(hrir.fs),
                    applied_gain_db,
                ))
            }
            StageKey::Resample => {
                if let Some(fs) = config.fs.filter(|fs| *fs != hrir.fs) {
                    hrir.resample(fs)?;
                    normalize(&mut hrir, config)?;
                }
            }
            StageKey::WriteBrirs => {
                hrir_tracks = output_tracks(
                    &hrir,
                    &HEXADECAGONAL_TRACK_ORDER,
                    config.remove_silent_channels,
                )?;
                hesuvi_tracks =
                    output_tracks(&hrir, &HESUVI_TRACK_ORDER, config.remove_silent_channels)?;
            }
            StageKey::TruehdLayouts => {
                for (label, order, min) in [
                    ("11ch", TRUEHD_11CH_ORDER.as_slice(), 8),
                    ("13ch", TRUEHD_13CH_ORDER.as_slice(), 10),
                ] {
                    let available: Vec<_> =
                        order.iter().filter(|s| hrir.get(s).is_some()).collect();
                    if available.len() >= min {
                        let names: Vec<_> = available
                            .iter()
                            .flat_map(|s| [format!("{s}-left"), format!("{s}-right")])
                            .collect();
                        let tracks = hrir.stack_tracks(
                            &names.iter().map(String::as_str).collect::<Vec<_>>(),
                            false,
                        )?;
                        truehd.push((
                            format!("truehd_{label}_{}ch.wav", available.len()),
                            names,
                            tracks,
                        ));
                    }
                }
            }
            StageKey::Jamesdsp => {
                let mut subset = hrir.subset(&["FL", "FR"]);
                normalize(&mut subset, config)?;
                jamesdsp = Some(
                    subset.stack_tracks(&["FL-left", "FL-right", "FR-left", "FR-right"], false)?,
                );
            }
            StageKey::Hangloose => {
                for s in SPEAKER_NAMES {
                    if hrir.get(s).is_some() {
                        hangloose.push((
                            s.into(),
                            hrir.stack_tracks(
                                &[&format!("{s}-left"), &format!("{s}-right")],
                                false,
                            )?,
                        ));
                    }
                }
            }
            _ => (),
        }
        observer.check_cancelled()?;
    }
    Ok(PipelineOutputs {
        hrir,
        applied_gain_db,
        readme: readme.unwrap(),
        hrir_tracks,
        hesuvi_tracks,
        responses_tracks,
        truehd,
        jamesdsp,
        hangloose,
    })
}
