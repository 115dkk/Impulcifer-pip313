//! Parallel file-free EQ worker, core/parallel_workers.py:69-131.
use super::{headphone::HeadphoneCompensation, room::RoomFrs};
use crate::{
    DspError,
    fr::{EqualizeParams, FrequencyResponse, generate_frequencies},
    hrir::Hrir,
};
use impulcifer_types::constants::Side;
use rayon::prelude::*;
pub struct EqInputs<'a> {
    pub room_frs: Option<&'a RoomFrs>,
    pub hp: Option<&'a HeadphoneCompensation>,
    pub eq_left: Option<&'a FrequencyResponse>,
    pub eq_right: Option<&'a FrequencyResponse>,
    pub target: &'a FrequencyResponse,
    pub fs: u32,
}
/// Python process_equalization_worker, core/parallel_workers.py:69-131; p10_eq_firs.
pub fn equalization_fir(
    inputs: &EqInputs<'_>,
    speaker: &str,
    side: Side,
) -> Result<Vec<f64>, DspError> {
    if side == Side::Center {
        return Err(DspError::InvalidArgument(
            "ear must be left or right".into(),
        ));
    }
    let mut fr = FrequencyResponse::constant(
        &format!("{speaker}-{side:?} eq"),
        Some(generate_frequencies(10.0, inputs.fs as f64 / 2.0, 1.01)),
        0.0,
        0.0,
    )?;
    let room = inputs.room_frs.and_then(|r| {
        r.0.iter()
            .find(|(s, e, _)| s == speaker && *e == side)
            .map(|(_, _, f)| f)
    });
    let hp = inputs.hp.map(|hp| {
        if side == Side::Left {
            &hp.left
        } else {
            &hp.right
        }
    });
    let eq = if side == Side::Left {
        inputs.eq_left
    } else {
        inputs.eq_right
    };
    for source in [room, hp, eq].into_iter().flatten() {
        if source.error.len() != fr.error.len() {
            return Err(DspError::InvalidArgument(
                "EQ error grid lengths differ".into(),
            ));
        }
        for (e, v) in fr.error.iter_mut().zip(&source.error) {
            *e += v;
        }
    }
    if inputs.target.raw.len() != fr.error.len() {
        return Err(DspError::InvalidArgument(
            "target grid lengths differ".into(),
        ));
    }
    for (e, t) in fr.error.iter_mut().zip(&inputs.target.raw) {
        *e -= t;
    }
    fr.smoothen_heavy_light()?;
    fr.equalize(&EqualizeParams {
        max_gain: 40.0,
        treble_f_lower: 10000.0,
        treble_f_upper: inputs.fs as f64 / 2.0,
        ..Default::default()
    })?;
    fr.minimum_phase_impulse_response(inputs.fs, 5.0, false)
}
/// Python _stage_equalize, core/pipeline.py:655-700; p10_default_equalize.
pub fn equalize_hrir(hrir: &mut Hrir, inputs: &EqInputs<'_>) -> Result<(), DspError> {
    let tasks: Vec<_> = hrir
        .speakers
        .iter()
        .flat_map(|s| {
            [(Side::Left, &s.left), (Side::Right, &s.right)]
                .into_iter()
                .filter(|(_, i)| i.is_some())
                .map(|(e, _)| (s.speaker.clone(), e))
        })
        .collect();
    let firs: Result<Vec<_>, _> = tasks
        .par_iter()
        .map(|(s, e)| equalization_fir(inputs, s, *e))
        .collect();
    for ((s, e), fir) in tasks.into_iter().zip(firs?) {
        let pair = hrir.get_mut(&s).unwrap();
        let ir = if e == Side::Left {
            &mut pair.left
        } else {
            &mut pair.right
        };
        ir.as_mut().unwrap().equalize(&fir);
    }
    Ok(())
}
