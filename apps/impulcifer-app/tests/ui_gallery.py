"""Render the 3.x UI against deterministic IPC fixtures, without launching the app.

py -3.14 apps/impulcifer-app/tests/ui_gallery.py --out target/ui-gallery
Uses the installed Playwright Chromium. No browser download or real service calls.
"""
from __future__ import annotations

import argparse
import itertools
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
SKINS = ("studio", "stable")
THEMES = ("dark", "light")
LANGUAGES = ("en", "ko")
VIEWS = ("settings", "info", "recorder")
RECOVERY_STATES = ("empty", "planning", "ready", "nothing", "error", "succeeded")
EXPECTED_SHOTS = 2 * 2 * 2 * 3 + 2 * 2 * 6 + 2 * 2 * 2 * 2


def catalog(language):
    merged = {}
    for directory in ("i18n/locales", "crates/impulcifer-service/locales"):
        for code in ("en", language):
            merged.update(json.loads((ROOT / directory / f"{code}.json").read_text(encoding="utf-8")))
    return merged


def mock_script(skin, theme, language, scenario):
    fixture = json.dumps({"skin": skin, "theme": theme, "language": language,
                         "scenario": scenario, "strings": catalog(language),
                         "catalogs": {code: catalog(code) for code in LANGUAGES}}, ensure_ascii=False)
    return "const fixture = " + fixture + ";\n" + r"""
(() => {
  const {skin, theme, language, scenario, strings} = fixture;
  const calls = [];
  window.galleryCalls = calls;
  window.galleryDialogs = [];
  window.confirm = text => { window.galleryDialogs.push(text); return true; };
  window.alert = text => window.galleryDialogs.push(text);
  const ok = data => Promise.resolve({ok: true, data});
  const error = (code, message, details = {}) => ({ok: false, error: {code, message, details, retryable: false}});
  const directory = "C:/Impulcifer/output";
  const meta = {source_kind: "hesuvi", source_path: `${directory}/hesuvi.wav`, output_dir: directory,
    sample_rate: 48000, sample_count: 24000, speakers: ["FL", "FR"], existing_files: [`${directory}/hesuvi.wav`]};
  let written = false;
  let kind = "output_recovery";
  let lastRequest = {};
  const plan = request => {
    const planned = [{path: `${directory}/hrir.wav`, kind: "hrir", channels: 16, speaker: null}];
    if (request.include_hangloose) for (const speaker of meta.speakers)
      planned.push({path: `${directory}/Hangloose/${speaker}.wav`, kind: "hangloose", channels: 2, speaker});
    const allPresent = scenario === "nothing" || written;
    return {...meta, existing_files: allPresent ? [...meta.existing_files, ...planned.map(file => file.path)] : meta.existing_files,
      planned_files: allPresent ? [] : planned, hangloose_dir: request.include_hangloose ? `${directory}/Hangloose` : null};
  };
  const job = status => ({job_id: "gallery", kind, status, cancellable: false, result: null, error: null});
  const settings = {skin, theme, language, strings, frontend: "webview", first_run: false,
    languages: [{code: "en", name: "English"}, {code: "ko", name: "한국어"}]};
  const methods = {
    bootstrap: () => ok({version: "3.0.0-alpha.1", platform: "windows", install_kind: "dev", webview_backend: "edgechromium",
      brir_defaults: {}, sweep: {layouts: ["stereo", "7.1"], default_fs: 48000, default_duration: 5, speaker_names: ["FL", "FR"]},
      capabilities: {recording: true, brir: true, output_recovery: true, recording_cancel: false,
        brir_cancel: true, output_recovery_cancel: false, share_modes: scenario === "disabled" ? ["auto"] : ["auto", "exclusive", "shared"]},
      active_job: null, ui: settings}),
    list_audio_devices: () => ok({host_apis: ["Windows WASAPI"], default_input_index: 1, default_output_index: 0,
      devices: [{index: 0, name: "Speakers (USB Audio)", host_api: "Windows WASAPI", max_input_channels: 0, max_output_channels: 2},
        {index: 1, name: "Binaural microphones (USB Audio)", host_api: "Windows WASAPI", max_input_channels: 2, max_output_channels: 0}]}),
    get_system_info: () => ok({version: "3.0.0-alpha.1", install_kind: "dev", os: "Windows 11", cpu_count: 16,
      runtime: {language: "Rust", toolchain: "rustc 1.97.0", audio_backend: "WASAPI", shell: "Tauri 2.11.5", webview: "WebView2 152.0.4191.66"},
      paths: {data_dir: "C:/Impulcifer/data", settings_path: "C:/Users/Listener/.impulcifer/settings.json"}, update_channel: "prerelease"}),
    get_ui_settings: () => ok(settings),
    set_language: code => ok({...settings, language: code, strings: fixture.catalogs[code]}),
    set_skin: skin => ok({skin}), set_theme: theme => ok({theme}), set_frontend: frontend => ok({frontend}),
    resolve_recording_paths: (dir, play, mode) => ok({record_path: `${dir}/${mode === "headphones" ? "headphones" : "FL,FR"}.wav`}),
    plan_output_recovery: async request => {
      if (scenario === "planning") return new Promise(() => {});
      if (scenario === "error") return error("NO_RECOVERY_SOURCE", strings.recovery_inventory_error);
      // The first request is deliberately slower, to exercise stale-result suppression.
      if (request.dir_path.endsWith("/slow")) {
        await new Promise(resolve => setTimeout(resolve, 1000));
        return error("STALE_RESPONSE", "Stale response must never be rendered");
      }
      return ok(plan(request));
    },
    start_output_recovery: request => {kind = "output_recovery"; lastRequest = request; return ok({job: job("running")});},
    start_recording: request => {
      lastRequest = request; kind = "recording";
      if (scenario === "unavailable") return Promise.resolve(error("INVALID_REQUEST", "backend cannot fix mode",
        {kind: "share_mode_unavailable", share_mode: request.share_mode, backend: "cpal"}));
      return ok({job: job("running")});
    },
    start_brir: () => {kind = "brir"; return ok({job: job("running")});},
    poll_job: () => {
      if (kind === "recording") {
        if (scenario === "refused") return ok({job: {...job("failed"), error: error("DEVICE_ERROR", "device refused exclusive",
          {kind: "share_mode_refused", share_mode: "exclusive", reason: "AUDCLNT_E_DEVICE_IN_USE"}).error}, events: [], next_seq: 1});
        const share = {requested: lastRequest.share_mode, output: "exclusive", input: "shared_auto_convert"};
        return ok({job: {...job("succeeded"), result: {mode: lastRequest.mode, record_path: "C:/recording/headphones.wav", summary: null,
          sweep: null, sidecar_path: null, share}}, events: [{seq: 1, timestamp_ms: 0, type: "log",
            payload: {level: "info", key: "recording_share_mode_opened", message: "untranslated fixture", share}}], next_seq: 1});
      }
      const result = {...meta, created_files: plan(lastRequest).planned_files.map(file => file.path)};
      written = true;
      return ok({job: {...job("succeeded"), result}, events: [], next_seq: 1});
    },
    cancel_job: () => ok({job: job("cancelled")}),
    check_for_updates: () => ok({update_available: false, current_version: "3.0.0-alpha.1", latest_version: "3.0.0-alpha.1",
      download_url: null, release_notes: null, release_url: null}),
    start_update: () => {kind = "update"; return ok({job: job("running")});},
    apply_pending_update: () => ok({restarting: true}),
    select_directory: () => ok({path: directory}), select_file: () => ok({path: null}),
    open_path: path => ok({path}), open_url: url => ok({url}),
    generate_sweep_set: () => ok({files: [], play_path: null}), detect_sweep: () => ok({found: false, sidecar: false}),
  };
  window.pywebview = {api: new Proxy(methods, {get(target, method) {
    if (!(method in target)) throw new Error(`Unexpected IPC method: ${String(method)}`);
    return (...args) => {calls.push({method, args}); return target[method](...args);};
  }})};
})();
"""


def open_page(browser, skin, theme, language, scenario):
    context = browser.new_context(viewport={"width": 1280, "height": 860}, device_scale_factor=1)
    page = context.new_page()
    errors = []
    page.on("pageerror", lambda error: errors.append(str(error)))
    page.on("console", lambda message: errors.append(message.text) if message.type == "error" else None)
    page.on("requestfailed", lambda request: errors.append(f"{request.url}: {request.failure}"))
    page.add_init_script(script=mock_script(skin, theme, language, scenario))
    page.goto((ROOT / "apps/impulcifer-app/ui/index.html").as_uri())
    page.wait_for_function("document.querySelector('#info-system').childElementCount > 0")
    page.evaluate("document.fonts.ready")
    assert page.locator("#sf-frontend").count() == 0
    assert page.locator("#view-settings .card").count() == 2
    assert page.locator("#rf-host-api-row").get_attribute("hidden") is not None
    assert "auto" in page.locator("#rf-share-mode option").evaluate_all("nodes => nodes.map(n => n.value)")
    assert not errors, errors
    return context, page, errors


def navigate(page, view):
    page.locator(f'#nav [data-view="{view}"]').click()


def shoot(page, output, name, errors):
    page.set_viewport_size({"width": 1280, "height": 860})
    height = page.evaluate("document.querySelector('#content').scrollHeight")
    page.set_viewport_size({"width": 1280, "height": min(3200, max(860, height + 96))})
    page.evaluate("document.querySelector('#content').scrollTop = 0")
    path = output / f"{name}.png"
    page.screenshot(path=str(path))
    assert not errors, errors
    print(path.as_posix(), flush=True)
    return path


def behavior_checks(browser):
    context, page, errors = open_page(browser, "studio", "dark", "en", "ready")
    navigate(page, "recovery")
    page.locator("#recovery-dir-path").fill("C:/Impulcifer/output/slow")
    page.wait_for_function("galleryCalls.some(c => c.method === 'plan_output_recovery')")
    page.locator("#recovery-dir-path").fill("C:/Impulcifer/output/new")
    page.locator('#recovery-inventory[data-state="ready"]').wait_for()
    page.wait_for_timeout(1100)
    assert page.locator("#recovery-inventory").get_attribute("data-state") == "ready"
    assert "STALE_RESPONSE" not in page.locator("#recovery-inventory").inner_text()
    page.evaluate("""() => { const n = document.querySelector('#recovery-dir-path');
      for (const value of ['a', 'ab', 'abc']) {n.value = value; n.dispatchEvent(new Event('input'));} }""")
    page.locator('#recovery-inventory[data-state="ready"]').wait_for()
    assert page.evaluate("galleryCalls.filter(c => c.method === 'plan_output_recovery').length") == 3
    page.locator("#recovery-include-hangloose").check()
    page.locator('#recovery-ledger [data-file="Hangloose/FL.wav"]').wait_for()
    page.locator("#recovery-remove-silent-channels").check()
    page.wait_for_function("galleryCalls.filter(c => c.method === 'plan_output_recovery').at(-1).args[0].remove_silent_channels")
    navigate(page, "settings")
    page.locator("#btn-open-settings").click()
    assert page.evaluate("galleryCalls.filter(c => c.method === 'open_path').at(-1).args[0]") == "C:/Users/Listener/.impulcifer"
    page.locator("#sf-language").select_option("ko")
    page.wait_for_function("document.documentElement.lang === 'ko'")
    assert "Python" not in page.locator("#runtime-status").inner_text()
    assert "연결하는 중" not in page.locator("#runtime-status").inner_text()
    page.locator("#sf-skin").select_option("stable")
    page.wait_for_function("document.documentElement.dataset.skin === 'stable'")
    before = page.evaluate("galleryCalls.filter(c => c.method === 'plan_output_recovery').length")
    navigate(page, "recovery")
    page.locator("#recovery-dir-path").fill("C:/stable")
    page.wait_for_timeout(450)
    assert page.evaluate("galleryCalls.filter(c => c.method === 'plan_output_recovery').length") == before
    assert page.locator("#btn-start-recovery").is_enabled()
    assert not page.locator("#recovery-inventory").is_visible()
    assert not errors, errors
    context.close()
    context, page, errors = open_page(browser, "studio", "dark", "en", "ready")
    navigate(page, "recovery")
    page.locator("#recovery-dir-path").fill("C:/Impulcifer/output")
    page.locator('#recovery-inventory[data-state="ready"]').wait_for()
    page.locator("#btn-start-recovery").click()
    page.locator('.recovery-result[data-status="succeeded"]').wait_for()
    page.locator("#recovery-include-hangloose").check()
    page.locator('#recovery-inventory[data-state="nothing"]').wait_for()
    assert page.locator('#recovery-ledger [data-status="created"]').count() == 0
    assert not errors, errors
    context.close()
    for scenario in ("success", "unavailable"):
        context, page, errors = open_page(browser, "studio", "dark", "ko", scenario)
        page.locator("#rf-share-mode").select_option("shared")
        page.locator("#btn-record-headphones").click()
        page.wait_for_function("galleryCalls.some(c => c.method === 'start_recording')")
        assert page.evaluate("galleryCalls.find(c => c.method === 'start_recording').args[0].share_mode") == "shared"
        confirmation = page.evaluate("galleryDialogs[0]")
        assert "{play_file}" not in confirmation and "{share_mode}" not in confirmation
        if scenario == "success":
            page.wait_for_function("document.querySelector('[data-rec-detail]').textContent.includes('자동 변환')")
            assert "untranslated fixture" not in page.locator("[data-log]").first.inner_text()
        else:
            page.wait_for_function("document.querySelector('[data-rec-detail]').textContent.includes('고정할 수 없습니다')")
        assert not errors, errors
        context.close()
    print("Gallery behavior checks: debounce, stale responses, option replan, Stable no-plan, folder parent, language refresh, confirmation, share success/error OK", flush=True)


def render_gallery(output):
    from playwright.sync_api import sync_playwright

    output.mkdir(parents=True, exist_ok=True)
    shots = []
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch()
        behavior_checks(browser)
        for skin, theme, language in itertools.product(SKINS, THEMES, LANGUAGES):
            context, page, errors = open_page(browser, skin, theme, language, "idle")
            for view in VIEWS:
                navigate(page, view)
                shots.append(shoot(page, output, f"{view}-{skin}-{language}-{theme}", errors))
            context.close()
        for theme, language, scenario in itertools.product(THEMES, LANGUAGES, RECOVERY_STATES):
            context, page, errors = open_page(browser, "studio", theme, language, scenario)
            navigate(page, "recovery")
            if scenario != "empty":
                page.locator("#recovery-include-hangloose").check()
                page.locator("#recovery-dir-path").fill("C:/Impulcifer/output")
                expected = "ready" if scenario == "succeeded" else scenario
                page.locator(f'#recovery-inventory[data-state="{expected}"]').wait_for()
                if scenario == "succeeded":
                    page.locator("#btn-start-recovery").click()
                    page.locator('.recovery-result[data-status="succeeded"]').wait_for()
                    assert page.locator('#recovery-ledger [data-file="hrir.wav"]').get_attribute("data-status") == "created"
                    assert page.locator("#btn-open-recovery-output").is_visible()
                elif scenario == "ready":
                    assert page.locator('#recovery-ledger [data-file="Hangloose/FL.wav"]').count() == 1
            assert page.locator("#btn-start-recovery").is_enabled() == (scenario == "ready")
            shots.append(shoot(page, output, f"recovery-studio-{language}-{theme}-{scenario}", errors))
            context.close()
        for skin, theme, language, scenario in itertools.product(SKINS, THEMES, LANGUAGES, ("disabled", "refused")):
            context, page, errors = open_page(browser, skin, theme, language, scenario)
            if scenario == "disabled":
                assert page.locator("#rf-share-mode").is_disabled()
                assert page.locator("#rf-share-mode-hint").is_visible()
            else:
                page.locator("#rf-share-mode").select_option("exclusive")
                page.locator("#btn-record-headphones").click()
                page.wait_for_function("document.querySelector('[data-rec-detail]').textContent.includes('AUDCLNT_E_DEVICE_IN_USE')")
                if skin == "stable":
                    assert page.evaluate("galleryDialogs.length") == 2
            shots.append(shoot(page, output, f"recorder-{skin}-{language}-{theme}-{scenario}", errors))
            context.close()
        browser.close()
    assert len(shots) == EXPECTED_SHOTS, f"expected {EXPECTED_SHOTS}, got {len(shots)}"
    assert len(set(shots)) == EXPECTED_SHOTS
    assert all(path.is_file() and path.stat().st_size for path in shots)
    print(f"Gallery wrote {len(shots)} PNGs to {output.as_posix()}")
    return shots


def main():
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=ROOT / "target/ui-gallery")
    args = parser.parse_args()
    render_gallery(args.out.resolve())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
