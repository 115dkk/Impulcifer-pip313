#!/usr/bin/env python3
"""SciPy-only exact bounded TRF references (no SECS code or execution)."""
import json
from pathlib import Path

import numpy as np
import scipy
from scipy.optimize import least_squares


def solve(name, x0, lower, upper, residual, **data):
    result = least_squares(
        residual, x0, bounds=(lower, upper), method="trf", jac="2-point",
        x_scale=1, loss="linear", ftol=1e-8, xtol=1e-8, gtol=1e-8,
        tr_solver="exact", max_nfev=150,
    )
    return dict(name=name, x0=list(x0), lower=list(lower), upper=list(upper),
                x=result.x.tolist(), cost=result.cost, nfev=result.nfev,
                status=result.status, **data)


def peq_response(grid, x):
    z = np.exp(-1j * 2 * np.pi * grid / 48000)
    p = np.ones(len(grid), complex)
    for lf, lq, gain in np.reshape(x, (-1, 3)):
        f, q = np.exp(lf), np.exp(lq)
        a = 10 ** (gain / 40)
        w = 2 * np.pi * f / 48000
        alpha = np.sin(w) / (2 * q)
        b = [1 + alpha*a, -2*np.cos(w), 1 - alpha*a]
        d = [1 + alpha/a, -2*np.cos(w), 1 - alpha/a]
        p *= (b[0] + b[1]*z + b[2]*z*z) / (d[0] + d[1]*z + d[2]*z*z)
    return 20*np.log10(np.abs(p) + 1e-12)


def main():
    t = np.linspace(0, 3, 41)
    y = 2.3*np.exp(-1.4*t) + 0.35 + 0.01*np.sin(7*t)
    cases = [solve("smooth", [1., 0.5, 0.1], [0., 0., -1.], [4., 4., 2.],
                   lambda x: x[0]*np.exp(-x[1]*t) + x[2] - y,
                   t=t.tolist(), y=y.tolist())]
    cases.append(solve("active", [0., 0.], [-1., -1.], [1., 1.],
                       lambda x: np.array([x[0]-3, 2*(x[1]+2), x[0]+x[1]])))
    grid = np.geomspace(15, 20000, 700)
    # An independent, deliberately overparameterized, irregular synthetic curve.
    curve = (7*np.sin(13*np.log(grid/15)) + 3*np.cos(23*np.log(grid/15)))
    curve *= np.clip((np.log2(grid)-np.log2(20/np.sqrt(2)))/.5, 0, 1)
    curve *= np.clip((np.log2(500*np.sqrt(2))-np.log2(grid))/.5, 0, 1)
    x0, lower, upper = [], [], []
    for i, f in enumerate(np.geomspace(22, 470, 16)):
        gain = 4. if i % 2 == 0 else -6.
        gl, gu = (0., 7.) if gain > 0 else (-25., -3.)
        qmax = max(min(20., np.pi*(f/1.2)*.030/10**(gu/40)), .31)
        low = [max(np.log(15), np.log(f/1.2)), np.log(.3), gl]
        high = [min(np.log(500), np.log(f*1.2)), np.log(qmax), gu]
        start = np.clip([np.log(f), np.log(2.), gain], np.array(low)+1e-6,
                        np.array(high)-1e-6)
        x0.extend(start); lower.extend(low); upper.extend(high)
    weight = np.where((grid >= 20) & (grid <= 500), 1., .15)
    cases.append(solve("peq16", x0, lower, upper,
                       lambda x: weight*(peq_response(grid, x)-curve),
                       grid=grid.tolist(), curve=curve.tolist()))
    assert cases[-1]["nfev"] == 150, cases[-1]["nfev"]
    output = Path(__file__).parent / "goldens/trf_scipy_reference.json"
    output.write_text(json.dumps(dict(scipy=scipy.__version__, cases=cases), indent=2) + "\n")
    for case in cases:
        print(case["name"], "nfev", case["nfev"], "cost", case["cost"], "status", case["status"])


if __name__ == "__main__":
    main()
