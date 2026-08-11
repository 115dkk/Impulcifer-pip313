#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Render the WebView frontend into a PNG review gallery.

Follows the EqualizerAPO-XT skin-gallery pattern: a headless browser
renders every view x theme x language combination against a mocked
pywebview bridge, the script self-checks the exact shot count, and any
mismatch exits non-zero so CI fails loudly instead of publishing a
partial gallery.

Usage:
    python build_scripts/webview_gallery.py --out webview-gallery

Requires playwright with the chromium browser installed:
    pip install playwright && playwright install chromium
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

PROJECT_ROOT = Path(__file__).resolve().parent.parent

SKINS = ("studio", "stable")
LANGUAGES = ("en", "ko")
THEMES = ("dark", "light")
VIEWS = ("recorder", "processing", "recovery", "settings", "info")

# 2 skins x 2 themes x 2 languages x 5 views + 3 busy-state shots
# (studio checklist BRIR run, studio recording run, stable modal dialog)
# + 2 failure shots (studio checklist abort, stable modal error)
# + 2 auto-update prompt shots (studio en, stable ko)
# + 1 first-run language picker shot + 2 recovery terminal-state shots.
EXPECTED_SHOTS = len(SKINS) * len(LANGUAGES) * len(THEMES) * len(VIEWS) + 3 + 2 + 2 + 1 + 2

VIEWPORT_WIDTH = 1280
MIN_HEIGHT = 860
MAX_HEIGHT = 3200

FAKE_LANGUAGES = [
    {"code": "en", "name": "English"},
    {"code": "ko", "name": "한국어"},
]


def _project_version() -> str:
    import tomllib

    with open(PROJECT_ROOT / "pyproject.toml", "rb") as handle:
        return tomllib.load(handle)["project"]["version"]


def _load_strings(language: str) -> dict[str, str]:
    locales = PROJECT_ROOT / "i18n" / "locales"
    merged: dict[str, str] = {}
    for code in ("en", language) if language != "en" else ("en",):
        merged.update(json.loads((locales / f"{code}.json").read_text(encoding="utf-8")))
    return merged


def _mock_bridge_js(language: str, theme: str, scenario: str, version: str, skin: str = "studio") -> str:
    """Return an init script that fakes window.pywebview.api."""
    strings = json.dumps(_load_strings(language), ensure_ascii=False)
    languages = json.dumps(FAKE_LANGUAGES, ensure_ascii=False)
    return f"""
(() => {{
  const STRINGS = {strings};
  const LANGUAGES = {languages};
  const LANGUAGE = {json.dumps(language)};
  const THEME = {json.dumps(theme)};
  const SKIN = {json.dumps(skin)};
  const SCENARIO = {json.dumps(scenario)};
  const VERSION = {json.dumps(version)};
  const respond = (data) => Promise.resolve({{ ok: true, data }});
  const recoveryResult = {{
    source_kind: "hangloose",
    source_path: "C:/Impulcifer/output/Hangloose",
    output_dir: "C:/Impulcifer/output",
    sample_rate: 48000,
    sample_count: 8192,
    speakers: ["FL", "FR", "FC", "BL", "BR", "SL", "SR"],
    created_files: ["C:/Impulcifer/output/hrir.wav", "C:/Impulcifer/output/hesuvi.wav"],
    existing_files: ["C:/Impulcifer/output/Hangloose/FL.wav", "C:/Impulcifer/output/Hangloose/FR.wav"],
  }};
  const jobFor = (kind, status) => ({{
    job_id: "gallery", kind, status,
    cancellable: kind === "brir" && status === "running",
    result: kind === "output_recovery" && status === "succeeded" ? recoveryResult : null,
    error: status === "failed" ? {{
      code: kind === "output_recovery" ? "CONFLICTING_OUTPUTS" : "INTERNAL_ERROR",
      message: kind === "output_recovery"
        ? "hrir.wav and hesuvi.wav contain different impulse responses."
        : "Impulse response peak not found for FL-left — test signal does not match the recording.",
      details: {{}},
      retryable: false,
    }} : null,
  }});
  const runningJob = (kind) => jobFor(kind, "running");
  const activeKind = SCENARIO === "brir-running" || SCENARIO === "brir-failed" ? "brir"
    : SCENARIO === "recovery-succeeded" || SCENARIO === "recovery-failed" ? "output_recovery"
    : SCENARIO === "recording-running" ? "recording" : null;
  const pollStatus = SCENARIO === "brir-failed" || SCENARIO === "recovery-failed" ? "failed"
    : SCENARIO === "recovery-succeeded" ? "succeeded" : "running";
  window.pywebview = {{ api: {{
    bootstrap: () => respond({{
      version: VERSION,
      platform: "windows",
      capabilities: {{ recording: true, brir: true, output_recovery: true,
                       recording_cancel: false, brir_cancel: true }},
      active_job: activeKind ? runningJob(activeKind) : null,
      ui: {{ language: LANGUAGE, theme: THEME, skin: SKIN, frontend: "webview",
             first_run: SCENARIO === "first-run",
             languages: LANGUAGES, strings: STRINGS }},
    }}),
    list_audio_devices: () => respond({{
      host_apis: ["Windows DirectSound", "MME", "Windows WASAPI"],
      devices: [
        {{ index: 0, name: "Speakers (Realtek HD Audio)", host_api: "Windows DirectSound",
           max_input_channels: 0, max_output_channels: 2 }},
        {{ index: 1, name: "Microphone Array (Binaural)", host_api: "Windows DirectSound",
           max_input_channels: 2, max_output_channels: 0 }},
        {{ index: 2, name: "MiniDSP EARS", host_api: "Windows WASAPI",
           max_input_channels: 2, max_output_channels: 0 }},
      ],
      default_input_index: 1,
      default_output_index: 0,
    }}),
    get_system_info: () => respond({{
      version: VERSION, install_kind: "pip", python_version: "3.13.9",
      os: "Windows 11", cpu_count: 16, gil_enabled: false, optimal_workers: 16,
    }}),
    get_ui_settings: () => respond({{
      language: LANGUAGE, theme: THEME, skin: SKIN, languages: LANGUAGES, strings: STRINGS,
    }}),
    resolve_recording_paths: (recordDir) => respond({{
      record_path: `${{recordDir}}/FL,FR.wav`,
    }}),
    poll_job: (jobId, afterSeq) => respond({{
      job: jobFor(activeKind || "brir", pollStatus),
      events: afterSeq === 0 ? (SCENARIO === "brir-failed" ? [
        {{ seq: 1, timestamp_ms: 0, type: "status", payload: {{ status: "running" }} }},
        {{ seq: 2, timestamp_ms: 0, type: "log",
           payload: {{ level: "INFO", message: STRINGS["cli_opening_measurements"] }} }},
        {{ seq: 3, timestamp_ms: 0, type: "progress",
           payload: {{ progress: 0.2, message: STRINGS["cli_cropping_responses"] }} }},
        {{ seq: 4, timestamp_ms: 0, type: "progress",
           payload: {{ progress: 0.34, message: STRINGS["cli_running_room_correction"] }} }},
        {{ seq: 5, timestamp_ms: 0, type: "log",
           payload: {{ level: "ERROR",
             message: "Impulse response peak not found for FL-left" }} }},
        {{ seq: 6, timestamp_ms: 0, type: "status", payload: {{ status: "failed" }} }},
      ] : activeKind === "recording" ? [
        {{ seq: 1, timestamp_ms: 0, type: "status", payload: {{ status: "running" }} }},
        {{ seq: 2, timestamp_ms: 0, type: "progress",
           payload: {{ phase: "devices", progress: 0.02, message: "Windows DirectSound" }} }},
        {{ seq: 3, timestamp_ms: 0, type: "progress",
           payload: {{ phase: "recording", progress: 0.24, elapsed: 7.1, duration: 29.6,
                       speaker: "FL", segment_index: 1, segment_total: 2,
                       speakers: ["FL", "FR"] }} }},
        {{ seq: 4, timestamp_ms: 0, type: "progress",
           payload: {{ phase: "recording", progress: 0.58, elapsed: 17.2, duration: 29.6,
                       speaker: "FR", segment_index: 2, segment_total: 2,
                       speakers: ["FL", "FR"] }} }},
      ] : [
        {{ seq: 1, timestamp_ms: 0, type: "status", payload: {{ status: "running" }} }},
        {{ seq: 2, timestamp_ms: 0, type: "log",
           payload: {{ level: "INFO", message: STRINGS["cli_opening_measurements"] }} }},
        {{ seq: 3, timestamp_ms: 0, type: "progress",
           payload: {{ progress: 0.2, message: STRINGS["cli_cropping_responses"] }} }},
        {{ seq: 4, timestamp_ms: 0, type: "progress",
           payload: {{ progress: 0.34, message: STRINGS["cli_running_room_correction"] }} }},
        {{ seq: 5, timestamp_ms: 0, type: "log",
           payload: {{ level: "INFO", message: STRINGS["cli_equalizing"] + " FL,FR" }} }},
        {{ seq: 6, timestamp_ms: 0, type: "progress",
           payload: {{ progress: 0.62, message: STRINGS["cli_normalizing_gain"] }} }},
      ]) : [],
      next_seq: 6,
    }}),
    cancel_job: () => respond({{ job: runningJob(activeKind || "brir") }}),
    check_for_updates: () => respond(SCENARIO === "update-available" ? {{
      update_available: true,
      current_version: VERSION,
      latest_version: "99.0.0",
      download_url: "https://github.com/115dkk/Impulcifer-pip313/releases/download/v99.0.0/Impulcifer-win-Setup.exe",
      release_notes: "## 99.0.0 - 2026-07-12\\n\\n### Gallery placeholder release\\n\\n- Auto-update prompt review shot\\n- Release notes render in this scrollable box\\n- Buttons mirror the CTk UpdateDialog",
      release_url: "https://github.com/115dkk/Impulcifer-pip313/releases/latest",
    }} : {{
      update_available: false, current_version: VERSION, latest_version: VERSION,
      download_url: null, release_notes: null, release_url: null,
    }}),
    start_update: () => respond({{ job: runningJob("update") }}),
    apply_pending_update: () => respond({{ restarting: true }}),
    set_language: (code) => respond({{ language: code, strings: STRINGS }}),
    set_theme: (theme) => respond({{ theme }}),
    set_skin: (skin) => respond({{ skin }}),
    set_frontend: (frontend) => respond({{ frontend }}),
    generate_sweep_set: () => respond({{ files: [], play_path: null }}),
    open_path: () => respond({{ path: "" }}),
    open_url: () => respond({{ url: "" }}),
    select_file: () => respond({{ path: null }}),
    select_directory: () => respond({{ path: null }}),
    start_recording: () => respond({{ job: runningJob("recording") }}),
    start_brir: () => respond({{ job: runningJob("brir") }}),
    start_output_recovery: () => respond({{ job: runningJob("output_recovery") }}),
  }} }};
}})();
"""


def _shoot(page, out_dir: Path, name: str) -> Path:
    # Reset to the base viewport first: scrollHeight can never shrink below
    # clientHeight, so measuring at a previously enlarged viewport would
    # carry the tallest view's height into every later shot.
    page.set_viewport_size({"width": VIEWPORT_WIDTH, "height": MIN_HEIGHT})
    page.wait_for_timeout(80)
    height = page.evaluate("document.querySelector('.content').scrollHeight")
    height = max(MIN_HEIGHT, min(MAX_HEIGHT, int(height) + 48))
    page.set_viewport_size({"width": VIEWPORT_WIDTH, "height": height})
    page.wait_for_timeout(120)
    target = out_dir / f"{name}.png"
    page.screenshot(path=str(target))
    return target


def _open_page(browser, index_uri: str, language: str, theme: str, scenario: str, version: str, skin: str = "studio"):
    context = browser.new_context(
        viewport={"width": VIEWPORT_WIDTH, "height": MIN_HEIGHT},
        device_scale_factor=1,
    )
    page = context.new_page()
    page.add_init_script(_mock_bridge_js(language, theme, scenario, version, skin))
    page.goto(index_uri)
    page.wait_for_function("document.getElementById('brand-version').textContent !== 'v—'")
    page.wait_for_timeout(250)
    return context, page


def render_gallery(out_dir: Path) -> list[Path]:
    from playwright.sync_api import sync_playwright

    version = _project_version()
    index_uri = (PROJECT_ROOT / "webview_ui" / "index.html").resolve().as_uri()
    out_dir.mkdir(parents=True, exist_ok=True)
    shots: list[Path] = []

    with sync_playwright() as playwright:
        browser = playwright.chromium.launch()

        for skin in SKINS:
            for language in LANGUAGES:
                for theme in THEMES:
                    context, page = _open_page(
                        browser, index_uri, language, theme, "idle", version, skin
                    )
                    for view in VIEWS:
                        page.click(f".nav-item[data-view='{view}']")
                        if view == "processing":
                            # Open every disclosure so the full option surface
                            # is part of the judging material.
                            page.eval_on_selector_all(
                                ".disclosure",
                                "nodes => nodes.forEach(n => n.classList.add('open'))",
                            )
                            page.check("#bf-decay-per-channel")
                            page.eval_on_selector(
                                "#bf-decay-per-channel",
                                "node => node.dispatchEvent(new Event('change'))",
                            )
                        shots.append(_shoot(page, out_dir, f"{view}-{skin}-{language}-{theme}"))
                    context.close()

        # Busy states: a BRIR run with the Studio pipeline checklist, a
        # capture run on the recorder view, and the Stable modal dialog.
        context, page = _open_page(browser, index_uri, "en", "dark", "brir-running", version)
        page.click(".nav-item[data-view='processing']")
        page.wait_for_timeout(400)
        shots.append(_shoot(page, out_dir, "processing-studio-en-dark-busy"))
        context.close()

        context, page = _open_page(browser, index_uri, "ko", "dark", "recording-running", version)
        page.wait_for_timeout(400)
        shots.append(_shoot(page, out_dir, "recorder-studio-ko-dark-busy"))
        context.close()

        # The Stable job modal is already covering the page: a normal click
        # fails playwright's actionability check and force=True lands on the
        # backdrop instead of the tab. Dispatch the DOM click directly so
        # the nav handler runs and the background view really switches.
        context, page = _open_page(
            browser, index_uri, "en", "dark", "brir-running", version, "stable"
        )
        page.eval_on_selector(".nav-item[data-view='processing']", "node => node.click()")
        page.wait_for_timeout(400)
        shots.append(_shoot(page, out_dir, "processing-stable-en-dark-busy"))
        context.close()

        # Failure semantics: the failed stage gets ✕, finished stages keep
        # their checkmarks, unreached stages stay dimmed circles; Stable
        # surfaces the same run in the modal with the error in the log.
        context, page = _open_page(browser, index_uri, "en", "dark", "brir-failed", version)
        page.click(".nav-item[data-view='processing']")
        page.wait_for_timeout(400)
        shots.append(_shoot(page, out_dir, "processing-studio-en-dark-failed"))
        context.close()

        context, page = _open_page(
            browser, index_uri, "en", "dark", "brir-failed", version, "stable"
        )
        page.eval_on_selector(".nav-item[data-view='processing']", "node => node.click()")
        page.wait_for_timeout(400)
        shots.append(_shoot(page, out_dir, "processing-stable-en-dark-failed"))
        context.close()

        # Output recovery terminal states use the real result surface rather
        # than a generic success toast. Capture both the rebuilt-file manifest
        # and the highest-risk conflict failure.
        context, page = _open_page(
            browser, index_uri, "ko", "dark", "recovery-succeeded", version
        )
        page.click(".nav-item[data-view='recovery']")
        page.wait_for_timeout(250)
        shots.append(_shoot(page, out_dir, "recovery-studio-ko-dark-succeeded"))
        context.close()

        context, page = _open_page(
            browser, index_uri, "en", "light", "recovery-failed", version, "stable"
        )
        page.eval_on_selector(".nav-item[data-view='recovery']", "node => node.click()")
        page.wait_for_timeout(250)
        shots.append(_shoot(page, out_dir, "recovery-stable-en-light-failed"))
        context.close()

        # First-run language picker (CTk LanguageSelectionDialog port).
        context, page = _open_page(browser, index_uri, "en", "dark", "first-run", version)
        page.wait_for_selector("#language-modal:not([hidden])")
        page.wait_for_timeout(200)
        shots.append(_shoot(page, out_dir, "firstrun-studio-en-dark"))
        context.close()

        # Auto-update prompt (CTk UpdateDialog port): version line, release
        # notes, Update Now / Remind / Skip. Triggered directly instead of
        # waiting out boot()'s 2-second background-check timer.
        for skin, language in (("studio", "en"), ("stable", "ko")):
            context, page = _open_page(
                browser, index_uri, language, "dark", "update-available", version, skin
            )
            page.evaluate("checkForUpdates(false)")
            page.wait_for_selector("#update-modal:not([hidden])")
            page.wait_for_timeout(200)
            shots.append(_shoot(page, out_dir, f"update-{skin}-{language}-dark"))
            context.close()

        browser.close()
    return shots


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", default="webview-gallery", help="Output directory for PNGs")
    args = parser.parse_args()

    out_dir = Path(args.out)
    shots = render_gallery(out_dir)

    missing = [shot for shot in shots if not shot.is_file() or shot.stat().st_size == 0]
    print(f"Gallery wrote {len(shots)} PNGs to {out_dir}")
    if missing:
        print(f"ERROR: {len(missing)} shots missing or empty: {missing}", file=sys.stderr)
        return 1
    if len(shots) != EXPECTED_SHOTS:
        print(
            f"ERROR: expected {EXPECTED_SHOTS} shots, got {len(shots)}"
            " — update EXPECTED_SHOTS if the matrix changed intentionally.",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
