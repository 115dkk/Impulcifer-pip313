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
EQ_SCENARIOS = ("eq-folder", "eq-mixed")
ADV_TABS = ("tone", "time", "output", "correction")
ALL_LANGUAGES = ("en", "ko", "ja", "de", "es", "fr", "ru", "zh_CN", "zh_TW")
EXPECTED_SHOTS = 2 * 2 * 2 * 3 + 2 * 2 * 6 + 2 * 2 * 2 * 2 + 2 * 2 * 2 * 2 + 2 * 2 + 2 * 2 * len(ADV_TABS)
ADVANCED_KEYS = {"fs", "target_level", "channel_balance", "bass_boost_gain", "bass_boost_fc", "bass_boost_q", "tilt",
                 "decay", "head_ms", "jamesdsp", "hangloose", "remove_silent_channels", "interactive_plots",
                 "microphone_deviation_correction", "mic_deviation_strength", "mic_deviation_debug_plots",
                 "output_truehd_layouts"}


def catalog(language):
    merged = {}
    for directory in ("i18n/locales", "crates/impulcifer-service/locales"):
        for code in ("en", language):
            merged.update(json.loads((ROOT / directory / f"{code}.json").read_text(encoding="utf-8")))
    return merged


def release_notes():
    """The update dialog shows the release body, which release-3x.yml takes
    from the workspace version's CHANGELOG section; render that same text."""
    sys.path.insert(0, str(ROOT / ".github/scripts"))
    from release_gate import workspace_version
    from release_notes import changelog_section
    version = workspace_version((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    notes = changelog_section((ROOT / "CHANGELOG.md").read_text(encoding="utf-8"), version)
    assert notes, f"CHANGELOG.md has no section for {version}"
    return version, notes


def mock_script(skin, theme, language, scenario):
    version, notes = release_notes() if scenario == "update" else (None, None)
    fixture = json.dumps({"skin": skin, "theme": theme, "language": language,
                         "scenario": scenario, "strings": catalog(language),
                         "release": {"version": version, "notes": notes},
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
    check_for_updates: () => scenario === "update"
      ? ok({update_available: true, current_version: "3.0.0-alpha.1", latest_version: fixture.release.version,
        download_url: "https://example.invalid/Impulcifer-win-Setup.exe", release_notes: fixture.release.notes, release_url: null})
      : ok({update_available: false, current_version: "3.0.0-alpha.1", latest_version: "3.0.0-alpha.1",
        download_url: null, release_notes: null, release_url: null}),
    start_update: () => {kind = "update"; return ok({job: job("running")});},
    apply_pending_update: () => ok({restarting: true}),
    select_directory: () => ok({path: directory}),
    select_file: kind => ok({path: kind === "text" ? "D:/EQ presets/HD800S left.csv" : null}),
    open_path: path => ok({path}), open_url: url => ok({url}),
    generate_sweep_set: () => ok({files: [], play_path: null}), detect_sweep: () => ok({found: false, sidecar: false}),
    inspect_eq: request => {
      const grid = Array.from({length: 121}, (_, i) => 20 * 2 ** (i / 12)).filter(f => f <= 20000.02);
      const bell = (gain, center, width) => grid.map(f => gain * Math.exp(-(Math.log2(f / center) ** 2) / width));
      const round = values => values.map(v => Math.round(v * 100) / 100);
      const slot = (name, extra) => ({slot: name, source: "folder", path: null, name: null,
        folder_default: name === "both" ? "eq.csv" : `eq-${name}.csv`, ignored: [], format: null, channels: null, error: null, ...extra});
      const both = request.eq_file === false ? slot("both", {source: "off"})
        : slot("both", {path: "C:/Impulcifer/my_hrir/eq.txt", name: "eq.txt", format: "eqapo", channels: "split",
          eqapo: {preamp_db: [-6, -6], applied: 3, bypassed: 1, skipped: 0}});
      let left = slot("left");
      if (typeof request.eq_left_file === "string") {
        const name = request.eq_left_file.split("/").pop();
        left = scenario === "eq-mixed"
          ? slot("left", {source: "file", path: request.eq_left_file, name, error: "missing CSV value"})
          : slot("left", {source: "file", path: request.eq_left_file, name, format: "csv", channels: "both"});
      }
      const right = request.eq_right_file === false ? slot("right", {source: "off"}) : slot("right");
      const on = both.source !== "off";
      const leftCurve = left.source === "file" && !left.error ? round(bell(-3, 3000, 0.8))
        : on ? round(bell(6, 1000, 0.6).map(v => v - 6)) : null;
      const rightCurve = on ? round(bell(-4, 1000, 0.6).map(v => v - 6)) : null;
      const blocked = Boolean(left.error);
      return ok({slots: [both, left, right], blocked,
        ears: blocked ? {left: null, right: null} : {
          left: left.source === "file" ? {slot: "left", channel: "both"} : on ? {slot: "both", channel: "left"} : null,
          right: on ? {slot: "both", channel: "right"} : null},
        curves: {frequency: grid, left: blocked ? null : leftCurve, right: blocked ? null : rightCurve}});
    },
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
    context, page, errors = open_page(browser, "studio", "dark", "en", "eq-folder")
    navigate(page, "processing")
    open_eq(page)
    page.locator('#bf-eq-slots .eq-slot[data-state="found"] .eq-file-name', has_text="eq.txt").wait_for()
    assert page.locator("#bf-eq-preview polyline").count() == 2
    assert "left channel" in page.locator('#bf-eq-ears [data-ear="left"]').inner_text()
    left_slot = page.locator("#bf-eq-slots .eq-slot").nth(1)
    left_slot.get_by_role("button", name="Choose file…").click()
    page.wait_for_function("galleryCalls.filter(c => c.method === 'inspect_eq').at(-1).args[0].eq_left_file === 'D:/EQ presets/HD800S left.csv'")
    left_slot.locator(".eq-file-name", has_text="HD800S left.csv").wait_for()
    page.locator("#bf-eq-slots .eq-slot").nth(2).get_by_role("button", name="Turn off").click()
    page.wait_for_function("galleryCalls.filter(c => c.method === 'inspect_eq').at(-1).args[0].eq_right_file === false")
    page.locator("#btn-generate-brir").click()
    page.wait_for_function("galleryCalls.some(c => c.method === 'start_brir')")
    request = page.evaluate("galleryCalls.find(c => c.method === 'start_brir').args[0]")
    assert request["eq_left_file"] == "D:/EQ presets/HD800S left.csv" and request["eq_right_file"] is False, request
    assert "eq_file" not in request, request
    page.locator("#bf-eq-slots .eq-slot").nth(1).get_by_role("button", name="Use folder").click()
    page.wait_for_function("!('eq_left_file' in galleryCalls.filter(c => c.method === 'inspect_eq').at(-1).args[0])")
    assert not errors, errors
    context.close()
    print("Gallery behavior checks: debounce, stale responses, option replan, Stable no-plan, folder parent, language refresh, confirmation, share success/error, EQ slots OK", flush=True)


def brir_request(page):
    """Press Generate and return the start_brir request it sent."""
    before = page.evaluate("galleryCalls.filter(c => c.method === 'start_brir').length")
    page.locator("#btn-generate-brir").click()
    page.wait_for_function(f"galleryCalls.filter(c => c.method === 'start_brir').length > {before}")
    request = page.evaluate("galleryCalls.filter(c => c.method === 'start_brir').at(-1).args[0]")
    page.evaluate("document.querySelector('#btn-cancel-brir').click()")
    return request


def disabled(page, selector):
    """:disabled as the browser computes it, fieldsets and their contents included."""
    return page.locator(selector).evaluate("n => n.matches(':disabled')")


def adv_switch(page, tab, on):
    page.locator(f"#adv-tab-{tab}").click()
    switch = page.locator(f"#adv-on-{tab}")
    if (switch.get_attribute("aria-checked") == "true") != on:
        switch.click()
    assert disabled(page, f"#adv-fields-{tab}") != on


def disclosure(page, name, open_):
    if ("open" in (page.locator(f"#dis-{name}").get_attribute("class") or "")) != open_:
        page.locator(f"#dis-{name} > .disclosure-head").click()


def advanced_checks(browser):
    """Studio's Advanced Options: tabs send only when their switch is on, and
    what cannot apply is locked rather than explained."""
    context, page, errors = open_page(browser, "studio", "dark", "en", "idle")
    navigate(page, "processing")
    assert not page.locator("#dis-advanced").is_visible() and page.locator("#adv-studio").is_visible()
    assert page.locator("#adv-studio .seg-btn.is-off").count() == 4
    request = brir_request(page)
    assert not ADVANCED_KEYS & request.keys(), request

    # No equalization stage: bass boost and tilt are locked, level still works.
    adv_switch(page, "tone", True)
    assert disabled(page, "#ba-tone-eq") and disabled(page, "#ba-bass-gain")
    assert page.locator("#ba-tone-eq").get_attribute("title")
    assert not disabled(page, "#ba-target-level") and not disabled(page, "#ba-balance")
    page.locator("#ba-target-level").fill("-12")
    request = brir_request(page)
    assert request["target_level"] == -12 and "bass_boost_gain" not in request and "tilt" not in request, request

    disclosure(page, "headphone", True)
    assert not disabled(page, "#ba-bass-gain") and disabled(page, "#ba-bass-fc")
    page.locator("#ba-bass-gain").fill("4")
    assert not disabled(page, "#ba-bass-fc") and not disabled(page, "#ba-bass-q")
    page.locator("#ba-balance").select_option("number")
    page.locator("#ba-balance-db").fill("2")
    request = brir_request(page)
    assert request["bass_boost_gain"] == 4 and request["bass_boost_fc"] == 105 and request["channel_balance"] == 2, request
    assert "fs" not in request and "jamesdsp" not in request and "head_ms" not in request, request

    # Headphone compensation on: mic deviation correction is locked and unchecked.
    adv_switch(page, "correction", True)
    assert disabled(page, "#ba-mic-deviation") and not page.locator("#ba-mic-deviation").is_checked()
    assert page.locator("#ba-mic-row").get_attribute("title")
    page.locator("#ba-interactive-plots").check()
    disclosure(page, "headphone", False)
    page.locator("#ba-mic-deviation").check()
    assert not disabled(page, "#ba-mic-strength")
    disclosure(page, "headphone", True)
    assert disabled(page, "#ba-mic-deviation") and not page.locator("#ba-mic-deviation").is_checked()
    request = brir_request(page)
    assert request["interactive_plots"] is True and request["microphone_deviation_correction"] is False, request
    disclosure(page, "headphone", False)
    assert page.locator("#ba-mic-deviation").is_checked(), "the choice returns when the lock lifts"
    disclosure(page, "headphone", True)

    adv_switch(page, "time", True)
    page.locator("#ba-decay-per-channel").check()
    assert disabled(page, "#ba-decay") and page.locator("#ba-decay-channels").is_visible()
    page.locator("#ba-decay-FL").fill("300")
    adv_switch(page, "output", True)
    page.locator("#adv-panel-output .file-opt", has_text="jamesdsp.wav").click()
    request = brir_request(page)
    assert request["decay"] == {"FL": 0.3} and request["head_ms"] == 1.0 and request["jamesdsp"] is True, request
    assert request["fs"] is None and request["output_truehd_layouts"] is False, request

    # A tab switched off keeps its values but sends none of them.
    adv_switch(page, "tone", False)
    assert page.locator("#adv-tab-tone").get_attribute("class").count("is-off") == 1
    assert page.locator("#ba-bass-gain").input_value() == "4"
    request = brir_request(page)
    assert not {"bass_boost_gain", "target_level", "channel_balance"} & request.keys(), request
    adv_switch(page, "tone", True)
    page.locator("#adv-reset-tone").click()
    assert page.locator("#ba-bass-gain").input_value() == "0" and page.locator("#ba-target-level").input_value() == ""
    assert page.locator("#ba-balance").input_value() == "none" and disabled(page, "#ba-bass-fc")
    page.locator("#ba-bass-gain").fill("3")
    adv_switch(page, "tone", False)

    # Arrow keys move between tabs.
    page.locator("#adv-tab-tone").focus()
    page.keyboard.press("ArrowRight")
    assert page.locator("#adv-tab-time").get_attribute("aria-selected") == "true"
    assert page.evaluate("document.activeElement.id") == "adv-tab-time"
    page.keyboard.press("End")
    assert page.locator("#adv-panel-correction").is_visible() and not page.locator("#adv-panel-time").is_visible()

    # Switching skins carries the values: tabs that are off hand Stable their defaults.
    navigate(page, "settings")
    page.locator("#sf-skin").select_option("stable")
    page.wait_for_function("document.documentElement.dataset.skin === 'stable'")
    if page.locator("#job-modal").is_visible():  # Stable shows the finished job in its dialog
        page.locator("#job-modal-close").click()
    navigate(page, "processing")
    assert "open" in page.locator("#dis-advanced").get_attribute("class")
    assert page.locator("#bf-jamesdsp").is_checked() and page.locator("#bf-bass-gain").input_value() == "0"
    assert page.locator("#bf-decay-per-channel").is_checked() and page.locator("#bf-decay-FL").input_value() == "300"
    page.locator("#bf-tilt").fill("1.5")
    navigate(page, "settings")
    page.locator("#sf-skin").select_option("studio")
    page.wait_for_function("document.documentElement.dataset.skin === 'studio'")
    navigate(page, "processing")
    assert page.locator("#adv-studio .seg-btn.is-off").count() == 0
    assert page.locator("#ba-tilt").input_value() == "1.5" and page.locator("#ba-jamesdsp").is_checked()
    assert not errors, errors
    context.close()

    # Every language fits: no tab strip or panel wider than the card.
    for language in ALL_LANGUAGES:
        context, page, errors = open_page(browser, "studio", "light", language, "idle")
        navigate(page, "processing")
        for tab in ADV_TABS:
            adv_switch(page, tab, True)
            if tab == "time":
                page.locator("#ba-decay-per-channel").check()
            overflow = page.evaluate("""() => [...document.querySelectorAll('#adv-studio, #adv-studio .adv-panel:not([hidden]) *')]
                .filter(n => !n.closest('.sr-only'))
                .filter(n => n.offsetParent && n.scrollWidth > n.clientWidth + 1 && getComputedStyle(n).overflowX !== 'visible'
                  || n.getBoundingClientRect && n.offsetParent && n.getBoundingClientRect().right > document.querySelector('#adv-studio').getBoundingClientRect().right + 1)
                .map(n => n.id || n.className || n.tagName)""")
            assert not overflow, (language, tab, overflow)
        assert page.locator("#adv-studio .seg").bounding_box()["height"] <= 42, language
        assert not errors, errors
        context.close()
    print("Gallery advanced checks: per-tab requests, locks, reset, keyboard, skin carry-over, 9-language layout OK", flush=True)


def open_eq(page):
    head = page.locator("#dis-eq > .disclosure-head")
    if "open" not in (page.locator("#dis-eq").get_attribute("class") or ""):
        head.click()
    page.locator("#bf-eq-slots .eq-slot").first.wait_for()


def render_gallery(output):
    from playwright.sync_api import sync_playwright

    output.mkdir(parents=True, exist_ok=True)
    shots = []
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch()
        behavior_checks(browser)
        advanced_checks(browser)
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
        for skin, theme, language, scenario in itertools.product(SKINS, THEMES, LANGUAGES, EQ_SCENARIOS):
            context, page, errors = open_page(browser, skin, theme, language, scenario)
            navigate(page, "processing")
            open_eq(page)
            page.wait_for_function("galleryCalls.some(c => c.method === 'inspect_eq')")
            if scenario == "eq-mixed":
                page.locator("#bf-eq-slots .eq-slot").nth(1).locator(".btn-link").first.click()
                page.locator('#bf-eq-slots .eq-slot[data-state="error"]').wait_for()
                page.locator("#bf-eq-slots .eq-slot").nth(2).locator(".btn-link").last.click()
                page.wait_for_function("galleryCalls.filter(c => c.method === 'inspect_eq').at(-1).args[0].eq_right_file === false")
                page.locator("#bf-eq-preview-label.eq-error").wait_for()
                assert page.locator("#bf-eq-preview").get_attribute("hidden") is not None
            else:
                page.locator("#bf-eq-preview polyline").first.wait_for()
            page.wait_for_timeout(50)
            shots.append(shoot(page, output, f"processing-eq-{skin}-{language}-{theme}-{scenario}", errors))
            context.close()
        for theme, language in itertools.product(THEMES, LANGUAGES):
            context, page, errors = open_page(browser, "studio", theme, language, "idle")
            navigate(page, "processing")
            disclosure(page, "headphone", True)
            adv_switch(page, "tone", True)
            page.locator("#ba-bass-gain").fill("4")
            page.locator("#ba-balance").select_option("mids")
            adv_switch(page, "time", True)
            page.locator("#ba-decay-per-channel").check()
            for channel, value in (("FL", "300"), ("FR", "300"), ("FC", "250")):
                page.locator(f"#ba-decay-{channel}").fill(value)
            adv_switch(page, "output", True)
            page.locator("#ba-jamesdsp").check()
            page.locator("#ba-resample").check()
            for tab in ADV_TABS:
                page.locator(f"#adv-tab-{tab}").click()
                page.locator("#adv-studio").scroll_into_view_if_needed()
                shots.append(shoot(page, output, f"processing-advanced-studio-{language}-{theme}-{tab}", errors))
            context.close()
        for theme, language in itertools.product(THEMES, LANGUAGES):
            context, page, errors = open_page(browser, "studio", theme, language, "update")
            navigate(page, "info")
            page.locator("#btn-check-updates").click()
            page.locator("#update-modal:not([hidden])").wait_for()
            notes = page.locator("#update-notes")
            assert notes.locator(".notes-heading").count() > 0 and notes.locator("li").count() > 0
            assert notes.locator("strong").count() > 0
            assert "**" not in notes.inner_text() and "## " not in notes.inner_text()
            shots.append(shoot(page, output, f"update-studio-{language}-{theme}", errors))
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
