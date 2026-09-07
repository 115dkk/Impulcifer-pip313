//! Scalar statistics and the application's zero-padded smoothing helpers.

#[derive(Debug, Clone)]
pub struct LinRegress {
    pub slope: f64,
    pub intercept: f64,
    pub rvalue: f64,
    pub pvalue: Option<f64>,
    pub stderr: f64,
}

/// scipy.stats.linregress consumed fields, with pvalue deliberately absent.
/// Panics on unequal lengths. Empty/singleton inputs, constant x and
/// nonfinite samples return NaNs, matching the installed SciPy 1.18 oracle.
/// Two samples have NaN stderr in 1.18 (older SciPy returned zero).
pub fn linregress(x: &[f64], y: &[f64]) -> LinRegress {
    assert_eq!(x.len(), y.len(), "regression lengths must match");
    let nan = || LinRegress {
        slope: f64::NAN,
        intercept: f64::NAN,
        rvalue: f64::NAN,
        pvalue: None,
        stderr: f64::NAN,
    };
    if x.len() < 2 || !x.iter().chain(y).all(|v| v.is_finite()) {
        return nan();
    }
    if x.iter().all(|v| *v == x[0]) {
        return nan();
    }
    let n = x.len() as f64;
    let mx = x.iter().sum::<f64>() / n;
    let my = y.iter().sum::<f64>() / n;
    let mut xx = 0.0;
    let mut yy = 0.0;
    let mut xy = 0.0;
    for (&x, &y) in x.iter().zip(y) {
        xx += (x - mx) * (x - mx);
        yy += (y - my) * (y - my);
        xy += (x - mx) * (y - my);
    }
    let slope = xy / xx;
    // SciPy 1.18 returns NaN for constant y (rather than the older zero).
    let rvalue = if xx == 0.0 || yy == 0.0 {
        f64::NAN
    } else {
        (xy / (xx * yy).sqrt()).clamp(-1.0, 1.0)
    };
    let stderr = if x.len() == 2 {
        f64::NAN
    } else {
        ((1.0 - rvalue * rvalue) * yy / xx / (n - 2.0)).sqrt()
    };
    LinRegress {
        slope,
        intercept: my - slope * mx,
        rvalue,
        pvalue: None,
        stderr,
    }
}

/// Stable logistic sigmoid. Accepts infinities (0/1) and propagates NaN.
pub fn expit(x: f64) -> f64 {
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let e = x.exp();
        e / (1.0 + e)
    }
}

/// core.audio_io.running_mean: differences of sequential cumulative sums,
/// output N-n+1, or empty when n>N. Panics for n=0; nonfinite values propagate.
pub fn running_mean(x: &[f64], n: usize) -> Vec<f64> {
    assert!(n > 0, "running mean width must be positive");
    if n > x.len() {
        return Vec::new();
    }
    let mut sum = Vec::with_capacity(x.len() + 1);
    sum.push(0.0);
    for v in x {
        sum.push(sum.last().unwrap() + v);
    }
    (n..sum.len())
        .map(|i| (sum[i] - sum[i - n]) / n as f64)
        .collect()
}

/// scipy.ndimage.uniform_filter(size=3, mode='constant', cval=0), axis 0
/// then axis 1, using the same sliding sums (not a differently rounded 3x3 sum).
/// Panics on shape overflow/mismatch or nonfinite data (SciPy documents NaN
/// behavior as undefined). Zero-sized dimensions return an empty vector.
pub fn uniform_filter2d_size3_zero(rows: usize, cols: usize, data: &[f64]) -> Vec<f64> {
    assert_eq!(
        rows.checked_mul(cols).expect("matrix shape overflow"),
        data.len(),
        "matrix shape mismatch"
    );
    assert!(
        data.iter().all(|x| x.is_finite()),
        "uniform filter requires finite input"
    );
    if data.is_empty() {
        return Vec::new();
    }
    let mut temp = vec![0.0; data.len()];
    for c in 0..cols {
        let mut sum = data[c] + if rows > 1 { data[cols + c] } else { 0.0 };
        for r in 0..rows {
            temp[r * cols + c] = sum / 3.0;
            let add = if r + 2 < rows {
                data[(r + 2) * cols + c]
            } else {
                0.0
            };
            let remove = if r > 0 { data[(r - 1) * cols + c] } else { 0.0 };
            sum += add - remove;
        }
    }
    let mut output = vec![0.0; data.len()];
    for r in 0..rows {
        let mut sum = temp[r * cols] + if cols > 1 { temp[r * cols + 1] } else { 0.0 };
        for c in 0..cols {
            output[r * cols + c] = sum / 3.0;
            let add = if c + 2 < cols {
                temp[r * cols + c + 2]
            } else {
                0.0
            };
            let remove = if c > 0 { temp[r * cols + c - 1] } else { 0.0 };
            sum += add - remove;
        }
    }
    output
}
