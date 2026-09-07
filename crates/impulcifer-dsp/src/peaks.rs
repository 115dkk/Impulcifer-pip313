//! Height-only peak selection and the 2.x impulse onset rule.

/// Mirrors scipy.signal.find_peaks(x, height=height), pinned by p05_peaks_*.json.
/// https://docs.scipy.org/doc/scipy-1.16.1/reference/generated/scipy.signal.find_peaks.html
/// Returns ascending indices, inclusive height, floor midpoint for plateaus,
/// and excludes endpoints. NaN comparisons are false, so NaNs break candidates;
/// an NaN height excludes all peaks. Infinities retain ordinary comparison rules.
pub fn find_peaks(x: &[f64], height: Option<f64>) -> Vec<usize> {
    let mut peaks = Vec::new();
    let mut i = 1;
    while i + 1 < x.len() {
        if x[i - 1] < x[i] {
            let mut right = i;
            while right + 1 < x.len() && x[right + 1] == x[i] {
                right += 1;
            }
            if right + 1 < x.len() && x[right + 1] < x[i] && height.is_none_or(|h| x[i] >= h) {
                peaks.push(i + (right - i) / 2);
            }
            i = right;
        }
        i += 1;
    }
    peaks
}

/// Mirrors ImpulseResponse.peak_index / core.decay._peak_index; pinned by
/// p05_first_peak_*.json. Normalize only the searched copy, seek both signs,
/// return the earliest eligible peak or the first absolute argmax. End is
/// exclusive and clipped like Python slicing; silence is strictly below 1e-20.
/// Wholly empty data returns 0 regardless of start (Python's early guard);
/// an empty search range inside non-empty data returns start.
/// NumPy max/argmax NaN behavior is preserved: NaN normalization yields all
/// NaNs, so fallback selects the first searched sample. Infinite maxima behave
/// similarly, selecting the first resulting NaN if there is one.
pub fn first_peak_index(data: &[f64], start: usize, end: Option<usize>, peak_height: f64) -> usize {
    if data.is_empty() {
        return 0;
    }
    let end = end.unwrap_or(data.len()).min(data.len());
    if start >= end {
        return start;
    }
    let slice = &data[start..end];
    let max = if slice.iter().any(|v| v.is_nan()) {
        f64::NAN
    } else {
        slice.iter().map(|v| v.abs()).fold(0.0, f64::max)
    };
    if max < 1e-20 {
        return start;
    }
    let normalized: Vec<_> = slice.iter().map(|v| v / max).collect();
    let positive = find_peaks(&normalized, Some(peak_height));
    let negative = find_peaks(
        &normalized.iter().map(|v| -v).collect::<Vec<_>>(),
        Some(peak_height),
    );
    if let Some(index) = positive.first().into_iter().chain(negative.first()).min() {
        return start + index;
    }
    let mut best = 0;
    for i in 0..normalized.len() {
        if normalized[i].is_nan() {
            return start + i;
        }
        if normalized[i].abs() > normalized[best].abs() {
            best = i;
        }
    }
    start + best
}
