# -*- coding: utf-8 -*-
"""scipy.stats shim — statistical functions using NumPy."""

import numpy as np
from collections import namedtuple

LinregressResult = namedtuple('LinregressResult',
                              ['slope', 'intercept', 'rvalue', 'pvalue', 'stderr'])


def linregress(x, y=None):
    """Calculate a linear least-squares regression for two sets of measurements.

    Returns (slope, intercept, r_value, p_value, std_err).
    Matches scipy.stats.linregress signature and return type.
    """
    if y is None:
        # x is a 2D array with shape (2, N) or (N, 2)
        x = np.asarray(x)
        if x.shape[0] == 2:
            x, y = x[0], x[1]
        elif x.shape[1] == 2:
            x, y = x[:, 0], x[:, 1]
        else:
            raise ValueError("With a single argument, x must be 2D with shape (2,N) or (N,2)")
    else:
        x = np.asarray(x, dtype=np.float64)
        y = np.asarray(y, dtype=np.float64)

    n = len(x)
    if n < 2:
        raise ValueError("Need at least 2 data points")

    x_mean = np.mean(x)
    y_mean = np.mean(y)

    # Sums of squares
    ssxm = np.sum((x - x_mean) ** 2)
    ssym = np.sum((y - y_mean) ** 2)
    ssxym = np.sum((x - x_mean) * (y - y_mean))

    if ssxm == 0:
        slope = 0.0
        intercept = y_mean
        r_value = 0.0
        p_value = 1.0
        stderr = 0.0
    else:
        slope = ssxym / ssxm
        intercept = y_mean - slope * x_mean

        # Correlation coefficient
        if ssym == 0:
            r_value = 0.0
        else:
            r_value = ssxym / np.sqrt(ssxm * ssym)
            r_value = np.clip(r_value, -1.0, 1.0)

        # p-value (two-sided test)
        # Using t-distribution approximation
        if abs(r_value) == 1.0:
            p_value = 0.0
        elif n > 2:
            t_stat = r_value * np.sqrt((n - 2) / (1 - r_value ** 2))
            # Approximate p-value using normal distribution for large n
            p_value = 2.0 * (1.0 - _norm_cdf(abs(t_stat)))
        else:
            p_value = 1.0

        # Standard error of the slope
        if n > 2:
            residuals = y - (slope * x + intercept)
            s2 = np.sum(residuals ** 2) / (n - 2)
            stderr = np.sqrt(s2 / ssxm)
        else:
            stderr = 0.0

    return LinregressResult(slope=slope, intercept=intercept,
                            rvalue=r_value, pvalue=p_value, stderr=stderr)


def _norm_cdf(x):
    """Approximate standard normal CDF using error function approximation."""
    # Abramowitz and Stegun approximation
    return 0.5 * (1.0 + _erf(x / np.sqrt(2.0)))


def _erf(x):
    """Approximate error function."""
    # Horner form of the Abramowitz and Stegun approximation (max error ~1.5e-7)
    sign = np.sign(x)
    x = abs(x)
    t = 1.0 / (1.0 + 0.3275911 * x)
    poly = t * (0.254829592 + t * (-0.284496736 + t * (1.421413741 +
           t * (-1.453152027 + t * 1.061405429))))
    return sign * (1.0 - poly * np.exp(-x * x))
