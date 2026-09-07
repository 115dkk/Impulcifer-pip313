//! Butterworth design and zero-state filtering with conventional denominator signs.
use crate::fft::Complex64;
use std::f64::consts::PI;

/// Rows are [b0,b1,b2,a0,a1,a2], with a0 exactly one.
#[derive(Debug, Clone)]
pub struct Sos(pub Vec<[f64; 6]>);
#[derive(Debug, Clone, Copy)]
pub enum BType {
    Lowpass,
    Highpass,
}

/// Normalized transfer function: numerator b, denominator [1,a1,a2].
/// AutoEQ returns *negated* a1,a2 for additive feedback; digital_coeffs
/// negates them again. This struct always stores the conventional denominator.
#[derive(Debug, Clone)]
pub struct Biquad {
    pub b: [f64; 3],
    pub a: [f64; 3],
}

/// Analog Butterworth prototype -> LP/HP -> prewarped bilinear -> nearest SOS.
/// https://raw.githubusercontent.com/scipy/scipy/v1.16.0/scipy/signal/_filter_design.py
/// Verified with SciPy 1.18.0. Nearest pairing consumes the poles nearest the
/// unit circle first, writes rows backwards, and puts gain in the first row.
/// Odd orders add origin roots; their zero and pole need not share a section.
/// Panics unless finite 0 < wn < 1, or if the order cannot fit in isize.
/// Order zero is the SciPy identity section.
pub fn butter(order: usize, wn: f64, btype: BType) -> Sos {
    assert!(
        wn.is_finite() && wn > 0.0 && wn < 1.0,
        "cutoff must be between zero and Nyquist"
    );
    let order_i = isize::try_from(order).expect("filter order overflow");
    if order == 0 {
        return Sos(vec![[1.0, 0.0, 0.0, 1.0, 0.0, 0.0]]);
    }
    let warped = 4.0 * (PI * wn / 2.0).tan();
    let mut analog = Vec::with_capacity(order);
    let mut prototype_product = Complex64::new(1.0, 0.0);
    for i in 0..order {
        let m = -(order_i - 1) + 2 * i as isize;
        let prototype = -Complex64::from_polar(1.0, PI * m as f64 / (2.0 * order as f64));
        prototype_product *= -prototype;
        analog.push(match btype {
            BType::Lowpass => prototype * warped,
            BType::Highpass => warped / prototype,
        });
    }
    let mut denominator = Complex64::new(1.0, 0.0);
    for p in &analog {
        denominator *= 4.0 - p;
    }
    let gain = match btype {
        BType::Lowpass => warped.powf(order as f64) * (1.0 / denominator).re,
        BType::Highpass => {
            (1.0 / prototype_product).re
                * (Complex64::new(4.0_f64.powf(order as f64), 0.0) / denominator).re
        }
    };
    // One representative per conjugate pair, as in scipy._cplxreal.
    let mut poles: Vec<_> = analog
        .iter()
        .map(|p| (4.0 + p) / (4.0 - p))
        .filter(|p| p.im >= 0.0)
        .collect();
    poles.sort_by(|a, b| a.re.total_cmp(&b.re).then(a.im.total_cmp(&b.im)));
    let zero = match btype {
        BType::Lowpass => -1.0,
        BType::Highpass => 1.0,
    };
    let mut zeros = vec![zero; order];
    if order % 2 == 1 {
        poles.push(Complex64::new(0.0, 0.0));
        zeros.push(0.0);
    }
    let mut rows = vec![[0.0; 6]; order.div_ceil(2)];
    for row in rows.iter_mut().rev() {
        let index = poles
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| (1.0 - a.norm()).abs().total_cmp(&(1.0 - b.norm()).abs()))
            .unwrap()
            .0;
        let p1 = poles.remove(index);
        let p2 = if p1.im == 0.0 {
            let index = poles
                .iter()
                .enumerate()
                .filter(|(_, p)| p.im == 0.0)
                .min_by(|(_, a), (_, b)| (1.0 - a.norm()).abs().total_cmp(&(1.0 - b.norm()).abs()))
                .unwrap()
                .0;
            poles.remove(index)
        } else {
            p1.conj()
        };
        let mut take_zero = || {
            let index = zeros
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| (p1 - **a).norm().total_cmp(&(p1 - **b).norm()))
                .unwrap()
                .0;
            zeros.remove(index)
        };
        let z1 = take_zero();
        let z2 = take_zero();
        *row = [1.0, -(z1 + z2), z1 * z2, 1.0, -(p1 + p2).re, (p1 * p2).re];
    }
    for v in &mut rows[0][..3] {
        *v *= gain;
    }
    Sos(rows)
}

/// Direct form II transposed, sections in the supplied order, fresh zero state.
/// Panics for empty samples or empty/malformed SOS (nonfinite coefficients
/// or a0 != 1). Nonfinite samples propagate arithmetically.
pub fn sosfilt(sos: &Sos, x: &[f64]) -> Vec<f64> {
    assert!(!x.is_empty(), "sosfilt samples must be nonempty");
    assert!(
        !sos.0.is_empty()
            && sos
                .0
                .iter()
                .all(|s| s[3] == 1.0 && s.iter().all(|v| v.is_finite())),
        "invalid normalized SOS"
    );
    let mut output = x.to_vec();
    // Process four sections together: independent section states let the CPU
    // overlap their feedback arithmetic, without changing per-section ordering.
    let mut groups = sos.0.chunks_exact(4);
    for group in groups.by_ref() {
        let mut state = [[0.0; 2]; 4];
        for sample in &mut output {
            let mut value = *sample;
            for (s, z) in group.iter().zip(&mut state) {
                let y = s[0] * value + z[0];
                z[0] = s[1] * value - s[4] * y + z[1];
                z[1] = s[2] * value - s[5] * y;
                value = y;
            }
            *sample = value;
        }
    }
    for s in groups.remainder() {
        let mut z1 = 0.0;
        let mut z2 = 0.0;
        for x in &mut output {
            let y = s[0] * *x + z1;
            z1 = s[1] * *x - s[4] * y + z2;
            z2 = s[2] * *x - s[5] * y;
            *x = y;
        }
    }
    output
}

/// scipy.tf2sos's real biquad subset, including constant/first-order cases.
/// Root reconstruction of a single section is algebraically its normalized
/// polynomial; avoiding the root round trip only changes roundoff.
/// Like scipy.normalize, leading numerator terms <= 1e-14 after a0 scaling
/// are trimmed (not treated as delay). Panics for empty, nonfinite or >3-term
/// arrays, or an all-zero denominator. All-zero numerator is valid.
pub fn tf2sos(b: &[f64], a: &[f64]) -> Sos {
    assert!(
        !a.is_empty()
            && !b.is_empty()
            && a.len() <= 3
            && b.len() <= 3
            && a.iter().chain(b).all(|x| x.is_finite()),
        "tf2sos requires finite biquad polynomials"
    );
    let start = a.iter().position(|x| *x != 0.0).expect("zero denominator");
    let a = &a[start..];
    let mut bn: Vec<_> = b.iter().map(|x| x / a[0]).collect();
    let start = bn
        .iter()
        .position(|x| x.abs() > 1e-14)
        .unwrap_or(bn.len() - 1);
    bn.drain(..start);
    let mut row = [0.0; 6];
    row[..bn.len()].copy_from_slice(&bn);
    for (i, v) in a.iter().enumerate() {
        row[3 + i] = v / a[0];
    }
    Sos(vec![row])
}

fn parameters(fc: f64, q: f64, gain_db: f64, fs: f64) -> (f64, f64, f64) {
    assert!(
        fs.is_finite()
            && fs > 0.0
            && fc.is_finite()
            && fc >= 0.0
            && fc <= fs / 2.0
            && q.is_finite()
            && q > 0.0
            && gain_db.is_finite(),
        "invalid RBJ parameters"
    );
    let a = 10.0_f64.powf(gain_db / 40.0);
    assert!(
        a.is_finite() && a > 0.0,
        "RBJ gain outside floating-point range"
    );
    let w = 2.0 * PI * fc / fs;
    (a, w.cos(), w.sin() / (2.0 * q))
}
fn normalized(b: [f64; 3], a: [f64; 3]) -> Biquad {
    let result = Biquad {
        b: b.map(|x| x / a[0]),
        a: [1.0, a[1] / a[0], a[2] / a[0]],
    };
    assert!(
        result.b.iter().chain(&result.a).all(|x| x.is_finite()),
        "RBJ coefficients overflow"
    );
    result
}

/// AutoEQ peaking coefficients. Panics unless fs>0, 0<=fc<=fs/2, Q>0,
/// all parameters finite and gain/coefficient arithmetic representable.
pub fn rbj_peaking(fc: f64, q: f64, gain_db: f64, fs: f64) -> Biquad {
    let (a, c, alpha) = parameters(fc, q, gain_db, fs);
    normalized(
        [1.0 + alpha * a, -2.0 * c, 1.0 - alpha * a],
        [1.0 + alpha / a, -2.0 * c, 1.0 - alpha / a],
    )
}
/// AutoEQ low shelf, with Q (not shelf slope S). Same panic policy as peaking.
pub fn rbj_low_shelf(fc: f64, q: f64, gain_db: f64, fs: f64) -> Biquad {
    let (a, c, alpha) = parameters(fc, q, gain_db, fs);
    let t = 2.0 * a.sqrt() * alpha;
    normalized(
        [
            a * ((a + 1.0) - (a - 1.0) * c + t),
            2.0 * a * ((a - 1.0) - (a + 1.0) * c),
            a * ((a + 1.0) - (a - 1.0) * c - t),
        ],
        [
            (a + 1.0) + (a - 1.0) * c + t,
            -2.0 * ((a - 1.0) + (a + 1.0) * c),
            (a + 1.0) + (a - 1.0) * c - t,
        ],
    )
}
/// AutoEQ high shelf. Same panic policy as peaking.
pub fn rbj_high_shelf(fc: f64, q: f64, gain_db: f64, fs: f64) -> Biquad {
    let (a, c, alpha) = parameters(fc, q, gain_db, fs);
    let t = 2.0 * a.sqrt() * alpha;
    normalized(
        [
            a * ((a + 1.0) + (a - 1.0) * c + t),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * c),
            a * ((a + 1.0) + (a - 1.0) * c - t),
        ],
        [
            (a + 1.0) - (a - 1.0) * c + t,
            2.0 * ((a - 1.0) - (a + 1.0) * c),
            (a + 1.0) - (a - 1.0) * c - t,
        ],
    )
}

/// AutoEQ digital_coeffs algebra, including unregularized log10 at zeros.
/// Panics for nonfinite coefficients/frequencies, a0 != 1, or invalid fs.
/// Arbitrary finite frequencies (including outside Nyquist) are periodic.
pub fn biquad_response_db(b: &Biquad, freqs: &[f64], fs: f64) -> Vec<f64> {
    assert!(
        fs.is_finite()
            && fs > 0.0
            && b.a[0] == 1.0
            && b.a.iter().chain(&b.b).chain(freqs).all(|x| x.is_finite()),
        "invalid response parameters"
    );
    freqs
        .iter()
        .map(|f| {
            let w = 2.0 * PI * f / fs;
            let phi = 4.0 * (w / 2.0).sin().powi(2);
            let power = |c: [f64; 3]| {
                (c[0] + c[1] + c[2]).powi(2)
                    + (c[0] * c[2] * phi - (c[1] * (c[0] + c[2]) + 4.0 * c[0] * c[2])) * phi
            };
            10.0 * power(b.b).log10() - 10.0 * power(b.a).log10()
        })
        .collect()
}
