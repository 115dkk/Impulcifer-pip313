# -*- coding: utf-8 -*_

import os
import csv
import matplotlib.pyplot as plt
import matplotlib.ticker as ticker
import math
from io import StringIO
from scipy.interpolate import InterpolatedUnivariateSpline
from scipy.optimize import least_squares
from scipy.signal import savgol_filter, find_peaks, minimum_phase, firwin2
from scipy.special import expit
from scipy.stats import linregress
from scipy.fftpack import next_fast_len
import numpy as np
import urllib
from tabulate import tabulate
from PIL import Image
import re
import warnings
from autoeq import biquad
from autoeq.constants import DEFAULT_F_MIN, DEFAULT_F_MAX, DEFAULT_STEP, DEFAULT_MAX_GAIN, DEFAULT_TREBLE_F_LOWER, \
    DEFAULT_TREBLE_F_UPPER, DEFAULT_TREBLE_MAX_GAIN, DEFAULT_TREBLE_GAIN_K, DEFAULT_SMOOTHING_WINDOW_SIZE, \
    DEFAULT_SMOOTHING_ITERATIONS, DEFAULT_TREBLE_SMOOTHING_F_LOWER, DEFAULT_TREBLE_SMOOTHING_F_UPPER, \
    DEFAULT_TREBLE_SMOOTHING_WINDOW_SIZE, DEFAULT_TREBLE_SMOOTHING_ITERATIONS, DEFAULT_TILT, DEFAULT_FS, \
    DEFAULT_F_RES, DEFAULT_BASS_BOOST_GAIN, DEFAULT_BASS_BOOST_FC, \
    DEFAULT_BASS_BOOST_Q, DEFAULT_GRAPHIC_EQ_STEP, HARMAN_INEAR_PREFENCE_FREQUENCIES, \
    HARMAN_ONEAR_PREFERENCE_FREQUENCIES

try:
    ADAPTIVE_PALETTE = Image.Palette.ADAPTIVE
except AttributeError:
    ADAPTIVE_PALETTE = getattr(Image, 'ADAPTIVE')


class FrequencyResponse:
    def __init__(self,
                 name=None,
                 frequency=None,
                 raw=None,
                 error=None,
                 smoothed=None,
                 error_smoothed=None,
                 equalization=None,
                 parametric_eq=None,
                 fixed_band_eq=None,
                 equalized_raw=None,
                 equalized_smoothed=None,
                 target=None):
        if not name:
            raise TypeError('Name must not be a non-empty string.')
        self.name = name.strip()

        self.frequency = self._init_data(frequency)
        if not len(self.frequency):
            self.frequency = self.generate_frequencies()

        self.raw = self._init_data(raw)
        self.smoothed = self._init_data(smoothed)
        self.error = self._init_data(error)
        self.error_smoothed = self._init_data(error_smoothed)
        self.equalization = self._init_data(equalization)
        self.parametric_eq = self._init_data(parametric_eq)
        self.fixed_band_eq = self._init_data(fixed_band_eq)
        self.equalized_raw = self._init_data(equalized_raw)
        self.equalized_smoothed = self._init_data(equalized_smoothed)
        self.target = self._init_data(target)
        self._sort()

    def copy(self, name=None):
        return FrequencyResponse(
            name=self.name + '_copy' if name is None else name,
            frequency=self._init_data(self.frequency),
            raw=self._init_data(self.raw),
            error=self._init_data(self.error),
            smoothed=self._init_data(self.smoothed),
            error_smoothed=self._init_data(self.error_smoothed),
            equalization=self._init_data(self.equalization),
            parametric_eq=self._init_data(self.parametric_eq),
            fixed_band_eq=self._init_data(self.fixed_band_eq),
            equalized_raw=self._init_data(self.equalized_raw),
            equalized_smoothed=self._init_data(self.equalized_smoothed),
            target=self._init_data(self.target)
        )

    def _init_data(self, data):
        """Initializes data to a clean format. If None is passed and empty array is created. Non-numbers are removed."""
        if data is None:
            # None means empty array
            data = []
        elif isinstance(data, (float, int)) and not isinstance(data, bool):
            # Scalar means all values are that, same shape as frequency
            data = np.ones(self.frequency.shape) * data
        # Replace nans with Nones
        data = [None if x is None or math.isnan(x) else x for x in data]
        # Wrap in Numpy array
        data = np.array(data)
        return data

    def _sort(self):
        sorted_inds = self.frequency.argsort()
        self.frequency = self.frequency[sorted_inds]
        for i in range(1, len(self.frequency)):
            if self.frequency[i] == self.frequency[i - 1]:
                raise ValueError('Duplicate values found at frequency {}. Remove duplicates manually.'.format(
                    self.frequency[i])
                )
        if len(self.raw):
            self.raw = self.raw[sorted_inds]
        if len(self.error):
            self.error = self.error[sorted_inds]
        if len(self.smoothed):
            self.smoothed = self.smoothed[sorted_inds]
        if len(self.error_smoothed):
            self.error_smoothed = self.error_smoothed[sorted_inds]
        if len(self.equalization):
            self.equalization = self.equalization[sorted_inds]
        if len(self.parametric_eq):
            self.parametric_eq = self.parametric_eq[sorted_inds]
        if len(self.fixed_band_eq):
            self.fixed_band_eq = self.fixed_band_eq[sorted_inds]
        if len(self.equalized_raw):
            self.equalized_raw = self.equalized_raw[sorted_inds]
        if len(self.equalized_smoothed):
            self.equalized_smoothed = self.equalized_smoothed[sorted_inds]
        if len(self.target):
            self.target = self.target[sorted_inds]

    def reset(self,
              raw=False,
              smoothed=True,
              error=True,
              error_smoothed=True,
              equalization=True,
              fixed_band_eq=True,
              parametric_eq=True,
              equalized_raw=True,
              equalized_smoothed=True,
              target=True):
        """Resets data."""
        if raw:
            self.raw = self._init_data(None)
        if smoothed:
            self.smoothed = self._init_data(None)
        if error:
            self.error = self._init_data(None)
        if error_smoothed:
            self.error_smoothed = self._init_data(None)
        if equalization:
            self.equalization = self._init_data(None)
        if parametric_eq:
            self.parametric_eq = self._init_data(None)
        if fixed_band_eq:
            self.fixed_band_eq = self._init_data(None)
        if equalized_raw:
            self.equalized_raw = self._init_data(None)
        if equalized_smoothed:
            self.equalized_smoothed = self._init_data(None)
        if target:
            self.target = self._init_data(None)

    @classmethod
    def read_from_csv(cls, file_path):
        """Reads data from CSV file and constructs class instance."""
        name = '.'.join(os.path.split(file_path)[1].split('.')[:-1])

        # Read file
        with open(file_path, 'r', encoding='utf-8') as f:
            s = f.read()

        # Regex for AutoEq style CSV
        header_pattern = r'frequency(,(raw|smoothed|error|error_smoothed|equalization|parametric_eq|fixed_band_eq|equalized_raw|equalized_smoothed|target))+'
        float_pattern = r'-?\d+\.?\d+'
        data_2_pattern = r'{fl}[ ,;:\t]+{fl}?'.format(fl=float_pattern)
        data_n_pattern = r'{fl}([ ,;:\t]+{fl})+?'.format(fl=float_pattern)
        autoeq_pattern = r'^{header}(\n{data})+\n*$'.format(header=header_pattern, data=data_n_pattern)

        if re.match(autoeq_pattern, s):
            # Known AutoEq CSV format
            rows = list(csv.DictReader(StringIO(s)))

            def column(name):
                if not rows or name not in rows[0]:
                    return None
                return [float(row[name]) for row in rows]

            frequency = column('frequency')
            raw = column('raw')
            smoothed = column('smoothed')
            error = column('error')
            error_smoothed = column('error_smoothed')
            equalization = column('equalization')
            parametric_eq = column('parametric_eq')
            fixed_band_eq = column('fixed_band_eq')
            equalized_raw = column('equalized_raw')
            equalized_smoothed = column('equalized_smoothed')
            target = column('target')
            return cls(
                name=name,
                frequency=frequency,
                raw=raw,
                smoothed=smoothed,
                error=error,
                error_smoothed=error_smoothed,
                equalization=equalization,
                parametric_eq=parametric_eq,
                fixed_band_eq=fixed_band_eq,
                equalized_raw=equalized_raw,
                equalized_smoothed=equalized_smoothed,
                target=target
            )
        else:
            # Unknown format, try to guess
            lines = s.split('\n')
            frequency = []
            raw = []
            for line in lines:
                if re.match(data_2_pattern, line):  # float separator float
                    floats = re.findall(float_pattern, line)
                    frequency.append(float(floats[0]))  # Assume first to be frequency
                    raw.append(float(floats[1]))  # Assume second to be raw
                # Discard all lines which don't match data pattern
            return cls(name=name, frequency=frequency, raw=raw)

    def to_dict(self):
        d = dict()
        if len(self.frequency):
            d['frequency'] = self.frequency.tolist()
        if len(self.raw):
            d['raw'] = [x if x is not None else 'NaN' for x in self.raw]
        if len(self.error):
            d['error'] = [x if x is not None else 'NaN' for x in self.error]
        if len(self.smoothed):
            d['smoothed'] = [x if x is not None else 'NaN' for x in self.smoothed]
        if len(self.error_smoothed):
            d['error_smoothed'] = [x if x is not None else 'NaN' for x in self.error_smoothed]
        if len(self.equalization):
            d['equalization'] = [x if x is not None else 'NaN' for x in self.equalization]
        if len(self.parametric_eq):
            d['parametric_eq'] = [x if x is not None else 'NaN' for x in self.parametric_eq]
        if len(self.fixed_band_eq):
            d['fixed_band_eq'] = [x if x is not None else 'NaN' for x in self.fixed_band_eq]
        if len(self.equalized_raw):
            d['equalized_raw'] = [x if x is not None else 'NaN' for x in self.equalized_raw]
        if len(self.equalized_smoothed):
            d['equalized_smoothed'] = [x if x is not None else 'NaN' for x in self.equalized_smoothed]
        if len(self.target):
            d['target'] = [x if x is not None else 'NaN' for x in self.target]
        return d

    def write_to_csv(self, file_path=None):
        """Writes data to files as CSV."""
        file_path = os.path.abspath(file_path)
        data = self.to_dict()
        fieldnames = list(data.keys())
        n_rows = max((len(values) for values in data.values()), default=0)

        def format_value(value):
            if isinstance(value, float):
                return f'{value:.2f}'
            return value

        with open(file_path, 'w', newline='', encoding='utf-8') as f:
            writer = csv.DictWriter(f, fieldnames=fieldnames)
            writer.writeheader()
            for i in range(n_rows):
                writer.writerow({
                    key: format_value(values[i]) if i < len(values) else ''
                    for key, values in data.items()
                })

    # Compatibility aliases used by autoeq-py313. The processing math above is
    # intentionally kept at AutoEQ 1.2.5 to match LionLion123/Impulcifer output.
    read_csv = read_from_csv
    write_csv = write_to_csv

    def eqapo_graphic_eq(self, normalize=True, f_step=DEFAULT_GRAPHIC_EQ_STEP):
        """Generates EqualizerAPO GraphicEQ string from equalization curve."""
        fr = FrequencyResponse(name='hack', frequency=self.frequency, raw=self.equalization)
        n = np.ceil(np.log(20000 / 20) / np.log(f_step))
        f = 20 * f_step ** np.arange(n)
        f = np.sort(np.unique(f.astype('int')))
        fr.interpolate(f=f)
        if normalize:
            fr.raw -= np.max(fr.raw) + 0.5
            if fr.raw[0] > 0.0:
                # Prevent bass boost below lowest frequency
                fr.raw[0] = 0.0

        # Remove leading zeros
        while np.abs(fr.raw[-1]) < 0.1 and np.abs(fr.raw[-2]) < 0.1:  # Last two are zeros
            fr.raw = fr.raw[:-1]

        s = '; '.join(['{f} {a:.1f}'.format(f=f, a=a) for f, a in zip(fr.frequency, fr.raw)])
        s = 'GraphicEQ: ' + s
        return s

    def write_eqapo_graphic_eq(self, file_path, normalize=True):
        """Writes equalization graph to a file as Equalizer APO config."""
        file_path = os.path.abspath(file_path)
        s = self.eqapo_graphic_eq(normalize=normalize)
        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(s)
        return s

    @staticmethod
    def _empty_biquad_result(frequency):
        empty = np.array([])
        coeffs = np.empty((0, 3))
        return np.zeros(np.asarray(frequency).shape), 0.0, empty, empty, empty, coeffs, coeffs

    @staticmethod
    def _biquad_eq_response(frequency, fc, q, gain, fs=DEFAULT_FS):
        fc = np.atleast_1d(np.asarray(fc, dtype=float))
        q = np.atleast_1d(np.asarray(q, dtype=float))
        gain = np.atleast_1d(np.asarray(gain, dtype=float))

        if len(fc) == 0:
            return np.zeros(np.asarray(frequency).shape), np.empty((0, 3)), np.empty((0, 3))

        fc_column = np.expand_dims(fc, axis=1)
        q_column = np.expand_dims(np.abs(q), axis=1)
        gain_column = np.expand_dims(gain, axis=1)
        a0, a1, a2, b0, b1, b2 = biquad.peaking(fc_column, q_column, gain_column, fs=fs)
        frequency_rows = np.repeat(np.expand_dims(frequency, axis=0), len(fc_column), axis=0)
        eq = np.sum(biquad.digital_coeffs(frequency_rows, fs, a0, a1, a2, b0, b1, b2), axis=0)
        coeffs_a = np.hstack((np.tile(a0, a1.shape), a1, a2))
        coeffs_b = np.hstack((b0, b1, b2))
        return eq, coeffs_a, coeffs_b

    @staticmethod
    def _optimize_biquad_filters_scipy(
            frequency, target, max_time=5, max_filters=None, fs=DEFAULT_FS, fc=None, q=None):
        """Fits biquad peaking filters without TensorFlow v1."""
        frequency = np.asarray(frequency, dtype=float)
        target = np.asarray(target, dtype=float)

        if fc is not None or q is not None:
            if fc is None:
                raise TypeError('"fc" must be given if "q" is given.')
            if q is None:
                raise TypeError('"q" must be given if "fc" is given.')
            if max_filters is not None:
                raise TypeError('"max_filters" must not be given when "fc" and "q" are given.')
            fc = np.atleast_1d(np.asarray(fc, dtype=float))
            q = np.atleast_1d(np.asarray(q, dtype=float))
            if len(q) == 1 and len(fc) > 1:
                q = np.repeat(q[0], len(fc))
            if len(fc) != len(q):
                raise ValueError('"fc" and "q" must have the same length.')

        parametric = fc is None
        fr_target = FrequencyResponse(name='Filter Initialization', frequency=frequency, raw=target)
        fr_target.smoothen_fractional_octave(window_size=1 / 7, iterations=1000)
        interpolator = InterpolatedUnivariateSpline(np.log10(frequency), fr_target.smoothed, k=1)

        if parametric:
            fr_target_pos = np.clip(fr_target.smoothed, a_min=0.0, a_max=None)
            peak_inds = find_peaks(fr_target_pos)[0]
            fr_target_neg = np.clip(-fr_target.smoothed, a_min=0.0, a_max=None)
            peak_inds = np.concatenate((peak_inds, find_peaks(fr_target_neg)[0]))
            peak_inds.sort()
            peak_inds = peak_inds[np.abs(fr_target.smoothed[peak_inds]) > 0.1]

            if len(peak_inds) == 0:
                return FrequencyResponse._empty_biquad_result(frequency)

            peak_fc = frequency[peak_inds].astype(float)
            if peak_fc[0] > 80:
                peak_fc = np.concatenate((np.array([20, 60], dtype=float), peak_fc))
            elif peak_fc[0] > 40:
                peak_fc = np.concatenate((np.array([20], dtype=float), peak_fc))
            peak_g = interpolator(np.log10(peak_fc)).astype(float)

            def remove_small_filters(min_gain):
                nonlocal peak_fc, peak_g
                keep = np.abs(peak_g) > min_gain
                peak_fc = peak_fc[keep]
                peak_g = peak_g[keep]

            def merge_filters():
                nonlocal peak_fc, peak_g
                pair_inds = []
                for j in range(len(peak_fc) - 1):
                    if np.sign(peak_g[j]) == np.sign(peak_g[j + 1]):
                        pair_inds.append(j)

                min_err = None
                min_err_ind = None
                for pair_ind in pair_inds:
                    f_0 = peak_fc[pair_ind]
                    g_0 = peak_g[pair_ind]
                    i_0 = np.argmin(np.abs(frequency - f_0))
                    f_1 = peak_fc[pair_ind + 1]
                    i_1 = np.argmin(np.abs(frequency - f_1))
                    g_1 = peak_g[pair_ind + 1]
                    interp = InterpolatedUnivariateSpline(np.log10([f_0, f_1]), [g_0, g_1], k=1)
                    line = interp(np.log10(frequency[i_0:i_1 + 1]))
                    err = line - fr_target.smoothed[i_0:i_1 + 1]
                    err = np.sqrt(np.mean(np.square(err)))
                    if min_err is None or err < min_err:
                        min_err = err
                        min_err_ind = pair_ind

                if min_err is None:
                    return False

                if min_err < 0.3:
                    c = peak_fc[min_err_ind] * np.sqrt(peak_fc[min_err_ind + 1] / peak_fc[min_err_ind])
                    c = frequency[np.argmin(np.abs(frequency - c))]
                    g = np.mean([peak_g[min_err_ind], peak_g[min_err_ind + 1]])
                    peak_fc = np.delete(peak_fc, [min_err_ind, min_err_ind + 1])
                    peak_g = np.delete(peak_g, [min_err_ind, min_err_ind + 1])
                    peak_fc = np.insert(peak_fc, min_err_ind, c)
                    peak_g = np.insert(peak_g, min_err_ind, g)
                    return True
                return False

            remove_small_filters(0.1)
            if len(peak_fc) == 0:
                return FrequencyResponse._empty_biquad_result(frequency)

            if max_filters is not None:
                if len(peak_fc) > max_filters:
                    remove_small_filters(0.2)
                if len(peak_fc) > max_filters:
                    remove_small_filters(0.33)
                while len(peak_fc) > max_filters and merge_filters():
                    pass
                if len(peak_fc) > max_filters:
                    sorted_inds = np.flip(np.argsort(np.abs(peak_g)))[:max_filters]
                    peak_fc = peak_fc[sorted_inds]
                    peak_g = peak_g[sorted_inds]

            sorted_inds = np.argsort(peak_fc)
            initial_fc = peak_fc[sorted_inds]
            initial_gain = peak_g[sorted_inds]
            initial_q = np.ones(len(initial_fc), dtype=float)
        else:
            initial_fc = fc
            initial_q = q
            initial_gain = interpolator(np.log10(initial_fc)).astype(float)

        if len(initial_fc) == 0:
            return FrequencyResponse._empty_biquad_result(frequency)

        if parametric:
            n_filters = len(initial_fc)
            x0 = np.concatenate((np.log10(initial_fc), np.log(initial_q), initial_gain))
            lower = np.concatenate((
                np.full(n_filters, np.log10(max(10.0, frequency[0]))),
                np.full(n_filters, np.log(0.1)),
                np.full(n_filters, -60.0),
            ))
            upper = np.concatenate((
                np.full(n_filters, np.log10(fs / 2)),
                np.full(n_filters, np.log(20.0)),
                np.full(n_filters, 60.0),
            ))

            def unpack(x):
                return 10 ** x[:n_filters], np.exp(x[n_filters:2 * n_filters]), x[2 * n_filters:]
        else:
            x0 = initial_gain
            lower = np.full(len(initial_fc), -60.0)
            upper = np.full(len(initial_fc), 60.0)

            def unpack(x):
                return initial_fc, initial_q, x

        def residual(x):
            _fc, _q, _gain = unpack(x)
            eq, _, _ = FrequencyResponse._biquad_eq_response(frequency, _fc, _q, _gain, fs=fs)
            return eq - target

        max_nfev = max(20, int(max_time * 120)) if max_time is not None else None
        result = least_squares(residual, x0, bounds=(lower, upper), max_nfev=max_nfev)
        _fc, _Q, _gain = unpack(result.x)
        rmse = np.sqrt(np.mean(np.square(residual(result.x))))

        _Q = np.abs(_Q)

        if parametric:
            sl = np.logical_and(np.abs(_gain) > 0.1, _fc > 10)
            _fc = _fc[sl]
            _Q = _Q[sl]
            _gain = _gain[sl]

        if len(_fc) == 0:
            return FrequencyResponse._empty_biquad_result(frequency)

        sorted_inds = np.argsort(_fc)
        _fc = _fc[sorted_inds]
        _Q = _Q[sorted_inds]
        _gain = _gain[sorted_inds]

        _eq, coeffs_a, coeffs_b = FrequencyResponse._biquad_eq_response(frequency, _fc, _Q, _gain, fs=fs)
        return _eq, rmse, _fc, _Q, _gain, coeffs_a, coeffs_b

    @staticmethod
    def optimize_biquad_filters(frequency, target, max_time=5, max_filters=None, fs=DEFAULT_FS, fc=None, q=None):
        return FrequencyResponse._optimize_biquad_filters_scipy(
            frequency=frequency,
            target=target,
            max_time=max_time,
            max_filters=max_filters,
            fs=fs,
            fc=fc,
            q=q,
        )

    def optimize_parametric_eq(self, max_filters=None, fs=DEFAULT_FS):
        """Fits multiple biquad filters to equalization curve. If max_filters is a list with more than one element, one
        optimization run will be ran for each element. Each optimization run will continue from the previous. Each
        optimization run results must be combined with results of all the previous runs but can be used independently of
        the preceeding runs' results. If max_filters is [5, 5, 5] the first 5, 10 and 15 filters can be used
        independently.

        Args:
            max_filters: List of maximum number of filters available for each filter group optimization.
            fs: Sampling frequency

        Returns:
            - **filters:** Numpy array of filters where each row contains one filter fc, Q and gain
            - **n_produced:** Actual number of filters produced for each filter group. Calling with [5, 5] max_filters
                              might actually produce [4, 5] filters meaning that first 4 filters can be used
                              independently.
            - **max_gains:** Maximum gain value of the equalizer frequency response after each filter group
                             optimization. When using sub-set of filters independently the actual max gain of that
                             sub-set's frequency response must be applied as a negative digital preamp to avoid
                             clipping.
        """
        if not len(self.equalization):
            raise ValueError('Equalization has not been done yet.')

        if not isinstance(max_filters, list):
            max_filters = [max_filters]

        self.parametric_eq = np.zeros(self.frequency.shape)
        fc = Q = gain = np.array([])
        coeffs_a = coeffs_b = np.empty((0, 3))
        n_produced = []
        max_gains = []
        for n in max_filters:
            _eq, rmse, _fc, _Q, _gain, _coeffs_a, _coeffs_b = self.optimize_biquad_filters(
                frequency=self.frequency,
                target=self.equalization - self.parametric_eq,
                max_filters=n,
                fs=fs
            )
            n_produced.append(len(_fc))
            # print('RMSE: {:.2f}dB'.format(rmse))
            self.parametric_eq += _eq
            max_gains.append(np.max(self.parametric_eq))
            fc = np.concatenate((fc, _fc))
            Q = np.concatenate((Q, _Q))
            gain = np.concatenate((gain, _gain))
            coeffs_a = np.vstack((coeffs_a, _coeffs_a))
            coeffs_b = np.vstack((coeffs_b, _coeffs_b))

        filters = np.transpose(np.vstack([fc, Q, gain]))
        return filters, n_produced, max_gains

    def optimize_fixed_band_eq(self, fc=None, q=None, fs=DEFAULT_FS):
        """Fits multiple fixed Fc and Q biquad filters to equalization curve.

        Args:
            fc: List of center frequencies for the filters
            q: List of Q values for the filters
            fs: Sampling frequency

        Returns:
            - **filters:** Numpy array of filters where each row contains one filter fc, Q and gain
            - **n_produced:** Number of filters. Equals to length or inputs.
            - **max_gains:** Maximum gain value of the equalizer frequency response.
        """
        eq, rmse, fc, Q, gain, coeffs_a, coeffs_b = self.optimize_biquad_filters(
            frequency=self.frequency,
            target=self.equalization,
            fc=fc,
            q=q,
            fs=fs
        )
        self.fixed_band_eq = eq
        filters = np.transpose(np.vstack([fc, Q, gain]))
        return filters, len(fc), np.max(self.fixed_band_eq)

    @staticmethod
    def write_eqapo_parametric_eq(file_path, filters):
        """Writes EqualizerAPO Parameteric eq settings to a file."""
        file_path = os.path.abspath(file_path)

        with open(file_path, 'w', encoding='utf-8') as f:
            f.write('\n'.join(['Filter {i}: ON {type} Fc {fc:.0f} Hz Gain {gain:.1f} dB Q {Q:.2f}'.format(
                i=i + 1,
                type='PK',
                fc=filters[i, 0],
                Q=filters[i, 1],
                gain=filters[i, 2]
            ) for i in range(len(filters))]))

    @staticmethod
    def _split_path(path):
        """Splits file system path into components."""
        folders = []
        while 1:
            path, folder = os.path.split(path)

            if folder != "":
                folders.append(folder)
            else:
                if path != "":
                    folders.append(path)

                break

        folders.reverse()
        return folders

    def minimum_phase_impulse_response(self, fs=DEFAULT_FS, f_res=DEFAULT_F_RES, normalize=True):
        """Generates minimum phase impulse response

        Inspired by:
        https://sourceforge.net/p/equalizerapo/code/HEAD/tree/tags/1.2/filters/GraphicEQFilter.cpp#l45

        Args:
            fs: Sampling frequency in Hz
            f_res: Frequency resolution as sampling interval. 20 would result in sampling at 0 Hz, 20 Hz, 40 Hz, ...
            normalize: Normalize gain to -0.5 dB

        Returns:
            Minimum phase impulse response
        """
        # Double frequency resolution because it will be halved when converting linear phase IR to minimum phase
        f_res /= 2
        # Interpolate to even sample interval
        fr = FrequencyResponse(name='fr_data', frequency=self.frequency.copy(), raw=self.equalization.copy())
        # Save gain at lowest available frequency
        f_min = np.max([fr.frequency[0], f_res])
        interpolator = InterpolatedUnivariateSpline(np.log10(fr.frequency), fr.raw, k=1)
        gain_f_min = interpolator(np.log10(f_min))
        # Filter length, optimized for FFT speed
        n = round(fs // 2 / f_res)
        n = next_fast_len(n)
        f = np.linspace(0.0, fs // 2, n)
        # Run interpolation
        fr.interpolate(f, pol_order=1)
        # Set gain for all frequencies below original minimum frequency to match gain at the original minimum frequency
        fr.raw[fr.frequency <= f_min] = gain_f_min
        if normalize:
            # Reduce by max gain to avoid clipping with 1 dB of headroom
            fr.raw -= np.max(fr.raw)
            fr.raw -= 0.5
        # Minimum phase transformation by scipy's homomorphic method halves dB gain
        fr.raw *= 2
        # Convert amplitude to linear scale
        fr.raw = 10 ** (fr.raw / 20)
        # Zero gain at Nyquist frequency
        fr.raw[-1] = 0.0
        # Calculate response
        ir = firwin2(len(fr.frequency) * 2, fr.frequency, fr.raw, fs=fs)
        # Convert to minimum phase
        ir = minimum_phase(ir, n_fft=len(ir))
        return ir

    def linear_phase_impulse_response(self, fs=DEFAULT_FS, f_res=DEFAULT_F_RES, normalize=True):
        """Generates impulse response implementation of equalization filter."""
        # Interpolate to even sample interval
        fr = FrequencyResponse(name='fr_data', frequency=self.frequency, raw=self.equalization)
        # Save gain at lowest available frequency
        f_min = np.max([fr.frequency[0], f_res])
        interpolator = InterpolatedUnivariateSpline(np.log10(fr.frequency), fr.raw, k=1)
        gain_f_min = interpolator(np.log10(f_min))
        # Run interpolation
        fr.interpolate(np.arange(0.0, fs // 2, f_res), pol_order=1)
        # Set gain for all frequencies below original minimum frequency to match gain at the original minimum frequency
        fr.raw[fr.frequency <= f_min] = gain_f_min
        if normalize:
            # Reduce by max gain to avoid clipping with 1 dB of headroom
            fr.raw -= np.max(fr.raw)
            fr.raw -= 0.5
        # Convert amplitude to linear scale
        fr.raw = 10 ** (fr.raw / 20)
        # Calculate response
        fr.frequency = np.append(fr.frequency, fs // 2)
        fr.raw = np.append(fr.raw, 0.0)
        ir = firwin2(len(fr.frequency) * 2, fr.frequency, fr.raw, fs=fs)
        return ir

    def write_readme(self, file_path, max_filters=None, max_gains=None):
        """Writes README.md with picture and Equalizer APO settings."""
        file_path = os.path.abspath(file_path)
        dir_path = os.path.dirname(file_path)
        model = self.name

        # Write model
        s = '# {}\n'.format(model)
        s += 'See [usage instructions](https://github.com/jaakkopasanen/AutoEq#usage) for more options and ' \
             'info.\n'

        # Add parametric EQ settings
        parametric_eq_path = os.path.join(dir_path, model + ' ParametricEQ.txt')
        if os.path.isfile(parametric_eq_path) and self.parametric_eq is not None and len(self.parametric_eq):
            max_gains = [x + 0.5 for x in max_gains]

            # Read Parametric eq
            with open(parametric_eq_path, 'r', encoding='utf-8') as f:
                parametric_eq_str = f.read().strip()

            # Filters as Markdown table
            filters = []
            for line in parametric_eq_str.split('\n'):
                if line == '':
                    continue
                filter_type = line[line.index('ON') + 3:line.index('Fc') - 1]
                if filter_type == 'PK':
                    filter_type = 'Peaking'
                if filter_type == 'LS':
                    filter_type = 'Low Shelf'
                if filter_type == 'HS':
                    filter_type = 'High Shelf'
                fc = line[line.index('Fc') + 3:line.index('Gain') - 1]
                gain = line[line.index('Gain') + 5:line.index('Q') - 1]
                q = line[line.index('Q') + 2:]
                filters.append([filter_type, fc, q, gain])
            filters_table_str = tabulate(
                filters,
                headers=['Type', 'Fc', 'Q', 'Gain'],
                tablefmt='orgtbl'
            ).replace('+', '|').replace('|-', '|:')

            max_filters_str = ''
            if isinstance(max_filters, list) and len(max_filters) > 1:
                n = [0]
                for x in max_filters:
                    n.append(n[-1] + x)
                del n[0]
                if len(max_filters) > 3:
                    max_filters_str = ', '.join([str(x) for x in n[:-2]]) + ' or {}'.format(n[-2])
                if len(max_filters) == 3:
                    max_filters_str = '{n0} or {n1}'.format(n0=n[0], n1=n[1])
                if len(max_filters) == 2:
                    max_filters_str = str(n[0])
                max_filters_str = 'The first {} filters can be used independently.'.format(max_filters_str)

            preamp_str = ''
            if isinstance(max_gains, list) and len(max_gains) > 1:
                max_gains = [x + 0.1 for x in max_gains]
                if len(max_gains) > 3:
                    _s = 'When using independent subset of filters, apply preamp of {}, respectively.'
                    preamp_str = ', '.join(['-{:.1f}dB'.format(x) for x in max_gains[:-2]])
                    preamp_str += ' or -{:.1f}dB'.format(max_gains[-2])
                if len(max_gains) == 3:
                    _s = 'When using independent subset of filters, apply preamp of {}, respectively.'
                    preamp_str = '-{g0:.1f}dB or -{g1:.1f}dB'.format(g0=max_gains[0], g1=max_gains[1])
                if len(max_gains) == 2:
                    _s = 'When using independent subset of filters, apply preamp of **{}**.'
                    preamp_str = '-{:.1f}dB'.format(max_gains[0])
                preamp_str = _s.format(preamp_str)

            s += '''
            ### Parametric EQs
            In case of using parametric equalizer, apply preamp of **-{preamp:.1f}dB** and build filters manually
            with these parameters. {max_filters_str}
            {preamp_str}

            {filters_table}
            '''.format(
                preamp=max_gains[-1],
                max_filters_str=max_filters_str,
                preamp_str=preamp_str,
                filters_table=filters_table_str
            )

        # Add fixed band eq
        fixed_band_eq_path = os.path.join(dir_path, model + ' FixedBandEQ.txt')
        if os.path.isfile(fixed_band_eq_path) and self.fixed_band_eq is not None and len(self.fixed_band_eq):
            preamp = np.min([0.0, float(-np.max(self.fixed_band_eq))]) - 0.5

            # Read Parametric eq
            with open(fixed_band_eq_path, 'r', encoding='utf-8') as f:
                fixed_band_eq_str = f.read().strip()

            # Filters as Markdown table
            filters = []
            for line in fixed_band_eq_str.split('\n'):
                if line == '':
                    continue
                filter_type = line[line.index('ON') + 3:line.index('Fc') - 1]
                if filter_type == 'PK':
                    filter_type = 'Peaking'
                if filter_type == 'LS':
                    filter_type = 'Low Shelf'
                if filter_type == 'HS':
                    filter_type = 'High Shelf'
                fc = line[line.index('Fc') + 3:line.index('Gain') - 1]
                gain = line[line.index('Gain') + 5:line.index('Q') - 1]
                q = line[line.index('Q') + 2:]
                filters.append([filter_type, fc, q, gain])
            filters_table_str = tabulate(
                filters,
                headers=['Type', 'Fc', 'Q', 'Gain'],
                tablefmt='orgtbl'
            ).replace('+', '|').replace('|-', '|:')

            s += '''
            ### Fixed Band EQs
            In case of using fixed band (also called graphic) equalizer, apply preamp of **{preamp:.1f}dB**
            (if available) and set gains manually with these parameters.

            {filters_table}
            '''.format(
                preamp=preamp,
                filters_table=filters_table_str
            )

        # Write image link
        img_path = os.path.join(dir_path, model + '.png')
        if os.path.isfile(img_path):
            img_url = f'./{os.path.split(img_path)[1]}'
            img_url = urllib.parse.quote(img_url, safe="%/:=&?~#+!$,;'@()*[]")
            s += '''
            ### Graphs
            ![]({})
            '''.format(img_url)

        # Write file
        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(re.sub('\n[ \t]+', '\n', s).strip())

    @staticmethod
    def generate_frequencies(f_min=DEFAULT_F_MIN, f_max=DEFAULT_F_MAX, f_step=DEFAULT_STEP):
        freq = []
        f = f_min
        while f <= f_max:
            freq.append(f)
            f *= f_step
        return np.array(freq)

    def interpolate(self, f=None, f_step=DEFAULT_STEP, pol_order=1, f_min=DEFAULT_F_MIN, f_max=DEFAULT_F_MAX):
        """Interpolates missing values from previous and next value. Resets all but raw data."""
        # Drop NaN entries (originally Nones turned into NaNs by ``_init_data``).
        # ``self.raw`` is a float ndarray here, so ``is None`` would never match
        # and the original Python loop was dead code.
        if len(self.raw):
            raw_arr = np.asarray(self.raw, dtype=float)
            valid = ~np.isnan(raw_arr)
            if not valid.all():
                self.raw = raw_arr[valid]
                self.frequency = self.frequency[valid]

        # Interpolation functions
        keys = 'raw error error_smoothed equalization equalized_raw equalized_smoothed target'.split()
        interpolators = dict()
        log_f = np.log10(self.frequency)
        for key in keys:
            if len(self.__dict__[key]):
                interpolators[key] = InterpolatedUnivariateSpline(log_f, self.__dict__[key], k=pol_order)

        if f is None:
            self.frequency = self.generate_frequencies(f_min=f_min, f_max=f_max, f_step=f_step)
        else:
            self.frequency = np.array(f)

        # Prevent log10 from exploding by replacing zero frequency with small value
        zero_freq_fix = False
        if self.frequency[0] == 0:
            self.frequency[0] = 0.001
            zero_freq_fix = True

        # Run interpolators
        log_f = np.log10(self.frequency)
        for key in keys:
            if len(self.__dict__[key]) and key in interpolators:
                self.__dict__[key] = interpolators[key](log_f)

        if zero_freq_fix:
            # Restore zero frequency
            self.frequency[0] = 0

        # Everything but the interpolated data is affected by interpolating, reset them
        self.reset(**{key: False for key in keys})

    def center(self, frequency=1000):
        """Removed bias from frequency response.

        Args:
            frequency: Frequency which is set to 0 dB. If this is a list with two values then an average between the two
                       frequencies is set to 0 dB.

        Returns:
            Gain shifted
        """
        equal_energy_fr = FrequencyResponse(name='equal_energy', frequency=self.frequency.copy(), raw=self.raw.copy())
        equal_energy_fr.interpolate()
        interpolator = InterpolatedUnivariateSpline(np.log10(equal_energy_fr.frequency), equal_energy_fr.raw, k=1)
        if isinstance(frequency, (list, np.ndarray)) and len(frequency) > 1:
            # Use the average of the gain values between the given frequencies as the difference to be subtracted
            diff = np.mean(equal_energy_fr.raw[np.logical_and(
                equal_energy_fr.frequency >= frequency[0],
                equal_energy_fr.frequency <= frequency[1]
            )])
        else:
            if isinstance(frequency, (list, np.ndarray)):
                # List or array with only one element
                frequency = frequency[0]
            # Use the gain value at the given frequency as the difference to be subtracted
            diff = interpolator(np.log10(frequency))

        self.raw -= diff
        if len(self.smoothed):
            self.smoothed -= diff
        if len(self.error):
            self.error += diff
        if len(self.error_smoothed):
            self.error_smoothed += diff

        # Everything but raw, smoothed, errors and target is affected by centering, reset them
        self.reset(raw=False, smoothed=False, error=False, error_smoothed=False, target=False)

        return -diff

    def _tilt(self, tilt=DEFAULT_TILT):
        """Creates a tilt for equalization.

        Args:
            tilt: Slope steepness in dB/octave

        Returns:
            Tilted data
        """
        # Center in logarithmic scale
        c = DEFAULT_F_MIN * np.sqrt(DEFAULT_F_MAX / DEFAULT_F_MIN)
        # N octaves above center
        n_oct = np.log2(self.frequency / c)
        return n_oct * tilt

    def create_target(self,
                      bass_boost_gain=DEFAULT_BASS_BOOST_GAIN,
                      bass_boost_fc=DEFAULT_BASS_BOOST_FC,
                      bass_boost_q=DEFAULT_BASS_BOOST_Q,
                      tilt=None):
        """Creates target curve with bass boost as described by harman target response.

        Args:
            bass_boost_gain: Bass boost amount in dB
            bass_boost_fc: Bass boost low shelf center frequency
            bass_boost_q: Bass boost low shelf quality
            tilt: Frequency response tilt (slope) in dB per octave, positive values make it brighter

        Returns:
            Target for equalization
        """
        bass_boost = biquad.digital_coeffs(
            self.frequency,
            DEFAULT_FS,
            *biquad.low_shelf(bass_boost_fc, bass_boost_q, bass_boost_gain, DEFAULT_FS)
        )
        if tilt is not None:
            tilt = self._tilt(tilt=tilt)
        else:
            tilt = np.zeros(len(self.frequency))
        return bass_boost + tilt

    def compensate(self,
                   compensation,
                   bass_boost_gain=DEFAULT_BASS_BOOST_GAIN,
                   bass_boost_fc=DEFAULT_BASS_BOOST_FC,
                   bass_boost_q=DEFAULT_BASS_BOOST_Q,
                   tilt=None,
                   sound_signature=None,
                   min_mean_error=False):
        """Sets target and error curves."""
        # Copy and center compensation data
        compensation = FrequencyResponse(name='compensation', frequency=compensation.frequency, raw=compensation.raw)
        compensation.center()

        # Set target
        self.target = compensation.raw + self.create_target(
            bass_boost_gain=bass_boost_gain,
            bass_boost_fc=bass_boost_fc,
            bass_boost_q=bass_boost_q,
            tilt=tilt
        )
        if sound_signature is not None:
            # Sound signature give, add it to target curve
            if not np.all(sound_signature.frequency == self.frequency):
                # Interpolate sound signature to match self on the frequency axis
                sound_signature.interpolate(self.frequency)
            self.target += sound_signature.raw

        # Set error
        self.error = self.raw - self.target
        if min_mean_error:
            # Shift error by it's mean in range 100 Hz to 10 kHz
            delta = np.mean(self.error[np.logical_and(self.frequency >= 100, self.frequency <= 10000)])
            self.error -= delta
            self.target += delta

        # Smoothed error and equalization results are affected by compensation, reset them
        self.reset(
            raw=False,
            smoothed=False,
            error=False,
            error_smoothed=True,
            equalization=True,
            parametric_eq=True,
            fixed_band_eq=True,
            equalized_raw=True,
            equalized_smoothed=True,
            target=False
        )

    def _window_size(self, octaves):
        """Calculates moving average window size in indices from octaves."""
        # Octaves to coefficient
        k = 2 ** octaves
        # Calculate average step size in frequencies
        steps = []
        for i in range(1, len(self.frequency)):
            steps.append(self.frequency[i] / self.frequency[i - 1])
        step_size = sum(steps) / len(steps)
        # Calculate window size in indices
        # step_size^x = k  --> x = ...
        window_size = math.log(k) / math.log(step_size)
        # Half window size
        window_size = window_size
        # Round to integer to be usable as index
        window_size = round(window_size)
        if not window_size % 2:
            window_size += 1
        return window_size

    def _sigmoid(self, f_lower, f_upper, a_normal=0.0, a_treble=1.0):
        f_center = np.sqrt(f_upper / f_lower) * f_lower
        half_range = np.log10(f_upper) - np.log10(f_center)
        f_center = np.log10(f_center)
        a = expit((np.log10(self.frequency) - f_center) / (half_range / 4))
        a = a * -(a_normal - a_treble) + a_normal
        return a

    def _smoothen_fractional_octave(self,
                                    data,
                                    window_size=DEFAULT_SMOOTHING_WINDOW_SIZE,
                                    iterations=DEFAULT_SMOOTHING_ITERATIONS,
                                    treble_window_size=None,
                                    treble_iterations=None,
                                    treble_f_lower=DEFAULT_TREBLE_SMOOTHING_F_LOWER,
                                    treble_f_upper=DEFAULT_TREBLE_SMOOTHING_F_UPPER):
        """Smooths data.

        Args:
            window_size: Filter window size in octaves.
            iterations: Number of iterations to run the filter. Each new iteration is using output of previous one.
            treble_window_size: Filter window size for high frequencies.
            treble_iterations: Number of iterations for treble filter.
            treble_f_lower: Lower boundary of transition frequency region. In the transition region normal filter is \
                        switched to treble filter with sigmoid weighting function.
            treble_f_upper: Upper boundary of transition frequency reqion. In the transition region normal filter is \
                        switched to treble filter with sigmoid weighting function.
        """
        # Reject NaN-bearing arrays. After ``_init_data`` Nones are stored as
        # NaN floats, so ``np.isnan`` is the appropriate gate.
        if np.any(np.isnan(np.asarray(self.frequency, dtype=float))) \
                or np.any(np.isnan(np.asarray(data, dtype=float))):
            raise ValueError('NaN values present, cannot smoothen!')

        # Normal filter
        y_normal = data
        with warnings.catch_warnings():
            # Savgol filter uses array indexing which is not future proof, ignoring the warning and trusting that this
            # will be fixed in the future release
            warnings.simplefilter("ignore")
            for i in range(iterations):
                y_normal = savgol_filter(y_normal, self._window_size(window_size), 2)

            # Treble filter
            y_treble = data
            for _ in range(treble_iterations):
                y_treble = savgol_filter(y_treble, self._window_size(treble_window_size), 2)

        # Transition weighted with sigmoid
        k_treble = self._sigmoid(treble_f_lower, treble_f_upper)
        k_normal = k_treble * -1 + 1
        return y_normal * k_normal + y_treble * k_treble

    def smoothen_fractional_octave(self,
                                   window_size=DEFAULT_SMOOTHING_WINDOW_SIZE,
                                   iterations=DEFAULT_SMOOTHING_ITERATIONS,
                                   treble_window_size=DEFAULT_TREBLE_SMOOTHING_WINDOW_SIZE,
                                   treble_iterations=DEFAULT_TREBLE_SMOOTHING_ITERATIONS,
                                   treble_f_lower=DEFAULT_TREBLE_SMOOTHING_F_LOWER,
                                   treble_f_upper=DEFAULT_TREBLE_SMOOTHING_F_UPPER):
        """Smooths data.

        Args:
            window_size: Filter window size in octaves.
            iterations: Number of iterations to run the filter. Each new iteration is using output of previous one.
            treble_window_size: Filter window size for high frequencies.
            treble_iterations: Number of iterations for treble filter.
            treble_f_lower: Lower boundary of transition frequency region. In the transition region normal filter is \
                        switched to treble filter with sigmoid weighting function.
            treble_f_upper: Upper boundary of transition frequency reqion. In the transition region normal filter is \
                        switched to treble filter with sigmoid weighting function.
        """
        if treble_f_upper <= treble_f_lower:
            raise ValueError('Upper transition boundary must be greater than lower boundary')

        # Smoothen raw data
        self.smoothed = self._smoothen_fractional_octave(
            self.raw,
            window_size=window_size,
            iterations=iterations,
            treble_window_size=treble_window_size,
            treble_iterations=treble_iterations,
            treble_f_lower=treble_f_lower,
            treble_f_upper=treble_f_upper
        )

        if len(self.error):
            # Smoothen error data
            self.error_smoothed = self._smoothen_fractional_octave(
                self.error,
                window_size=window_size,
                iterations=iterations,
                treble_window_size=treble_window_size,
                treble_iterations=treble_iterations,
                treble_f_lower=treble_f_lower,
                treble_f_upper=treble_f_upper
            )

        # Equalization is affected by smoothing, reset equalization results
        self.reset(
            raw=False,
            smoothed=False,
            error=False,
            error_smoothed=False,
            equalization=True,
            parametric_eq=True,
            fixed_band_eq=True,
            equalized_raw=True,
            equalized_smoothed=True,
            target=False
        )

    def smoothen(self,
                 window_size=DEFAULT_SMOOTHING_WINDOW_SIZE,
                 treble_window_size=DEFAULT_TREBLE_SMOOTHING_WINDOW_SIZE,
                 treble_f_lower=DEFAULT_TREBLE_SMOOTHING_F_LOWER,
                 treble_f_upper=DEFAULT_TREBLE_SMOOTHING_F_UPPER):
        """Compatibility wrapper for autoeq-py313's smoothen() method name."""
        self.smoothen_fractional_octave(
            window_size=window_size,
            iterations=1,
            treble_window_size=treble_window_size,
            treble_iterations=1,
            treble_f_lower=treble_f_lower,
            treble_f_upper=treble_f_upper,
        )

    def smoothen_heavy_light(self):
        """Smoothens data by combining light and heavy smoothing and taking maximum.

        Returns:
            None
        """
        light = self.copy()
        light.name = 'Light'
        light.smoothen_fractional_octave(
            window_size=1 / 6,
            iterations=1,
            treble_f_lower=100,
            treble_f_upper=10000,
            treble_window_size=1 / 3,
            treble_iterations=1
        )

        heavy = self.copy()
        heavy.name = 'Heavy'
        heavy.smoothen_fractional_octave(
            window_size=1 / 3,
            iterations=1,
            treble_f_lower=1000,
            treble_f_upper=6000,
            treble_window_size=1.3,
            treble_iterations=1
        )

        combination = self.copy()
        combination.name = 'Combination'
        combination.error = np.max(np.vstack([light.error_smoothed, heavy.error_smoothed]), axis=0)
        combination.smoothen_fractional_octave(
            window_size=1 / 3,
            iterations=1,
            treble_f_lower=100,
            treble_f_upper=10000,
            treble_window_size=1 / 3,
            treble_iterations=1
        )

        self.smoothed = combination.smoothed.copy()
        self.error_smoothed = combination.error_smoothed.copy()

        # Equalization is affected by smoothing, reset equalization results
        self.reset(
            raw=False,
            smoothed=False,
            error=False,
            error_smoothed=False,
            equalization=True,
            parametric_eq=True,
            fixed_band_eq=True,
            equalized_raw=True,
            equalized_smoothed=True,
            target=False
        )

    def equalize(self,
                 max_gain=DEFAULT_MAX_GAIN,
                 smoothen=True,
                 treble_f_lower=DEFAULT_TREBLE_F_LOWER,
                 treble_f_upper=DEFAULT_TREBLE_F_UPPER,
                 treble_max_gain=DEFAULT_TREBLE_MAX_GAIN,
                 treble_gain_k=DEFAULT_TREBLE_GAIN_K):
        """Creates equalization curve and equalized curve.

        Args:
            max_gain: Maximum positive gain in dB
            smoothen: Smooth kinks caused by clipping gain to max gain?
            treble_f_lower: Lower frequency boundary for transition region between normal parameters and treble parameters
            treble_f_upper: Upper frequency boundary for transition reqion between normal parameters and treble parameters
            treble_max_gain: Maximum positive gain in dB in treble region
            treble_gain_k: Coefficient for treble gain, positive and negative. Useful for disbling or reducing \
                           equalization power in treble region. Defaults to 1.0 (not limited).
        """
        self.equalization = []
        self.equalized_raw = []

        if len(self.error_smoothed):
            error = self.error_smoothed
        elif len(self.error):
            error = self.error
        else:
            raise ValueError('Error data is missing. Call FrequencyResponse.compensate().')

        # ``equalization`` and ``equalized_raw`` were just reset to empty lists,
        # so only ``error`` (an ndarray of floats with NaN replacing None) needs
        # checking here. ``np.any(np.isnan(...))`` works whether ``error`` is a
        # list or an ndarray.
        if np.any(np.isnan(np.asarray(error, dtype=float))):
            raise ValueError('NaN values detected during equalization, interpolating data with default parameters.')

        # Invert with max gain clipping
        max_gain = self._sigmoid(treble_f_lower, treble_f_upper, a_normal=max_gain, a_treble=treble_max_gain)
        gain_k = self._sigmoid(treble_f_lower, treble_f_upper, a_normal=1.0, a_treble=treble_gain_k)
        gain = -error * gain_k
        clipped = gain > max_gain
        kink_inds = np.flatnonzero(np.concatenate(([clipped[0]], clipped[1:] != clipped[:-1])))
        if len(kink_inds) and kink_inds[0] == 0:
            kink_inds = kink_inds[1:]
        self.equalization = np.where(clipped, max_gain, gain)

        if smoothen:
            # Smooth out kinks
            window_size = self._window_size(1 / 12)
            doomed_inds = set()
            for i in kink_inds:
                start = i - min(i, (window_size - 1) // 2)
                end = i + 1 + min(len(self.equalization) - i - 1, (window_size - 1) // 2)
                doomed_inds.update(range(start, end))
            doomed_inds = sorted(doomed_inds)

            for i in range(1, 3):
                if len(self.frequency) - i in doomed_inds:
                    del doomed_inds[doomed_inds.index(len(self.frequency) - i)]

            keep = np.ones(len(self.frequency), dtype=bool)
            keep[doomed_inds] = False
            f = self.frequency[keep]
            e = self.equalization[keep]
            interpolator = InterpolatedUnivariateSpline(np.log10(f), e, k=2)
            self.equalization = interpolator(np.log10(self.frequency))

        # Equalized
        self.equalized_raw = self.raw + self.equalization
        if len(self.smoothed):
            self.equalized_smoothed = self.smoothed + self.equalization

    @staticmethod
    def kwarg_defaults(kwargs, **defaults):
        if kwargs is None:
            kwargs = {}
        else:
            kwargs = {key: val for key, val in kwargs.items()}
        for key, val in defaults.items():
            if key not in kwargs:
                kwargs[key] = val
        return kwargs

    def plot_graph(self,
                   fig=None,
                   ax=None,
                   show=True,
                   raw=True,
                   error=True,
                   smoothed=True,
                   error_smoothed=True,
                   equalization=True,
                   parametric_eq=True,
                   fixed_band_eq=True,
                   equalized=True,
                   target=True,
                   file_path=None,
                   f_min=DEFAULT_F_MIN,
                   f_max=DEFAULT_F_MAX,
                   a_min=None,
                   a_max=None,
                   color='black',
                   raw_plot_kwargs=None,
                   smoothed_plot_kwargs=None,
                   error_plot_kwargs=None,
                   error_smoothed_plot_kwargs=None,
                   equalization_plot_kwargs=None,
                   parametric_eq_plot_kwargs=None,
                   fixed_band_eq_plot_kwargs=None,
                   equalized_plot_kwargs=None,
                   target_plot_kwargs=None,
                   close=False):
        """Plots frequency response graph."""
        if fig is None:
            fig, ax = plt.subplots()
            fig.set_size_inches(12, 8)
        if not len(self.frequency):
            raise ValueError('\'frequency\' has no data!')

        if target and len(self.target):
            ax.plot(
                self.frequency, self.target,
                **self.kwarg_defaults(target_plot_kwargs, label='Target', linewidth=5, color='lightblue')
            )

        if smoothed and len(self.smoothed):
            ax.plot(
                self.frequency, self.smoothed,
                **self.kwarg_defaults(smoothed_plot_kwargs, label='Raw Smoothed', linewidth=5, color='lightgrey')
            )

        if error_smoothed and len(self.error_smoothed):
            ax.plot(
                self.frequency, self.error_smoothed,
                **self.kwarg_defaults(error_smoothed_plot_kwargs, label='Error Smoothed', linewidth=5, color='pink')
            )

        if raw and len(self.raw):
            ax.plot(
                self.frequency, self.raw,
                **self.kwarg_defaults(raw_plot_kwargs, label='Raw', linewidth=1, color=color)
            )

        if error and len(self.error):
            ax.plot(
                self.frequency, self.error,
                **self.kwarg_defaults(error_plot_kwargs, label='Error', linewidth=1, color='red')
            )

        if equalization and len(self.equalization):
            ax.plot(
                self.frequency, self.equalization,
                **self.kwarg_defaults(equalization_plot_kwargs, label='Equalization', linewidth=5, color='lightgreen')
            )

        if parametric_eq and len(self.parametric_eq):
            ax.plot(
                self.frequency, self.parametric_eq,
                **self.kwarg_defaults(parametric_eq_plot_kwargs, label='Parametric Eq', linewidth=1, color='darkgreen')
            )

        if fixed_band_eq and len(self.fixed_band_eq):
            ax.plot(
                self.frequency, self.fixed_band_eq,
                **self.kwarg_defaults(
                    fixed_band_eq_plot_kwargs,
                    label='Fixed Band Eq', linewidth=1, color='darkgreen', linestyle='--'
                )
            )

        if equalized and len(self.equalized_raw):
            ax.plot(
                self.frequency, self.equalized_raw,
                **self.kwarg_defaults(equalized_plot_kwargs, label='Equalized', linewidth=1, color='blue')
            )

        ax.set_xlabel('Frequency (Hz)')
        ax.semilogx()
        ax.set_xlim([f_min, f_max])
        ax.set_ylabel('Amplitude (dBr)')
        ax.set_ylim([a_min, a_max])
        ax.set_title(self.name)
        ax.legend(fontsize=8)
        ax.grid(True, which='major')
        ax.grid(True, which='minor')
        ax.xaxis.set_major_formatter(ticker.StrMethodFormatter('{x:.0f}'))
        if file_path is not None:
            file_path = os.path.abspath(file_path)
            fig.savefig(file_path, dpi=120)
            im = Image.open(file_path)
            im = im.convert('P', palette=ADAPTIVE_PALETTE, colors=60)
            im.save(file_path, optimize=True)
        if show:
            plt.show()
        elif close:
            plt.close(fig)
        return fig, ax

    def plot(self, *args, show_fig=True, **kwargs):
        """Compatibility wrapper for autoeq-py313's plot() method name."""
        if 'show' not in kwargs:
            kwargs['show'] = show_fig
        return self.plot_graph(*args, **kwargs)

    def harman_onear_preference_score(self):
        """Calculates Harman preference score for over-ear and on-ear headphones.

        Returns:
            - score: Preference score
            - std: Standard deviation of error
            - slope: Slope of linear regression of error
        """
        fr = self.copy()
        fr.interpolate(HARMAN_ONEAR_PREFERENCE_FREQUENCIES)
        sl = np.logical_and(fr.frequency >= 50, fr.frequency <= 10000)
        x = fr.frequency[sl]
        y = fr.error[sl]

        std = np.std(y, ddof=1)  # ddof=1 is required to get the exact same numbers as the Excel from Listen Inc gives
        slope, _, _, _, _ = linregress(np.log(x), y)
        score = 114.490443008238 - 12.62 * std - 15.5163857197367 * np.abs(slope)

        return score, std, slope

    def harman_inear_preference_score(self):
        """Calculates Harman preference score for in-ear headphones.

        Returns:
            - score: Preference score
            - std: Standard deviation of error
            - slope: Slope of linear regression of error
            - mean: Mean of absolute error
        """
        fr = self.copy()
        fr.interpolate(HARMAN_INEAR_PREFENCE_FREQUENCIES)
        sl = np.logical_and(fr.frequency >= 20, fr.frequency <= 10000)
        x = fr.frequency[sl]
        y = fr.error[sl]

        std = np.std(y, ddof=1)  # ddof=1 is required to get the exact same numbers as the Excel from Listen Inc gives
        slope, _, _, _, _ = linregress(np.log(x), y)
        # Mean of absolute of error centered by 500 Hz
        delta = fr.error[np.where(fr.frequency == 500.0)[0][0]]
        y = fr.error[np.logical_and(fr.frequency >= 40, fr.frequency <= 10000)] - delta
        mean = np.mean(np.abs(y))
        # Final score
        score = 100.0795 - 8.5 * std - 6.796 * np.abs(slope) - 3.475 * mean

        return score, std, slope, mean

    def process(self,
                compensation=None,
                min_mean_error=False,
                equalize=False,
                parametric_eq=False,
                fixed_band_eq=False,
                fc=None,
                q=None,
                ten_band_eq=None,
                max_filters=None,
                bass_boost_gain=None,
                bass_boost_fc=None,
                bass_boost_q=None,
                tilt=None,
                sound_signature=None,
                max_gain=DEFAULT_MAX_GAIN,
                treble_f_lower=DEFAULT_TREBLE_F_LOWER,
                treble_f_upper=DEFAULT_TREBLE_F_UPPER,
                treble_max_gain=DEFAULT_TREBLE_MAX_GAIN,
                treble_gain_k=DEFAULT_TREBLE_GAIN_K,
                fs=DEFAULT_FS):
        """Runs processing pipeline with interpolation, centering, compensation and equalization.

        Args:
            compensation: Compensation FrequencyResponse. Must be interpolated and centered.
            min_mean_error: Minimize mean error. Normally all curves cross at 1 kHz but this makes it possible to shift
                            error curve so that mean between 100 Hz and 10 kHz is at minimum. Target curve is shifted
                            accordingly. Useful for avoiding large bias caused by a narrow notch or peak at 1 kHz.
            equalize: Run equalization?
            parametric_eq: Optimize peaking filters for parametric eq?
            fixed_band_eq: Optimize peaking filters for fixed band (graphic) eq?
            fc: List of center frequencies for fixed band eq
            q: List of Q values for fixed band eq
            ten_band_eq: Optimize filters for standard ten band eq?
            max_filters: List of maximum number of peaking filters for each additive filter optimization run.
            bass_boost_gain: Bass boost amount in dB.
            bass_boost_fc: Bass boost low shelf center frequency.
            bass_boost_q: Bass boost low shelf quality.
            tilt: Target frequency response tilt in db / octave
            sound_signature: Sound signature as FrequencyResponse instance. Raw data will be used.
            max_gain: Maximum positive gain in dB
            treble_f_lower: Lower bound for treble transition region
            treble_f_upper: Upper boud for treble transition region
            treble_max_gain: Maximum gain in treble region
            treble_gain_k: Gain coefficient in treble region
            fs: Sampling frequency

        Returns:
            - **peq_filters:** Numpy array of produced parametric eq peaking filters. Each row contains Fc, Q and gain
            - **n_peq_filters:** Number of produced parametric eq peaking filters for each group.
            - **peq_max_gains:** Maximum positive gains in each parametric eq peaking filter group.
            - **fbeq_filters:** Numpy array of produced fixed band peaking filters. Each row contains Fc, Q and gain
            - **n_fbeq_filters:** Number of produced fixed band peaking filters.
            - **fbeq_max_gains:** Maximum positive gain for fixed band eq.
        """
        if parametric_eq and not equalize:
            raise ValueError('equalize must be True when parametric_eq is True.')

        if ten_band_eq:
            # Ten band eq is a shortcut for setting Fc and Q values to standard 10-band equalizer filters parameters
            fixed_band_eq = True
            fc = np.array([31.25, 62.5, 125, 250, 500, 1000, 2000, 4000, 8000, 16000], dtype='float32')
            q = np.ones(10, dtype='float32') * np.sqrt(2)

        if fixed_band_eq:
            if fc is None or q is None:
                raise ValueError('"fc" and "q" must be given when "fixed_band_eq" is given.')
            # Center frequencies are given but Q is a single value
            # Repeat Q to length of Fc
            if isinstance(q, (list, np.ndarray)):
                if len(q) == 1:
                    q = np.repeat(q[0], len(fc))
                elif len(q) != len(fc):
                    raise ValueError('q must have one elemet or the same number of elements as fc.')
            elif not isinstance(q, (list, np.ndarray)):
                q = np.repeat(q, len(fc))

        if fixed_band_eq and not equalize:
            raise ValueError('equalize must be True when fixed_band_eq or ten_band_eq is True.')

        if max_filters is not None and not isinstance(max_filters, list):
            max_filters = [max_filters]

        # Interpolate to standard frequency vector
        self.interpolate()

        # Center by 1kHz
        self.center()

        if compensation is not None:
            # Compensate
            self.compensate(
                compensation,
                bass_boost_gain=bass_boost_gain,
                bass_boost_fc=bass_boost_fc,
                bass_boost_q=bass_boost_q,
                tilt=tilt,
                sound_signature=sound_signature,
                min_mean_error=min_mean_error
            )

        # Smooth data
        self.smoothen_heavy_light()
        self.smoothen_fractional_octave(
            window_size=1 / 3,
            treble_window_size=1.4,
            treble_f_lower=6000,
            treble_f_upper=12000
        )

        peq_filters = n_peq_filters = peq_max_gains = fbeq_filters = n_fbeq_filters = nfbeq_max_gains = None
        # Equalize
        if equalize:
            self.equalize(
                max_gain=max_gain,
                smoothen=True,
                treble_f_lower=treble_f_lower,
                treble_f_upper=treble_f_upper,
                treble_max_gain=treble_max_gain,
                treble_gain_k=treble_gain_k
            )
            if parametric_eq:
                # Get the filters
                peq_filters, n_peq_filters, peq_max_gains = self.optimize_parametric_eq(max_filters=max_filters, fs=fs)
            if fixed_band_eq:
                fbeq_filters, n_fbeq_filters, nfbeq_max_gains = self.optimize_fixed_band_eq(fc=fc, q=q, fs=fs)

        return peq_filters, n_peq_filters, peq_max_gains, fbeq_filters, n_fbeq_filters, nfbeq_max_gains
