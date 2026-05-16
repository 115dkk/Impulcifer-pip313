# -*- coding: utf-8 -*-
"""Regression tests for the GUI scroll performance optimization.

The Impulcifer GUI's BRIR/recorder/settings/info tabs each host a
``CTkScrollableFrame`` containing ~50–100 widgets — in *both* the Stable
(``gui/tabs/``) and Studio (``gui/skins/``) skins. CustomTkinter installs
an unconditional ``<Configure>`` handler that calls
``canvas.bbox('all')`` and ``canvas.configure(scrollregion=...)`` on every
event. On Win32, Tk fires ``<Configure>`` for *position* changes too, so
every scroll step (each ``yview_moveto`` call) re-walks the canvas item
tree and rewrites the scrollregion — pushing the GPU to ~30% during scroll.

The fix in ``gui.utils.install_smooth_scrolling`` rebinds the handler to a
size-change-only variant and coalesces wheel-driven canvas movement to the
active monitor's refresh cadence. These tests pin three contracts:

  1. **Static (always-runnable):** every ``CTkScrollableFrame`` constructed
     inside a tab module — across both the Stable and Studio skins — is
     followed by a call to ``install_smooth_scrolling``. If a future tab
     (in either skin) forgets the call, the scroll regression returns
     silently — this static check catches it in CI without needing a
     display.

  2. **Functional (skipped without a display):** when
     ``install_smooth_scrolling`` is applied to a real
     ``CTkScrollableFrame`` and the frame is scrolled programmatically, the
     ``bbox`` and ``configure(scrollregion=...)`` calls during scroll are
     **zero**. Without the fix, they fire ~once per ``yview_moveto`` step.

  3. **Frame pacing (always-runnable):** wheel ``xview/yview("scroll", ...)``
     calls are accumulated and flushed at the detected display refresh
     interval, so bursty wheel input cannot ask Tk/DWM to repaint faster
     than the user's monitor.

The functional test creates a real Tk root, so it skips silently in
headless environments (CI without an X server). The static test runs
everywhere.
"""

from __future__ import annotations

import ast
import os
import sys
import unittest
from collections import Counter
from pathlib import Path
from typing import List, Set
from unittest import mock


PROJECT_ROOT = Path(__file__).resolve().parent.parent
# Both skins must keep every CTkScrollableFrame wired to the smooth
# scrolling fix. The Stable tabs (gui/tabs/) had this guard from the
# start; the Studio tabs (gui/skins/) were added later in the Pulse
# redesign and call install_smooth_scrolling at runtime too, but were
# never covered here — so a future Studio refactor could silently drop
# the call (re-introducing the ~30% GPU scroll spike) without any test
# catching it. Cover both skins so the guarantee is symmetric.
GUI_TABS = [
    PROJECT_ROOT / "gui" / "tabs" / "impulcifer_tab.py",
    PROJECT_ROOT / "gui" / "tabs" / "recorder_tab.py",
    PROJECT_ROOT / "gui" / "tabs" / "settings_tab.py",
    PROJECT_ROOT / "gui" / "tabs" / "info_tab.py",
    PROJECT_ROOT / "gui" / "skins" / "studio_impulcifer_tab.py",
    PROJECT_ROOT / "gui" / "skins" / "studio_recorder_tab.py",
    PROJECT_ROOT / "gui" / "skins" / "studio_settings_tab.py",
    PROJECT_ROOT / "gui" / "skins" / "studio_info_tab.py",
]


def _find_scrollable_frame_calls(tree: ast.AST) -> List[ast.Call]:
    """Return every ``ctk.CTkScrollableFrame(...)`` call in the AST."""
    calls: List[ast.Call] = []
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue
        func = node.func
        # Match `ctk.CTkScrollableFrame(...)` or `customtkinter.CTkScrollableFrame(...)`.
        if isinstance(func, ast.Attribute) and func.attr == "CTkScrollableFrame":
            calls.append(node)
        # Also match a bare `CTkScrollableFrame(...)` if someone imports it directly.
        elif isinstance(func, ast.Name) and func.id == "CTkScrollableFrame":
            calls.append(node)
    return calls


def _names_assigned_from_call(call: ast.Call, parent_module: ast.AST) -> Set[str]:
    """Find the variable names assigned from the result of ``call``.

    Walks the parent module to find an Assign / AugAssign whose value is the
    given ``call`` node, returning the target identifier(s). This lets us
    check whether ``install_smooth_scrolling(<that name>)`` is called later.
    """
    names: Set[str] = set()
    for node in ast.walk(parent_module):
        if isinstance(node, ast.Assign) and node.value is call:
            for target in node.targets:
                if isinstance(target, ast.Name):
                    names.add(target.id)
                elif isinstance(target, ast.Attribute):
                    names.add(target.attr)
    return names


def _has_install_smooth_scrolling_call(module: ast.AST, var_names: Set[str]) -> bool:
    """Return True if any of ``var_names`` is passed to install_smooth_scrolling."""
    for node in ast.walk(module):
        if not isinstance(node, ast.Call):
            continue
        func = node.func
        # `install_smooth_scrolling(name)` or `gui.utils.install_smooth_scrolling(name)`
        if isinstance(func, ast.Name) and func.id == "install_smooth_scrolling":
            pass
        elif isinstance(func, ast.Attribute) and func.attr == "install_smooth_scrolling":
            pass
        else:
            continue

        for arg in node.args:
            if isinstance(arg, ast.Name) and arg.id in var_names:
                return True
            if isinstance(arg, ast.Attribute) and arg.attr in var_names:
                return True
    return False


class StaticScrollFixTest(unittest.TestCase):
    """Each tab's CTkScrollableFrame must be wired to the smooth-scrolling fix."""

    def test_every_tab_calls_install_smooth_scrolling(self) -> None:
        offenders: List[str] = []
        for module_path in GUI_TABS:
            self.assertTrue(module_path.exists(),
                            f"Tab module not found: {module_path}")
            source = module_path.read_text(encoding="utf-8")
            tree = ast.parse(source, filename=str(module_path))

            calls = _find_scrollable_frame_calls(tree)
            self.assertGreaterEqual(
                len(calls), 1,
                f"{module_path.name} should contain at least one CTkScrollableFrame",
            )

            for call in calls:
                names = _names_assigned_from_call(call, tree)
                if not names:
                    offenders.append(
                        f"{module_path.name}:{call.lineno} CTkScrollableFrame is not assigned to a variable"
                    )
                    continue
                if not _has_install_smooth_scrolling_call(tree, names):
                    offenders.append(
                        f"{module_path.name}:{call.lineno} CTkScrollableFrame "
                        f"({', '.join(sorted(names))}) is missing install_smooth_scrolling()"
                    )

        self.assertEqual(
            offenders, [],
            "Every CTkScrollableFrame must be patched with install_smooth_scrolling. "
            "Without the patch, scrolling triggers per-step canvas.bbox('all') + "
            "canvas.configure(scrollregion=...) calls (~30% GPU spike). Offenders:\n  "
            + "\n  ".join(offenders),
        )


class ScrollFrameLimiterTest(unittest.TestCase):
    """Unit-test the wheel scroll coalescer without creating a Tk root."""

    class FakeCanvas:
        def __init__(self) -> None:
            self.calls = []
            self.after_ms = []
            self._callbacks = {}
            self._cancelled = set()
            self._next_after_id = 0

        def xview(self, *args):
            if not args:
                return (0.0, 0.5)
            self.calls.append(("x", args))
            return None

        def yview(self, *args):
            if not args:
                return (0.0, 0.5)
            self.calls.append(("y", args))
            return None

        def after(self, delay_ms, callback):
            self._next_after_id += 1
            after_id = f"after-{self._next_after_id}"
            self.after_ms.append(delay_ms)
            self._callbacks[after_id] = callback
            return after_id

        def after_cancel(self, after_id):
            self._cancelled.add(after_id)

        def run_pending(self):
            callbacks = list(self._callbacks.items())
            self._callbacks.clear()
            for after_id, callback in callbacks:
                if after_id not in self._cancelled:
                    callback()

    def test_wheel_scroll_is_coalesced_to_frame_cadence(self) -> None:
        from gui.utils import (
            _install_frame_limited_canvas_scroll,
            _refresh_rate_to_frame_interval_ms,
        )

        canvas = self.FakeCanvas()
        frame_interval_ms = _refresh_rate_to_frame_interval_ms(144)
        _install_frame_limited_canvas_scroll(canvas, frame_interval_ms=frame_interval_ms)

        self.assertLessEqual(
            frame_interval_ms,
            20,
            "Scroll frame limiter must keep visual cadence at or above 50 Hz.",
        )
        self.assertEqual(frame_interval_ms, 7)
        self.assertEqual(canvas.yview(), (0.0, 0.5))

        canvas.yview("scroll", 2, "units")
        canvas.yview("scroll", 3, "units")
        canvas.xview("scroll", -1, "units")

        self.assertEqual(canvas.calls, [])
        self.assertEqual(canvas.after_ms, [frame_interval_ms, frame_interval_ms])

        canvas.run_pending()

        self.assertEqual(
            canvas.calls,
            [
                ("y", ("scroll", 5, "units")),
                ("x", ("scroll", -1, "units")),
            ],
        )

    def test_scroll_interval_uses_detected_monitor_refresh_rate(self) -> None:
        from gui import utils

        with mock.patch.object(utils, "_get_display_refresh_rate_hz", return_value=75):
            self.assertEqual(utils._get_scroll_frame_interval_ms(), 14)

        with mock.patch.object(utils, "_get_display_refresh_rate_hz", return_value=240):
            self.assertEqual(utils._get_scroll_frame_interval_ms(), 5)

    def test_direct_view_command_cancels_pending_wheel_scroll(self) -> None:
        from gui.utils import _install_frame_limited_canvas_scroll

        canvas = self.FakeCanvas()
        _install_frame_limited_canvas_scroll(canvas)

        canvas.yview("scroll", 5, "units")
        canvas.yview("moveto", 0.25)
        canvas.run_pending()

        self.assertEqual(canvas.calls, [("y", ("moveto", 0.25))])


class FunctionalScrollFixTest(unittest.TestCase):
    """End-to-end: bbox + scrollregion are stable during scroll after the fix.

    These tests skip dynamically when no display is available — and crucially
    avoid spinning a probe ``tk.Tk()`` at module import time. CustomTkinter's
    Tcl interpreter has reuse quirks where a destroyed root can poison
    subsequent ``ctk.CTk()`` creation in the same process (manifests as
    ``tcl_findLibrary`` errors). Lazy detection inside ``setUp`` avoids that.
    """

    @classmethod
    def setUpClass(cls) -> None:
        # Project root must be on sys.path so ``gui.utils`` resolves regardless
        # of how the test runner is invoked.
        if str(PROJECT_ROOT) not in sys.path:
            sys.path.insert(0, str(PROJECT_ROOT))

    def setUp(self) -> None:
        """Skip if no display — done per-test to avoid module-level Tk probes."""
        if os.name != "nt" and not os.environ.get("DISPLAY"):
            self.skipTest("No display available for Tk functional test")

    def _build_scrollable(self, fix: bool):
        """Spawn a CTkScrollableFrame seeded with widgets and return (root, scroll, counter)."""
        import customtkinter as ctk

        ctk.set_appearance_mode("dark")
        root = ctk.CTk()
        root.geometry("700x500")

        scroll = ctk.CTkScrollableFrame(root)
        scroll.pack(fill="both", expand=True)
        scroll.grid_columnconfigure(0, weight=1)

        # Seed with enough widgets to force scrolling.
        for r in range(40):
            f = ctk.CTkFrame(scroll, corner_radius=0)
            f.grid(row=r, column=0, sticky="ew", padx=5, pady=5)
            ctk.CTkLabel(f, text=f"Row {r}").pack(side="left", padx=10, pady=10)
            ctk.CTkEntry(f).pack(side="left", padx=10, pady=10, expand=True, fill="x")
            ctk.CTkButton(f, text="OK").pack(side="left", padx=10, pady=10)

        if fix:
            from gui.utils import install_smooth_scrolling
            install_smooth_scrolling(scroll)

        # Let layout settle BEFORE instrumenting.
        root.update_idletasks()
        root.update()

        # Wrap canvas methods to count scroll-time work.
        canvas = scroll._parent_canvas  # type: ignore[attr-defined]
        counter: Counter = Counter()
        for method_name in ("bbox", "configure", "config"):
            original = getattr(canvas, method_name)
            counter_key = f"{method_name}_calls"

            def make_wrapper(m, orig, k):
                def wrapped(*args, **kwargs):
                    counter[k] += 1
                    return orig(*args, **kwargs)
                return wrapped

            setattr(canvas, method_name, make_wrapper(method_name, original, counter_key))

        return root, scroll, counter

    def _scroll_pass(self, scroll, steps: int = 20) -> None:
        canvas = scroll._parent_canvas  # type: ignore[attr-defined]
        for i in range(steps + 1):
            canvas.yview_moveto(i / steps)
            scroll.update_idletasks()
        for i in range(steps + 1):
            canvas.yview_moveto(1.0 - i / steps)
            scroll.update_idletasks()

    def test_baseline_makes_per_step_bbox_calls(self) -> None:
        """Sanity: without the fix, bbox/configure fire repeatedly during scroll."""
        root, scroll, counter = self._build_scrollable(fix=False)
        try:
            self._scroll_pass(scroll, steps=10)
        finally:
            root.destroy()

        # Without the fix, each yview_moveto generates one bbox + one
        # configure call. With 22 yview_moveto steps we expect at least
        # several of each. The exact count varies by platform / Tk version
        # but >= 10 is a stable lower bound.
        self.assertGreaterEqual(
            counter["bbox_calls"], 10,
            f"Without the fix we expect many bbox calls during scroll; "
            f"got {counter['bbox_calls']}. If this drops to zero, the "
            f"upstream CTkScrollableFrame behaviour changed and the "
            f"performance regression test below is no longer needed.",
        )

    def test_fix_eliminates_per_step_bbox_calls(self) -> None:
        """With install_smooth_scrolling, scroll-only events do zero work."""
        root, scroll, counter = self._build_scrollable(fix=True)
        try:
            self._scroll_pass(scroll, steps=20)
        finally:
            root.destroy()

        self.assertEqual(
            counter["bbox_calls"], 0,
            f"After install_smooth_scrolling the canvas bbox should not be "
            f"recomputed during scroll. Got {counter['bbox_calls']} calls — "
            f"the fix is broken or has been undone.",
        )
        self.assertEqual(
            counter["configure_calls"], 0,
            f"After install_smooth_scrolling the canvas configure should "
            f"not fire during scroll. Got {counter['configure_calls']} "
            f"calls — scrollregion churn has returned.",
        )


if __name__ == "__main__":
    unittest.main()
