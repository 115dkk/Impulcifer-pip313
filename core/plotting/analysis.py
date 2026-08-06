"""바이노럴 IR 분석 지표 계산 (Bokeh 플롯의 수치 코어).

플롯 코드와 분리된 순수 함수들이라 합성 신호로 직접 검증할 수 있다.
``tests/test_bokeh_analysis.py``가 각 지표의 음향학적 정합성을 고정한다.
"""

import numpy as np
from scipy import signal
from scipy.fft import fft, next_fast_len

DEFAULT_OCTAVE_CENTERS = (125, 250, 500, 1000, 2000, 4000, 8000, 16000)


def octave_bands(fs, centers=DEFAULT_OCTAVE_CENTERS):
    """옥타브 중심 주파수 목록을 (하한, 상한) 대역 리스트로 변환한다.

    상한은 나이퀴스트로 클램프되며, 나이퀴스트에 도달하면 이후 대역은
    생성하지 않는다.
    """
    bands = []
    for center in centers:
        lower = center / (2 ** (1 / 2))
        upper = min(center * (2 ** (1 / 2)), fs / 2)
        if lower < upper:
            bands.append((lower, upper))
        if upper >= fs / 2:
            break
    return bands


def _band_cross_spectra(left, right, fs, bands):
    """대역별 (좌 파워, 우 파워, 크로스 스펙트럼 합)을 산출한다.

    FFT는 전체 신호에 대해 한 번만 계산하고 대역 루프에서는 빈 선택만
    수행한다.
    """
    left = np.asarray(left, dtype=np.float64)
    right = np.asarray(right, dtype=np.float64)
    fft_len = next_fast_len(max(len(left), len(right)))
    fft_l = fft(left, n=fft_len)
    fft_r = fft(right, n=fft_len)
    freqs = np.fft.fftfreq(fft_len, d=1 / fs)
    cross = fft_l * np.conj(fft_r)

    results = []
    for f_low, f_high in bands:
        f_high = min(f_high, fs / 2)
        if f_low >= f_high:
            results.append((np.nan, np.nan, complex(np.nan)))
            continue
        band_idx = np.where((freqs >= f_low) & (freqs < f_high))[0]
        if not len(band_idx):
            results.append((np.nan, np.nan, complex(np.nan)))
            continue
        power_l = np.sum(np.abs(fft_l[band_idx]) ** 2)
        power_r = np.sum(np.abs(fft_r[band_idx]) ** 2)
        results.append((power_l, power_r, np.sum(cross[band_idx])))
    return results


def band_interaural_level_difference(left, right, fs, bands):
    """대역별 ILD(dB, 좌/우 파워비)를 계산한다."""
    ilds = []
    for power_l, power_r, _cross in _band_cross_spectra(left, right, fs, bands):
        if np.isnan(power_l):
            ilds.append(np.nan)
            continue
        ilds.append(10 * np.log10((power_l + 1e-12) / (power_r + 1e-12)))
    return ilds


def band_interaural_phase_difference(left, right, fs, bands):
    """대역별 IPD(도, 좌 - 우)를 계산한다.

    대역 내 크로스 스펙트럼 합 ``sum(L(f) * conj(R(f)))``의 위상각을
    사용한다. 이는 에너지 가중 원형 평균 위상차로, 순수 지연 Δt에 대해
    대역 중심 주파수의 위상차 ``2*pi*f_mid*Δt``(wrap)를 준다. (개별
    스펙트럼을 합한 뒤 위상을 취하면 빈 간 위상 회전으로 상쇄가 일어나
    무의미한 값이 나온다.)
    """
    ipds = []
    for _power_l, _power_r, cross in _band_cross_spectra(left, right, fs, bands):
        if np.isnan(cross.real):
            ipds.append(np.nan)
            continue
        ipds.append(float(np.degrees(np.angle(cross))))
    return ipds


def energy_decay_curve_db(data, floor_db=-80.0):
    """Schroeder 역적분 에너지 감쇠 곡선(EDC)을 dB로 계산한다.

    EDC(t) = 10*log10( integral_t^inf p^2 / integral_0^inf p^2 ) 이므로
    0dB에서 시작해 단조감소한다. 에너지가 0인 신호는 floor_db로 채운다.
    """
    data = np.asarray(data, dtype=np.float64)
    energy = np.cumsum((data**2)[::-1])[::-1]
    if not len(energy) or np.max(energy) <= 1e-12:
        return np.full(len(energy), floor_db)
    return 10 * np.log10(energy / (np.max(energy) + 1e-12) + 1e-12)


def interaural_cross_correlation(left, right, fs, max_delay_ms=1.0):
    """정규화된 IACF와 IACC를 계산한다 (ISO 3382-1).

    IACF(tau) = sum(l(t) * r(t+tau)) / sqrt(sum(l^2) * sum(r^2)) 이므로
    값은 항상 [-1, 1] 범위이고, IACC는 |tau| <= max_delay_ms 창에서의
    max |IACF| 이다.

    Args:
        left: 좌측 귀 IR (1-D array)
        right: 우측 귀 IR (1-D array)
        fs: 샘플레이트 (Hz)
        max_delay_ms: IACC 탐색 창 (기본 ±1ms)

    Returns:
        (lags_ms, iacf, iacc, tau_ms) 튜플.
        lags_ms/iacf는 탐색 창 내부의 지연축(ms)과 정규화 상관 값,
        iacc는 max |IACF|, tau_ms는 그 위치의 지연(ms).
    """
    left = np.asarray(left, dtype=np.float64)
    right = np.asarray(right, dtype=np.float64)
    energy = np.sum(left**2) * np.sum(right**2)
    if energy <= 0:
        return np.array([]), np.array([]), np.nan, np.nan

    correlation = signal.correlate(left, right, mode="full") / np.sqrt(energy)
    lags = signal.correlation_lags(len(left), len(right), mode="full")

    max_delay_samples = round(max_delay_ms * fs / 1000)
    mask = np.abs(lags) <= max_delay_samples
    lags_ms = lags[mask] * 1000 / fs
    iacf = correlation[mask]
    if not len(iacf):
        return np.array([]), np.array([]), np.nan, np.nan

    peak = int(np.argmax(np.abs(iacf)))
    return lags_ms, iacf, float(np.abs(iacf[peak])), float(lags_ms[peak])
