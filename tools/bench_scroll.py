#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Scroll performance harness for the Modern GUI scrollable tabs.

Spawns a real ``CTkScrollableFrame`` tree that mirrors the Impulcifer tab's
widget census (CTkLabel/CTkEntry/CTkButton/CTkCheckBox/CTkOptionMenu, ~100
widgets), then drives the scrollbar programmatically — first up, then down,
many full passes — while counting:

  * how many ``<Configure>`` events fire on each CTk widget (a redraw flag),
  * how many ``yview`` commands are issued on the parent Canvas, and
  * elapsed wall time per scroll pass.

GPU usage itself is hard to measure portably, so we use the proxies above.
A good fix should:
  * keep ``yview`` call count proportional to wheel ticks, not pixels, and
  * keep ``<Configure>`` redraw count near zero (scrolling moves widgets,
    it doesn't resize them — so any non-zero redraw count is wasted work).

Usage:
    # baseline (current behavior)
    python tools/bench_scroll.py

    # with a candidate fix loaded from gui.utils.install_smooth_scrolling
    python tools/bench_scroll.py --fix

The harness needs a real display (no headless mode in CTk), so it's a
manual / opt-in benchmark — the regression test in tests/test_scroll_perf.py
covers the same code paths headlessly via Tk's ``-f`` virtual display where
available, and falls back to a pure-counter assertion otherwise.
"""

from __future__ import annotations

import argparse
import os
import sys
import time
from collections import Counter

# Ensure the project root is on sys.path so ``gui.utils`` resolves when this
# script is run from inside the ``tools/`` subdirectory.
_PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if _PROJECT_ROOT not in sys.path:
    sys.path.insert(0, _PROJECT_ROOT)

import customtkinter as ctk  # noqa: E402 — sys.path setup must precede this import
import tkinter as tk  # noqa: E402 — sys.path setup must precede this import


WIDGET_KINDS = {
    "Label": 62,
    "Frame": 46,
    "Entry": 22,
    "Button": 20,
    "CheckBox": 16,
    "OptionMenu": 9,
}


def _build_tab_layout(parent: ctk.CTkFrame, total_widgets: int = 175):
    """Populate ``parent`` with widgets in roughly the same proportions as the
    Impulcifer tab, so the bench mirrors the production widget census.
    """
    rows_per_section = 8
    section_count = max(1, total_widgets // (rows_per_section * 2))

    for section in range(section_count):
        section_frame = ctk.CTkFrame(parent, corner_radius=0)
        section_frame.grid(row=section, column=0, sticky="ew", padx=10, pady=10)
        section_frame.grid_columnconfigure(1, weight=1)

        ctk.CTkLabel(section_frame, text=f"Section {section}").grid(
            row=0, column=0, columnspan=3, sticky="w", padx=15, pady=10
        )

        for r in range(1, rows_per_section + 1):
            ctk.CTkLabel(section_frame, text=f"Field {r}").grid(
                row=r, column=0, sticky="w", padx=15, pady=5
            )
            ctk.CTkEntry(section_frame).grid(row=r, column=1, sticky="ew", padx=15, pady=5)
            if r % 3 == 0:
                ctk.CTkOptionMenu(
                    section_frame, values=["a", "b", "c"]
                ).grid(row=r, column=2, padx=15, pady=5)
            elif r % 3 == 1:
                ctk.CTkButton(section_frame, text="Browse").grid(
                    row=r, column=2, padx=15, pady=5
                )
            else:
                ctk.CTkCheckBox(section_frame, text="").grid(
                    row=r, column=2, padx=15, pady=5
                )


def _instrument_canvas(canvas: tk.Canvas, counter: Counter) -> None:
    """Wrap canvas scroll + bbox + configure methods to count call rates.

    The interesting counters are ``bbox_calls`` and ``configure_calls`` —
    each is potentially O(N) work over the canvas item tree, and each fires
    inside CTkScrollableFrame's default <Configure> handler. They're the
    smoking gun for scroll-time CPU/GPU spikes.
    """
    for method_name in ("yview", "yview_moveto", "yview_scroll", "xview",
                        "xview_moveto", "xview_scroll", "bbox", "configure",
                        "config", "itemconfigure", "itemconfig"):
        original = getattr(canvas, method_name)

        def make_wrapper(m, orig):
            def wrapped(*args, **kwargs):
                counter[f"{m}_calls"] += 1
                return orig(*args, **kwargs)
            return wrapped

        setattr(canvas, method_name, make_wrapper(method_name, original))


def _instrument_configure(scroll_frame: ctk.CTkScrollableFrame, counter: Counter) -> None:
    """Bind a counter to every CTk widget's ``<Configure>`` event.

    Counts events by widget class so we can see which widgets are redrawing
    most frequently — large counts on CTkButton/CTkEntry are the smoking gun
    for unnecessary draw calls during scroll.
    """
    by_class: Counter = counter  # alias for readability

    def visit(widget):
        cls = type(widget).__name__

        def on_configure(_e, cls=cls):
            by_class[f"configure:{cls}"] += 1
            by_class["configure_events"] += 1

        widget.bind("<Configure>", on_configure, add="+")
        for child in widget.winfo_children():
            visit(child)

    visit(scroll_frame)


def _measure_scroll(scroll_frame: ctk.CTkScrollableFrame, passes: int = 3) -> dict:
    """Drive the scrollbar up/down ``passes`` times and return measurements.

    Layout is forced to settle BEFORE instrumentation so the counters reflect
    only scroll-triggered work, not initial-layout work.
    """
    # Drain any pending layout events from initial widget creation BEFORE
    # we install instrumentation, so the counters are clean.
    scroll_frame.update_idletasks()
    scroll_frame.update()
    # Touch update again to soak up secondary fonts/geometry settling.
    scroll_frame.after(50)
    scroll_frame.update_idletasks()

    canvas = scroll_frame._parent_canvas  # noqa: SLF001 — we own the harness
    counter: Counter = Counter()
    _instrument_canvas(canvas, counter)
    _instrument_configure(scroll_frame, counter)

    fraction_top = 0.0
    fraction_bottom = 1.0
    steps_per_pass = 30  # discrete fractions per direction (≈ a fast wheel scroll)
    step = (fraction_bottom - fraction_top) / steps_per_pass

    start = time.perf_counter()
    for _ in range(passes):
        # Scroll down
        for i in range(steps_per_pass + 1):
            canvas.yview_moveto(fraction_top + step * i)
            scroll_frame.update_idletasks()
        # Scroll up
        for i in range(steps_per_pass + 1):
            canvas.yview_moveto(fraction_bottom - step * i)
            scroll_frame.update_idletasks()
    elapsed = time.perf_counter() - start

    result = {
        "elapsed_seconds": elapsed,
        "passes": passes,
        "steps_per_pass": steps_per_pass,
    }
    result.update(counter)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fix", action="store_true",
                        help="Apply the smooth-scrolling fix from gui.utils")
    parser.add_argument("--passes", type=int, default=3,
                        help="Up/down scroll passes to run (default 3)")
    args = parser.parse_args()

    ctk.set_appearance_mode("dark")
    ctk.set_default_color_theme("blue")

    root = ctk.CTk()
    root.geometry("900x720")
    root.title("Scroll perf bench")

    scroll = ctk.CTkScrollableFrame(root, corner_radius=10)
    scroll.pack(fill="both", expand=True, padx=10, pady=10)
    scroll.grid_columnconfigure(0, weight=1)

    _build_tab_layout(scroll)

    if args.fix:
        try:
            from gui.utils import install_smooth_scrolling
        except ImportError as exc:
            print(f"Cannot apply --fix: {exc}", file=sys.stderr)
            return 2
        install_smooth_scrolling(scroll)

    # Let the layout settle before instrumenting.
    root.update_idletasks()
    root.update()

    result = _measure_scroll(scroll, passes=args.passes)
    root.destroy()

    print(f"passes={result['passes']} steps_per_pass={result['steps_per_pass']}")
    print(f"elapsed_seconds={result['elapsed_seconds']:.3f}")
    print(f"yview_moveto_calls={result.get('yview_moveto_calls', 0)}")
    print(f"yview_scroll_calls={result.get('yview_scroll_calls', 0)}")
    print(f"bbox_calls={result.get('bbox_calls', 0)}")
    print(f"canvas_configure_calls={result.get('configure_calls', 0)}")
    print(f"itemconfigure_calls={result.get('itemconfigure_calls', 0)}")
    print(f"configure_events={result['configure_events']}")
    by_class = sorted(
        ((k, v) for k, v in result.items() if k.startswith("configure:")),
        key=lambda kv: -kv[1],
    )
    for k, v in by_class:
        print(f"  {k}: {v}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
