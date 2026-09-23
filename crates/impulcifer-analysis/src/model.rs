//! Numeric analysis model behind the interactive report.
//!
//! The five functions are ports of `core/plotting/analysis.py` (same formulas,
//! same NaN and degenerate-input handling). [`build_report`] is the data half of
//! the 2.x Bokeh generators (`core/plotting/hrir_plotter.py`, registry
//! `core/plotting/bokeh_registry.py`): which speakers get a chart, what is on
//! it and in which order. Pinned by `tests/migration/goldens/p24_interactive_*`.
//! Pure computation; no I/O, and the HRIR is only borrowed.

use impulcifer_dsp::{
    fft::{self, Complex64},
    fr::{FrequencyResponse, magnitude_to_frequency_response},
    hrir::Hrir,
    peaks,
};
use impulcifer_types::constants::SPEAKER_NAMES;

/// Python `DEFAULT_OCTAVE_CENTERS`.
pub const DEFAULT_OCTAVE_CENTERS: [f64; 8] =
    [125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];
/// Python `ImpulseResponse.peak_index` default height (-18 dBFS).
pub const PEAK_HEIGHT: f64 = 0.12589;
/// 2.x `generate_interaural_impulse_overlay_bokeh_layout(time_range_ms=(-5, 30))`.
pub const OVERLAY_RANGE_MS: (f64, f64) = (-5.0, 30.0);
/// 2.x `generate_iacc_bokeh_layout(max_delay_ms=1)`.
pub const IACC_MAX_DELAY_MS: f64 = 1.0;
/// 2.x `generate_etc_bokeh_layout(time_range_ms=(0, 200), y_range_db=(-80, 0))`.
pub const EDC_RANGE_MS: (f64, f64) = (0.0, 200.0);
/// See [`EDC_RANGE_MS`]; the lower bound is also the EDC floor.
pub const EDC_RANGE_DB: (f64, f64) = (-80.0, 0.0);

/// Python `octave_bands(fs, centers)`: `(center / sqrt 2, min(center * sqrt 2, fs / 2))`,
/// dropping empty bands and stopping at the first band that reaches Nyquist.
pub fn octave_bands(fs: f64, centers: &[f64]) -> Vec<(f64, f64)> {
    let nyquist = fs / 2.0;
    let mut bands = Vec::new();
    for &center in centers {
        let lower = center / std::f64::consts::SQRT_2;
        let upper = (center * std::f64::consts::SQRT_2).min(nyquist);
        if lower < upper {
            bands.push((lower, upper));
        }
        if upper >= nyquist {
            break;
        }
    }
    bands
}

/// One band of Python `_band_cross_spectra`: left power, right power and the
/// summed cross spectrum `sum(L * conj(R))`; `None` is Python's NaN triple.
type BandSpectrum = Option<(f64, f64, Complex64)>;

/// Python `_band_cross_spectra`. Both inputs are zero-padded to
/// `scipy.fft.next_fast_len(max(len))` (the complex 2,3,5,7,11 size, see
/// [`fft::next_fast_len_complex`]) and transformed once. Bins are selected on
/// `numpy.fft.fftfreq(n, d=1/fs)` with `f_low <= f < f_high`, the upper edge
/// clamped to Nyquist; a band without bins is NaN. The negative-frequency half
/// is the conjugate of the real transform, so bands reaching below 0 Hz see
/// the same bins as NumPy's full complex transform. Empty input has no bins.
fn band_cross_spectra(
    left: &[f64],
    right: &[f64],
    fs: f64,
    bands: &[(f64, f64)],
) -> Vec<BandSpectrum> {
    let n = fft::next_fast_len_complex(left.len().max(right.len()));
    if n == 0 {
        return vec![None; bands.len()];
    }
    let spectrum = |x: &[f64]| {
        let mut padded = vec![0.0; n];
        padded[..x.len()].copy_from_slice(x);
        fft::rfft(&padded)
    };
    let (spectrum_l, spectrum_r) = (spectrum(left), spectrum(right));
    let bin = |spectrum: &[Complex64], k: usize| {
        if k <= n / 2 {
            spectrum[k]
        } else {
            spectrum[n - k].conj()
        }
    };
    // numpy.fft.fftfreq: val = 1.0 / (n * d); [0, 1, ..., (n-1)//2, -(n//2), ..., -1] * val.
    let val = 1.0 / (n as f64 * (1.0 / fs));
    let positive = (n - 1) / 2 + 1;
    let frequency = |k: usize| {
        if k < positive {
            k as f64 * val
        } else {
            (k as f64 - n as f64) * val
        }
    };
    bands
        .iter()
        .map(|&(f_low, f_high)| {
            let f_high = f_high.min(fs / 2.0);
            if f_low >= f_high {
                return None;
            }
            let mut found = false;
            let (mut power_l, mut power_r) = (0.0, 0.0);
            let mut cross = Complex64::new(0.0, 0.0);
            for k in 0..n {
                let f = frequency(k);
                if f >= f_low && f < f_high {
                    let (l, r) = (bin(&spectrum_l, k), bin(&spectrum_r, k));
                    // NumPy: np.abs(x) ** 2 (hypot, then square).
                    power_l += l.norm().powi(2);
                    power_r += r.norm().powi(2);
                    cross += l * r.conj();
                    found = true;
                }
            }
            found.then_some((power_l, power_r, cross))
        })
        .collect()
}

fn ild_from(band: &BandSpectrum) -> f64 {
    match band {
        Some((power_l, power_r, _)) if !power_l.is_nan() => {
            10.0 * ((power_l + 1e-12) / (power_r + 1e-12)).log10()
        }
        _ => f64::NAN,
    }
}

fn ipd_from(band: &BandSpectrum) -> f64 {
    match band {
        Some((_, _, cross)) if !cross.re.is_nan() => cross.im.atan2(cross.re).to_degrees(),
        _ => f64::NAN,
    }
}

/// Python `band_interaural_level_difference`: `10 log10((PL + 1e-12) / (PR + 1e-12))`
/// per band, NaN for bands without FFT bins.
pub fn band_interaural_level_difference(
    left: &[f64],
    right: &[f64],
    fs: f64,
    bands: &[(f64, f64)],
) -> Vec<f64> {
    band_cross_spectra(left, right, fs, bands)
        .iter()
        .map(ild_from)
        .collect()
}

/// Python `band_interaural_phase_difference`: the angle (degrees, left minus
/// right, principal value) of the band's summed cross spectrum, NaN for bands
/// without FFT bins. A cross spectrum on the negative real axis sits on the
/// ±180° branch cut, where rounding noise decides the sign (as in 2.x).
pub fn band_interaural_phase_difference(
    left: &[f64],
    right: &[f64],
    fs: f64,
    bands: &[(f64, f64)],
) -> Vec<f64> {
    band_cross_spectra(left, right, fs, bands)
        .iter()
        .map(ipd_from)
        .collect()
}

/// NumPy `max` over a nonempty slice: NaN propagates.
fn numpy_max(x: &[f64]) -> f64 {
    x.iter().copied().fold(f64::NEG_INFINITY, |max, v| {
        if max.is_nan() || v.is_nan() {
            f64::NAN
        } else {
            max.max(v)
        }
    })
}

/// Python `energy_decay_curve_db`: Schroeder backward integration,
/// `10 log10(E(t) / (max E + 1e-12) + 1e-12)`. Empty input returns an empty
/// curve; a total energy of at most 1e-12 returns `floor_db` everywhere.
pub fn energy_decay_curve_db(data: &[f64], floor_db: f64) -> Vec<f64> {
    // np.cumsum((data**2)[::-1])[::-1]: sequential accumulation from the end.
    let mut energy = vec![0.0; data.len()];
    let mut sum = 0.0;
    for (e, x) in energy.iter_mut().zip(data).rev() {
        sum += x * x;
        *e = sum;
    }
    if energy.is_empty() {
        return energy;
    }
    let max = numpy_max(&energy);
    if max <= 1e-12 {
        return vec![floor_db; energy.len()];
    }
    energy
        .iter()
        .map(|e| 10.0 * (e / (max + 1e-12) + 1e-12).log10())
        .collect()
}

/// Python `interaural_cross_correlation` result.
#[derive(Clone, Debug, PartialEq)]
pub struct Iacf {
    /// `correlation_lags(full)` inside `|lag| <= round(max_delay_ms * fs / 1000)`, in ms.
    pub lags_ms: Vec<f64>,
    /// Normalized correlation `sum l(t + lag) r(t) / sqrt(sum l^2 sum r^2)` at those lags.
    pub iacf: Vec<f64>,
    /// `max |iacf|`; NaN when there is no energy or no lag.
    pub iacc: f64,
    /// Lag of the first maximum of `|iacf|` in ms; NaN with `iacc`.
    pub tau_ms: f64,
}

impl Iacf {
    fn empty() -> Self {
        Self {
            lags_ms: Vec::new(),
            iacf: Vec::new(),
            iacc: f64::NAN,
            tau_ms: f64::NAN,
        }
    }
}

/// Dot product with eight independent accumulators (vectorizes; NumPy's own
/// sums are pairwise, so neither side is the naive sequential order).
fn dot(a: &[f64], b: &[f64]) -> f64 {
    debug_assert_eq!(a.len(), b.len());
    let mut lanes = [0.0; 8];
    let (chunks_a, chunks_b) = (a.chunks_exact(8), b.chunks_exact(8));
    let tail: f64 = chunks_a
        .remainder()
        .iter()
        .zip(chunks_b.remainder())
        .map(|(x, y)| x * y)
        .sum();
    for (x, y) in chunks_a.zip(chunks_b) {
        for ((lane, x), y) in lanes.iter_mut().zip(x).zip(y) {
            *lane += x * y;
        }
    }
    ((lanes[0] + lanes[1]) + (lanes[2] + lanes[3]))
        + ((lanes[4] + lanes[5]) + (lanes[6] + lanes[7]))
        + tail
}

/// Python `interaural_cross_correlation` (ISO 3382-1 IACF/IACC). SciPy's
/// `correlate(left, right, mode="full")` is evaluated directly, only at the
/// lags inside the window: `c(lag) = sum_n left[n + lag] * right[n]`, with
/// `correlation_lags` running from `-(len(right) - 1)` to `len(left) - 1`.
/// A later right ear therefore peaks at a negative lag. The window radius is
/// Python's `round` (half to even). Zero energy (including empty input)
/// returns empty arrays and NaN.
pub fn interaural_cross_correlation(
    left: &[f64],
    right: &[f64],
    fs: f64,
    max_delay_ms: f64,
) -> Iacf {
    let energy = dot(left, left) * dot(right, right);
    if energy <= 0.0 {
        return Iacf::empty();
    }
    let norm = energy.sqrt();
    let radius = (max_delay_ms * fs / 1000.0).round_ties_even();
    let (nl, nr) = (left.len() as i64, right.len() as i64);
    let lo = if radius.is_nan() {
        1
    } else {
        (-radius).max(-(nr - 1) as f64) as i64
    };
    let hi = if radius.is_nan() {
        0
    } else {
        radius.min((nl - 1) as f64) as i64
    };
    let mut result = Iacf::empty();
    for lag in lo..=hi {
        // right[n] pairs with left[n + lag] for n in [max(0, -lag), min(nr, nl - lag)).
        let start = (-lag).max(0);
        let end = nr.min(nl - lag).max(start);
        let sum = dot(
            &left[(start + lag) as usize..(end + lag) as usize],
            &right[start as usize..end as usize],
        );
        result.lags_ms.push((lag * 1000) as f64 / fs);
        result.iacf.push(sum / norm);
    }
    if result.iacf.is_empty() {
        return Iacf::empty();
    }
    // np.argmax(np.abs(iacf)): first maximum, or the first NaN.
    let mut peak = 0;
    for (i, v) in result.iacf.iter().enumerate() {
        if v.is_nan() {
            peak = i;
            break;
        }
        if v.abs() > result.iacf[peak].abs() {
            peak = i;
        }
    }
    result.iacc = result.iacf[peak].abs();
    result.tau_ms = result.lags_ms[peak];
    result
}

/// The six 2.x interactive panels, in `BOKEH_ANALYSIS_GENERATORS` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Panel {
    InterauralOverlay,
    Ild,
    Ipd,
    Iacc,
    Edc,
    ResultOverview,
}

impl Panel {
    pub const ALL: [Panel; 6] = [
        Panel::InterauralOverlay,
        Panel::Ild,
        Panel::Ipd,
        Panel::Iacc,
        Panel::Edc,
        Panel::ResultOverview,
    ];
    /// Registry `name`; also the `plots/<name>/<name>_analysis.html` stem.
    pub fn name(self) -> &'static str {
        match self {
            Panel::InterauralOverlay => "interaural_overlay",
            Panel::Ild => "ild",
            Panel::Ipd => "ipd",
            Panel::Iacc => "iacc",
            Panel::Edc => "etc",
            Panel::ResultOverview => "result_overview",
        }
    }
    /// Registry `title` (tab label).
    pub fn title(self) -> &'static str {
        match self {
            Panel::InterauralOverlay => "Interaural Overlay",
            Panel::Ild => "ILD",
            Panel::Ipd => "IPD",
            Panel::Iacc => "IACC",
            Panel::Edc => "EDC",
            Panel::ResultOverview => "Result Overview",
        }
    }
    /// Registry `save_individually`: written under `plots/` when `--plot` is on.
    pub fn save_individually(self) -> bool {
        matches!(self, Panel::Ild | Panel::Ipd | Panel::Iacc | Panel::Edc)
    }
}

/// Which ear a series belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ear {
    Left,
    Right,
}

impl Ear {
    /// 2.x legend label (`f"{side.capitalize()} Ear"`).
    pub fn label(self) -> &'static str {
        match self {
            Ear::Left => "Left Ear",
            Ear::Right => "Right Ear",
        }
    }
}

/// Interaural overlay of one speaker: both full responses on one time axis in
/// ms relative to `min(peak_left, peak_right)`.
#[derive(Clone, Debug)]
pub struct OverlayChart<'a> {
    pub speaker: &'a str,
    pub fs: u32,
    pub left: &'a [f64],
    pub right: &'a [f64],
    /// `ImpulseResponse.peak_index()` of each ear.
    pub left_peak: usize,
    pub right_peak: usize,
}

impl OverlayChart<'_> {
    /// The alignment sample (`align_peak_idx`).
    pub fn origin(&self) -> usize {
        self.left_peak.min(self.right_peak)
    }
    /// 2.x time vector: `(i - origin) / fs * 1000`.
    pub fn time_ms(&self, index: usize) -> f64 {
        (index as f64 - self.origin() as f64) / f64::from(self.fs) * 1000.0
    }
}

/// ILD or IPD bars of one speaker. Only bands with a finite value are listed,
/// in band order (2.x `valid_indices`).
#[derive(Clone, Debug, PartialEq)]
pub struct BandChart<'a> {
    pub speaker: &'a str,
    /// `(lower, upper)` Hz of each listed band.
    pub bands: Vec<(f64, f64)>,
    /// 2.x category labels, `f"{int(lower)}-{int(upper)}Hz"`.
    pub labels: Vec<String>,
    pub values: Vec<f64>,
}

/// IACF of one speaker with a finite IACC.
#[derive(Clone, Debug, PartialEq)]
pub struct IaccChart<'a> {
    pub speaker: &'a str,
    pub max_delay_ms: f64,
    pub result: Iacf,
}

impl IaccChart<'_> {
    /// 2.x legend label, `f"Max: {iacc:.2f} at {tau:.2f}ms"`.
    pub fn legend(&self) -> String {
        format!(
            "Max: {:.2} at {:.2}ms",
            self.result.iacc, self.result.tau_ms
        )
    }
}

/// Energy decay curves of one speaker: one per nonempty ear, left first.
#[derive(Clone, Debug, PartialEq)]
pub struct EdcChart<'a> {
    pub speaker: &'a str,
    pub fs: u32,
    pub curves: Vec<(Ear, Vec<f64>)>,
}

/// 2.x `generate_result_bokeh_figure` series: the frequency response of the
/// left and right ears summed over all speakers, raw and smoothed, and the
/// smoothed left minus right difference, on the shared log grid.
#[derive(Clone, Debug, PartialEq)]
pub struct Overview {
    pub frequency: Vec<f64>,
    pub left_raw: Vec<f64>,
    pub left_smoothed: Vec<f64>,
    pub right_raw: Vec<f64>,
    pub right_smoothed: Vec<f64>,
    pub difference: Vec<f64>,
}

/// A panel that could not be computed; 2.x logs these as
/// `cli_warning_interactive_plot_error` and continues with the other panels.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelError {
    pub panel: Panel,
    pub message: String,
}

/// Everything the interactive report shows, borrowed from one HRIR snapshot.
#[derive(Clone, Debug)]
pub struct Report<'a> {
    pub fs: u32,
    pub speakers: usize,
    pub overlay: Vec<OverlayChart<'a>>,
    pub ild: Vec<BandChart<'a>>,
    pub ipd: Vec<BandChart<'a>>,
    pub iacc: Vec<IaccChart<'a>>,
    pub edc: Vec<EdcChart<'a>>,
    pub overview: Option<Overview>,
    pub errors: Vec<PanelError>,
}

impl Report<'_> {
    /// Whether the 2.x generator for `panel` would have returned a layout.
    pub fn has(&self, panel: Panel) -> bool {
        match panel {
            Panel::InterauralOverlay => !self.overlay.is_empty(),
            Panel::Ild => !self.ild.is_empty(),
            Panel::Ipd => !self.ipd.is_empty(),
            Panel::Iacc => !self.iacc.is_empty(),
            Panel::Edc => !self.edc.is_empty(),
            Panel::ResultOverview => self.overview.is_some(),
        }
    }
    /// No panel at all: 2.x logs `cli_warning_no_interactive` and writes nothing.
    pub fn is_empty(&self) -> bool {
        !Panel::ALL.iter().any(|panel| self.has(*panel))
    }
}

/// 2.x result overview smoothing: `window_size=1/3, treble_window_size=1/5,
/// treble_f_lower=20000, treble_f_upper=max(20001, int(fs / 2 - 1))`.
pub fn overview_treble_f_upper(fs: u32) -> f64 {
    (f64::from(fs) / 2.0 - 1.0).trunc().max(20001.0)
}

/// Frequency response of one summed ear, as `ImpulseResponse.frequency_response()`
/// followed by the result-overview smoothing.
fn overview_response(data: &[f64], fs: u32) -> Result<FrequencyResponse, String> {
    let mut fr = magnitude_to_frequency_response("Frequency response", fs, data)
        .map_err(|e| e.to_string())?;
    fr.smoothen(1.0 / 3.0, 1.0 / 5.0, 20000.0, overview_treble_f_upper(fs))
        .map_err(|e| e.to_string())?;
    Ok(fr)
}

/// The result overview on its own: `Ok(None)` when either ear has no nonempty
/// response or the sums are at most one sample long, `Err` when the responses
/// have different lengths (2.x `np.vstack` raises) or the response fails.
pub fn result_overview(hrir: &Hrir) -> Result<Option<Overview>, String> {
    let sum = |ear: Ear| -> Result<Option<Vec<f64>>, String> {
        let mut sum: Option<Vec<f64>> = None;
        for speaker in &hrir.speakers {
            let ir = match ear {
                Ear::Left => &speaker.left,
                Ear::Right => &speaker.right,
            };
            let Some(ir) = ir.as_ref().filter(|ir| !ir.is_empty()) else {
                continue;
            };
            match &mut sum {
                None => sum = Some(ir.data.clone()),
                Some(sum) if sum.len() == ir.len() => {
                    for (s, v) in sum.iter_mut().zip(&ir.data) {
                        *s += v;
                    }
                }
                Some(sum) => {
                    return Err(format!(
                        "all the input array dimensions except for the concatenation axis must match exactly ({} != {})",
                        sum.len(),
                        ir.len()
                    ));
                }
            }
        }
        Ok(sum)
    };
    let (Some(left), Some(right)) = (sum(Ear::Left)?, sum(Ear::Right)?) else {
        return Ok(None);
    };
    if left.len() <= 1 || right.len() <= 1 {
        return Ok(None);
    }
    let left = overview_response(&left, hrir.fs)?;
    let right = overview_response(&right, hrir.fs)?;
    if right.frequency != left.frequency {
        return Err("left and right frequency grids differ".into());
    }
    let difference = left
        .smoothed
        .iter()
        .zip(&right.smoothed)
        .map(|(l, r)| l - r)
        .collect();
    Ok(Some(Overview {
        frequency: left.frequency,
        left_raw: left.raw,
        left_smoothed: left.smoothed,
        right_raw: right.raw,
        right_smoothed: right.smoothed,
        difference,
    }))
}

/// 2.x band label, `f"{int(lower)}-{int(upper)}Hz"` (truncation toward zero).
pub fn band_label(band: (f64, f64)) -> String {
    format!("{}-{}Hz", band.0.trunc() as i64, band.1.trunc() as i64)
}

fn band_chart<'a>(speaker: &'a str, bands: &[(f64, f64)], values: &[f64]) -> Option<BandChart<'a>> {
    let mut chart = BandChart {
        speaker,
        bands: Vec::new(),
        labels: Vec::new(),
        values: Vec::new(),
    };
    for (band, value) in bands.iter().zip(values) {
        if !value.is_nan() {
            chart.bands.push(*band);
            chart.labels.push(band_label(*band));
            chart.values.push(*value);
        }
    }
    (!chart.values.is_empty()).then_some(chart)
}

/// Compute every panel of the 2.x interactive summary from one HRIR snapshot.
/// Overlay, ILD, IPD and IACC need both ears nonempty; EDC takes any nonempty
/// ear; ILD/IPD charts without a finite band and IACC charts without energy
/// are dropped, exactly as the 2.x generators skip them. Charts follow the
/// canonical speaker order (`SPEAKER_NAMES`, as README.md does), other names
/// after them in measurement order; 2.x used the order the files were read.
pub fn build_report(hrir: &Hrir) -> Report<'_> {
    let fs = f64::from(hrir.fs);
    let bands = octave_bands(fs, &DEFAULT_OCTAVE_CENTERS);
    let mut report = Report {
        fs: hrir.fs,
        speakers: hrir.speakers.len(),
        overlay: Vec::new(),
        ild: Vec::new(),
        ipd: Vec::new(),
        iacc: Vec::new(),
        edc: Vec::new(),
        overview: None,
        errors: Vec::new(),
    };
    let mut speakers: Vec<_> = hrir.speakers.iter().collect();
    speakers.sort_by_key(|s| {
        SPEAKER_NAMES
            .iter()
            .position(|name| *name == s.speaker)
            .unwrap_or(usize::MAX)
    });
    for speaker in speakers {
        let name = speaker.speaker.as_str();
        let left = speaker.left.as_ref().filter(|ir| !ir.is_empty());
        let right = speaker.right.as_ref().filter(|ir| !ir.is_empty());
        if let (Some(left), Some(right)) = (left, right) {
            report.overlay.push(OverlayChart {
                speaker: name,
                fs: hrir.fs,
                left: &left.data,
                right: &right.data,
                left_peak: peaks::first_peak_index(&left.data, 0, None, PEAK_HEIGHT),
                right_peak: peaks::first_peak_index(&right.data, 0, None, PEAK_HEIGHT),
            });
            // One transform pair serves both ILD and IPD (2.x computes it twice).
            let spectra = band_cross_spectra(&left.data, &right.data, fs, &bands);
            let ild: Vec<_> = spectra.iter().map(ild_from).collect();
            let ipd: Vec<_> = spectra.iter().map(ipd_from).collect();
            report.ild.extend(band_chart(name, &bands, &ild));
            report.ipd.extend(band_chart(name, &bands, &ipd));
            let result =
                interaural_cross_correlation(&left.data, &right.data, fs, IACC_MAX_DELAY_MS);
            if !result.iacf.is_empty() && !result.iacc.is_nan() {
                report.iacc.push(IaccChart {
                    speaker: name,
                    max_delay_ms: IACC_MAX_DELAY_MS,
                    result,
                });
            }
        }
        let curves: Vec<_> = [(Ear::Left, left), (Ear::Right, right)]
            .into_iter()
            .filter_map(|(ear, ir)| {
                ir.map(|ir| (ear, energy_decay_curve_db(&ir.data, EDC_RANGE_DB.0)))
            })
            .collect();
        if !curves.is_empty() {
            report.edc.push(EdcChart {
                speaker: name,
                fs: hrir.fs,
                curves,
            });
        }
    }
    match result_overview(hrir) {
        Ok(overview) => report.overview = overview,
        Err(message) => report.errors.push(PanelError {
            panel: Panel::ResultOverview,
            message,
        }),
    }
    report
}
