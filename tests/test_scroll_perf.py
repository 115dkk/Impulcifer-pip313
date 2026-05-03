# -*- coding: utf-8 -*-
"""Regression tests for the GUI scroll performance optimization.

The Impulcifer GUI's BRIR/recorder/settings/info tabs each host a
``CTkScrollableFrame`` containing ~50–100 widgets. CustomTkinter installs
an unconditional ``<Configure>`` handler that calls
``canvas.bbox('all')`` and ``canvas.configure(scrollregion=...)`` on every
event. On Win32, Tk fires ``<Configure>`` for *position* changes too, so
every scroll step (each ``yview_moveto`` call) re-walks the canvas item
tree and rewrites the scrollregion — pushing the GPU to ~30% during scroll.

The fix in ``gui.utils.install_smooth_scrolling`` rebinds the handler to a
size-change-only variant. These tests pin two contracts:

  1. **Static (always-runnable):** every ``CTkScrollableFrame`` constructed
     inside a tab module is followed by a call to
     ``install_smooth_scrolling``. If a future tab forgets the call, the
     scroll regression returns silently — this static check catches it in
     CI without needing a display.

  2. **Functional (skipped without a display):** when
     ``install_smooth_scrolling`` is applied to a real
     ``CTkScrollableFrame`` and the frame is scrolled programmatically, the
     ``bbox`` and ``configure(scrollregion=...)`` calls during scroll are
     **zero**. Without the fix, they fire ~once per ``yview_moveto`` step.

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


PROJECT_ROOT = Path(__file__).resolve().parent.parent
GUI_TABS = [
    PROJECT_ROOT / "gui" / "tabs" / "impulcifer_tab.py",
    PROJECT_ROOT / "gui" / "tabs" / "recorder_tab.py",
    PROJECT_ROOT / "gui" / "tabs" / "settings_tab.py",
    PROJECT_ROOT / "gui" / "tabs" / "info_tab.py",
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
