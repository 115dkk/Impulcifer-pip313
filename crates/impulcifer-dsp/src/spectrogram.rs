//! Recorded-sweep parameter selection and SciPy 1.18.0 legacy PSD spectrogram.
use crate::{DspError, fft, windows};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpectrogramParams {
    pub nfft: usize,
    pub noverlap: usize,
}

/// Mirrors ImpulseResponsePlotter.plot_spectrogram, core/plotting/
/// impulse_response_plotter.py:114-222; p06_params.json pins every valid branch.
/// Python defaults are f_res=10, n_segments=200, min_time_segments=3.
/// Uses ties-to-even round(fs/f_res), (2*n)//4, true division for step_size,
/// truncation toward zero for overlap, and Python's half-overlap fallbacks.
/// Empty input/zero rounded FFT returns None. Invalid fs/f_res or sizes not
/// representable as usize also return None instead of Python's numeric error.
pub fn spectrogram_params(
    n: usize,
    fs: u32,
    f_res: f64,
    n_segments: usize,
) -> Option<SpectrogramParams> {
    if n == 0 || fs == 0 || !f_res.is_finite() || f_res <= 0.0 {
        return None;
    }
    let target = (fs as f64 / f_res).round_ties_even();
    if !target.is_finite() || target >= usize::MAX as f64 {
        return None;
    }
    // Algebraically (2*n)//(3+1), without overflowing 2*n.
    let max_nfft = if n / 2 == 0 { n } else { n / 2 };
    let nfft = (target as usize).min(max_nfft).min(n);
    if nfft == 0 {
        return None;
    }
    let mut noverlap = nfft / 2;
    if n_segments > 0 && n > nfft {
        let step = (n - nfft) as f64 / n_segments as f64;
        if step > 1.0 {
            let overlap = (nfft as f64 - step).trunc();
            // Clamping a negative overlap before casting preserves int()+clamp.
            noverlap = overlap.max(0.0) as usize;
        }
    }
    if noverlap >= nfft {
        noverlap = nfft - 1;
    }
    Some(SpectrogramParams { nfft, noverlap })
}

#[derive(Debug, Clone)]
pub struct Spectrogram {
    pub freqs: Vec<f64>,
    pub times: Vec<f64>,
    pub power: Vec<Vec<f64>>,
}

/// Mirrors scipy.signal.spectrogram (SciPy 1.18.0) with explicit periodic Hann,
/// nfft=nperseg, detrend='constant', scaling='density', return_onesided=True,
/// mode='psd', axis=-1, boundary=None, padded=False. p06_spectrogram_*.json
/// pins full frequency-major power, frequencies and segment-center timestamps.
/// https://docs.scipy.org/doc/scipy/reference/generated/scipy.signal.spectrogram.html
/// Rejects fs=0, zero/oversized explicit window, and noverlap>=nperseg. Unlike
/// a named window, SciPy's explicit array does NOT shrink to short input.
/// Nonfinite samples propagate through their segment's mean and FFT.
/// Work O(segments*nperseg*log(nperseg)); output O((nperseg/2+1)*segments).
pub fn spectrogram(
    x: &[f64],
    fs: u32,
    nperseg: usize,
    noverlap: usize,
) -> Result<Spectrogram, DspError> {
    if fs == 0 || nperseg == 0 || nperseg > x.len() || noverlap >= nperseg {
        return Err(DspError::InvalidArgument(
            "spectrogram requires positive fs, 0 < nperseg <= len and noverlap < nperseg".into(),
        ));
    }
    let step = nperseg - noverlap;
    let count = (x.len() - nperseg) / step + 1;
    let window = windows::hann(nperseg, false);
    let scale = 1.0 / (fs as f64 * window.iter().map(|v| v * v).sum::<f64>());
    let bins = nperseg / 2 + 1;
    // Match scipy.fft.rfftfreq's reciprocal construction rather than fs/n.
    let spacing = 1.0 / (nperseg as f64 * (1.0 / fs as f64));
    let freqs = (0..bins).map(|i| i as f64 * spacing).collect();
    let times = (0..count)
        .map(|i| (nperseg as f64 / 2.0 + (i * step) as f64) / fs as f64)
        .collect();
    let mut power = vec![vec![0.0; count]; bins];
    for segment in 0..count {
        let data = &x[segment * step..segment * step + nperseg];
        let mean = data.iter().sum::<f64>() / nperseg as f64;
        let centered: Vec<_> = data
            .iter()
            .zip(&window)
            .map(|(v, w)| (v - mean) * w)
            .collect();
        for (bin, value) in fft::rfft(&centered).iter().enumerate() {
            let doubled = bin > 0 && !(nperseg.is_multiple_of(2) && bin == nperseg / 2);
            power[bin][segment] = value.norm_sqr() * scale * if doubled { 2.0 } else { 1.0 };
        }
    }
    Ok(Spectrogram {
        freqs,
        times,
        power,
    })
}
