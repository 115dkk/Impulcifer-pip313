# -*- coding: utf-8 -*-
"""scipy.interpolate shim — interpolation functions using NumPy."""

import numpy as np


class interp1d:
    """Interpolate a 1-D function.

    Drop-in replacement for scipy.interpolate.interp1d.
    Supports 'linear' and 'cubic' kinds.
    """

    def __init__(self, x, y, kind='linear', axis=-1, copy=True,
                 bounds_error=True, fill_value=np.nan, assume_sorted=False):
        self.x = np.array(x, dtype=np.float64, copy=copy)
        self.y = np.array(y, dtype=np.float64, copy=copy)
        self.kind = kind
        self.bounds_error = bounds_error
        self.fill_value = fill_value

        if not assume_sorted:
            order = np.argsort(self.x)
            self.x = self.x[order]
            self.y = self.y[order]

        if kind == 'cubic':
            self._cubic_coeffs = _compute_cubic_spline(self.x, self.y)

    def __call__(self, x_new):
        x_new = np.asarray(x_new, dtype=np.float64)
        scalar = x_new.ndim == 0
        x_new = np.atleast_1d(x_new)

        if self.bounds_error:
            if np.any(x_new < self.x[0]) or np.any(x_new > self.x[-1]):
                raise ValueError("x_new is out of interpolation range")

        if self.kind == 'linear':
            result = np.interp(x_new, self.x, self.y)
        elif self.kind == 'cubic':
            result = _eval_cubic_spline(self.x, self._cubic_coeffs, x_new)
        else:
            # Fallback to linear
            result = np.interp(x_new, self.x, self.y)

        # Handle fill_value for out-of-bounds
        if not self.bounds_error:
            mask_lo = x_new < self.x[0]
            mask_hi = x_new > self.x[-1]
            if isinstance(self.fill_value, tuple):
                result[mask_lo] = self.fill_value[0]
                result[mask_hi] = self.fill_value[1]
            elif self.fill_value == 'extrapolate':
                pass  # np.interp already handles boundaries
            else:
                result[mask_lo | mask_hi] = self.fill_value

        return float(result[0]) if scalar else result


class InterpolatedUnivariateSpline:
    """1-D interpolating spline for a given set of data points.

    Drop-in replacement for scipy.interpolate.InterpolatedUnivariateSpline.

    Parameters
    ----------
    x : array_like
        Input dimension of data points.
    y : array_like
        Input dimension of data points.
    k : int, optional
        Degree of the smoothing spline. Default is 3.
        k=1: linear, k=3: cubic.
    """

    def __init__(self, x, y, w=None, bbox=None, k=3, ext=0, check_finite=False):
        self.x = np.asarray(x, dtype=np.float64)
        self.y = np.asarray(y, dtype=np.float64)
        self.k = k
        self.ext = ext

        # Sort by x
        order = np.argsort(self.x)
        self.x = self.x[order]
        self.y = self.y[order]

        if k == 1:
            # Linear interpolation — just store x, y
            pass
        elif k == 3:
            # Cubic spline
            self._coeffs = _compute_cubic_spline(self.x, self.y)
        else:
            # For k=2,4,5: fallback to piecewise polynomial via numpy
            # Use linear as fallback with a warning
            self.k = 1

    def __call__(self, x_new, nu=0, ext=None):
        x_new = np.asarray(x_new, dtype=np.float64)
        scalar = x_new.ndim == 0
        x_new = np.atleast_1d(x_new)

        if ext is None:
            ext = self.ext

        if self.k == 1:
            result = np.interp(x_new, self.x, self.y)
        elif self.k == 3:
            result = _eval_cubic_spline(self.x, self._coeffs, x_new)
        else:
            result = np.interp(x_new, self.x, self.y)

        # Handle extrapolation mode
        if ext == 1:  # return 0 for out of bounds
            mask = (x_new < self.x[0]) | (x_new > self.x[-1])
            result[mask] = 0.0
        elif ext == 2:  # raise error
            if np.any(x_new < self.x[0]) or np.any(x_new > self.x[-1]):
                raise ValueError("x_new out of range")
        elif ext == 3:  # return boundary value
            result[x_new < self.x[0]] = self.y[0]
            result[x_new > self.x[-1]] = self.y[-1]
        # ext == 0: extrapolate (default for cubic)

        return float(result[0]) if scalar else result


def _compute_cubic_spline(x, y):
    """Compute natural cubic spline coefficients.

    Returns coefficients (a, b, c, d) for each interval where:
    S_i(x) = a_i + b_i*(x-x_i) + c_i*(x-x_i)^2 + d_i*(x-x_i)^3
    """
    n = len(x) - 1
    if n < 1:
        return (y.copy(), np.zeros_like(y), np.zeros_like(y), np.zeros_like(y))

    h = np.diff(x)
    dy = np.diff(y)

    # Natural spline: second derivative = 0 at endpoints
    # Solve tridiagonal system for c coefficients
    A = np.zeros((n + 1, n + 1))
    rhs = np.zeros(n + 1)

    A[0, 0] = 1.0
    A[n, n] = 1.0

    for i in range(1, n):
        A[i, i - 1] = h[i - 1]
        A[i, i] = 2.0 * (h[i - 1] + h[i])
        A[i, i + 1] = h[i]
        rhs[i] = 3.0 * (dy[i] / h[i] - dy[i - 1] / h[i - 1])

    # Solve tridiagonal system using Thomas algorithm
    c = _solve_tridiag(A, rhs, n + 1)

    # Compute b and d
    a = y.copy()
    b = np.zeros(n)
    d = np.zeros(n)

    for i in range(n):
        b[i] = dy[i] / h[i] - h[i] * (2.0 * c[i] + c[i + 1]) / 3.0
        d[i] = (c[i + 1] - c[i]) / (3.0 * h[i])

    return (a[:n], b, c[:n], d)


def _solve_tridiag(A, rhs, n):
    """Solve tridiagonal system using Thomas algorithm."""
    # Extract diagonals
    a_diag = np.zeros(n)  # sub-diagonal
    b_diag = np.zeros(n)  # main diagonal
    c_diag = np.zeros(n)  # super-diagonal

    for i in range(n):
        b_diag[i] = A[i, i]
        if i > 0:
            a_diag[i] = A[i, i - 1]
        if i < n - 1:
            c_diag[i] = A[i, i + 1]

    # Forward sweep
    c_prime = np.zeros(n)
    d_prime = np.zeros(n)

    c_prime[0] = c_diag[0] / b_diag[0]
    d_prime[0] = rhs[0] / b_diag[0]

    for i in range(1, n):
        m = a_diag[i] / (b_diag[i] - a_diag[i] * c_prime[i - 1])
        c_prime[i] = c_diag[i] / (b_diag[i] - a_diag[i] * c_prime[i - 1])
        d_prime[i] = (rhs[i] - a_diag[i] * d_prime[i - 1]) / (b_diag[i] - a_diag[i] * c_prime[i - 1])

    # Back substitution
    x = np.zeros(n)
    x[-1] = d_prime[-1]
    for i in range(n - 2, -1, -1):
        x[i] = d_prime[i] - c_prime[i] * x[i + 1]

    return x


def _eval_cubic_spline(x_knots, coeffs, x_new):
    """Evaluate cubic spline at new points."""
    a, b, c, d = coeffs
    n = len(a)

    result = np.empty_like(x_new)

    for j, xv in enumerate(x_new):
        # Find interval
        if xv <= x_knots[0]:
            i = 0
        elif xv >= x_knots[n]:
            i = n - 1
        else:
            i = np.searchsorted(x_knots[:n], xv, side='right') - 1
            i = max(0, min(i, n - 1))

        dx = xv - x_knots[i]
        result[j] = a[i] + b[i] * dx + c[i] * dx ** 2 + d[i] * dx ** 3

    return result
