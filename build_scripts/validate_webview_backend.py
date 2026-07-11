"""Validate the real pywebview backend on the current platform.

Unlike ``build_scripts/webview_gallery.py`` (headless Chromium + mock bridge),
this script drives the ACTUAL rendering engine pywebview selects for the
platform — Edge WebView2 on Windows, WKWebView (cocoa) on macOS, WebKit2GTK on
Linux — with the real :class:`impulcifer_webview.WebviewBridge` and the real
``webview_ui/index.html``. It is the gate for shipping the WebView frontend in
the Nuitka standalone builds: a platform whose job fails here must not default
to the WebView frontend.

Checks, in order:

1. The window's document reaches ``readyState == "complete"``.
2. The sidebar navigation rendered (>= 4 ``.nav-item[data-view]`` elements).
3. ``boot()`` ran end-to-end: the brand badge shows the bridge-supplied
   version, which requires a successful ``bootstrap()`` JS→Python roundtrip.
4. A second explicit bridge roundtrip (``get_system_info``) resolves through
   the backend's promise machinery and returns ``ok: true``.

Exit codes: 0 = validated, 1 = a probe step failed, 2 = startup error,
3 = watchdog timeout (GUI loop hang).
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import threading
import time
import traceback
from typing import Any, Callable

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def _wait_js(
    window: Any,
    script: str,
    predicate: Callable[[Any], bool],
    timeout: float,
    interval: float = 0.5,
) -> Any:
    """Poll ``evaluate_js(script)`` until ``predicate`` accepts the value."""
    deadline = time.monotonic() + timeout
    last: Any = None
    while time.monotonic() < deadline:
        try:
            last = window.evaluate_js(script)
        except Exception:
            last = None
        if predicate(last):
            return last
        time.sleep(interval)
    raise TimeoutError(f"JS condition not met within {timeout:.0f}s: {script!r} (last={last!r})")


def _probe(window: Any, results: dict[str, Any]) -> None:
    """Run inside webview.start(): exercise DOM, boot() and the JS bridge."""
    try:
        _wait_js(window, "document.readyState", lambda v: v == "complete", 60)

        nav_count = _wait_js(
            window,
            "document.querySelectorAll('.nav-item[data-view]').length",
            lambda v: isinstance(v, (int, float)) and v >= 4,
            30,
        )

        # boot() sets the brand badge from bootstrap()'s version — proving the
        # pywebviewready event fired and the first bridge call succeeded. The
        # HTML placeholder is "v—", so require a digit after the "v".
        brand = _wait_js(
            window,
            "(document.getElementById('brand-version') || {}).textContent",
            lambda v: isinstance(v, str) and len(v) > 1 and v[0] == "v" and v[1].isdigit(),
            60,
        )

        # Explicit promise-based roundtrip through the backend's bridge.
        window.evaluate_js(
            "window.__impulciferProbe = null;"
            "window.pywebview.api.get_system_info().then("
            "  (r) => { window.__impulciferProbe = JSON.stringify(r); },"
            "  (e) => { window.__impulciferProbe = JSON.stringify("
            "    {ok: false, error: {message: String(e)}}); }"
            ");"
        )
        raw = _wait_js(window, "window.__impulciferProbe", lambda v: bool(v), 60)
        payload = json.loads(raw)
        if not payload.get("ok"):
            raise RuntimeError(f"bridge get_system_info() failed: {payload}")

        results["ok"] = True
        results["nav_count"] = int(nav_count)
        results["brand"] = brand
        results["info"] = payload["data"]
    except Exception:
        results["error"] = traceback.format_exc()
    finally:
        try:
            window.destroy()
        except Exception:
            pass


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--timeout",
        type=float,
        default=240.0,
        help="Hard watchdog in seconds; expiry force-exits with code 3.",
    )
    args = parser.parse_args(argv)

    try:
        import webview

        from impulcifer_webview import WebviewBridge, create_app_window, select_gui_backend
    except Exception:
        print("FATAL: could not import webview frontend modules", flush=True)
        traceback.print_exc()
        return 2

    backend = select_gui_backend()
    bridge = WebviewBridge()
    window = create_app_window(webview, bridge)

    # GUI event loops can hang without raising (e.g. missing system WebKit);
    # a daemon watchdog guarantees the CI job fails fast instead of timing out.
    def _watchdog() -> None:
        time.sleep(args.timeout)
        print(f"FATAL: watchdog expired after {args.timeout:.0f}s", flush=True)
        os._exit(3)

    threading.Thread(target=_watchdog, name="validate-watchdog", daemon=True).start()

    results: dict[str, Any] = {}
    try:
        webview.start(_probe, (window, results), gui=backend, debug=False)
    except Exception:
        print(f"FATAL: webview.start(gui={backend!r}) raised", flush=True)
        traceback.print_exc()
        return 2

    guilib = getattr(webview, "guilib", None)
    guilib_name = getattr(guilib, "__name__", None) or repr(guilib)

    if results.get("ok"):
        info = results["info"]
        print(
            f"webview backend OK: gui={backend} guilib={guilib_name} "
            f"nav_items={results['nav_count']} brand={results['brand']!r} "
            f"version={info.get('version')} python={info.get('python_version')} "
            f"os={info.get('os')!r}",
            flush=True,
        )
        return 0

    print(
        f"webview backend FAILED: gui={backend} guilib={guilib_name}\n"
        f"{results.get('error', 'probe did not run (window closed before probe?)')}",
        flush=True,
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
