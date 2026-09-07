//! Lundeby and Schroeder analysis, core/decay.py. All sample arithmetic is f64.
use crate::{peaks, stats, windows};

#[derive(Clone, Debug)]
pub struct DecayParams {
    pub peak_index: usize,
    pub knee_index: usize,
    pub noise_floor_db: f64,
    pub window_size: usize,
}
#[derive(Clone, Debug, Default)]
pub struct DecayTimes {
    pub edt: Option<f64>,
    pub rt20: Option<f64>,
    pub rt30: Option<f64>,
    pub rt60: Option<f64>,
}
#[derive(Clone, Debug)]
pub struct DecayAdjustment {
    pub window_start: usize,
    pub half_window: usize,
    pub knee_point_index: usize,
    pub window_level: f64,
}

/// Python int(), core/decay.py:81-86; p08_rounding.json.
fn py_int(x: f64) -> i64 {
    x.trunc() as i64
}
/// Python //, core/decay.py:287-290; p08_rounding.json.
fn py_floor_div(a: i64, b: i64) -> i64 {
    let q = a / b;
    if a % b != 0 && (a < 0) != (b < 0) {
        q - 1
    } else {
        q
    }
}
/// Python round(), estimator:118 and hrir:158; p08_rounding.json.
pub(crate) fn py_round(x: f64) -> i64 {
    x.round_ties_even() as i64
}
/// Python np.mean, core/decay.py:104; p08_decay_*.json.
fn mean(x: &[f64]) -> f64 {
    x.iter().sum::<f64>() / x.len() as f64
}
/// Python linspace(0,n/fs,n), core/decay.py:78; p08_decay_*.json.
fn times(n: usize, fs: u32) -> Vec<f64> {
    let end = n as f64 / fs as f64;
    let mut t: Vec<_> = (0..n)
        .map(|i| i as f64 * (end / (n.saturating_sub(1)) as f64))
        .collect();
    if n == 1 {
        t[0] = 0.0;
    } else if n > 1 {
        t[n - 1] = end;
    }
    t
}
/// Python argmin(abs(t-value)), core/decay.py:196-197; p08_decay_*.json.
fn nearest(t: &[f64], value: f64) -> usize {
    let mut best = 0;
    for i in 1..t.len() {
        if (t[i] - value).abs() < (t[best] - value).abs() {
            best = i;
        }
    }
    best
}
/// Python reshape/mean/log10, core/decay.py:103-105; p08_decay_*.json.
fn power_windows(squared: &[f64], n: usize, w: usize) -> Vec<f64> {
    squared[..n * w]
        .chunks_exact(w)
        .map(|x| 10.0 * mean(x).max(1e-20).log10())
        .collect()
}

/// Python decay_params, core/decay.py:44-260; p08_decay_*.json.
pub fn decay_params(data: &[f64], fs: u32) -> DecayParams {
    decay_params_with_first_pass(data, fs, |_, _, _| {})
}

/// Python decay_params:44-260; observe first-pass locals for p08_decay_first_pass.json.
fn decay_params_with_first_pass(
    data: &[f64],
    fs: u32,
    first_pass: impl FnOnce(&[f64], &[f64], f64),
) -> DecayParams {
    if data.len() < 10 {
        return DecayParams {
            peak_index: 0,
            knee_index: data.len(),
            noise_floor_db: -200.0,
            window_size: data.len().max(1),
        };
    }
    let peak = peaks::first_peak_index(data, 0, None, 0.12589);
    let data = &data[peak..(peak + 2 * fs as usize).min(data.len())];
    let maximum = data.iter().map(|x| x.abs()).fold(0.0, f64::max);
    let squared: Vec<_> = data
        .iter()
        .map(|x| {
            if maximum >= 1e-20 {
                (x / maximum).powi(2)
            } else {
                x * x
            }
        })
        .collect();
    let t = times(squared.len(), fs);
    let fallback = |noise, w| DecayParams {
        peak_index: peak,
        knee_index: peak + squared.len(),
        noise_floor_db: noise,
        window_size: w,
    };
    let mut wd = 0.03;
    let mut n = if fs > 0 {
        py_int(squared.len() as f64 / fs as f64 / wd) as usize
    } else {
        0
    };
    if n == 0 {
        return fallback(
            10.0 * mean(&squared).max(1e-20).log10(),
            squared.len().max(1),
        );
    }
    let mut w = (squared.len() / n).max(1);
    let mut tw: Vec<_> = (0..n).map(|i| i as f64 * wd + wd / 2.0).collect();
    // Python's slice truncates before reshape; retain the original time axis.
    if squared.len() < n * w {
        n = squared.len() / w;
    }
    if n == 0 {
        return fallback(10.0 * mean(&squared).max(1e-20).log10(), w);
    }
    let mut powers = power_windows(&squared, n, w);
    let tail = &squared[py_int(squared.len() as f64 * 0.9) as usize..];
    let mut noise = 10.0 * mean(tail).max(1e-20).log10();
    first_pass(&powers, &tw, noise);
    let end = powers
        .iter()
        .position(|x| *x <= noise + 10.0)
        .filter(|i| *i > 0)
        .unwrap_or(n);
    let end = if end < 2 { n } else { end };
    if end < 2 {
        return fallback(noise, w);
    }
    let regression = stats::linregress(&tw[..end], &powers[..end]);
    if regression.slope.is_nan() || regression.slope.abs() < 1e-20 {
        return fallback(noise, w);
    }
    let mut knee_time =
        ((noise - regression.intercept) / regression.slope).clamp(t[0], t[t.len() - 1]);
    wd = 10.0 / (regression.slope.abs() * 3.0);
    n = (py_int(squared.len() as f64 / fs as f64 / wd) as usize).max(1);
    w = (squared.len() / n).max(1);
    n = n.min(squared.len() / w);
    if n == 0 {
        return DecayParams {
            peak_index: peak,
            knee_index: peak + nearest(&t, knee_time),
            noise_floor_db: noise,
            window_size: w,
        };
    }
    tw = (0..n).map(|i| i as f64 * wd + wd / 2.0).collect();
    powers = power_windows(&squared, n, w);
    let mut knee = tw.iter().position(|x| *x >= knee_time).unwrap_or_else(|| {
        knee_time = tw[n - 1];
        n - 1
    });
    let mut knee_value = powers[knee];
    for _ in 0..5 {
        let Some(start_index) = powers.iter().position(|x| *x <= knee_value - 5.0) else {
            break;
        };
        let total = t[t.len() - 1];
        let start_time = tw[start_index].max(0.1 * total);
        if start_time > tw[n - 1] {
            break;
        }
        let end_time = (start_time + knee_time).min(total);
        let (start, end) = (nearest(&t, start_time), nearest(&t, end_time));
        if start >= end {
            break;
        }
        noise = 10.0 * mean(&squared[start..end]).max(1e-20).log10();
        let Some(end) = powers.iter().position(|x| *x <= noise + 8.0) else {
            break;
        };
        let Some(start) = powers.iter().position(|x| *x <= noise + 28.0) else {
            break;
        };
        let start = start.saturating_sub(1);
        if end as i64 - 1 <= start as i64 + 1 {
            break;
        }
        let end = end - 1;
        let late = stats::linregress(&tw[start..end], &powers[start..end]);
        if late.slope.is_nan() || late.slope.abs() < 1e-20 {
            break;
        }
        let new_time = ((noise - late.intercept) / late.slope).clamp(tw[0], tw[n - 1]);
        let new_knee = tw.iter().position(|x| *x >= new_time).unwrap_or(n - 1);
        knee_time = tw[new_knee];
        if new_knee == knee {
            break;
        }
        knee = new_knee;
        knee_value = powers[knee];
    }
    DecayParams {
        peak_index: peak,
        knee_index: peak + nearest(&t, knee_time),
        noise_floor_db: noise,
        window_size: w,
    }
}

/// Python decay_times, core/decay.py:263-352; p08_decay_*.json.
pub fn decay_times(data: &[f64], fs: u32, params: Option<&DecayParams>) -> DecayTimes {
    let owned;
    let p = match params {
        Some(p) => p,
        None => {
            owned = decay_params(data, fs);
            &owned
        }
    };
    let peak = p.peak_index;
    if data.is_empty() || peak >= data.len() {
        return DecayTimes::default();
    }
    let knee = p.knee_index - peak;
    let t = times(data.len(), fs);
    let maximum = data[peak..].iter().map(|x| x.abs()).fold(0.0, f64::max);
    let power: Vec<_> = data[peak..].iter().map(|x| (x / maximum).powi(2)).collect();
    let denominator = power[..knee.min(power.len())].iter().sum::<f64>();
    let mut sum = 0.0;
    let mut schroeder: Vec<_> = power[..(knee + 1).min(power.len())]
        .iter()
        .rev()
        .map(|x| {
            sum += x / denominator;
            sum
        })
        .collect();
    schroeder.reverse();
    schroeder.pop();
    schroeder.iter_mut().for_each(|x| *x = 10.0 * x.log10());
    let half = py_floor_div(p.window_size as i64, 2) as usize;
    let head = half.min(peak);
    let tail = half.min(data.len().saturating_sub(peak + knee));
    let offset_index = half - head;
    let segment = &data[peak - head..(peak + knee + tail).min(data.len())];
    let maximum = segment.iter().map(|x| x.abs()).fold(0.0, f64::max);
    let squared: Vec<_> = segment.iter().map(|x| (x / maximum).powi(2)).collect();
    let avg: Vec<_> = stats::running_mean(&squared, p.window_size)
        .iter()
        .map(|x| 10.0 * (x + 1e-18).log10())
        .collect();
    let start = (py_int(schroeder.len() as f64 * 0.1) as usize).max(offset_index);
    let end = (py_int(schroeder.len() as f64 * 0.9) as usize).min(offset_index + avg.len());
    let offset = if end > start {
        (start..end)
            .map(|i| schroeder[i] - avg[i - offset_index])
            .sum::<f64>()
            / (end - start) as f64
    } else {
        f64::NAN
    };
    let mut result = [None; 4];
    for (i, (start_target, end_target, level)) in [
        (-1.0, -10.0, -10.0),
        (-5.0, -25.0, -20.0),
        (-5.0, -35.0, -30.0),
        (-5.0, -65.0, -60.0),
    ]
    .iter()
    .enumerate()
    {
        if *end_target < p.noise_floor_db + offset + 10.0 {
            continue;
        }
        let Some(start) = schroeder.iter().position(|x| x <= start_target) else {
            continue;
        };
        let Some(end) = schroeder.iter().position(|x| x <= end_target) else {
            continue;
        };
        if end >= start + 2 {
            result[i] =
                Some(level / stats::linregress(&t[start..end], &schroeder[start..end]).slope);
        }
    }
    DecayTimes {
        edt: result[0],
        rt20: result[1],
        rt30: result[2],
        rt60: result[3],
    }
}

/// Python decay_adjustment_params, core/decay.py:355-380; p08_decay_*.json.
/// Uses the LAST consecutively available time, stopping at the first missing one.
///
/// # Panics
/// A zero target panics, matching Python's ZeroDivisionError rather than producing
/// an infinite window level. The fixed Option return type cannot return an error.
pub fn decay_adjustment_params(
    data: &[f64],
    fs: u32,
    target_seconds: f64,
) -> Option<DecayAdjustment> {
    assert!(target_seconds != 0.0, "decay target must be nonzero");
    let p = decay_params(data, fs);
    let t = decay_times(data, fs, Some(&p));
    let mut slope = None;
    for (time, level) in [
        (t.edt, -10.0),
        (t.rt20, -20.0),
        (t.rt30, -30.0),
        (t.rt60, -60.0),
    ] {
        match time {
            Some(time) if time != 0.0 => slope = Some(level / time),
            _ => break,
        }
    }
    let slope = slope?;
    let target = -60.0 / target_seconds;
    if target > slope {
        return None;
    }
    let knee_time = p.knee_index as f64 / fs as f64;
    let start = p.peak_index + 2 * py_floor_div(fs as i64, 1000) as usize;
    Some(DecayAdjustment {
        window_start: start,
        half_window: p.knee_index.checked_sub(start)?,
        knee_point_index: p.knee_index,
        window_level: target * knee_time - slope * knee_time,
    })
}

/// Python apply_decay_window, core/decay.py:383-403; p08_decay_*.json.
pub fn apply_decay_window(data: &mut [f64], params: Option<&DecayAdjustment>) {
    let Some(p) = params else { return };
    let window = windows::hann(2 * p.half_window, true);
    for (i, x) in data.iter_mut().enumerate() {
        let w = if i < p.window_start {
            1.0
        } else if i < p.knee_point_index {
            window[p.half_window + i - p.window_start]
        } else {
            0.0
        };
        *x *= 10.0_f64.powf((w - 1.0) * -p.window_level / 20.0);
    }
}

#[cfg(test)]
mod tests {
    /// Python decay_params:91-110; full first-pass locals in p08_decay_first_pass.json.
    #[test]
    fn golden_decay_first_pass_matches_python() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/migration/goldens");
        let fixture: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("p08_decay_first_pass.json")).unwrap())
                .unwrap();
        let descriptor = &fixture["inputs"]["data"];
        let bytes = std::fs::read(root.join(descriptor["file"].as_str().unwrap())).unwrap();
        assert_eq!(
            bytes.len(),
            descriptor["length"].as_u64().unwrap() as usize * 8
        );
        let data: Vec<_> = bytes
            .chunks_exact(8)
            .map(|x| f64::from_le_bytes(x.try_into().unwrap()))
            .collect();
        let mut observed = false;
        super::decay_params_with_first_pass(
            &data,
            fixture["inputs"]["fs"].as_u64().unwrap() as u32,
            |windows, times, noise| {
                observed = true;
                let expected = &fixture["outputs"]["first_pass"];
                for (name, actual, tolerance) in [
                    ("windows", windows, 1e-9),
                    ("t_windows", times, 0.0),
                    ("noise_floor", std::slice::from_ref(&noise), 1e-9),
                ] {
                    let expected: Vec<_> = if name == "noise_floor" {
                        vec![expected[name].as_f64().unwrap()]
                    } else {
                        expected[name]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_f64().unwrap())
                            .collect()
                    };
                    assert_eq!(actual.len(), expected.len(), "{name} count");
                    let mut max = 0.0_f64;
                    for (i, (&a, b)) in actual.iter().zip(expected).enumerate() {
                        let error = (a - b).abs();
                        assert!(
                            error.is_finite() && error <= tolerance,
                            "first-pass {name}/{i}: actual={a}, expected={b}, error={error}"
                        );
                        max = max.max(error);
                    }
                    println!("MEASURE first-pass {name}: max_abs={max:.17e}, atol={tolerance:.3e}");
                }
            },
        );
        assert!(observed, "first-pass comparison must run");
    }

    /// CPython int, //, round; p08_rounding.json, decay.py:81,287.
    #[test]
    fn python_rounding_helpers_match_cpython() {
        assert_eq!(super::py_int(-2.9), -2);
        assert_eq!(super::py_floor_div(-7, 3), -3);
        assert_eq!(super::py_floor_div(7, -3), -3);
        assert_eq!(super::py_round(2.5), 2);
        assert_eq!(super::py_round(3.5), 4);
        assert_eq!(super::py_round(-2.5), -2);
    }
}
