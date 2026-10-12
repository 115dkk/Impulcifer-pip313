#!/usr/bin/env python3
"""Replay a PEQ fit from ROOM_CORRECTION.md, without loading the oracle.

Run with NPY_DISABLE_CPU_FEATURES="X86_V3 X86_V4 AVX512_ICL AVX512_SPR"
and OPENBLAS_NUM_THREADS=1. --out writes the first outer iteration (including
rejected trials); --all-iterations retains the entire diagnostic trace.
"""

import argparse
import functools
import hashlib
import inspect
import json
import sys
from pathlib import Path

import numpy as np
from scipy.optimize import least_squares
from scipy.optimize._lsq import trf


def array(value, root):
    if isinstance(value, dict):
        name = value["file"]
        if Path(name).name != name:
            raise ValueError("sidecar must be adjacent to the fixture")
        raw = (root / name).read_bytes()
        assert len(raw) == 8 * value["length"]
        assert hashlib.sha256(raw).hexdigest() == value["sha256"]
        return np.frombuffer(raw, dtype="<f8")
    return np.asarray(value, dtype=float)


def plain(value):
    if isinstance(value, np.ndarray):
        return plain(value.tolist())
    if isinstance(value, np.generic):
        return plain(value.item())
    if isinstance(value, float) and not np.isfinite(value):
        return str(value)
    if isinstance(value, dict):
        return {k: plain(v) for k, v in value.items()}
    if isinstance(value, (tuple, list)):
        return [plain(v) for v in value]
    return value


def replay(path):
    fixture = json.loads(path.read_text())
    inputs, expected = fixture["inputs"], fixture["outputs"]
    grid = array(inputs["g"], path.parent)
    curve = array(inputs["corr_db"], path.parent)
    kw = inputs["kwargs"]
    weights = np.where(
        (grid >= kw.get("lo", 20)) & (grid <= kw.get("hi", 500)), 1.0, 0.15
    )
    z = np.exp(-2j * np.pi * grid / 48000.0)

    def residual(x):
        product = np.ones(grid.size, dtype=complex)
        for lf, lq, gain in x.reshape(-1, 3):
            frequency, q = np.exp(lf), np.exp(lq)
            amplitude = 10.0 ** (gain / 40.0)
            omega = 2.0 * np.pi * frequency / 48000.0
            alpha = np.sin(omega) / (2.0 * q)
            b = (1.0 + alpha * amplitude, -2.0 * np.cos(omega), 1.0 - alpha * amplitude)
            a = (1.0 + alpha / amplitude, -2.0 * np.cos(omega), 1.0 - alpha / amplitude)
            product *= (b[0] + b[1] * z + b[2] * z * z) / (
                a[0] + a[1] * z + a[2] * z * z
            )
        return weights * (20.0 * np.log10(np.abs(product) + 1e-12) - curve)

    trials, main_x, main_cost = [], [], []
    originals = {}
    current = {}
    select_names = (
        "p_value",
        "r_value",
        "ag_value",
        "p_stride",
        "r_stride",
        "r_stride_l",
        "r_stride_u",
        "ag_stride",
        "to_tr",
        "to_bound",
    )

    def wrap(name):
        original = getattr(trf, name)
        originals[name] = original

        @functools.wraps(original)
        def call(*args, **kwargs):
            nonlocal current
            caller = inspect.currentframe().f_back
            state = caller.f_locals
            if name == "solve_lsq_trust_region":
                current = {
                    k: plain(state[k])
                    for k in (
                        "iteration",
                        "g_norm",
                        "v",
                        "dv",
                        "Delta",
                        "s",
                        "uf",
                        "alpha",
                    )
                }
                current["alpha_in"] = current.pop("alpha")
                current["helpers"] = []
                trials.append(current)
            captured = {}
            old_profile = sys.getprofile()
            if name in ("select_step", "solve_lsq_trust_region"):

                def profile(frame, event, arg):
                    if event == "return" and frame.f_code is original.__code__:
                        names = (
                            select_names if name == "select_step" else ("full_rank",)
                        )
                        captured.update(
                            {
                                k: plain(frame.f_locals[k])
                                for k in names
                                if k in frame.f_locals
                            }
                        )

                sys.setprofile(profile)
            try:
                result = original(*args, **kwargs)
            finally:
                if name in ("select_step", "solve_lsq_trust_region"):
                    sys.setprofile(old_profile)
            if name == "solve_lsq_trust_region":
                current.update(captured)
                current.update(
                    p_h=plain(result[0]), alpha_out=plain(result[1]), n_iter=result[2]
                )
            elif name == "select_step":
                if "p_stride" not in captured:
                    branch = "p"
                else:
                    pv, rv, av = (
                        float(captured[k]) for k in ("p_value", "r_value", "ag_value")
                    )
                    branch = (
                        "p"
                        if pv < rv and pv < av
                        else "reflected"
                        if rv < pv and rv < av
                        else "anti-gradient"
                    )
                current.update(
                    select={"branch": branch, **captured},
                    step=plain(result[0]),
                    step_h=plain(result[1]),
                    predicted_reduction=plain(result[2]),
                )
            elif name == "make_strictly_feasible" and current:
                current["x_new"] = plain(result)
            elif name == "update_tr_radius":
                current.update(
                    actual_reduction=plain(args[1]),
                    Delta_new=plain(result[0]),
                    ratio=plain(result[1]),
                )
            if current and name not in ("svd", "select_step", "solve_lsq_trust_region"):
                # Small helper results retain inner-step decisions without copying J.
                current["helpers"].append({"name": name, "result": plain(result)})
            return result

        setattr(trf, name, call)

    for name in (
        "svd",
        "solve_lsq_trust_region",
        "make_strictly_feasible",
        "step_size_to_bound",
        "intersect_trust_region",
        "build_quadratic_1d",
        "minimize_quadratic_1d",
        "CL_scaling_vector",
        "evaluate_quadratic",
        "update_tr_radius",
        "check_termination",
        "select_step",
    ):
        wrap(name)
    original_bounds = trf.trf_bounds
    initial = {}

    def bounds(fun, jac, x0, f0, j0, *args, **kwargs):
        initial.update(cost=float(0.5 * np.dot(f0, f0)), j0=j0)

        def main(x):
            f = fun(x)
            main_x.append(x.copy())
            main_cost.append(float(0.5 * np.dot(f, f)))
            return f

        return original_bounds(main, jac, x0, f0, j0, *args, **kwargs)

    trf.trf_bounds = bounds
    try:
        result = least_squares(
            residual,
            array(expected["x0"], path.parent),
            bounds=(
                array(expected["lb"], path.parent),
                array(expected["ub"], path.parent),
            ),
            method="trf",
            jac="2-point",
            x_scale=1.0,
            loss="linear",
            ftol=1e-8,
            xtol=1e-8,
            gtol=1e-8,
            tr_solver="exact",
            max_nfev=expected["max_nfev"],
        )
    finally:
        trf.trf_bounds = original_bounds
        for name, original in originals.items():
            setattr(trf, name, original)
    reference = array(expected["main_x"], path.parent).reshape(-1, expected["params"])
    distances = np.max(
        np.abs(np.array(main_x[: len(reference)]) - reference[: len(main_x)]), axis=1
    )
    distances /= np.max(np.abs(reference[: len(main_x)]), axis=1)
    first = np.flatnonzero(distances > 1e-9)
    horizon = expected.get("horizon")
    gated = len(reference) if horizon is None else max(0, horizon - 1)
    agreement = {
        "initial_cost": initial["cost"],
        "initial_cost_exact": initial["cost"] == expected["f0_cost"],
        "main_cost_0": main_cost[0],
        "reference_main_cost_0": expected["main_cost"][0],
        "main_cost_0_exact": main_cost[0] == expected["main_cost"][0],
        "first_divergence": int(first[0]) if first.size else None,
        "horizon": expected.get("horizon"),
        "prefix_max_relative": float(max(distances[:gated], default=0.0)),
        "prefix_count_ok": len(main_x) >= gated,
        "cost": result.cost,
        "reference_cost": expected["result"]["cost"],
    }
    if expected["J0"] is not None:
        j0 = array(expected["J0"], path.parent).reshape(initial["j0"].shape)
        different = np.argwhere(initial["j0"] != j0)
        agreement["j0_exact"] = not different.size
        if different.size:
            row, col = different[0]
            agreement["j0_first_difference"] = [
                int(row),
                int(col),
                initial["j0"][row, col],
                j0[row, col],
            ]
    print("REPLAY", path.name, json.dumps(agreement))
    if first.size:
        k = int(first[0])
        col = int(np.argmax(np.abs(main_x[k] - reference[k])))
        print(
            f"FIRST main_x[{k}][{col}]: replay={main_x[k][col]:.17g} fixture={reference[k, col]:.17g} relative={distances[k]:.17g}"
        )
    return {"fixture": path.name, "agreement": agreement, "trials": trials}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fixture", type=Path)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--all-iterations", action="store_true")
    args = parser.parse_args()
    output = replay(args.fixture)
    if not args.all_iterations:
        output["trials"] = [
            trial for trial in output["trials"] if trial["iteration"] == 0
        ]
    if args.out:
        args.out.write_text(json.dumps(output, indent=2, allow_nan=False) + "\n")
    agreement = output["agreement"]
    return (
        0
        if agreement["main_cost_0_exact"]
        and agreement["prefix_count_ok"]
        and agreement["prefix_max_relative"] <= 1e-9
        else 1
    )


if __name__ == "__main__":
    raise SystemExit(main())
