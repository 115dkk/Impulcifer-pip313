#![forbid(unsafe_code)]
//! File-free port of core/eqapo.py:168-941; p12_* fixtures.
use crate::{DspError, fft::Complex64};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    f64::consts::PI,
    path::{Component, Path, PathBuf},
    sync::LazyLock,
};

/// Python _read_include_file/_handle_convolution (593-600,827-938); p12_files_*.
/// The caller owns byte decoding (UTF-8-sig, then cp1252), existence checks,
/// symlink identity and platform path policy. WAV tracks are channels, not frames.
pub trait EqApoLoader {
    /// Python _read_include_file (593-600); p12_files_include_*.
    fn read_text(&mut self, path: &Path) -> Result<String, String>;
    /// Python soundfile.read (860-874); p12_files_convolution_*.
    fn read_wav(&mut self, path: &Path) -> Result<(u32, Vec<Vec<f64>>), String>;
}
/// Python missing-file behavior (854-858,920-923); p12_files_missing.
#[derive(Default)]
pub struct NoFilesLoader;
impl EqApoLoader for NoFilesLoader {
    /// Python _read_include_file (593-600); p12_files_missing.
    fn read_text(&mut self, path: &Path) -> Result<String, String> {
        Err(format!("no text loader for {}", path.display()))
    }
    /// Python _handle_convolution (854-874); p12_files_missing.
    fn read_wav(&mut self, path: &Path) -> Result<(u32, Vec<Vec<f64>>), String> {
        Err(format!("no WAV loader for {}", path.display()))
    }
}
/// Python EqApoCommandReport (127-134); all p12_* report lists.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EqApoCommandReport {
    pub line_number: usize,
    pub command: String,
    pub text: String,
    pub reason: String,
}
/// Python EqApoEqualization (148-165); all p12_*.
#[derive(Clone, Debug, Default)]
pub struct EqApoEqualization {
    pub left_db: Vec<f64>,
    pub right_db: Vec<f64>,
    pub applied_left: usize,
    pub applied_right: usize,
    pub preamp_left: f64,
    pub preamp_right: f64,
    pub applied: Vec<String>,
    pub bypassed: Vec<EqApoCommandReport>,
    pub skipped: Vec<EqApoCommandReport>,
}
impl EqApoEqualization {
    /// Python channel_split (163-165), including NaN inequality; p12_scopes.
    pub fn channel_split(&self) -> bool {
        self.left_db != self.right_db
    }
}

/// Python regular expressions (70-94); p12_grammar and p12_detection.
struct Grammar {
    known: Regex,
    filter: Regex,
    kind: Regex,
    freq: Regex,
    gain: Regex,
    q: Regex,
    bw: Regex,
    slope: Regex,
    order: Regex,
    coefficients: Regex,
    number: Regex,
    leading: Regex,
    condition: Regex,
}
/// Python regex compilation (70-94); p12_grammar.
static GRAMMAR: LazyLock<Grammar> = LazyLock::new(|| {
    // Python re's Unicode whitespace additionally includes U+001C..U+001F.
    let re = |s: &str| {
        Regex::new(&s.replace(r"\s", r"[\s\x{1c}-\x{1f}]")).expect("constant Python regex")
    };
    Grammar {
        known: re(r"^[A-Za-z][A-Za-z0-9]*(?:\s+\d+)?$"),
        filter: re(r"^Filter(?:\s*\d+)?$"),
        kind: re(r"^\s*ON\s+([A-Za-z]+)"),
        freq: re(r"\s+Fc\s*([-+0-9.eE\x{00a0}]+)\s*H\s*z"),
        gain: re(r"\s+Gain\s*([-+0-9.eE]+)\s*dB"),
        q: re(r"\s+Q\s*([-+0-9.eE]+)"),
        bw: re(r"\s+BW\s+Oct\s*([-+0-9.eE]+)"),
        slope: re(r"^\s*([-+0-9.eE]+)\s*dB"),
        order: re(r"\s*Order\s+([0-9]+)"),
        coefficients: re(r"\s+Coefficients((?: [-+0-9.eE]+)+)"),
        number: re(r"[-+0-9.eE]+"),
        leading: re(r"^\s*[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?"),
        condition: re(
            r"^\s*\(?\s*sampleRate\s*(==|!=|<=|>=|<|>)\s*([0-9]+(?:\.[0-9]+)?)\s*\)?\s*$",
        ),
    }
});
/// Python str.strip whitespace (618-625); p12_line_endings.
fn strip(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c))
}
/// Python str.splitlines (180,578); p12_line_endings. CRLF counts as one line.
fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.split_inclusive([
        '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}',
        '\u{2029}',
    ])
    .scan(false, |after_cr, part| {
        let skip = *after_cr && part == "\n";
        *after_cr = part.ends_with('\r');
        Some((!skip).then(|| {
            part.trim_end_matches([
                '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}',
                '\u{2029}',
            ])
        }))
    })
    .flatten()
}
/// Python looks_like_eqapo_config (168-189); p12_detection.
pub fn looks_like_eqapo_config(text: &str) -> bool {
    let known = [
        "Preamp",
        "GraphicEQ",
        "Channel",
        "Include",
        "Convolution",
        "Delay",
        "Copy",
        "MultiConvolution",
        "Eval",
        "VSTPlugin",
        "LoudnessCorrection",
        "Device",
        "Stage",
        "If",
        "ElseIf",
        "Else",
        "EndIf",
    ];
    lines(text).any(|line| {
        line.split_once(':').is_some_and(|(key, _)| {
            let key = strip(key);
            !key.starts_with('#') && (known.contains(&key) || GRAMMAR.filter.is_match(key))
        })
    })
}
/// Python float Unicode decimal conversion in _parse_double (191-199);
/// Unicode 16 Nd zero codepoints (Python 3.14); p12_unicode_decimal.
fn decimal_ascii(c: char) -> char {
    const ZEROES: &[u32] = &[
        0x30, 0x660, 0x6f0, 0x7c0, 0x966, 0x9e6, 0xa66, 0xae6, 0xb66, 0xbe6, 0xc66, 0xce6, 0xd66,
        0xde6, 0xe50, 0xed0, 0xf20, 0x1040, 0x1090, 0x17e0, 0x1810, 0x1946, 0x19d0, 0x1a80, 0x1a90,
        0x1b50, 0x1bb0, 0x1c40, 0x1c50, 0xa620, 0xa8d0, 0xa900, 0xa9d0, 0xa9f0, 0xaa50, 0xabf0,
        0xff10, 0x104a0, 0x10d30, 0x10d40, 0x11066, 0x110f0, 0x11136, 0x111d0, 0x112f0, 0x11450,
        0x114d0, 0x11650, 0x116c0, 0x116d0, 0x116da, 0x11730, 0x118e0, 0x11950, 0x11bf0, 0x11c50,
        0x11d50, 0x11da0, 0x11f50, 0x16130, 0x16a60, 0x16ac0, 0x16b50, 0x16d70, 0x1ccf0, 0x1d7ce,
        0x1d7d8, 0x1d7e2, 0x1d7ec, 0x1d7f6, 0x1e140, 0x1e2f0, 0x1e4f0, 0x1e5f1, 0x1e950, 0x1fbf0,
    ];
    let code = c as u32;
    ZEROES
        .iter()
        .find(|&&zero| code >= zero && code - zero < 10)
        .map_or(c, |zero| (b'0' + (code - zero) as u8) as char)
}
/// Python _parse_double (191-199); p12_grammar/p12_unicode_decimal.
fn parse_double(s: &str) -> f64 {
    GRAMMAR
        .leading
        .find(s)
        .and_then(|m| {
            strip(m.as_str())
                .chars()
                .map(decimal_ascii)
                .collect::<String>()
                .parse()
                .ok()
        })
        .unwrap_or(0.0)
}
/// Python _parse_freq (202-215); p12_rew_frequency.
fn parse_freq(token: &str) -> f64 {
    let s = token.replace('\u{a0}', "");
    if !GRAMMAR.leading.is_match(&s) {
        return -1.0;
    }
    let mut result = parse_double(&s);
    if s.len() >= 5 && !s.contains(['e', 'E']) && s.as_bytes()[s.len() - 4] == b'.' {
        result *= 1000.0;
    }
    result
}
/// Python :g formatting (387-395), six significant digits; p12_formatting.
fn general(x: f64) -> String {
    if !x.is_finite() {
        return if x.is_nan() {
            "nan".into()
        } else if x.is_sign_negative() {
            "-inf".into()
        } else {
            "inf".into()
        };
    }
    let scientific = format!("{x:.5e}");
    let (mantissa, exponent) = scientific.split_once('e').expect("scientific format");
    let exponent: i32 = exponent.parse().expect("decimal exponent");
    if !(-4..6).contains(&exponent) {
        format!(
            "{}e{exponent:+03}",
            mantissa.trim_end_matches('0').trim_end_matches('.')
        )
    } else {
        let fixed = format!("{:.*}", (5 - exponent).max(0) as usize, x);
        if fixed.contains('.') {
            fixed.trim_end_matches('0').trim_end_matches('.').into()
        } else {
            fixed
        }
    }
}
/// Python biquadTypeFromName (96-121); p12_biquad_*.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Peak,
    Low,
    High,
    Band,
    LowShelf,
    HighShelf,
    Notch,
    All,
}
/// Python _BiquadSpec (137-145); p12_biquad_*.
struct Spec {
    kind: Kind,
    gain: f64,
    freq: f64,
    q: f64,
    bandwidth: bool,
    corner: bool,
    description: String,
}
/// Python _parse_biquad (306-408); p12_biquad_* and p12_grammar.
fn parse_biquad(parameters: &str) -> Result<Spec, &'static str> {
    let parameters = parameters.replace(',', ".");
    let m = GRAMMAR.kind.captures(&parameters).ok_or("disabled")?;
    let name = &m[1];
    let kind = match name {
        "PK" | "PEQ" | "Modal" => Kind::Peak,
        "LP" | "LPQ" => Kind::Low,
        "HP" | "HPQ" => Kind::High,
        "BP" => Kind::Band,
        "LS" | "LSC" => Kind::LowShelf,
        "HS" | "HSC" => Kind::HighShelf,
        "NO" => Kind::Notch,
        "AP" => Kind::All,
        "None" => return Err("disabled"),
        _ => return Err("malformed"),
    };
    let parameters = &parameters[m.get(0).unwrap().end()..];
    let freq = GRAMMAR
        .freq
        .captures(parameters)
        .map(|m| parse_freq(&m[1]))
        .ok_or("malformed")?;
    if !freq.is_finite() || freq <= 0.0 {
        return Err("malformed");
    }
    let shelf = matches!(kind, Kind::LowShelf | Kind::HighShelf);
    let gain_kind = shelf || kind == Kind::Peak;
    let mut gain = 0.0;
    if let Some(m) = GRAMMAR.gain.captures(parameters) {
        if !matches!(kind, Kind::Low | Kind::High | Kind::Notch | Kind::All) {
            gain = parse_double(&m[1]);
        }
    } else if gain_kind {
        return Err("malformed");
    }
    let mut q = GRAMMAR
        .q
        .captures(parameters)
        .map_or(0.0, |m| parse_double(&m[1]));
    let mut bandwidth = false;
    let mut corner = false;
    if let Some(m) = GRAMMAR.bw.captures(parameters)
        && !shelf
    {
        q = parse_double(&m[1]);
        bandwidth = true;
    }
    if let Some(m) = GRAMMAR.slope.captures(parameters)
        && shelf
    {
        q = parse_double(&m[1]);
        bandwidth = true;
    }
    if q == 0.0 {
        q = match kind {
            Kind::Peak | Kind::All => return Err("malformed"),
            Kind::Low | Kind::High | Kind::Band => 1.0 / 2.0_f64.sqrt(),
            Kind::LowShelf | Kind::HighShelf => {
                bandwidth = true;
                0.9
            }
            Kind::Notch => 30.0,
        };
    } else {
        if !q.is_finite() || q < 0.0 {
            return Err("malformed");
        }
        if shelf {
            if bandwidth {
                q /= 12.0;
            }
            corner = !name.ends_with('C');
        }
    }
    let mut description = format!("{name} Fc {} Hz", general(freq));
    if gain_kind {
        description += &format!(" Gain {} dB", general(gain));
    }
    let label = if bandwidth && shelf {
        "S"
    } else if bandwidth {
        "BW Oct"
    } else {
        "Q"
    };
    description += &format!(" {label} {}", general(q));
    Ok(Spec {
        kind,
        gain,
        freq,
        q,
        bandwidth,
        corner,
        description,
    })
}
/// Python scalar arithmetic exceptions (218-303); p12_arithmetic boundary tests.
fn invalid(message: &str) -> DspError {
    DspError::InvalidArgument(format!("EqualizerAPO: {message}"))
}
/// Python float division (218-303); p12_arithmetic.
fn divide(a: f64, b: f64) -> Result<f64, DspError> {
    if b == 0.0 {
        Err(invalid("float division by zero"))
    } else {
        Ok(a / b)
    }
}
/// Python 10.0 ** exponent (221-223,293-295); p12_arithmetic.
fn power10(exponent: f64) -> Result<f64, DspError> {
    let value = 10.0_f64.powf(exponent);
    if exponent.is_finite() && value.is_infinite() {
        Err(invalid("numerical result out of range"))
    } else {
        Ok(value)
    }
}
/// Python math.sqrt (231,235); p12_arithmetic.
fn sqrt(value: f64) -> Result<f64, DspError> {
    if value < 0.0 {
        Err(invalid("math domain error"))
    } else {
        Ok(value.sqrt())
    }
}
/// Python _biquad_coefficients (218-272), literal APO coefficients; p12_biquad_*.
fn coefficients(spec: &Spec, freq: f64, srate: u32) -> Result<[f64; 5], DspError> {
    use Kind::*;
    let kind = spec.kind;
    let a = power10(
        spec.gain
            / if matches!(kind, Peak | LowShelf | HighShelf) {
                40.0
            } else {
                20.0
            },
    )?;
    let omega = divide(2.0 * PI * freq, srate as f64)?;
    if omega.is_infinite() {
        return Err(invalid("math domain error"));
    }
    let sn = omega.sin();
    let cs = omega.cos();
    let alpha = if !spec.bandwidth {
        divide(sn, 2.0 * spec.q)?
    } else if matches!(kind, LowShelf | HighShelf) {
        sn / 2.0 * sqrt((a + divide(1.0, a)?) * (divide(1.0, spec.q)? - 1.0) + 2.0)?
    } else {
        let arg = divide(2.0_f64.ln() / 2.0 * spec.q * omega, sn)?;
        let sinh = arg.sinh();
        if arg.is_finite() && sinh.is_infinite() {
            return Err(invalid("math range error"));
        }
        sn * sinh
    };
    let beta = 2.0 * sqrt(a)? * alpha;
    let (b0, b1, b2, a0, a1, a2) = match kind {
        Low => (
            (1.0 - cs) / 2.0,
            1.0 - cs,
            (1.0 - cs) / 2.0,
            1.0 + alpha,
            -2.0 * cs,
            1.0 - alpha,
        ),
        High => (
            (1.0 + cs) / 2.0,
            -(1.0 + cs),
            (1.0 + cs) / 2.0,
            1.0 + alpha,
            -2.0 * cs,
            1.0 - alpha,
        ),
        Band => (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cs, 1.0 - alpha),
        Notch => (1.0, -2.0 * cs, 1.0, 1.0 + alpha, -2.0 * cs, 1.0 - alpha),
        All => (
            1.0 - alpha,
            -2.0 * cs,
            1.0 + alpha,
            1.0 + alpha,
            -2.0 * cs,
            1.0 - alpha,
        ),
        Peak => (
            1.0 + alpha * a,
            -2.0 * cs,
            1.0 - alpha * a,
            1.0 + divide(alpha, a)?,
            -2.0 * cs,
            1.0 - divide(alpha, a)?,
        ),
        LowShelf => (
            a * ((a + 1.0) - (a - 1.0) * cs + beta),
            2.0 * a * ((a - 1.0) - (a + 1.0) * cs),
            a * ((a + 1.0) - (a - 1.0) * cs - beta),
            (a + 1.0) + (a - 1.0) * cs + beta,
            -2.0 * ((a - 1.0) + (a + 1.0) * cs),
            (a + 1.0) + (a - 1.0) * cs - beta,
        ),
        HighShelf => (
            a * ((a + 1.0) + (a - 1.0) * cs + beta),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * cs),
            a * ((a + 1.0) + (a - 1.0) * cs - beta),
            (a + 1.0) - (a - 1.0) * cs + beta,
            2.0 * ((a - 1.0) - (a + 1.0) * cs),
            (a + 1.0) - (a - 1.0) * cs - beta,
        ),
    };
    Ok([
        divide(b0, a0)?,
        divide(b1, a0)?,
        divide(b2, a0)?,
        divide(a1, a0)?,
        divide(a2, a0)?,
    ])
}
/// Python _evaluate_biquad/_biquad_gain_db (275-303); p12_biquad_*.
fn evaluate_biquad(spec: &Spec, frequency: &[f64], fs: u32) -> Result<Vec<f64>, DspError> {
    let mut freq = spec.freq;
    if spec.corner && matches!(spec.kind, Kind::LowShelf | Kind::HighShelf) {
        let mut s = spec.q;
        if !spec.bandwidth {
            let a = power10(spec.gain / 40.0)?;
            s = divide(
                1.0,
                divide(divide(1.0, spec.q * spec.q)? - 2.0, a + divide(1.0, a)?)? + 1.0,
            )?;
        }
        let factor = power10(divide(spec.gain.abs() / 80.0, s)?)?;
        if spec.kind == Kind::LowShelf {
            freq *= factor;
        } else {
            freq = divide(freq, factor)?;
        }
    }
    let [b0, b1, b2, a1, a2] = coefficients(spec, freq, fs)?;
    // Python scalar **2 can raise OverflowError before NumPy vector arithmetic.
    let numerator = (b0 + b1 + b2).powi(2);
    let denominator = (1.0 + a1 + a2).powi(2);
    if ((b0 + b1 + b2).is_finite() && numerator.is_infinite())
        || ((1.0 + a1 + a2).is_finite() && denominator.is_infinite())
    {
        return Err(invalid("numerical result out of range"));
    }
    Ok(frequency
        .iter()
        .map(|f| {
            let sn = (PI * f / fs as f64).sin();
            let phi = sn * sn;
            let num = numerator - 4.0 * (b0 * b1 + 4.0 * b0 * b2 + b1 * b2) * phi
                + 16.0 * b0 * b2 * phi * phi;
            let den = denominator - 4.0 * (a1 + 4.0 * a2 + a1 * a2) * phi + 16.0 * a2 * phi * phi;
            10.0 * maximum(num, 1e-30).log10() - 10.0 * maximum(den, 1e-30).log10()
        })
        .collect())
}
/// Python np.maximum (283,472) propagates NaN; p12_arithmetic.
fn maximum(a: f64, b: f64) -> f64 {
    if a.is_nan() { a } else { a.max(b) }
}
/// Python _parse_iir (411-430); p12_iir*. Overflowing orders cannot match an in-memory coefficient list.
fn parse_iir(parameters: &str) -> Option<(Vec<f64>, Vec<f64>)> {
    if &GRAMMAR.kind.captures(parameters)?[1] != "IIR" {
        return None;
    }
    let order: usize = GRAMMAR.order.captures(parameters)?[1].parse().ok()?;
    if order < 1 {
        return None;
    }
    let m = GRAMMAR.coefficients.captures(parameters)?;
    let coefficients: Vec<_> = m[1]
        .split(' ')
        .filter(|s| !s.is_empty())
        .map(parse_double)
        .collect();
    let n = order.checked_add(1)?;
    if coefficients.len() != n.checked_mul(2)? {
        return None;
    }
    Some((coefficients[..n].to_vec(), coefficients[n..].to_vec()))
}
/// Python _evaluate_iir/_evaluate_fir -> scipy.freqz polynomial evaluation (433-446); p12_iir*, p12_files_convolution_*.
fn evaluate_iir(b: &[f64], a: &[f64], frequency: &[f64], fs: u32) -> Vec<f64> {
    frequency
        .iter()
        .map(|f| {
            let z = Complex64::from_polar(1.0, -(2.0 * PI * f / fs as f64));
            let polynomial = |c: &[f64]| {
                c.iter()
                    .rev()
                    .fold(Complex64::new(0.0, 0.0), |v, c| v * z + c)
            };
            20.0 * ((polynomial(b) / polynomial(a)).norm() + 1e-30).log10()
        })
        .collect()
}
/// Python _parse_graphic_eq_nodes (449-461); p12_graphic_*.
fn graphic_nodes(parameters: &str) -> Vec<(f64, f64)> {
    let value = if parameters.contains('.') {
        parameters.to_owned()
    } else {
        parameters.replace(',', ".")
    };
    let numbers: Vec<_> = GRAMMAR
        .number
        .find_iter(&value)
        .map(|m| parse_double(m.as_str()))
        .collect();
    let mut nodes: Vec<_> = numbers.chunks_exact(2).map(|p| (p[0], p[1])).collect();
    nodes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    nodes
}
/// Python _evaluate_graphic_eq/np.interp (464-473); p12_graphic_duplicates.
fn evaluate_graphic(nodes: &[(f64, f64)], frequency: &[f64]) -> Vec<f64> {
    let xs: Vec<_> = nodes.iter().map(|n| n.0.max(1e-9).ln()).collect();
    frequency
        .iter()
        .map(|f| {
            let x = maximum(*f, 1e-9).ln();
            if x.is_nan() {
                return x;
            }
            if x < xs[0] {
                return nodes[0].1;
            }
            let right = xs.partition_point(|v| *v <= x);
            if right == xs.len() {
                return nodes[right - 1].1;
            }
            let left = right - 1;
            if x == xs[left] {
                return nodes[left].1;
            }
            let slope = (nodes[right].1 - nodes[left].1) / (xs[right] - xs[left]);
            let mut y = slope * (x - xs[left]) + nodes[left].1;
            if y.is_nan() {
                y = slope * (x - xs[right]) + nodes[right].1;
                if y.is_nan() && nodes[left].1 == nodes[right].1 {
                    y = nodes[left].1;
                }
            }
            y
        })
        .collect()
}
/// Python _try_evaluate_condition (476-494); p12_condition_*.
fn condition(expression: &str, fs: u32) -> Option<bool> {
    let m = GRAMMAR.condition.captures(expression)?;
    let rhs: f64 = m[2].parse().ok()?;
    let lhs = fs as f64;
    Some(match &m[1] {
        "==" => lhs == rhs,
        "!=" => lhs != rhs,
        "<=" => lhs <= rhs,
        ">=" => lhs >= rhs,
        "<" => lhs < rhs,
        _ => lhs > rhs,
    })
}
/// Python _ConditionFrame (497-512); p12_condition_*.
struct Frame {
    active: bool,
    satisfied: bool,
    evaluable: bool,
    silent: bool,
}
/// Python _parse_channel_scope (515-534); p12_scopes.
fn channel_scope(parameters: &str) -> [bool; 2] {
    let tokens: Vec<_> = parameters
        .split(|c: char| c.is_whitespace() || c == ',' || ('\u{1c}'..='\u{1f}').contains(&c))
        .filter(|s| !s.is_empty())
        .collect();
    if tokens.is_empty() {
        return [true, true];
    }
    let mut scope = [false, false];
    for t in tokens {
        match t.to_uppercase().as_str() {
            "ALL" => scope = [true, true],
            "L" | "1" => scope[0] = true,
            "R" | "2" => scope[1] = true,
            _ => {}
        }
    }
    scope
}
/// Python _ParseState (537-562); all p12_*.
struct State<'a> {
    frequency: &'a [f64],
    fs: u32,
    scope: [bool; 2],
    result: EqApoEqualization,
}
impl State<'_> {
    /// Python EqApoCommandReport construction (628-736); p12_reports.
    fn report(&mut self, line: &Line<'_>, reason: &str) {
        let report = EqApoCommandReport {
            line_number: line.number,
            command: line.key.into(),
            text: strip(line.text).into(),
            reason: reason.into(),
        };
        if reason == "disabled" {
            self.result.skipped.push(report);
        } else {
            self.result.bypassed.push(report);
        }
    }
    /// Python _scope_or_report (740-747); p12_scopes.
    fn scoped(&mut self, line: &Line<'_>) -> bool {
        if !self.scope[0] && !self.scope[1] {
            self.report(line, "channel_scope");
            false
        } else {
            true
        }
    }
    /// Python _ParseState.add_response and convolution mapping (552-562,895-904); p12_*.
    fn add(&mut self, left: &[f64], right: &[f64], description: &str) {
        if self.scope[0] {
            for (v, g) in self.result.left_db.iter_mut().zip(left) {
                *v += g;
            }
            self.result.applied_left += 1;
        }
        if self.scope[1] {
            for (v, g) in self.result.right_db.iter_mut().zip(right) {
                *v += g;
            }
            self.result.applied_right += 1;
        }
        let suffix = match self.scope {
            [true, true] => "L+R",
            [true, false] => "L",
            [false, true] => "R",
            _ => "",
        };
        self.result
            .applied
            .push(format!("{description} [{suffix}]"));
    }
}
/// Python _parse_lines local line context (618-624); p12_reports.
struct Line<'a> {
    number: usize,
    key: &'a str,
    value: &'a str,
    text: &'a str,
}
/// Python parse_eqapo_config (565-590); all p12_*.
pub fn parse_eqapo_config(
    text: &str,
    srate: u32,
    frequency: &[f64],
    base_dir: Option<&Path>,
    loader: &mut dyn EqApoLoader,
) -> Result<EqApoEqualization, DspError> {
    let mut state = State {
        frequency,
        fs: srate,
        scope: [true, true],
        result: EqApoEqualization {
            left_db: vec![0.0; frequency.len()],
            right_db: vec![0.0; frequency.len()],
            ..Default::default()
        },
    };
    parse_lines(text, &mut state, base_dir, 0, &mut Vec::new(), loader)?;
    Ok(state.result)
}
/// Python _parse_lines (603-737); p12_condition_* and p12_files_include_*.
fn parse_lines(
    text: &str,
    state: &mut State<'_>,
    base: Option<&Path>,
    depth: usize,
    visited: &mut Vec<PathBuf>,
    loader: &mut dyn EqApoLoader,
) -> Result<(), DspError> {
    let mut stack: Vec<Frame> = Vec::new();
    for (index, text) in lines(text).enumerate() {
        let Some((key, value)) = text.split_once(':') else {
            continue;
        };
        let key = strip(key);
        if key.is_empty() || key.starts_with('#') {
            continue;
        }
        let line = Line {
            number: index + 1,
            key,
            value,
            text,
        };
        let active = stack.iter().all(|f| f.active);
        let report = stack.iter().any(|f| !f.evaluable && !f.silent);
        match key {
            "If" => {
                if !active {
                    if report {
                        state.report(&line, "conditional");
                    }
                    stack.push(Frame {
                        active: false,
                        satisfied: true,
                        evaluable: true,
                        silent: true,
                    });
                } else if let Some(c) = condition(value, state.fs) {
                    stack.push(Frame {
                        active: c,
                        satisfied: c,
                        evaluable: true,
                        silent: false,
                    });
                } else {
                    state.report(&line, "conditional");
                    stack.push(Frame {
                        active: false,
                        satisfied: true,
                        evaluable: false,
                        silent: false,
                    });
                }
            }
            "ElseIf" => {
                if let Some(frame) = stack.last_mut() {
                    if frame.silent || !frame.evaluable {
                        if report {
                            state.report(&line, "conditional");
                        }
                    } else if frame.satisfied {
                        frame.active = false;
                    } else if let Some(c) = condition(value, state.fs) {
                        frame.active = c;
                        frame.satisfied = c;
                    } else {
                        state.report(&line, "conditional");
                        frame.evaluable = false;
                        frame.active = false;
                        frame.satisfied = true;
                    }
                } else {
                    state.report(&line, "unsupported");
                }
            }
            "Else" => {
                if let Some(frame) = stack.last_mut() {
                    if !frame.silent && frame.evaluable {
                        frame.active = !frame.satisfied;
                        frame.satisfied = true;
                    }
                } else {
                    state.report(&line, "unsupported");
                }
            }
            "EndIf" => {
                if stack.pop().is_none() {
                    state.report(&line, "unsupported");
                }
            }
            _ if !active => {
                if report && GRAMMAR.known.is_match(key) {
                    state.report(&line, "conditional");
                }
            }
            _ if key.starts_with("Filter") => handle_filter(&line, state)?,
            "Preamp" => handle_preamp(&line, state)?,
            "GraphicEQ" => handle_graphic(&line, state),
            "Channel" => state.scope = channel_scope(value),
            "Include" => handle_include(&line, state, base, depth, visited, loader)?,
            "Convolution" => handle_convolution(&line, state, base, loader),
            "Device" | "Stage" => state.report(&line, "scoping_ignored"),
            _ if GRAMMAR.known.is_match(key) => state.report(&line, "unsupported"),
            _ => {}
        }
    }
    Ok(())
}
/// Python _handle_filter (750-782); p12_biquad_*, p12_iir*, p12_reports.
fn handle_filter(line: &Line<'_>, state: &mut State<'_>) -> Result<(), DspError> {
    if line.value.contains('`') {
        state.report(line, "expression");
        return Ok(());
    }
    if let Some((b, a)) = parse_iir(line.value) {
        if state.scoped(line) {
            let gain = evaluate_iir(&b, &a, state.frequency, state.fs);
            state.add(&gain, &gain, &format!("IIR Order {}", b.len() - 1));
        }
    } else {
        match parse_biquad(line.value) {
            Err(reason) => state.report(line, reason),
            Ok(spec) => {
                if state.scoped(line) {
                    let gain = evaluate_biquad(&spec, state.frequency, state.fs)?;
                    state.add(&gain, &gain, &spec.description);
                }
            }
        }
    }
    Ok(())
}
/// Python _handle_preamp (785-806); p12_preamp and p12_reports.
fn handle_preamp(line: &Line<'_>, state: &mut State<'_>) -> Result<(), DspError> {
    if line.value.contains('`') {
        state.report(line, "expression");
        return Ok(());
    }
    let normalized = line.value.replace(',', ".");
    let Some(m) = GRAMMAR.leading.find(&normalized) else {
        state.report(line, "malformed");
        return Ok(());
    };
    if !state.scoped(line) {
        return Ok(());
    }
    // float() rejects these controls even though Python re accepts them as whitespace.
    if m.as_str()
        .contains(['\u{1c}', '\u{1d}', '\u{1e}', '\u{1f}'])
    {
        return Err(invalid("could not convert string to float"));
    }
    let db = parse_double(&normalized);
    if state.scope[0] {
        state.result.preamp_left += db;
        for v in &mut state.result.left_db {
            *v += db;
        }
    }
    if state.scope[1] {
        state.result.preamp_right += db;
        for v in &mut state.result.right_db {
            *v += db;
        }
    }
    Ok(())
}
/// Python _handle_graphic_eq (809-824); p12_graphic_*.
fn handle_graphic(line: &Line<'_>, state: &mut State<'_>) {
    if line.value.contains('`') {
        state.report(line, "expression");
        return;
    }
    let nodes = graphic_nodes(line.value);
    if nodes.is_empty() {
        state.report(line, "malformed");
        return;
    }
    if !state.scoped(line) {
        return;
    }
    let gain = evaluate_graphic(&nodes, state.frequency);
    state.add(&gain, &gain, &format!("GraphicEQ ({} nodes)", nodes.len()));
}
/// Python abspath/normpath in _handle_include/_handle_convolution (841-852,908-918);
/// p12_files_*. Lexical only: no cwd access or symlink resolution.
fn resolve(value: &str, base: Option<&Path>) -> Option<PathBuf> {
    if value.is_empty() {
        return None;
    }
    let path = Path::new(value);
    let path = if path.has_root() {
        path.to_path_buf()
    } else {
        base?.join(path)
    };
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if result.file_name().is_some_and(|n| n != "..") {
                    result.pop();
                } else if !result.has_root() {
                    result.push("..");
                }
            }
            c => result.push(c.as_os_str()),
        }
    }
    Some(result)
}
/// Python os.path.expandvars in _handle_convolution (841-842); p12_files_environment.
/// Matches native os.path: Windows also expands %name%, respects single quotes,
/// and treats $$ / %% as escaped literals; POSIX expands only $name/${name}.
fn expand_vars(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if cfg!(windows) && c == '\'' {
            out.push(c);
            i += 1;
            while i < chars.len() {
                let c = chars[i];
                out.push(c);
                i += 1;
                if c == '\'' {
                    break;
                }
            }
            continue;
        }
        if c != '$' && !(cfg!(windows) && c == '%') {
            out.push(c);
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        if cfg!(windows) && chars.get(i) == Some(&c) {
            out.push(c);
            i += 1;
            continue;
        }
        let name;
        if c == '%' || chars.get(i) == Some(&'{') {
            let endchar = if c == '%' {
                '%'
            } else {
                i += 1;
                '}'
            };
            let begin = i;
            while i < chars.len() && chars[i] != endchar {
                i += 1;
            }
            if i == chars.len() {
                out.extend(chars[start..].iter());
                break;
            }
            name = chars[begin..i].iter().collect::<String>();
            i += 1;
        } else {
            let begin = i;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric()
                    || chars[i] == '_'
                    || (cfg!(windows) && chars[i] == '-'))
            {
                i += 1;
            }
            name = chars[begin..i].iter().collect::<String>();
        }
        if let Ok(value) = std::env::var(&name) {
            out.push_str(&value);
        } else {
            out.extend(chars[start..i].iter());
        }
    }
    out
}
/// Python _handle_convolution (827-904); p12_files_convolution_*.
fn handle_convolution(
    line: &Line<'_>,
    state: &mut State<'_>,
    base: Option<&Path>,
    loader: &mut dyn EqApoLoader,
) {
    if line.value.contains('`') {
        state.report(line, "expression");
        return;
    }
    let value = expand_vars(strip(line.value).trim_matches('"'));
    let Some(path) = resolve(&value, base) else {
        state.report(line, "convolution_not_found");
        return;
    };
    let Ok((fs, tracks)) = loader.read_wav(&path) else {
        state.report(line, "convolution_not_found");
        return;
    };
    if tracks.is_empty()
        || tracks[0].is_empty()
        || tracks.iter().any(|t| t.len() != tracks[0].len())
    {
        state.report(line, "convolution_not_found");
        return;
    }
    if fs.abs_diff(state.fs) > 1 {
        state.report(line, "convolution_fs_mismatch");
        return;
    }
    if !state.scoped(line) {
        return;
    }
    let left = evaluate_iir(&tracks[0], &[1.0], state.frequency, state.fs);
    let right = if tracks.len() == 1 {
        left.clone()
    } else {
        evaluate_iir(&tracks[1], &[1.0], state.frequency, state.fs)
    };
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    state.add(
        &left,
        &right,
        &format!(
            "Convolution {name} ({} taps, {} ch)",
            tracks[0].len(),
            tracks.len()
        ),
    );
}
/// Python _handle_include (907-940); p12_files_include_*.
fn handle_include(
    line: &Line<'_>,
    state: &mut State<'_>,
    base: Option<&Path>,
    depth: usize,
    visited: &mut Vec<PathBuf>,
    loader: &mut dyn EqApoLoader,
) -> Result<(), DspError> {
    let Some(path) = resolve(strip(line.value).trim_matches('"'), base) else {
        state.report(line, "include_not_found");
        return Ok(());
    };
    // Keep spelling for the loader; Windows normcase affects chain identity only.
    let identity = if cfg!(windows) {
        PathBuf::from(path.to_string_lossy().to_lowercase())
    } else {
        path.clone()
    };
    if depth >= 8 || visited.contains(&identity) {
        state.report(line, "include_not_found");
        return Ok(());
    }
    let Ok(text) = loader.read_text(&path) else {
        state.report(line, "include_not_found");
        return Ok(());
    };
    let saved = state.scope;
    visited.push(identity);
    let result = parse_lines(&text, state, path.parent(), depth + 1, visited, loader);
    visited.pop();
    state.scope = saved;
    result
}
