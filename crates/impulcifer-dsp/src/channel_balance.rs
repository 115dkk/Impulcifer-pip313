#![forbid(unsafe_code)]
//! Channel balance, core/hrir.py:674-818.
use crate::{
    DspError,
    fr::{
        CenterAt, EqualizeParams, FrequencyResponse, SmoothingParams,
        magnitude_to_frequency_response,
    },
    hrir::Hrir,
};
use impulcifer_types::constants::IPSILATERAL_PAIRS;
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChannelBalance {
    Trend,
    Left,
    Right,
    Avg,
    Min,
    Mids,
    GainDb(f64),
}
impl ChannelBalance {
    /// Python channel_balance_firs numeric fallback, core/hrir.py:772-781; p10_channel_balance.
    pub fn parse(s: &str) -> Result<Self, DspError> {
        Ok(match s {
            "trend" => Self::Trend,
            "left" => Self::Left,
            "right" => Self::Right,
            "avg" => Self::Avg,
            "min" => Self::Min,
            "mids" => Self::Mids,
            _ => Self::GainDb(s.trim().parse().map_err(|_| {
                DspError::InvalidArgument(format!(
                    "\"{s}\" is not valid value for channel balance method."
                ))
            })?),
        })
    }
}
/// Python unit_impulse, core/hrir.py:694-695; p10_channel_balance.
fn impulse(n: usize) -> Vec<f64> {
    let mut x = vec![0.0; n];
    if n > 0 {
        x[0] = 1.0;
    }
    x
}
/// Python channel_balance_firs, core/hrir.py:674-783; p10_channel_balance.
pub fn channel_balance_firs(
    left_fr: &mut FrequencyResponse,
    right_fr: &mut FrequencyResponse,
    method: ChannelBalance,
    fs: u32,
) -> Result<[Vec<f64>; 2], DspError> {
    let smooth = |window, upper| SmoothingParams {
        window_size: window,
        treble_f_lower: 20000.0,
        treble_f_upper: upper,
        ..Default::default()
    };
    let eq = |lower| EqualizeParams {
        max_gain: 15.0,
        treble_f_lower: lower,
        treble_f_upper: fs as f64 / 2.0,
        ..Default::default()
    };
    match method {
        ChannelBalance::Mids | ChannelBalance::GainDb(_) => {
            let gain = if let ChannelBalance::GainDb(g) = method {
                g
            } else {
                right_fr.center_value((100.0, 3000.0)) - left_fr.center_value((100.0, 3000.0))
            };
            let left = impulse((fs as f64 * 0.1).round_ties_even() as usize);
            let right = left
                .iter()
                .map(|v| v * 10.0_f64.powf(gain / 20.0))
                .collect();
            Ok([left, right])
        }
        ChannelBalance::Trend => {
            let mut trend = FrequencyResponse::new(
                "trend",
                Some(left_fr.frequency.clone()),
                Some(
                    left_fr
                        .raw
                        .iter()
                        .zip(&right_fr.raw)
                        .map(|(l, r)| l - r)
                        .collect(),
                ),
            )?;
            trend.smoothen_fractional_octave(&smooth(2.0, (fs as f64 / 2.0).round_ties_even()))?;
            right_fr.equalization = trend.smoothed;
            let fir = right_fr.minimum_phase_impulse_response(fs, 10.0, false)?;
            Ok([impulse(fir.len()), fir])
        }
        ChannelBalance::Left | ChannelBalance::Right => {
            let (reference, subject) = if method == ChannelBalance::Left {
                (left_fr, right_fr)
            } else {
                (right_fr, left_fr)
            };
            reference.smoothen_fractional_octave(&smooth(
                1.0 / 3.0,
                (fs as f64 / 2.0).round_ties_even(),
            ))?;
            let gain = reference.center(CenterAt::Band(100.0, 10000.0))?;
            for x in &mut subject.raw {
                *x += gain;
            }
            subject.target = reference.smoothed.clone();
            subject.error = subject
                .raw
                .iter()
                .zip(&subject.target)
                .map(|(r, t)| r - t)
                .collect();
            subject.smoothen_heavy_light()?;
            subject.equalize(&eq(20000.0))?;
            let fir = subject.minimum_phase_impulse_response(fs, 10.0, false)?;
            Ok(if method == ChannelBalance::Left {
                [impulse(fir.len()), fir]
            } else {
                [fir.clone(), impulse(fir.len())]
            })
        }
        ChannelBalance::Avg | ChannelBalance::Min => {
            let gain = (left_fr.center_value((100.0, 10000.0))
                + right_fr.center_value((100.0, 10000.0)))
                / 2.0;
            for fr in [&mut *left_fr, &mut *right_fr] {
                for x in &mut fr.raw {
                    *x += gain;
                }
                fr.smoothen_fractional_octave(&smooth(1.0 / 3.0, 23999.0))?;
            }
            let target: Vec<_> = left_fr
                .raw
                .iter()
                .zip(&right_fr.raw)
                .map(|(&l, &r)| {
                    if method == ChannelBalance::Avg {
                        (l + r) / 2.0
                    } else {
                        l.min(r)
                    }
                })
                .collect();
            let mut firs = Vec::new();
            for fr in [left_fr, right_fr] {
                fr.target = target.clone();
                fr.error = fr.raw.iter().zip(&target).map(|(r, t)| r - t).collect();
                fr.smoothen_fractional_octave(&smooth(1.0 / 3.0, 23999.0))?;
                fr.equalize(&eq(2000.0))?;
                firs.push(fr.minimum_phase_impulse_response(fs, 10.0, false)?);
            }
            Ok([firs.remove(0), firs.remove(0)])
        }
    }
}
/// Python correct_channel_balance, core/hrir.py:785-818; p10_channel_balance.
pub fn correct_channel_balance(hrir: &mut Hrir, method: ChannelBalance) -> Result<(), DspError> {
    for (a, b) in IPSILATERAL_PAIRS {
        let names = if a == b { vec![a] } else { vec![a, b] };
        if names.iter().any(|s| hrir.get(s).is_none()) {
            continue;
        }
        let mut responses = Vec::new();
        for left in [true, false] {
            let rows: Result<Vec<_>, _> = names
                .iter()
                .map(|s| {
                    let p = hrir.get(s).unwrap();
                    if left {
                        p.left.as_ref()
                    } else {
                        p.right.as_ref()
                    }
                    .ok_or_else(|| {
                        DspError::InvalidArgument("channel balance requires both ears".into())
                    })
                })
                .collect();
            let rows = rows?;
            let mut data = vec![0.0; rows[0].len()];
            for ir in rows {
                if ir.len() != data.len() {
                    return Err(DspError::InvalidArgument(
                        "channel balance group lengths differ".into(),
                    ));
                }
                for (x, y) in data.iter_mut().zip(&ir.data) {
                    *x += y;
                }
            }
            for x in &mut data {
                *x /= names.len() as f64;
            }
            responses.push(magnitude_to_frequency_response(
                "Frequency response",
                hrir.fs,
                &data,
            )?);
        }
        let mut right = responses.pop().unwrap();
        let mut left = responses.pop().unwrap();
        let firs = channel_balance_firs(&mut left, &mut right, method, hrir.fs)?;
        for name in names {
            let pair = hrir.get_mut(name).unwrap();
            pair.left.as_mut().unwrap().equalize(&firs[0]);
            pair.right.as_mut().unwrap().equalize(&firs[1]);
        }
    }
    Ok(())
}
