# -*- coding: utf-8 -*-
"""scipy.signal shim — pure Python+NumPy implementations."""

import numpy as np
from . import windows
from .windows import hann, get_window


def convolve(in1, in2, mode='full', method='auto'):
    """Convolve two N-dimensional arrays.

    Supports mode='full', 'same', 'valid'.
    The 'method' parameter is accepted but ignored (always uses FFT for large inputs).
    """
    in1 = np.asarray(in1)
    in2 = np.asarray(in2)

    if in1.ndim != 1 or in2.ndim != 1:
        raise NotImplementedError("scipy_shim.signal.convolve only supports 1D arrays")

    # For small arrays, use direct convolution; for large, use FFT
    if len(in1) + len(in2) < 500 and method != 'fft':
        result = np.convolve(in1, in2, mode=mode)
    else:
        result = _fftconvolve(in1, in2, mode=mode)
    return result


def _fftconvolve(in1, in2, mode='full'):
    """FFT-based convolution for 1D arrays."""
    s1, s2 = len(in1), len(in2)
    shape = s1 + s2 - 1

    # Use power-of-2 for speed
    fshape = 1
    while fshape < shape:
        fshape *= 2

    sp1 = np.fft.rfft(in1, fshape)
    sp2 = np.fft.rfft(in2, fshape)
    ret = np.fft.irfft(sp1 * sp2, fshape)[:shape]

    if mode == 'full':
        return ret
    elif mode == 'same':
        start = (s2 - 1) // 2
        return ret[start:start + s1]
    elif mode == 'valid':
        start = s2 - 1
        return ret[start:start + s1 - s2 + 1]
    else:
        raise ValueError(f"Unknown mode: {mode}")


def correlate(in1, in2, mode='full'):
    """Cross-correlate two 1D arrays."""
    in1 = np.asarray(in1)
    in2 = np.asarray(in2)
    return convolve(in1, in2[::-1].conj(), mode=mode)


def correlation_lags(in1_len, in2_len, mode='full'):
    """Return lag indices for cross-correlation."""
    if mode == 'full':
        lags = np.arange(-(in2_len - 1), in1_len)
    elif mode == 'same':
        lags = np.arange(-(in2_len - 1) // 2, in1_len - (in2_len - 1) // 2)
    elif mode == 'valid':
        lags = np.arange(0, in1_len - in2_len + 1)
    else:
        raise ValueError(f"Unknown mode: {mode}")
    return lags


def find_peaks(x, height=None, threshold=None, distance=None,
               prominence=None, width=None, wlen=None,
               rel_height=0.5, plateau_size=None):
    """Find peaks (local maxima) in a 1D array.

    Simplified implementation supporting the parameters used by Impulcifer:
    - height: minimum peak height
    Returns (peaks, properties) where properties contains 'peak_heights'.
    """
    x = np.asarray(x, dtype=np.float64)
    if x.ndim != 1:
        raise ValueError("x must be 1D")

    # Find local maxima: x[i] > x[i-1] and x[i] > x[i+1]
    # Also handles plateaus (first index of flat peak)
    peaks = []
    i = 1
    while i < len(x) - 1:
        if x[i] > x[i - 1]:
            # Rising edge found, look for the peak end
            j = i
            while j + 1 < len(x) and x[j + 1] == x[j]:
                j += 1
            if j == len(x) - 1 or x[j + 1] < x[j]:
                # This is a peak (or plateau peak)
                peaks.append(i)
            i = j + 1
        else:
            i += 1

    peaks = np.array(peaks, dtype=np.intp)
    properties = {}

    # Apply height filter
    if height is not None:
        peak_heights = x[peaks]
        if np.isscalar(height):
            mask = peak_heights >= height
        else:
            mask = peak_heights >= height[0]
            if len(height) > 1:
                mask &= peak_heights <= height[1]
        peaks = peaks[mask]
        properties['peak_heights'] = x[peaks]
    else:
        properties['peak_heights'] = x[peaks] if len(peaks) > 0 else np.array([])

    # Apply distance filter
    if distance is not None and len(peaks) > 1:
        keep = np.ones(len(peaks), dtype=bool)
        priority = x[peaks]
        # Sort by height (descending) to keep tallest peaks
        order = np.argsort(-priority)
        for idx in order:
            if not keep[idx]:
                continue
            # Remove peaks too close to this one
            for other in range(len(peaks)):
                if other != idx and keep[other]:
                    if abs(peaks[idx] - peaks[other]) < distance:
                        keep[other] = False
        peaks = peaks[keep]
        properties['peak_heights'] = x[peaks]

    return peaks, properties


def unit_impulse(shape, idx=None, dtype=float):
    """Unit impulse signal (Kronecker delta)."""
    if isinstance(shape, int):
        shape = (shape,)
    out = np.zeros(shape, dtype=dtype)
    if idx is None:
        idx = 0
    if isinstance(idx, str) and idx == 'mid':
        idx = tuple(s // 2 for s in shape)
    if isinstance(idx, int):
        idx = (idx,)
    out[idx] = 1.0
    return out


def butter(N, Wn, btype='low', analog=False, output='ba', fs=None):
    """Butterworth digital filter design.

    Returns second-order sections (SOS) format when output='sos'.
    Supports low, high, and bandpass filter types.
    Uses bilinear transform of analog prototype.
    """
    if analog:
        raise NotImplementedError("Analog Butterworth not implemented in shim")

    if fs is not None:
        # Normalize frequency to Nyquist
        nyq = fs / 2.0
        if np.isscalar(Wn):
            Wn = Wn / nyq
        else:
            Wn = [w / nyq for w in Wn]

    if output != 'sos':
        raise NotImplementedError("Only output='sos' is supported in shim")

    if btype in ('low', 'high'):
        if not np.isscalar(Wn):
            Wn = Wn[0] if btype == 'low' else Wn[-1]
        return _butter_single(N, Wn, btype)
    elif btype in ('band', 'bandpass'):
        Wn = list(Wn)
        return _butter_bandpass(N, Wn[0], Wn[1])
    else:
        raise ValueError(f"Unknown btype: {btype}")


def _butter_single(N, Wn, btype):
    """Design lowpass or highpass Butterworth as SOS."""
    # Pre-warp the cutoff frequency
    wn_warped = 2.0 * np.tan(np.pi * Wn / 2.0)

    # Analog prototype poles (unit circle, left half-plane)
    poles = []
    for k in range(N):
        theta = np.pi * (2 * k + N + 1) / (2 * N)
        poles.append(wn_warped * np.exp(1j * theta))

    # Bilinear transform: s = 2*(z-1)/(z+1)
    # Map analog poles to digital
    z_poles = []
    for p in poles:
        z_poles.append((2.0 + p) / (2.0 - p))

    # For Butterworth lowpass, all zeros are at z = -1
    # For highpass, all zeros are at z = 1
    if btype == 'low':
        z_zeros = [-1.0] * N
    else:
        z_zeros = [1.0] * N

    z_poles = np.array(z_poles)
    z_zeros = np.array(z_zeros, dtype=complex)

    # Compute gain
    # For lowpass: H(1) should equal 1 (DC gain = 1)
    # For highpass: H(-1) should equal 1 (Nyquist gain = 1)
    if btype == 'low':
        z_eval = 1.0
    else:
        z_eval = -1.0

    num = np.prod(z_eval - z_zeros)
    den = np.prod(z_eval - z_poles)
    k = np.real(den / num)

    return _zpk2sos(z_zeros, z_poles, k)


def _butter_bandpass(N, Wn_low, Wn_high):
    """Design bandpass Butterworth as SOS."""
    # Pre-warp
    w1 = 2.0 * np.tan(np.pi * Wn_low / 2.0)
    w2 = 2.0 * np.tan(np.pi * Wn_high / 2.0)
    bw = w2 - w1
    w0 = np.sqrt(w1 * w2)

    # Analog prototype poles for lowpass of order N
    analog_poles = []
    for k in range(N):
        theta = np.pi * (2 * k + N + 1) / (2 * N)
        analog_poles.append(np.exp(1j * theta))

    # Transform lowpass to bandpass: each pole p becomes two poles
    bp_poles = []
    for p in analog_poles:
        sp = p * bw / 2.0
        sq = np.sqrt(sp * sp - w0 * w0 + 0j)
        bp_poles.append(sp + sq)
        bp_poles.append(sp - sq)

    # Bandpass has N zeros at z=1 and N zeros at z=-1
    bp_zeros_analog = [0.0] * N  # analog zeros at s=0

    # Bilinear transform
    z_poles = [(2.0 + p) / (2.0 - p) for p in bp_poles]
    z_zeros = []
    # N zeros at z=1, N zeros at z=-1 (from bilinear transform of s=0 and s=inf)
    for _ in range(N):
        z_zeros.append(1.0 + 0j)
        z_zeros.append(-1.0 + 0j)

    z_poles = np.array(z_poles)
    z_zeros = np.array(z_zeros)

    # Compute gain at center frequency
    w_center = (Wn_low + Wn_high) / 2.0
    z_eval = np.exp(1j * np.pi * w_center)
    num = np.prod(z_eval - z_zeros)
    den = np.prod(z_eval - z_poles)
    k = np.abs(den / num)

    return _zpk2sos(z_zeros, z_poles, k)


def _zpk2sos(zeros, poles, gain):
    """Convert zeros, poles, gain to second-order sections.

    Pairs complex conjugate poles/zeros into second-order sections.
    """
    zeros = np.array(zeros, dtype=complex)
    poles = np.array(poles, dtype=complex)

    # Pair complex conjugate poles
    pole_pairs = _pair_conjugates(poles)
    zero_pairs = _pair_conjugates(zeros)

    n_sections = max(len(pole_pairs), len(zero_pairs))

    # Pad with trivial sections if needed
    while len(zero_pairs) < n_sections:
        zero_pairs.append(np.array([], dtype=complex))
    while len(pole_pairs) < n_sections:
        pole_pairs.append(np.array([], dtype=complex))

    # Build SOS matrix [b0, b1, b2, 1, a1, a2] per section
    sos = np.zeros((n_sections, 6))
    remaining_gain = gain

    for i in range(n_sections):
        zs = zero_pairs[i]
        ps = pole_pairs[i]

        # Numerator from zeros
        if len(zs) == 2:
            b = np.real(np.polymul([1, -zs[0]], [1, -zs[1]]))
        elif len(zs) == 1:
            b = np.real([1, -zs[0], 0])
        else:
            b = np.array([0, 0, 1.0])

        # Denominator from poles
        if len(ps) == 2:
            a = np.real(np.polymul([1, -ps[0]], [1, -ps[1]]))
        elif len(ps) == 1:
            a = np.real([1, -ps[0], 0])
        else:
            a = np.array([1, 0, 0])

        # Normalize so a[0] = 1
        if a[0] != 0:
            b = b / a[0]
            a = a / a[0]

        sos[i, :3] = b[:3] if len(b) >= 3 else np.pad(b, (0, 3 - len(b)))
        sos[i, 3:] = a[:3] if len(a) >= 3 else np.pad(a, (0, 3 - len(a)))

    # Distribute gain across sections
    if n_sections > 0:
        sos[0, :3] *= remaining_gain

    return sos


def _pair_conjugates(arr):
    """Group complex numbers into conjugate pairs."""
    arr = list(arr)
    pairs = []
    used = [False] * len(arr)

    for i in range(len(arr)):
        if used[i]:
            continue
        if np.imag(arr[i]) == 0:
            # Real value, standalone or pair with another real
            pairs.append(np.array([arr[i]]))
            used[i] = True
        else:
            # Find conjugate
            used[i] = True
            found = False
            for j in range(i + 1, len(arr)):
                if not used[j] and np.abs(arr[i] - np.conj(arr[j])) < 1e-12:
                    pairs.append(np.array([arr[i], arr[j]]))
                    used[j] = True
                    found = True
                    break
            if not found:
                # Add conjugate manually
                pairs.append(np.array([arr[i], np.conj(arr[i])]))

    # Merge single-element pairs where possible
    merged = []
    singles = [p for p in pairs if len(p) == 1]
    doubles = [p for p in pairs if len(p) == 2]

    while len(singles) >= 2:
        merged.append(np.array([singles[0][0], singles[1][0]]))
        singles = singles[2:]
    for s in singles:
        merged.append(s)
    merged.extend(doubles)

    return merged


def sosfilt(sos, x, axis=-1):
    """Filter data along one dimension using cascaded second-order sections.

    Each row of sos is [b0, b1, b2, a0, a1, a2].
    """
    sos = np.atleast_2d(np.asarray(sos, dtype=np.float64))
    x = np.asarray(x, dtype=np.float64)

    y = x.copy()

    for section in sos:
        b0, b1, b2 = section[0], section[1], section[2]
        a0, a1, a2 = section[3], section[4], section[5]

        # Normalize
        if a0 != 1.0:
            b0 /= a0
            b1 /= a0
            b2 /= a0
            a1 /= a0
            a2 /= a0

        # Direct Form II transposed
        d1 = 0.0
        d2 = 0.0
        out = np.empty_like(y)

        for n in range(len(y)):
            xn = y[n]
            out[n] = b0 * xn + d1
            d1 = b1 * xn - a1 * out[n] + d2
            d2 = b2 * xn - a2 * out[n]

        y = out

    return y


def minimum_phase(h, method='homomorphic', n_fft=None, half=True):
    """Convert a linear-phase FIR filter to minimum phase.

    Matches scipy.signal.minimum_phase behavior.

    Parameters
    ----------
    h : array
        Linear-phase FIR filter coefficients.
    method : str
        Only 'homomorphic' is supported.
    n_fft : int
        FFT length for computation.
    half : bool
        If True (default), return filter with half the original length.
    """
    h = np.asarray(h, dtype=np.float64)

    if method != 'homomorphic':
        raise NotImplementedError(f"Only 'homomorphic' method supported, got {method}")

    h_len = len(h)
    n_half = h_len // 2
    if n_fft is None:
        n_fft = 2 ** int(np.ceil(np.log2(2 * (h_len - 1) / 0.01)))
    n_fft = max(n_fft, h_len)

    # Compute magnitude spectrum
    h_temp = np.abs(np.fft.fft(h, n_fft))

    # Avoid log(0): add small fraction of minimum positive value
    pos_vals = h_temp[h_temp > 0]
    if len(pos_vals) > 0:
        h_temp += 1e-7 * np.min(pos_vals)
    else:
        h_temp += 1e-20

    h_temp = np.log(h_temp)
    if half:
        h_temp *= 0.5  # square root of magnitude spectrum

    # IDFT to cepstral domain
    h_temp = np.real(np.fft.ifft(h_temp))

    # Apply causal window (double positive frequencies, zero negative)
    win = np.zeros(n_fft)
    win[0] = 1.0
    stop = n_fft // 2
    win[1:stop] = 2.0
    if n_fft % 2:
        win[stop] = 1.0

    h_temp *= win

    # Back to time domain via exp(fft(cepstrum))
    h_minimum = np.fft.ifft(np.exp(np.fft.fft(h_temp))).real

    if half:
        n_out = n_half + (h_len % 2)
    else:
        n_out = h_len
    return h_minimum[:n_out]


def spectrogram(x, fs=1.0, window='hann', nperseg=256, noverlap=None,
                nfft=None, detrend=False, return_onesided=True,
                scaling='density', axis=-1, mode='psd'):
    """Compute a spectrogram using short-time Fourier transform.

    Simplified implementation supporting modes used by Impulcifer: 'psd' and 'magnitude'.
    """
    x = np.asarray(x, dtype=np.float64)

    if noverlap is None:
        noverlap = nperseg // 2
    if nfft is None:
        nfft = nperseg

    # Get window
    if isinstance(window, str):
        win = get_window(window, nperseg)
    elif isinstance(window, np.ndarray):
        win = window
        nperseg = len(win)
    else:
        win = np.array(window, dtype=np.float64)
        nperseg = len(win)

    step = nperseg - noverlap
    n_segments = max(0, (len(x) - nperseg) // step + 1)

    if return_onesided:
        n_freqs = nfft // 2 + 1
    else:
        n_freqs = nfft

    freqs = np.fft.rfftfreq(nfft, 1.0 / fs) if return_onesided else np.fft.fftfreq(nfft, 1.0 / fs)
    t = np.arange(n_segments) * step / fs + nperseg / (2.0 * fs)
    Sxx = np.zeros((n_freqs, n_segments))

    for i in range(n_segments):
        start = i * step
        segment = x[start:start + nperseg] * win

        if return_onesided:
            spec = np.fft.rfft(segment, n=nfft)
        else:
            spec = np.fft.fft(segment, n=nfft)

        if mode == 'psd':
            Sxx[:, i] = np.abs(spec) ** 2
        elif mode == 'magnitude':
            Sxx[:, i] = np.abs(spec)
        elif mode == 'complex':
            pass  # Would need complex output array
        else:
            Sxx[:, i] = np.abs(spec) ** 2

    # Scaling
    if mode == 'psd' and scaling == 'density':
        win_sum_sq = np.sum(win ** 2)
        Sxx /= (fs * win_sum_sq)
    elif mode == 'psd' and scaling == 'spectrum':
        win_sum = np.sum(win) ** 2
        Sxx /= win_sum

    return freqs, t, Sxx


def resample(x, num, t=None, axis=0):
    """Resample x to num samples using FFT method."""
    x = np.asarray(x)
    n_orig = x.shape[axis]

    X = np.fft.rfft(x, axis=axis)

    # Create new spectrum of appropriate length
    n_new_freq = num // 2 + 1
    n_old_freq = len(X) if x.ndim == 1 else X.shape[axis]
    n_copy = min(n_new_freq, n_old_freq)

    if x.ndim == 1:
        Y = np.zeros(n_new_freq, dtype=complex)
        Y[:n_copy] = X[:n_copy]
    else:
        shape = list(X.shape)
        shape[axis] = n_new_freq
        Y = np.zeros(shape, dtype=complex)
        slc_src = [slice(None)] * x.ndim
        slc_dst = [slice(None)] * x.ndim
        slc_src[axis] = slice(0, n_copy)
        slc_dst[axis] = slice(0, n_copy)
        Y[tuple(slc_dst)] = X[tuple(slc_src)]

    y = np.fft.irfft(Y, n=num, axis=axis) * (num / n_orig)

    if t is not None:
        new_t = np.linspace(t[0], t[-1], num)
        return y, new_t
    return y
