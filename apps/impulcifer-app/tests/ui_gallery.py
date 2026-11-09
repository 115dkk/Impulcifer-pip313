"""Render the 3.x UI against deterministic IPC fixtures, without launching the app.

py -3.14 apps/impulcifer-app/tests/ui_gallery.py --out target/ui-gallery
Uses the installed Playwright Chromium. No browser download or real service calls.
"""
from __future__ import annotations

import argparse
import itertools
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SKINS = ("studio", "stable")
THEMES = ("dark", "light")
LANGUAGES = ("en", "ko")
VIEWS = ("settings", "info", "recorder")
RECOVERY_STATES = ("empty", "planning", "ready", "nothing", "error", "succeeded")
EQ_SCENARIOS = ("eq-folder", "eq-mixed")
ADV_TABS = ("tone", "time", "output", "correction")
ALL_LANGUAGES = ("en", "ko", "ja", "de", "es", "fr", "ru", "zh_CN", "zh_TW")
EXPECTED_SHOTS = (2 * 2 * 2 * 3 + 2 * 2 * 6 + 2 * 2 * 2 * 2 + 2 * 2 * 2 * 2 + 2 * 2 + 2 * 2 * (len(ADV_TABS) + 1)
                  + 2 * 2 * 2 + 2 * 2 * 2 + 2 * 2 + 2 * 2 * 2 * 2 + 2 * (2 * 2 * 2 * 2) + 3 * (2 * 2 * 2) + 2 * 2 + 2 * 2
                  + 2 * 2 * 2  # bundle 2: mixed and zero, English/Korean, dark/light
                  + 2 * 2  # bundle 3: lenient, ×2 and input high-pass, English/Korean, dark/light
                  + 2 * 2  # bundle 4: MIMO early interval and ear guard, English/Korean, dark/light
                  + 2)  # bundle 5: speaker polarity warnings, English/Korean, dark
# NHK 22.2 as bootstrap describes it (ADR 0007): SMPTE order, each channel's
# speaker code. 22.2's FL is the ±60° pair, Impulcifer's WL.
NHK_22_2 = [("FL", "WL"), ("FR", "WR"), ("FC", "FC"), ("LFE1", "LFE"), ("BL", "BL"), ("BR", "BR"),
            ("FLc", "FL"), ("FRc", "FR"), ("BC", "BC"), ("LFE2", "LFE2"), ("SiL", "SL"), ("SiR", "SR"),
            ("TpFL", "HFL"), ("TpFR", "HFR"), ("TpFC", "HFC"), ("TpC", "TC"), ("TpBL", "HBL"), ("TpBR", "HBR"),
            ("TpSiL", "TSL"), ("TpSiR", "TSR"), ("TpBC", "HBC"), ("BtFC", "DFC"), ("BtFL", "DFL"), ("BtFR", "DFR")]
IMMERSIVE = [{"id": "22.2", "name": "NHK 22.2", "file_name": "nhk_22.2.wav",
              "channels": [{"label": label, "slot": slot} for label, slot in NHK_22_2]}]
# 7.1 as bootstrap describes it: the Windows (WAVE) device order the sweep
# tracks are written in, and the order an HDMI output on macOS takes (ADR 0012).
TRACK_ORDERS = {"7.1": ["FL", "FR", "FC", "LFE", "BL", "BR", "SL", "SR"],
                "7.1.4": ["FL", "FR", "FC", "LFE", "BL", "BR", "SL", "SR", "TFL", "TFR", "TBL", "TBR"]}
HDMI_ORDERS = {"7.1": ["FL", "FR", "LFE", "FC", "SL", "SR", "BL", "BR"],
               "7.1.4": ["FL", "FR", "LFE", "FC", "SL", "SR", "BL", "BR", "TFL", "TFR", "TBL", "TBR"]}


def wrap(azimuth):
    wrapped = (azimuth + 180) % 360 - 180
    return 180.0 if wrapped == -180 else float(wrapped)


def guided_take(speakers, yaw, outputs):
    return {"speakers": speakers, "tracks": "stereo", "yaw": float(yaw),
            "sources": [{"speaker": s, "azimuth": wrap(outputs[i] - yaw), "output": i} for i, s in enumerate(speakers)]}


# bootstrap sweep.guided as the service sends it (impulcifer-types guided.rs): ITU angles,
# positive left; a step's yaw is the listener's turn, positive left.
STEREO_OUT, MONO_OUT = [30.0, -30.0], [0.0]
GUIDED = {
    "mono": {"outputs": MONO_OUT, "plans": {"7.1": [guided_take([s], -az, MONO_OUT) for s, az in
             [("FC", 0), ("FL", 30), ("FR", -30), ("SL", 90), ("SR", -90), ("BL", 135), ("BR", -135)]]}},
    "stereo": {"outputs": STEREO_OUT, "plans": {
        "7.1": [guided_take(["FL", "FR"], 0, STEREO_OUT), guided_take(["BR", "BL"], 180, STEREO_OUT),
                guided_take(["SL"], -60, STEREO_OUT), guided_take(["SR"], 120, STEREO_OUT),
                guided_take(["FC"], 30, STEREO_OUT)],
        "5.1": [guided_take(["FL", "FR"], 0, STEREO_OUT), guided_take(["BR", "BL"], 180, STEREO_OUT),
                guided_take(["FC"], 30, STEREO_OUT)],
        "9.1": [guided_take(["FL", "FR"], 0, STEREO_OUT), guided_take(["BR", "BL"], 180, STEREO_OUT),
                guided_take(["SL"], -60, STEREO_OUT), guided_take(["SR"], 120, STEREO_OUT),
                guided_take(["FC", "WR"], 30, STEREO_OUT), guided_take(["WL"], -30, STEREO_OUT)]}},
}
# bootstrap sweep.speaker_names: the 40 slot codes in slot order (ADR 0007), as
# crates/impulcifer-types/src/layouts.rs SLOTS lists them.
SPEAKER_NAMES = ["FL", "FR", "FC", "BL", "BR", "SL", "SR", "WL", "WR", "TFL", "TFR", "TSL", "TSR", "TBL", "TBR",
                 "FCL", "FCR", "SCL", "SCR", "SL1", "SR1", "SL2", "SR2", "BL1", "BR1", "BL2", "BR2", "BCL", "BCR",
                 "BC", "HFL", "HFR", "HFC", "HBL", "HBR", "HBC", "TC", "DFL", "DFR", "DFC"]
# bootstrap sweep.layout_speakers: every speaker a layout sweeps, without LFE.
LAYOUT_SPEAKERS = {"stereo": ["FL", "FR"], "7.1": ["FL", "FR", "FC", "BL", "BR", "SL", "SR"],
                   "7.1.4": ["FL", "FR", "FC", "BL", "BR", "SL", "SR", "TFL", "TFR", "TBL", "TBR"],
                   "22.2": [slot for _, slot in NHK_22_2 if not slot.startswith("LFE")]}
ADVANCED_KEYS = {"fs", "target_level", "channel_balance", "bass_boost_gain", "bass_boost_fc", "bass_boost_q", "tilt",
                 "decay", "head_ms", "itd_mode", "head_radius", "jamesdsp", "hangloose", "remove_silent_channels", "interactive_plots",
                 "microphone_deviation_correction", "mic_deviation_strength", "mic_deviation_debug_plots",
                 "output_truehd_layouts", "layout_files"}


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
                         "release": {"version": version, "notes": notes}, "immersive": IMMERSIVE,
                         "layoutSpeakers": LAYOUT_SPEAKERS, "speakerNames": SPEAKER_NAMES, "guided": GUIDED,
                         "trackOrders": TRACK_ORDERS, "hdmiOrders": HDMI_ORDERS,
                         "catalogs": {code: catalog(code) for code in LANGUAGES}}, ensure_ascii=False)
    return "const fixture = " + fixture + ";\n" + r"""
(() => {
  const {skin, theme, language, scenario, strings} = fixture;
  const calls = [];
  window.galleryCalls = calls;
  // WKWebView (macOS) answers window.confirm with false and drops alert and
  // prompt, so the screen must ask with its own dialog (askDialog). The mock
  // behaves the same way and records any call, which no check allows.
  window.galleryNativeDialogs = [];
  window.confirm = text => { window.galleryNativeDialogs.push(text); return false; };
  window.alert = text => { window.galleryNativeDialogs.push(text); };
  window.prompt = text => { window.galleryNativeDialogs.push(text); return null; };
  const mac = scenario === "darwin";
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
  const job = status => ({job_id: "gallery", kind, status, cancellable: kind === "recording" || kind === "brir",
    result: null, error: null});
  // "recording-running": the recording keeps running until it is cancelled.
  let cancelRequested = false;
  const settings = {skin, theme, language, strings, frontend: "webview", first_run: false,
    languages: [{code: "en", name: "English"}, {code: "ko", name: "한국어"}]};
  const methods = {
    bootstrap: () => ok({version: "3.0.0-alpha.1", platform: mac ? "darwin" : "windows", install_kind: "dev",
      webview_backend: mac ? "cocoa" : "edgechromium",
      brir_defaults: {itd_mode: "off", head_radius: null}, sweep: {layouts: ["stereo", "7.1", "7.1.4", "22.2"], default_fs: 48000, default_duration: 5,
        speaker_names: fixture.speakerNames, layout_speakers: fixture.layoutSpeakers, immersive: fixture.immersive,
        guided: fixture.guided, track_orders: fixture.trackOrders, hdmi_orders: fixture.hdmiOrders},
      capabilities: {recording: true, brir: true, output_recovery: true, recording_cancel: false,
        brir_cancel: true, output_recovery_cancel: false, share_modes: scenario === "disabled" ? ["auto"] : ["auto", "exclusive", "shared"]},
      active_job: null, ui: settings}),
    list_audio_devices: () => ok(mac
      ? {host_apis: ["CoreAudio"], default_input_index: 1, default_output_index: 0,
        devices: [{index: 0, name: "DENON-AVR", host_api: "CoreAudio", max_input_channels: 0, max_output_channels: 8, hdmi: true},
          {index: 1, name: "Binaural microphones", host_api: "CoreAudio", max_input_channels: 2, max_output_channels: 0, hdmi: false},
          {index: 2, name: "Scarlett 18i20 USB", host_api: "CoreAudio", max_input_channels: 18, max_output_channels: 20, hdmi: false}]}
      : {host_apis: ["Windows WASAPI"], default_input_index: 1, default_output_index: 0,
        devices: [{index: 0, name: "Speakers (USB Audio)", host_api: "Windows WASAPI", max_input_channels: 0, max_output_channels: 2, hdmi: false},
          {index: 1, name: "Binaural microphones (USB Audio)", host_api: "Windows WASAPI", max_input_channels: 2, max_output_channels: 0, hdmi: false}]}),
    get_system_info: () => ok({version: "3.0.0-alpha.1", install_kind: "dev", os: "Windows 11", cpu_count: 16,
      runtime: {language: "Rust", toolchain: "rustc 1.99.0", audio_backend: "WASAPI", shell: "Tauri 2.11.5", webview: "WebView2 152.0.4191.66"},
      paths: {data_dir: "C:/Impulcifer/data", settings_path: "C:/Users/Listener/.impulcifer/settings.json"}, update_channel: "prerelease"}),
    get_ui_settings: () => ok(settings),
    set_language: code => ok({...settings, language: code, strings: fixture.catalogs[code]}),
    set_skin: skin => ok({skin}), set_theme: theme => ok({theme}), set_frontend: frontend => ok({frontend}),
    resolve_recording_paths: (dir, play, mode, sweep) =>
      ok({record_path: `${dir}/${mode === "headphones" ? "headphones" : sweep ? sweep.speakers : "FL,FR"}.wav`}),
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
      // An 8-channel DENON with a 7.1.4 sweep of its height speakers: the service asks first.
      const speakers = String(request.sweep?.speakers || "").split(",");
      if (mac && !request.confirm_warnings && !request.output_device && request.sweep?.tracks === "7.1.4"
          && speakers.includes("TFL")) {
        return Promise.resolve(error("CONFIRMATION_REQUIRED", "DENON-AVR has 8 output channels",
          {warning: "output_channels_short", device: "DENON-AVR", available: 8, required: 12, first: 9, last: 12,
           missing: "TFL, TFR, TBL, TBR", layout: "7.1.4"}));
      }
      lastRequest = request; kind = "recording";
      if (scenario === "unavailable") return Promise.resolve(error("INVALID_REQUEST", "backend cannot fix mode",
        {kind: "share_mode_unavailable", share_mode: request.share_mode, backend: "cpal"}));
      return ok({job: job("running")});
    },
    start_brir: () => {kind = "brir"; return ok({job: job("running")});},
    poll_job: () => {
      if (kind === "recording" && scenario === "recording-running") {
        return ok({job: job(cancelRequested ? "cancelled" : "running"), events: [], next_seq: 1});
      }
      if (kind === "recording") {
        if (scenario === "refused") return ok({job: {...job("failed"), error: error("DEVICE_ERROR", "device refused exclusive",
          {kind: "share_mode_refused", share_mode: "exclusive", reason: "AUDCLNT_E_DEVICE_IN_USE"}).error}, events: [], next_seq: 1});
        const share = {requested: lastRequest.share_mode, output: "exclusive", input: "shared_auto_convert"};
        // A guided step that turned too far: the rear pair's BR reads 170° instead of 150°.
        const checks = scenario === "polarity"
          ? [{key: "record_check_speaker_polarity", speaker: "SL", severity: "warning",
              values: {compared: 3, opposite: 3, confidence: "0.78"}},
             {key: "record_check_speaker_polarity_split", speaker: "FL", severity: "warning",
              values: {group_a: "FL", group_b: "FC", confidence: "1.00"}}]
          : (lastRequest.expected_azimuths || {}).BR === undefined ? []
            : [{key: "record_check_angle_too_far", speaker: "BR", severity: "warning",
                values: {itd_us: 89, measured: 170, expected: 150}}];
        return ok({job: {...job("succeeded"), result: {mode: lastRequest.mode,
          record_path: `${lastRequest.record_dir}/${lastRequest.mode === "headphones" ? "headphones" : lastRequest.sweep?.speakers || "FL,FR"}.wav`,
          summary: null,
          sweep: null, sidecar_path: null, share, checks,
          measurements: Object.entries(lastRequest.expected_azimuths || {}).map(([speaker, azimuth]) => ({
            speaker, peak_db: -12, snr_db: 60, itd_us: Math.sign(azimuth) * 250, lf_db: -20, lf_ild_db: -6.5, azimuth,
            ...(lastRequest.mode === "speakers" ? {direct: {fs: 48000,
              left: [0, azimuth / 180, -0.25], right: [0, 0.125, -azimuth / 180]}} : {})}))}},
          events: [{seq: 1, timestamp_ms: 0, type: "log",
            payload: {level: "info", key: "recording_share_mode_opened", message: "untranslated fixture", share}}], next_seq: 1});
      }
      if (kind === "brir") {
        const status = window.galleryBrirStatus || "succeeded";
        const alignment = {mode: "off", yaw_deg: 4.4, common_offset_us: -20,
          input_balance_db: -6.5, headphone_balance_db: 7.0, predicted_balance_db: -13.5,
          hf_asymmetry_db: 2.1, head_radius_cm: 11.2, head_radius_source: "estimated",
          speakers: [["FL", 288, 334, -7.3, 0.4], ["FR", -309, -334, -6.5, -9.2],
            ["FC", -25, 0, -6.2, -4.4], ["BL", 329, 487, -6.7, -1.2], ["BR", -327, -487, -6.4, -7.6],
            ["SL", 882, 839, -6.9, 1.0], ["SR", -874, -839, -6.1, -9.8]]
            .map(([speaker, measured_us, model_us, lf_ild_db, hf_ild_db]) => ({speaker, measured_us, model_us,
              after_us: measured_us, target_us: null, applied_us: null, lf_ild_db, hf_ild_db, skipped: false})),
          pairs: [{left: "FL", right: "FR", sum_us: -21, yaw_deg: -0.9},
            {left: "BL", right: "BR", sum_us: 2, yaw_deg: 4.4},
            {left: "SL", right: "SR", sum_us: 8, yaw_deg: null}]};
        return ok({job: {...job(status), result: status === "succeeded" ? {output_path: directory, alignment} : null},
          events: [], next_seq: 1});
      }
      const result = {...meta, created_files: plan(lastRequest).planned_files.map(file => file.path)};
      written = true;
      return ok({job: {...job("succeeded"), result}, events: [], next_seq: 1});
    },
    cancel_job: () => { cancelRequested = true; return ok({job: job("cancelled")}); },
    check_for_updates: () => scenario === "update"
      ? ok({update_available: true, current_version: "3.0.0-alpha.1", latest_version: fixture.release.version,
        download_url: "https://example.invalid/Impulcifer-win-Setup.exe", release_notes: fixture.release.notes, release_url: null})
      : ok({update_available: false, current_version: "3.0.0-alpha.1", latest_version: "3.0.0-alpha.1",
        download_url: null, release_notes: null, release_url: null}),
    start_update: () => {kind = "update"; return ok({job: job("running")});},
    apply_pending_update: () => ok({restarting: true}),
    select_directory: () => ok({path: directory}),
    select_file: kind => ok({path: kind === "text" ? "D:/EQ presets/HD800S left.csv"
      : kind === "measurement" ? "D:/Measurements/living room.mdat" : null}),
    // ADR 0010 and 0011: a REW file like the io crate's fixture (two impulse responses and a
    // frequency-response-only measurement) and a Dirac Live 4.2 project, each assigned for the
    // request's purpose the way the io crate's automatic rules do. A Dirac path with "single"
    // has only position 1.
    inspect_room_measurements: request => {
      const path = request.path;
      const purpose = request.purpose || "room";
      if (path.includes("missing")) return Promise.resolve(error("FILE_NOT_FOUND", "file not found", {path}));
      if (path.includes("broken")) return Promise.resolve(error("INVALID_REQUEST", "not a REW measurement file", {path}));
      // A path with "slow" answers after 1.5 s, so a check can act while it is pending.
      const answer = response => path.includes("slow")
        ? new Promise(resolve => setTimeout(() => resolve(response), 1500)) : response;
      if (path.endsWith(".liveproject")) {
        const names = ["Left", "Right", "LeftSurround", "RightSurround", "Subwoofer", "Subwoofer"];
        const slots = ["FL", "FR", "SL", "SR", null, null];
        const positions = path.includes("single") ? [1] : [1, 2, 3, 4, 5, 6, 7, 8, 9];
        // "partial": the Right speaker was not measured at position 2 (no matrix element).
        const missing = path.includes("partial") ? new Set(["2@2"]) : new Set();
        const right = purpose === "room" ? 1 : positions[1] || positions[0];
        const mapping = purpose === "headphones"
          ? [{slot: "FL", side: "left", id: "1@1"}, {slot: "FR", side: "right", id: `2@${right}`}]
          : slots.flatMap((slot, i) => slot ? [{slot, side: "left", id: `${i + 1}@1`}, {slot, side: "right", id: `${i + 1}@${right}`}] : [])
            .filter(entry => !missing.has(entry.id));
        const map = purpose === "headphones" ? `left=1@1,right=2@${right}`
          : purpose === "brir" ? mapping.map(e => `${e.slot}.${e.side}=${e.id}`).join(",")
          : "FL=1@1,FR=2@1,SL=3@1,SR=4@1";
        return answer(ok({format: "dirac", path, title: "Living room", sample_rate: 48000,
          microphone: "Umik-1  Gain: 18dB", mic_calibration: true, positions,
          speakers: names.map((name, i) => ({index: i + 1, name, subwoofer: i >= 4, slot: slots[i]})),
          measurements: positions.flatMap(position => names.map((name, i) => ({id: `${i + 1}@${position}`, name,
            speaker: i + 1, position, samples: 32769, skipped: null}))).filter(m => !missing.has(m.id)),
          mapping, map, auto: true, purpose}));
      }
      return ok({format: "rew", path, title: null, sample_rate: 48000, microphone: null, mic_calibration: false,
        positions: [], speakers: [],
        measurements: [
          {id: "1", name: "FL left", speaker: null, position: null, samples: 8192, skipped: null},
          {id: "2", name: "FL 오른쪽 🎧", speaker: null, position: null, samples: 8192, skipped: null},
          {id: "3", name: "FR only", speaker: null, position: null, samples: 0, skipped: "no_impulse_response"},
          {id: "4", name: "secs aver", speaker: null, position: null, samples: 131072, skipped: null}],
        mapping: purpose === "headphones"
          ? [{slot: "FL", side: "left", id: "1"}, {slot: "FR", side: "right", id: "2"}]
          : [{slot: "FL", side: "left", id: "1"}, {slot: "FL", side: "right", id: "2"}],
        map: purpose === "headphones" ? "left=1,right=2" : "FL.left=1,FL.right=2", auto: true, purpose});
    },
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


def answer(page, ok=True):
    """Answer the screen's own dialog (askDialog) and return its message."""
    page.locator("#ask-modal:not([hidden])").wait_for()
    message = page.locator("#ask-modal-message").inner_text()
    page.locator("#ask-modal-ok" if ok else "#ask-modal-cancel").click()
    # Closed, or already showing the next dialog (Stable tells a refusal in a notice).
    page.wait_for_function("text => document.querySelector('#ask-modal').hidden"
                           " || document.querySelector('#ask-modal-message').innerText !== text", arg=message)
    return message


def native_dialogs(page):
    return page.evaluate("galleryNativeDialogs.length")


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
        confirmation = answer(page)
        page.wait_for_function("galleryCalls.some(c => c.method === 'start_recording')")
        assert page.evaluate("galleryCalls.find(c => c.method === 'start_recording').args[0].share_mode") == "shared"
        assert "{play_file}" not in confirmation and "{share_mode}" not in confirmation
        assert native_dialogs(page) == 0
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


def dialog_checks(browser):
    """WKWebView refuses window.confirm, which left the record buttons dead on
    macOS. The screen's own dialog keeps focus, cancels on Escape and returns
    focus to the button, and only OK starts the recording."""
    context, page, errors = open_page(browser, "studio", "dark", "en", "darwin")
    navigate(page, "recorder")
    starts = "galleryCalls.filter(c => c.method === 'start_recording').length"
    button = page.locator("#btn-record-headphones")
    button.click()
    page.locator("#ask-modal:not([hidden])").wait_for()
    assert page.locator("#ask-modal [role=alertdialog]").get_attribute("aria-modal") == "true"
    assert page.evaluate("document.activeElement.id") == "ask-modal-ok"
    page.keyboard.press("Tab")
    assert page.evaluate("document.activeElement.id") == "ask-modal-cancel"
    page.keyboard.press("Tab")
    assert page.evaluate("document.activeElement.id") == "ask-modal-ok"
    page.keyboard.press("Escape")
    page.locator("#ask-modal").wait_for(state="hidden")
    assert page.evaluate("document.activeElement.id") == "btn-record-headphones"
    button.click()
    answer(page, ok=False)
    assert page.evaluate(starts) == 0
    page.locator("#btn-start-recording").click()
    message = answer(page)
    assert "{" not in message and message.strip(), message
    page.wait_for_function(f"{starts} === 1")
    assert native_dialogs(page) == 0
    assert not errors, errors
    context.close()
    print("Gallery dialog checks: own dialog, focus, Escape, Cancel, OK OK", flush=True)


def channel_order_checks(browser):
    """On a Mac an HDMI or DisplayPort output takes LFE on 3 and FC on 4, the
    side pair on 5-6 and the back pair on 7-8 (ADR 0012). The choice shows on
    macOS for layouts with an HDMI order; auto follows the output's hdmi flag,
    the device map shows the order the recording will use, and the request
    carries the choice."""
    standard, hdmi = TRACK_ORDERS["7.1"], HDMI_ORDERS["7.1"]
    context, page, errors = open_page(browser, "studio", "dark", "en", "idle")
    navigate(page, "recorder")
    page.locator("#rf-sweep-layout").select_option("7.1")
    # No channel order on Windows; the map shows here only for the 2-channel
    # output's shortage warning (short_output_checks).
    assert page.locator("#rf-channel-order-row").is_hidden()
    assert page.locator("#rf-layout-row").is_visible() and page.locator("#rf-channel-shortage").is_visible()
    page.locator("#btn-start-recording").click()
    answer(page)
    page.wait_for_function("galleryCalls.some(c => c.method === 'start_recording')")
    assert page.evaluate("galleryCalls.find(c => c.method === 'start_recording').args[0].channel_order") == "auto"
    assert not errors, errors
    context.close()

    strings = catalog("en")
    context, page, errors = open_page(browser, "studio", "dark", "en", "darwin")
    navigate(page, "recorder")
    row, auto = page.locator("#rf-channel-order-row"), page.locator("#rf-channel-order-auto")
    slots = lambda: page.locator("#rf-layout-map .layout-ch-label").all_inner_texts()
    assert row.is_hidden()
    page.locator("#rf-sweep-layout").select_option("7.1")
    assert row.is_visible() and page.locator("#rf-layout-row").is_visible()
    assert slots() == hdmi, slots()
    assert page.locator('#rf-layout-map .layout-ch[data-lfe="true"] .layout-ch-num').inner_text() == "3"
    assert auto.inner_text() == strings["hint_channel_order_auto_hdmi"]
    page.locator("#rf-output-device").select_option("Scarlett 18i20 USB")
    assert slots() == standard, slots()
    assert auto.inner_text() == strings["hint_channel_order_auto_standard"]
    page.locator("#rf-channel-order").select_option("hdmi")
    assert auto.is_hidden() and slots() == hdmi
    page.locator("#rf-channel-order").select_option("standard")
    assert slots() == standard
    page.locator("#rf-channel-order").select_option("hdmi")
    page.locator("#btn-start-recording").click()
    answer(page)
    page.wait_for_function("galleryCalls.some(c => c.method === 'start_recording')")
    request = page.evaluate("galleryCalls.find(c => c.method === 'start_recording').args[0]")
    assert request["channel_order"] == "hdmi" and request["sweep"]["tracks"] == "7.1", request
    page.locator("#rf-sweep-layout").select_option("stereo")
    assert row.is_hidden() and page.locator("#rf-layout-row").is_hidden()
    page.locator("#rf-sweep-source").select_option("file")
    play = page.locator("#rf-play")
    play.fill("D:/Sweeps/sweep-seg-FL,FR,FC,BL,BR,SL,SR-7.1-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")
    assert row.is_visible()
    play.fill("D:/Sweeps/sweep-seg-FL,FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")
    assert row.is_hidden()
    assert native_dialogs(page) == 0
    assert not errors, errors
    context.close()
    print("Gallery channel order checks: Windows hidden, macOS auto by HDMI flag, map order, request OK", flush=True)


def short_output_checks(browser):
    """An output with fewer channels than the layout still records (the
    maintainer chose a warning over a block): the note names the device and the
    swept speakers that will not play, the device map strikes the channels the
    device lacks, the record button carries the warning and stays enabled, and
    the confirmation repeats it. Speakers within the device bring no warning."""
    context, page, errors = open_page(browser, "studio", "dark", "en", "darwin")
    navigate(page, "recorder")
    note, record = page.locator("#rf-channel-shortage"), page.locator("#btn-start-recording")
    struck = lambda: page.locator('#rf-layout-map .layout-ch[data-missing="true"] .layout-ch-slot').all_inner_texts()
    assert note.is_hidden() and record.get_attribute("data-warn") is None
    page.locator("#rf-sweep-layout").select_option("7.1.4")
    text = note.inner_text()
    assert note.is_visible() and "DENON-AVR" in text and "TFL, TFR, TBL, TBR" in text and "9" in text and "12" in text, text
    assert struck() == ["TFL", "TFR", "TBL", "TBR"], struck()
    assert record.get_attribute("data-warn") == "true" and record.is_enabled()
    record.click()
    answer(page)  # the setup confirmation
    # The service asks for the short output (confirm_warnings), worded like the note.
    assert answer(page) == text
    page.wait_for_function("galleryCalls.filter(c => c.method === 'start_recording').length === 2")
    assert page.evaluate("galleryCalls.filter(c => c.method === 'start_recording').at(-1).args[0].confirm_warnings") is True
    page.wait_for_function("!document.querySelector('#btn-start-recording').disabled")
    page.locator("#rf-sweep-speakers").fill("FL,FR")
    assert note.is_hidden() and record.get_attribute("data-warn") is None
    assert struck() == ["TFL", "TFR", "TBL", "TBR"]  # the map still shows what the device lacks
    page.locator("#rf-layout-use-all").click()
    assert note.is_visible()
    page.locator("#rf-output-device").select_option("Scarlett 18i20 USB")
    assert note.is_hidden() and struck() == []
    assert native_dialogs(page) == 0 and not errors, errors
    context.close()

    # Windows, a stereo output and 7.1: the map appears for the warning alone.
    context, page, errors = open_page(browser, "studio", "dark", "ko", "idle")
    navigate(page, "recorder")
    page.locator("#rf-sweep-layout").select_option("7.1")
    text = page.locator("#rf-channel-shortage").inner_text()
    assert "Speakers (USB Audio)" in text and "FC, BL, BR, SL, SR" in text, text
    assert page.locator("#rf-layout-row").is_visible()
    assert struck() == ["FC", "—", "BL", "BR", "SL", "SR"], struck()
    assert not errors, errors
    context.close()
    print("Gallery short output checks: note, struck channels, warned record button, confirmation, quiet within the device OK", flush=True)


def recording_cancel_checks(browser):
    """A running recording can be cancelled: from the page header in Studio,
    from the job dialog in Stable; the status then says nothing was saved."""
    strings = catalog("en")
    status = ("[...document.querySelectorAll('[data-rec-status]')]"
              ".some(node => node.textContent.includes(%s))" % json.dumps(strings["recording_status_cancelled"]))
    context, page, errors = open_page(browser, "studio", "dark", "en", "recording-running")
    navigate(page, "recorder")
    cancel = page.locator("#btn-cancel-recording")
    assert cancel.is_hidden()
    page.locator("#btn-record-headphones").click()
    answer(page)
    cancel.wait_for(state="visible")
    cancel.click()
    page.wait_for_function("galleryCalls.some(c => c.method === 'cancel_job')")
    page.wait_for_function(status)
    cancel.wait_for(state="hidden")
    assert not errors, errors
    context.close()

    context, page, errors = open_page(browser, "stable", "dark", "en", "recording-running")
    navigate(page, "recorder")
    page.locator("#btn-record-headphones").click()
    answer(page)
    page.locator("#job-modal-cancel").wait_for(state="visible")
    assert page.locator("#btn-cancel-recording").is_hidden()
    page.locator("#job-modal-cancel").click()
    page.wait_for_function("galleryCalls.some(c => c.method === 'cancel_job')")
    page.wait_for_function(status)
    assert not errors, errors
    context.close()
    print("Gallery recording cancel checks: Studio header, Stable dialog, cancelled status OK", flush=True)


def layout_checks(browser):
    """Immersive layouts (ADR 0007): the recorder shows which speaker code each
    device channel plays; choosing any layout fills in its speakers, which name
    the recording; the output options send
    layout_files as auto, none or the formats forced with missing speakers."""
    context, page, errors = open_page(browser, "studio", "dark", "en", "idle")
    navigate(page, "recorder")
    assert not page.locator("#rf-layout-row").is_visible()
    layout = page.locator("#rf-sweep-layout")
    assert layout.locator('option[value="22.2"]').inner_text() == "NHK 22.2"
    layout.select_option("22.2")
    assert page.locator("#rf-layout-row").is_visible()
    items = page.locator("#rf-layout-map .layout-ch")
    assert items.count() == len(NHK_22_2)
    first = items.nth(0)
    assert [first.locator(cls).inner_text() for cls in (".layout-ch-num", ".layout-ch-label", ".layout-ch-slot")] == ["1", "FL", "WL"]
    assert items.nth(3).get_attribute("data-lfe") == "true" and items.nth(3).locator(".layout-ch-slot").inner_text() == "—"
    assert items.nth(6).locator(".layout-ch-slot").inner_text() == "FL"
    # Choosing a layout fills Speakers with all of it; the button restores that after an edit.
    speakers = page.locator("#rf-sweep-speakers")
    expected = ",".join(LAYOUT_SPEAKERS["22.2"])
    assert speakers.input_value() == expected
    speakers.fill("WL,WR")
    page.locator("#rf-layout-use-all").click()
    assert speakers.input_value() == expected
    # 7.1 has no channel map of its own (the mock's 2-channel output shows one only
    # with the shortage warning), and still names the recording after its seven speakers.
    layout.select_option("7.1")
    assert page.locator("#rf-layout-row").is_visible() == page.locator("#rf-channel-shortage").is_visible()
    assert speakers.input_value() == "FL,FR,FC,BL,BR,SL,SR"
    page.wait_for_function("document.getElementById('rf-resolved-path').textContent.includes('FL,FR,FC,BL,BR,SL,SR.wav')")
    layout.select_option("stereo")
    assert not page.locator("#rf-layout-row").is_visible()
    assert speakers.input_value() == "FL,FR"

    navigate(page, "processing")
    assert "layout_files" not in brir_request(page)
    adv_switch(page, "output", True)
    assert brir_request(page)["layout_files"] == "auto"
    # Each mode explains what it writes; the format list opens only when the
    # user asks for formats with speakers missing, and its picks count only there.
    row = page.locator("#ba-layout-files-row")
    pick = row.locator(".layout-force-pick")
    mode = page.locator("#ba-layout-files")
    assert row.locator('[data-layout-mode="auto"]').is_visible() and not pick.is_visible()
    mode.select_option("listed")
    assert pick.is_visible() and not row.locator('[data-layout-mode="auto"]').is_visible()
    assert brir_request(page)["layout_files"] == "auto"
    pick.locator(".file-opt", has_text="atmos_24.1.10.wav").click()
    pick.locator(".file-opt", has_text="dtsx_30.2.wav").click()
    assert brir_request(page)["layout_files"] == "24.1.10,30.2"
    mode.select_option("auto")
    assert not pick.is_visible() and brir_request(page)["layout_files"] == "auto"
    mode.select_option("none")
    assert row.locator('[data-layout-mode="none"]').is_visible() and not pick.is_visible()
    assert disabled(page, "#ba-layout-force-5")
    assert brir_request(page)["layout_files"] == "none"
    page.locator("#adv-reset-output").click()
    assert mode.input_value() == "auto" and not pick.is_visible()
    assert not page.locator("#ba-layout-force-5").is_checked()
    assert brir_request(page)["layout_files"] == "auto"
    assert not errors, errors
    context.close()
    print("Gallery layout checks: recorder channel map, layout speakers, use-all, layout_files auto/none/forced OK", flush=True)


def fill_lfe_checks(browser):
    """ADR 0013: what the output files carry for unmeasured speakers and the LFE
    channel. Studio sends both only with the Output tab on, Stable always; each
    LFE choice shows its own explanation; reset returns the defaults."""
    context, page, errors = open_page(browser, "studio", "dark", "en", "idle")
    navigate(page, "processing")
    request = brir_request(page)
    assert "missing_speakers" not in request and "lfe_mode" not in request, request
    assert "itd_mode" not in request and "head_radius" not in request, request
    adv_switch(page, "time", True)
    request = brir_request(page)
    assert request["itd_mode"] == "off" and request["head_radius"] is None, request
    page.locator("#ba-itd-mode").select_option("model")
    page.locator("#ba-head-radius").fill("9")
    request = brir_request(page)
    assert request["itd_mode"] == "model" and request["head_radius"] == 9, request
    assert page.locator('#ba-itd-mode-row [data-itd-mode="model"]').is_visible()
    assert not page.locator('#ba-itd-mode-row [data-itd-mode="off"]').is_visible()
    page.locator("#ba-itd-mode").select_option("symmetric")
    assert page.locator('#ba-itd-mode-row [data-itd-mode="symmetric"]').is_visible()
    assert brir_request(page)["itd_mode"] == "symmetric"
    adv_switch(page, "time", False)
    request = brir_request(page)
    assert "head_ms" not in request and "itd_mode" not in request and "head_radius" not in request, request
    assert page.locator("#ba-head-radius").input_value() == "9"
    adv_switch(page, "time", True)
    page.locator("#adv-reset-time").click()
    assert page.locator("#ba-itd-mode").input_value() == "off" and page.locator("#ba-head-radius").input_value() == ""
    assert page.locator('#ba-itd-mode-row [data-itd-mode="off"]').is_visible()
    request = brir_request(page)
    assert request["itd_mode"] == "off" and request["head_radius"] is None, request
    adv_switch(page, "output", True)
    request = brir_request(page)
    assert request["missing_speakers"] == "silent" and request["lfe_mode"] == "none", request
    row = page.locator("#ba-lfe-mode-row")
    hints = lambda: [mode for mode in ("none", "center", "all")
                     if row.locator(f'[data-lfe-mode="{mode}"]').is_visible()]
    assert hints() == ["none"]
    page.locator("#ba-missing-speakers").select_option("neighbor")
    page.locator("#ba-lfe-mode").select_option("all")
    assert hints() == ["all"]
    request = brir_request(page)
    assert request["missing_speakers"] == "neighbor" and request["lfe_mode"] == "all", request
    page.locator("#ba-lfe-mode").select_option("center")
    assert hints() == ["center"] and brir_request(page)["lfe_mode"] == "center"
    page.locator("#adv-reset-output").click()
    assert page.locator("#ba-missing-speakers").input_value() == "silent"
    assert page.locator("#ba-lfe-mode").input_value() == "none" and hints() == ["none"]
    assert not errors, errors
    context.close()

    context, page, errors = open_page(browser, "stable", "dark", "ko", "idle")
    navigate(page, "processing")
    disclosure(page, "advanced", True)
    payload = page.evaluate("gatherBrirPayload()")
    assert payload["missing_speakers"] == "silent" and payload["lfe_mode"] == "none", payload
    assert payload["itd_mode"] == "off" and payload["head_radius"] is None, payload
    page.locator("#bf-itd-mode").select_option("model")
    page.locator("#bf-head-radius").fill("9")
    payload = page.evaluate("gatherBrirPayload()")
    assert payload["itd_mode"] == "model" and payload["head_radius"] == 9, payload
    assert page.locator('#bf-itd-mode-row [data-itd-mode="model"]').is_visible()
    assert not page.locator('#bf-itd-mode-row [data-itd-mode="off"]').is_visible()
    page.locator("#bf-missing-speakers").select_option("neighbor")
    page.locator("#bf-lfe-mode").select_option("all")
    assert page.locator('#bf-lfe-mode-row [data-lfe-mode="all"]').is_visible()
    assert not page.locator('#bf-lfe-mode-row [data-lfe-mode="none"]').is_visible()
    payload = page.evaluate("gatherBrirPayload()")
    assert payload["missing_speakers"] == "neighbor" and payload["lfe_mode"] == "all", payload
    assert not errors, errors
    context.close()
    print("Gallery fill/LFE checks: Studio sends only with Output on, Stable always, LFE hints per choice, reset OK", flush=True)


def itd_checks(browser):
    """ADR 0014: report lifecycle, service defaults and folder-scoped references,
    including headphone requests without replacing the speaker references."""
    context, page, errors = open_page(browser, "studio", "dark", "en", "idle")
    navigate(page, "processing")
    page.evaluate("state.brirDefaults = {itd_mode: 'model', head_radius: 9}; applyTimeDefaults(); applyStrings()")
    for prefix in ("bf", "ba"):
        assert page.locator(f"#{prefix}-itd-mode").input_value() == "model"
        assert page.locator(f"#{prefix}-head-radius").input_value() == "9"
    adv_switch(page, "time", True)
    page.locator("#ba-itd-mode").select_option("symmetric")
    page.locator("#ba-head-radius").fill("10")
    page.locator("#adv-reset-time").click()
    request = brir_request(page)
    assert request["itd_mode"] == "model" and request["head_radius"] == 9, request
    page.evaluate("state.brirDefaults = {}; applyTimeDefaults(); applyStrings()")
    assert page.locator("#ba-itd-mode").input_value() == "off"
    assert page.locator("#ba-head-radius").input_value() == ""
    page.locator("#brir-alignment:not([hidden])").wait_for()
    assert page.locator("#brir-alignment-rows tr").count() == 7
    assert page.locator("#brir-alignment .align-flag.warn").count() == 3
    assert "-13.5 dB" in page.locator("#brir-alignment-summary").inner_text()
    page.evaluate("window.galleryBrirStatus = 'running'")
    page.locator("#btn-generate-brir").click()
    page.wait_for_function("state.jobId !== null && document.querySelector('#brir-alignment').hidden")
    page.evaluate("window.galleryBrirStatus = 'succeeded'")
    page.locator("#brir-alignment:not([hidden])").wait_for()
    for status in ("failed", "cancelled"):
        page.evaluate("status => { window.galleryBrirStatus = status; }", status)
        page.locator("#btn-generate-brir").click()
        page.wait_for_function("status => state.lastJob?.status === status && state.jobId === null", arg=status)
        assert page.locator("#brir-alignment").is_hidden()

    # Missing older fields become null; headphone measurements never replace
    # the session's speaker references, and a different folder sees none.
    page.evaluate("""() => {
      const direct = {fs: 48000, left: [0, 0.5, -0.25], right: [0, 0.25, -0.125]};
      const measurements = [{speaker: 'FL', peak_db: -12, snr_db: 60, itd_us: 250, lf_ild_db: -6.5, direct},
        {speaker: 'FR', peak_db: -12, snr_db: 60, itd_us: -250}];
      // A retake replaces the old window, including clearing it when absent.
      rememberMeasurements({mode: 'speakers', record_dir: 'data/my_hrir',
        measurements: measurements.map(item => ({...item, direct: {fs: 48000, left: [1], right: [-1]}}))});
      rememberMeasurements({mode: 'speakers', record_dir: 'data/my_hrir', measurements});
      rememberMeasurements({mode: 'headphones', record_dir: 'data/my_hrir',
        measurements: [{...measurements[0], lf_ild_db: 7, direct: null}]});
    }""")
    refs = page.evaluate("gatherRecordingPayload('headphones').reference_measurements")
    assert [(item["speaker"], item["lf_ild_db"]) for item in refs] == [("FL", -6.5), ("FR", None)], refs
    assert refs[0]["direct"] == {"fs": 48000, "left": [0, 0.5, -0.25], "right": [0, 0.25, -0.125]}, refs
    assert refs[1]["direct"] is None, refs
    assert page.evaluate("gatherRecordingPayload('speakers').reference_measurements") == refs
    assert page.evaluate("state.takeMeasurements[folderKey('data/my_hrir')][1].lf_ild_db") is None
    assert page.evaluate("state.takeMeasurements[folderKey('data/my_hrir')][1].direct") is None
    assert "direct" not in page.evaluate("gatherRecordingPayload('headphones')")
    navigate(page, "recorder")
    page.locator("#btn-record-headphones").click()
    answer(page)
    page.wait_for_function("state.lastJob?.kind === 'recording' && state.jobId === null")
    sent = page.evaluate("galleryCalls.filter(c => c.method === 'start_recording').at(-1).args[0]")
    assert sent["mode"] == "headphones" and sent["reference_measurements"] == refs, sent
    page.locator("#rf-record-dir").fill("data/other_hrir")
    assert page.evaluate("gatherRecordingPayload('headphones').reference_measurements") == []
    assert page.evaluate("gatherRecordingPayload('speakers').reference_measurements") == []
    page.locator("#rf-record-dir").fill("data/my_hrir")
    assert page.evaluate("gatherRecordingPayload('speakers').reference_measurements") == refs
    assert not errors, errors
    context.close()
    print("Gallery ITD checks: defaults, reset, report success/start/failure/cancel, warning badges, headphone references OK", flush=True)


def guided_checks(browser):
    """Head-turn measurement: each step's turn, ear and diagram; a step records
    its speakers with the angles it expects after a countdown; a clean step moves
    on, a warned one stays with the service's warning; the countdown cancels."""
    context, page, errors = open_page(browser, "studio", "dark", "en", "idle")
    navigate(page, "recorder")
    assert not page.locator("#gd-body").is_visible()
    page.locator("#gd-setup").select_option("stereo")
    # 7.1 stays the default target although 9.1 comes after it.
    assert page.locator("#gd-target option").all_text_contents() == ["5.1", "7.1", "9.1"]
    assert page.locator("#gd-target").input_value() == "7.1"
    page.locator("#gd-target").select_option("9.1")
    assert page.locator("#gd-steps .gd-step").count() == 6
    page.locator("#gd-target").select_option("7.1")
    steps = page.locator("#gd-steps .gd-step")
    assert steps.count() == 5
    assert page.locator("#gd-turn").inner_text() == "Face the point midway between the two speakers."
    assert page.locator('#gd-diagram .gd-spk[data-on="true"]').count() == 2
    assert page.locator("#gd-diagram .gd-spk-label").all_text_contents() == ["FL", "FR"]
    page.evaluate("document.getElementById('guided-card').dataset.countdown = '0'")
    starts = "galleryCalls.filter(c => c.method === 'start_recording')"
    page.locator("#btn-gd-record").click()
    page.wait_for_function(f"{starts}.length === 1")
    request = page.evaluate(f"{starts}.at(-1).args[0]")
    assert request["sweep"]["speakers"] == "FL,FR" and request["sweep"]["tracks"] == "stereo", request
    assert request["expected_azimuths"] == {"FL": 30, "FR": -30} and request["append"] is False, request
    assert "play_path" not in request, request
    page.wait_for_function("document.querySelectorAll('#gd-steps .gd-step')[0].dataset.status === 'ok'")
    assert steps.nth(1).get_attribute("aria-current") == "step"
    assert page.locator("#gd-turn").inner_text() == "Turn around so the speakers are behind you."
    assert sorted(page.locator("#gd-diagram .gd-spk-label").all_text_contents()) == ["BL", "BR"]
    page.locator("#btn-gd-record").click()
    page.wait_for_function(f"{starts}.length === 2")
    references = page.evaluate(f"{starts}.at(-1).args[0]")["reference_measurements"]
    assert [(item["speaker"], item["azimuth"]) for item in references] == [("FL", 30), ("FR", -30)], references
    assert all(item["lf_ild_db"] == -6.5 for item in references), references
    for item, azimuth in zip(references, (30, -30), strict=True):
        assert item["direct"] == {"fs": 48000, "left": [0, azimuth / 180, -0.25],
                                  "right": [0, 0.125, -azimuth / 180]}, references
    page.wait_for_function("document.querySelectorAll('#gd-steps .gd-step')[1].dataset.status === 'warning'")
    assert steps.nth(1).get_attribute("aria-current") == "step"
    warning = page.locator('#gd-checks .gd-check[data-severity="warning"]')
    assert "170°" in warning.inner_text() and "150°" in warning.inner_text(), warning.inner_text()
    assert "170°" in page.locator("#view-recorder [data-log]").inner_text()
    # Steps belong to their folder: another folder starts with nothing recorded.
    page.locator("#rf-record-dir").fill("data/other_hrir")
    assert steps.nth(0).get_attribute("data-status") == "todo"
    page.locator("#rf-record-dir").fill("data/my_hrir")
    assert steps.nth(0).get_attribute("data-status") == "ok"
    # A recording started by hand during the countdown is not taken for the step.
    page.evaluate("document.getElementById('guided-card').dataset.countdown = '1'")
    page.locator("#btn-gd-record").click()
    page.evaluate("state.jobId = 'started-by-hand'")
    page.wait_for_timeout(1500)
    assert page.evaluate(f"{starts}.length") == 2
    assert page.evaluate("state.guided.pending") is None
    page.evaluate("state.jobId = null")
    assert steps.nth(1).get_attribute("data-status") == "warning"
    page.locator("#btn-gd-next").click()
    assert page.locator("#gd-turn").inner_text() == "Turn 60° to your right."
    assert page.locator("#gd-ear").inner_text() == "Turn so your left ear points toward the left speaker."
    assert page.locator('#gd-diagram .gd-spk[data-on="true"]').count() == 1
    page.evaluate("document.getElementById('guided-card').dataset.countdown = '3'")
    page.locator("#btn-gd-record").click()
    assert page.locator("#gd-countdown").is_visible() and "3" in page.locator("#gd-countdown").inner_text()
    page.locator("#btn-gd-cancel").click()
    assert not page.locator("#gd-countdown").is_visible() and page.locator("#btn-gd-record").is_visible()
    page.wait_for_timeout(1200)
    assert page.evaluate(f"{starts}.length") == 2
    page.locator("#gd-setup").select_option("mono")
    assert page.locator("#gd-steps .gd-step").count() == 7
    assert page.locator("#gd-turn").inner_text() == "Face the speaker."
    page.locator("#btn-gd-next").click()
    assert page.locator("#gd-turn").inner_text() == "Turn 30° to your right."
    assert page.locator("#gd-plays").inner_text() == "FL (30° left)"
    page.locator("#gd-setup").select_option("")
    assert not page.locator("#gd-body").is_visible()
    assert not errors, errors
    context.close()
    print("Gallery guided checks: steps, turn and ear text, diagram, request angles, countdown, warnings OK", flush=True)


def polarity_take(page):
    """Show both speaker-polarity warnings through the recording result."""
    navigate(page, "recorder")
    page.locator("#gd-setup").select_option("stereo")
    page.locator("#gd-steps .gd-step").nth(2).click()  # SL
    page.evaluate("document.getElementById('guided-card').dataset.countdown = '0'")
    page.locator("#btn-gd-record").click()
    page.wait_for_function("document.querySelectorAll('#gd-steps .gd-step')[2].dataset.status === 'warning'")
    warnings = page.locator('#gd-checks .gd-check[data-severity="warning"]')
    assert warnings.count() == 2
    polarity, split = warnings.all_text_contents()
    assert all(value in polarity for value in ("SL", "3", "0.78")), polarity
    assert all(value in split for value in ("FL", "FC", "1.00")), split
    assert "{" not in polarity + split and "record_check_" not in polarity + split
    log = page.locator("#view-recorder [data-log]").inner_text()
    assert polarity in log and split in log, log
    page.locator("#gd-checks").scroll_into_view_if_needed()


def polarity_checks(browser):
    for language in LANGUAGES:
        context, page, errors = open_page(browser, "studio", "dark", language, "polarity")
        polarity_take(page)
        assert not errors, errors
        context.close()
    print("Gallery polarity checks: direct windows round-trip, missing windows null, retakes, folder/headphone isolation, "
          "both warnings in English/Korean OK", flush=True)


def room_phase_checks(browser):
    """Virtual room tuning: its own rows replace the Room EQ rows, the timing
    limit is an eight-stop slider in Studio and one checkbox in Stable, the
    rows it governs (room facts, delay) follow it, a skin switch keeps its
    meaning, and only tuning mode sends the room_tuning_* fields."""
    context, page, errors = open_page(browser, "studio", "dark", "en", "idle")
    navigate(page, "processing")
    disclosure(page, "room", True)
    eq_rows = ("#bf-room-range-row", "#bf-room-max-boost-row", "#bf-fr-combination-row")
    tuning_rows = ("#bf-room-tuning-max-boost-row", "#bf-room-tuning-curtain-row", "#bf-room-tuning-level-match-row",
                   'button.info-tip[aria-describedby="bf-room-phase-limit-tip"]')
    facts = ("#bf-room-volume-row", "#bf-schroeder-freq-row")

    def shown(selectors):
        return [page.locator(selector).is_visible() for selector in selectors]

    request = brir_request(page)
    assert request["room_mode"] == "eq" and not any(key.startswith("room_tuning") for key in request), request
    assert all(shown(eq_rows + facts)) and not any(shown(tuning_rows + ("#bf-room-tuning-delay-row",)))
    # The mode and range tooltips follow the choice.

    def tip_text(tip):
        return page.locator(f"#{tip}").text_content()

    assert tip_text("bf-room-mode-tip") == page.evaluate("t('tooltip_room_mode_eq')")
    page.locator("#bf-room-range").select_option("modes")
    assert tip_text("bf-room-range-tip") == page.evaluate("t('tooltip_room_range_modes')")
    page.locator("#bf-room-range").select_option("schroeder")

    page.locator("#bf-room-mode-tuning").click()
    assert tip_text("bf-room-mode-tip") == page.evaluate("t('tooltip_room_mode_tuning')")
    slider = page.locator("#bf-room-phase-limit")
    assert slider.is_visible() and not page.locator("#bf-room-phase-schroeder").is_visible()
    assert all(shown(tuning_rows)) and not any(shown(eq_rows + facts))
    assert slider.input_value() == "7" and page.locator("#bf-room-tuning-delay-row").is_visible()
    request = brir_request(page)
    assert request["room_tuning_phase_limit"] == "full" and request["room_tuning_delay"] == 10, request
    assert request["room_tuning_max_boost"] == 6 and request["room_tuning_curtain"] == 300, request
    assert request["room_tuning_level_match"] is True, request
    assert not {"room_range", "room_max_boost", "fr_combination_method", "room_volume", "schroeder_freq"} & request.keys(), request

    named = {"off": "option_room_tuning_phase_off", "schroeder": "option_room_tuning_phase_schroeder",
             "full": "option_room_tuning_phase_full"}
    for stop, limit in enumerate(("off", "schroeder", 500, 1000, 2000, 5000, 10000, "full")):
        slider.focus()  # Generate takes the focus
        page.keyboard.press("Home")
        for _ in range(stop):
            page.keyboard.press("ArrowRight")
        text = page.locator("#bf-room-phase-limit-value").inner_text()
        assert slider.get_attribute("aria-valuetext") == text, stop
        if limit in named:
            assert text == page.evaluate(f"t('{named[limit]}')"), (stop, text)
        assert page.locator("#bf-room-tuning-delay-row").is_visible() == (limit != "off"), stop
        assert all(shown(facts)) == (limit == "schroeder") and any(shown(facts)) == (limit == "schroeder"), stop
        request = brir_request(page)
        assert request["room_tuning_phase_limit"] == limit, (stop, request)
        assert ("room_tuning_delay" in request) == (limit != "off"), (stop, request)
        assert ("room_volume" in request) == (limit == "schroeder"), (stop, request)

    # Automatic delay locks the number and sends "auto"; level matching can be turned off.
    page.locator("#bf-room-tuning-delay-auto").check()
    assert disabled(page, "#bf-room-tuning-delay")
    page.locator("#bf-room-tuning-level-match").uncheck()
    request = brir_request(page)
    assert request["room_tuning_delay"] == "auto" and request["room_tuning_level_match"] is False, request
    page.locator("#bf-room-tuning-delay-auto").uncheck()
    page.locator("#bf-room-tuning-level-match").check()
    assert not disabled(page, "#bf-room-tuning-delay")

    # Defaults arrive as CLI specs: a numeric or named limit, a delay of "auto" or a number as text.
    page.evaluate("state.brirDefaults = {room_mode: 'tuning', room_tuning_phase_limit: '2000', room_tuning_delay: 'auto'};"
                  "applyRoomDefaults()")
    assert slider.input_value() == "4" and page.locator("#bf-room-tuning-delay-auto").is_checked()
    page.evaluate("state.brirDefaults = {room_mode: 'tuning', room_tuning_phase_limit: 'schroeder', room_tuning_delay: '7'};"
                  "applyRoomDefaults()")
    assert slider.input_value() == "1" and page.locator("#bf-room-tuning-delay").input_value() == "7"
    assert not page.locator("#bf-room-tuning-delay-auto").is_checked() and all(shown(facts))
    page.evaluate("state.brirDefaults = {room_mode: 'tuning'}; applyRoomDefaults()")
    assert slider.input_value() == "7" and page.locator("#bf-room-tuning-delay").input_value() == "10"

    def switch(skin):
        navigate(page, "settings")
        page.locator("#sf-skin").select_option(skin)
        page.wait_for_function(f"document.documentElement.dataset.skin === '{skin}'")
        if page.locator("#job-modal").is_visible():  # Stable shows the finished job in its dialog
            page.locator("#job-modal-close").click()
        navigate(page, "processing")

    # Stable runs jobs in a dialog, so its requests are read without starting one.
    def stable_request():
        return page.evaluate("gatherBrirPayload()")

    # Stable shows only whether the limit is the Schroeder stop. A limit it cannot show (Off, a Hz stop) is
    # kept until the checkbox is used, so switching skins never changes what is processed, and the line
    # under the checkbox names it.
    checkbox = page.locator("#bf-room-phase-schroeder")
    current = page.locator("#bf-room-phase-current")
    for stop, limit in ((0, "off"), (1, "schroeder"), (4, 2000), (7, "full")):
        slider.focus()
        page.keyboard.press("Home")
        for _ in range(stop):
            page.keyboard.press("ArrowRight")
        assert not current.is_visible(), stop
        readout = page.locator("#bf-room-phase-limit-value").inner_text()
        switch("stable")
        schroeder = limit == "schroeder"
        assert checkbox.is_visible() and checkbox.is_checked() == schroeder and not slider.is_visible(), stop
        assert page.locator('button.info-tip[aria-describedby="bf-room-phase-schroeder-tip"]').is_visible()
        assert page.locator("#bf-room-tuning-delay-row").is_visible() == (limit != "off"), stop
        assert all(shown(facts)) == schroeder, stop
        assert current.is_visible() == (limit in ("off", 2000)), stop
        if limit in ("off", 2000):  # named as Studio's readout names it
            expected = page.evaluate("(name) => fmt(t('label_room_tuning_phase_current'), {limit: name})", readout)
            assert current.inner_text() == expected and readout in expected, (stop, current.inner_text())
        request = stable_request()
        assert request["room_tuning_phase_limit"] == limit, (stop, request)
        switch("studio")
        assert slider.input_value() == str(stop), stop
    switch("stable")
    assert not current.is_visible()  # the loop ends on the full band
    checkbox.check()
    assert stable_request()["room_tuning_phase_limit"] == "schroeder" and all(shown(facts))
    assert not current.is_visible()
    checkbox.uncheck()
    assert stable_request()["room_tuning_phase_limit"] == "full" and not any(shown(facts))
    assert not current.is_visible()
    checkbox.check()
    assert stable_request()["room_tuning_phase_limit"] == "schroeder" and all(shown(facts))
    page.locator("#bf-room-mode-eq").click()
    request = stable_request()
    assert not any(key.startswith("room_tuning") for key in request) and not checkbox.is_visible(), request
    assert all(shown(eq_rows + facts))
    assert not errors, errors
    context.close()
    print("Gallery room tuning checks: rows per mode and limit, eight stops, auto delay, defaults, Stable checkbox, "
          "skin carry-over OK", flush=True)


def room_tuning2_checks(browser):
    """Virtual room tuning 2 (ADR 0008): its rows replace the other modes' rows,
    the filter shows the latency row only for a custom latency, an empty upper
    band end shows the room facts and sends them, MIMO shows and sends its
    limits, the tooltips follow the choices, bootstrap defaults apply, and only
    tuning 2 sends room_tuning2_* fields. Both skins send the same request."""
    bundle1_rows = ("#bf-room-tuning2-bands-row", "#bf-room-tuning2-dip-limit-row", "#bf-room-tuning2-low-protect-row")
    bundle2_options = ("mode-decay", "lf-align", "hf-phase")
    bundle2_rows = tuple(f"#bf-room-tuning2-{option}-row" for option in bundle2_options)
    fir_hint = "#bf-room-tuning2-fir-hint"
    high_rows = ("#bf-room-tuning2-high-basis-row", "#bf-room-tuning2-high-transition-row")
    volume_hint = "#bf-room-tuning2-high-volume-hint"
    hp_row = "#bf-room-tuning2-input-hp-row"
    t2_rows = ("#bf-room-tuning2-mode-row", "#bf-room-tuning2-max-boost-row", "#bf-room-tuning2-low-row",
               "#bf-room-tuning2-high-row", "#bf-room-tuning-level-match-row", "#bf-room-tuning2-mimo-row") + bundle1_rows + bundle2_rows
    others = ("#bf-room-range-row", "#bf-room-max-boost-row", "#bf-fr-combination-row", "#bf-room-tuning-max-boost-row",
              "#bf-room-tuning-curtain-row", "#bf-room-tuning-delay-row", "#bf-room-phase-limit-row",
              "#bf-room-phase-schroeder-row")
    facts = ("#bf-room-volume-row", "#bf-schroeder-freq-row")
    mimo_rows = ("#bf-room-tuning2-fsiso-row", "#bf-room-tuning2-mimo-low-row",
                 "#bf-room-tuning2-mimo-early-row", "#bf-room-tuning2-mimo-ear-guard-row")
    mimo_keys = {"room_tuning2_fsiso", "room_tuning2_mimo_low", "room_tuning2_mimo_early", "room_tuning2_mimo_ear_guard"}
    latency = "#bf-room-tuning2-latency-row"
    for skin in SKINS:
        context, page, errors = open_page(browser, skin, "dark", "en", "idle")
        navigate(page, "processing")
        disclosure(page, "room", True)

        def shown(selectors, page=page):
            return [page.locator(selector).is_visible() for selector in selectors]

        def tip(name, page=page):
            return page.locator(f"#{name}").text_content()

        def request(page=page):
            return page.evaluate("gatherBrirPayload()")

        for mode in ("eq", "tuning"):
            page.locator(f"#bf-room-mode-{mode}").click()
            assert not any(key.startswith("room_tuning2") for key in request()), (skin, mode)
            assert not any(shown(("#bf-room-tuning2-mode-row", "#bf-room-tuning2-high-row", "#bf-room-tuning2-mimo-row")
                                 + bundle1_rows + bundle2_rows + mimo_rows + high_rows
                                 + (latency, fir_hint, volume_hint, hp_row))), (skin, mode)
        page.locator("#bf-room-mode-tuning2").click()
        assert tip("bf-room-mode-tip") == page.evaluate("t('tooltip_room_mode_tuning2')")
        assert tip("bf-room-tuning-level-match-tip") == page.evaluate("t('tooltip_room_tuning2_level_match')")
        assert tip("bf-room-tuning2-mode-tip") == page.evaluate("t('tooltip_room_tuning2_mode_mixed')")
        assert all(shown(t2_rows + facts)) and not any(shown(others + mimo_rows + (latency, fir_hint))), skin
        sent = request()
        assert sent["room_mode"] == "tuning2" and sent["room_tuning2_mode"] == "mixed", sent
        assert sent["room_tuning2_max_boost"] == 10 and sent["room_tuning2_low"] == 20, sent
        assert sent["room_tuning2_high"] == "auto" and sent["room_tuning2_mimo"] is False, sent
        assert sent["room_tuning_level_match"] is True and {"room_volume", "schroeder_freq"} <= sent.keys(), sent
        assert not (mimo_keys | {"room_range", "room_max_boost", "fr_combination_method", "room_tuning_delay",
                                 "room_tuning_phase_limit", "room_tuning_max_boost", "room_tuning_curtain"}) & sent.keys(), sent
        assert sent["room_tuning2_dip_limit"] is False and sent["room_tuning2_low_protect"] is False, sent
        assert sent["room_tuning2_bands"] == "", sent
        assert all(shown(high_rows + (hp_row,))) and not page.locator(volume_hint).is_visible(), skin
        assert sent["room_tuning2_high_basis"] == "strict" and sent["room_tuning2_high_transition"] == 1, sent
        assert sent["room_tuning2_input_hp"] == "off", sent
        assert page.locator("#bf-room-tuning2-input-hp").get_attribute("placeholder") == page.evaluate(
            "t('placeholder_room_tuning2_off')")
        basis = page.locator("#bf-room-tuning2-high-basis")
        transition = page.locator("#bf-room-tuning2-high-transition")
        hp = page.locator("#bf-room-tuning2-input-hp")
        basis.select_option("lenient")
        assert page.locator(volume_hint).is_visible(), skin
        page.locator("#bf-room-volume").fill("50")
        assert not page.locator(volume_hint).is_visible(), skin
        page.locator("#bf-room-volume").fill("")
        assert page.locator(volume_hint).is_visible(), skin
        page.locator("#bf-schroeder-freq").fill("180")
        assert not page.locator(volume_hint).is_visible(), skin
        page.locator("#bf-schroeder-freq").fill("")
        basis.select_option("strict")
        assert not page.locator(volume_hint).is_visible(), skin
        transition.select_option("4")
        assert page.locator(volume_hint).is_visible() and request()["room_tuning2_high_transition"] == 4, skin
        transition.select_option("1")
        assert not page.locator(volume_hint).is_visible(), skin
        basis.select_option("lenient")
        transition.select_option("2")
        hp.fill("25")
        sent = request()
        assert sent["room_tuning2_high_basis"] == "lenient" and sent["room_tuning2_high_transition"] == 2, sent
        assert sent["room_tuning2_input_hp"] == 25 and page.locator(volume_hint).is_visible(), sent
        for option in bundle2_options:
            control = page.locator(f"#bf-room-tuning2-{option}")
            key = "room_tuning2_" + option.replace("-", "_")
            assert sent[key] is False and not control.is_checked() and control.is_enabled(), (skin, option, sent)
            control.check()
            assert request()[key] is True, (skin, option)
        # PEQ-only keeps the ticks but never sends enabled FIR-only options.
        mode = page.locator("#bf-room-tuning2-mode")
        for value in ("zero", "mixed"):
            mode.select_option(value)
            assert page.locator(fir_hint).is_visible() == (value == "zero"), (skin, value)
            assert all(shown(bundle2_rows)), (skin, value)
            sent = request()
            for option in bundle2_options:
                control = page.locator(f"#bf-room-tuning2-{option}")
                enabled = option == "mode-decay" or value != "zero"
                assert control.is_enabled() == enabled and control.is_checked(), (skin, value, option)
                assert sent["room_tuning2_" + option.replace("-", "_")] is enabled, (skin, value, sent)
        page.locator("#bf-room-tuning2-dip-limit").check()
        page.locator("#bf-room-tuning2-low-protect").check()
        page.locator("#bf-room-tuning2-bands").fill("  1000-3000, 6000-9000  ")
        sent = request()
        assert sent["room_tuning2_dip_limit"] is True and sent["room_tuning2_low_protect"] is True, sent
        assert sent["room_tuning2_bands"] == "1000-3000, 6000-9000", sent
        for other_mode in ("eq", "tuning"):
            page.locator(f"#bf-room-mode-{other_mode}").click()
            assert not any(key.startswith("room_tuning2") for key in request()), (skin, other_mode)
            assert not any(shown(bundle1_rows + bundle2_rows + high_rows + (fir_hint, volume_hint, hp_row))), (skin, other_mode)
        page.locator("#bf-room-mode-tuning2").click()
        assert request()["room_tuning2_bands"] == "1000-3000, 6000-9000"
        assert page.locator("#bf-room-tuning2-dip-limit").is_checked()
        assert page.locator("#bf-room-tuning2-low-protect").is_checked()
        # Only trim here; the service owns validation of the band syntax.
        page.locator("#bf-room-tuning2-bands").fill("  bad band  ")
        assert request()["room_tuning2_bands"] == "bad band"
        page.locator("#bf-room-tuning2-bands").fill("   ")
        assert request()["room_tuning2_bands"] == ""
        for value in ("zero", "linear", "long"):
            mode.select_option(value)
            assert request()["room_tuning2_mode"] == value and not page.locator(latency).is_visible(), (skin, value)
            assert tip("bf-room-tuning2-mode-tip") == page.evaluate(f"t('tooltip_room_tuning2_mode_{value}')"), value
            assert page.locator(fir_hint).is_visible() == (value == "zero"), (skin, value)
            for option in bundle2_options:
                control = page.locator(f"#bf-room-tuning2-{option}")
                enabled = option == "mode-decay" or value != "zero"
                assert control.is_enabled() == enabled and control.is_checked(), (skin, value, option)
                assert request()["room_tuning2_" + option.replace("-", "_")] is enabled, (skin, value, option)
        mode.select_option("custom")
        assert page.locator(latency).is_visible() and not page.locator(fir_hint).is_visible()
        for option in bundle2_options:
            assert page.locator(f"#bf-room-tuning2-{option}").is_enabled(), (skin, option)
            assert request()["room_tuning2_" + option.replace("-", "_")] is True, (skin, option)
        assert tip("bf-room-tuning2-mode-tip") == page.evaluate("t('tooltip_room_tuning2_mode_custom')")
        assert request()["room_tuning2_mode"] == 200
        page.locator("#bf-room-tuning2-latency").fill("60")
        assert request()["room_tuning2_mode"] == 60
        # A manual upper end hides the automatic controls, not the independent high-pass.
        page.locator("#bf-room-tuning2-high").fill("300")
        assert not any(shown(facts + high_rows + (volume_hint,))) and page.locator(hp_row).is_visible(), skin
        sent = request()
        assert sent["room_tuning2_high"] == 300 and not {"room_volume", "schroeder_freq"} & sent.keys(), sent
        assert sent["room_tuning2_high_basis"] == "lenient" and sent["room_tuning2_high_transition"] == 2, sent
        assert sent["room_tuning2_input_hp"] == 25, sent
        page.locator("#bf-room-tuning2-high").fill("")
        assert all(shown(facts + high_rows + (volume_hint, hp_row))), skin
        page.locator("#bf-room-tuning2-mimo").check()
        assert all(shown(mimo_rows))
        sent = request()
        assert sent["room_tuning2_mimo"] is True and sent["room_tuning2_fsiso"] == 150, sent
        assert sent["room_tuning2_mimo_low"] == "auto", sent
        early = page.locator("#bf-room-tuning2-mimo-early")
        ear_guard = page.locator("#bf-room-tuning2-mimo-ear-guard")
        assert early.input_value() == "60" and not ear_guard.is_checked(), skin
        assert sent["room_tuning2_mimo_early"] == 60 and sent["room_tuning2_mimo_ear_guard"] is False, sent
        early.fill("")
        assert request()["room_tuning2_mimo_early"] == 60, skin
        early.fill("20")
        ear_guard.check()
        page.locator("#bf-room-tuning2-fsiso").fill("100")
        page.locator("#bf-room-tuning2-mimo-low").fill("40")
        sent = request()
        assert sent["room_tuning2_fsiso"] == 100 and sent["room_tuning2_mimo_low"] == 40, sent
        assert sent["room_tuning2_mimo_early"] == 20 and sent["room_tuning2_mimo_ear_guard"] is True, sent
        for other_mode in ("eq", "tuning"):
            page.locator(f"#bf-room-mode-{other_mode}").click()
            assert not any(shown(mimo_rows)) and not mimo_keys & request().keys(), (skin, other_mode)
        page.locator("#bf-room-mode-tuning2").click()
        assert all(shown(mimo_rows)), skin
        page.locator("#bf-room-tuning2-mimo").uncheck()
        assert not any(shown(mimo_rows)) and not mimo_keys & request().keys(), skin
        page.locator("#bf-room-tuning2-mimo").check()
        assert early.input_value() == "20" and ear_guard.is_checked(), skin
        assert page.locator("#bf-room-tuning2-high").get_attribute("placeholder") == page.evaluate(
            "t('placeholder_room_tuning2_auto')")
        page.evaluate("state.brirDefaults = {room_mode: 'tuning2', room_tuning2_mode: '60', room_tuning2_high: '250',"
                      " room_tuning2_mimo: true, room_tuning2_mimo_low: 'auto', room_tuning2_fsiso: 120,"
                      " room_tuning2_mimo_early: 30, room_tuning2_mimo_ear_guard: true,"
                      " room_tuning2_dip_limit: true, room_tuning2_low_protect: true,"
                      " room_tuning2_mode_decay: true, room_tuning2_lf_align: true, room_tuning2_hf_phase: true,"
                      " room_tuning2_high_basis: 'lenient', room_tuning2_high_transition: 4, room_tuning2_input_hp: '30',"
                      " room_tuning2_bands: '1200-2400'}; applyRoomDefaults()")
        assert mode.input_value() == "custom" and page.locator("#bf-room-tuning2-latency").input_value() == "60"
        assert page.locator("#bf-room-tuning2-high").input_value() == "250" and not any(shown(facts))
        assert page.locator("#bf-room-tuning2-mimo").is_checked()
        assert page.locator("#bf-room-tuning2-mimo-low").input_value() == ""
        assert page.locator("#bf-room-tuning2-fsiso").input_value() == "120"
        assert early.input_value() == "30" and ear_guard.is_checked(), skin
        sent = request()
        assert sent["room_tuning2_mimo_early"] == 30 and sent["room_tuning2_mimo_ear_guard"] is True, sent
        early.fill("")
        assert request()["room_tuning2_mimo_early"] == 30, skin
        assert page.locator("#bf-room-tuning2-dip-limit").is_checked()
        assert page.locator("#bf-room-tuning2-low-protect").is_checked()
        assert page.locator("#bf-room-tuning2-bands").input_value() == "1200-2400"
        assert basis.input_value() == "lenient" and transition.input_value() == "4" and hp.input_value() == "30", skin
        assert request()["room_tuning2_input_hp"] == 30, skin
        page.evaluate("state.brirDefaults.room_tuning2_high_basis = 'unknown';"
                      " state.brirDefaults.room_tuning2_high_transition = 3;"
                      " state.brirDefaults.room_tuning2_input_hp = 'off'; applyRoomDefaults()")
        assert basis.input_value() == "strict" and transition.input_value() == "1" and hp.input_value() == "", skin
        assert request()["room_tuning2_input_hp"] == "off", skin
        page.evaluate("state.brirDefaults.room_tuning2_input_hp = 25; applyRoomDefaults()")
        assert hp.input_value() == "25" and request()["room_tuning2_input_hp"] == 25, skin
        for value in ("zero", "mixed"):
            page.evaluate("value => { state.brirDefaults.room_tuning2_mode = value; applyRoomDefaults(); }", value)
            assert page.locator(fir_hint).is_visible() == (value == "zero"), (skin, value)
            for option in bundle2_options:
                control = page.locator(f"#bf-room-tuning2-{option}")
                enabled = option == "mode-decay" or value != "zero"
                assert control.is_checked() and control.is_enabled() == enabled, (skin, value, option)
                assert request()["room_tuning2_" + option.replace("-", "_")] is enabled, (skin, value, option)
        # Reset directly from zero to verify defaults also remove the disabled state.
        page.evaluate("state.brirDefaults.room_tuning2_mode = 'zero'; applyRoomDefaults()")
        page.evaluate("state.brirDefaults = {}; applyRoomDefaults()")
        assert page.locator("#bf-room-mode-eq").get_attribute("aria-checked") == "true"
        assert mode.input_value() == "mixed" and page.locator("#bf-room-tuning2-high").input_value() == ""
        assert not page.locator("#bf-room-tuning2-mimo").is_checked()
        assert early.input_value() == "60" and not ear_guard.is_checked(), skin
        assert not any(shown(mimo_rows)), skin
        assert not page.locator("#bf-room-tuning2-dip-limit").is_checked()
        assert not page.locator("#bf-room-tuning2-low-protect").is_checked()
        assert page.locator("#bf-room-tuning2-bands").input_value() == ""
        for option in bundle2_options:
            control = page.locator(f"#bf-room-tuning2-{option}")
            assert not control.is_checked() and control.is_enabled(), (skin, option)
        assert basis.input_value() == "strict" and transition.input_value() == "1" and hp.input_value() == "", skin
        assert not any(shown(bundle2_rows + high_rows + (fir_hint, volume_hint, hp_row))), skin
        assert not any(key.startswith("room_tuning2") for key in request()), skin
        page.locator("#bf-room-mode-tuning2").click()
        for option in bundle2_options:
            assert request()["room_tuning2_" + option.replace("-", "_")] is False, (skin, option)
        assert not errors, errors
        context.close()
    print("Gallery room tuning 2 checks: rows per mode, filters and latency, automatic band facts, MIMO limits, "
          "dip limit, low-end protection, extra bands, mode decay, bass arrival, treble phase, FIR-only state, "
          "upper-limit basis and multiplier, volume hint, input high-pass, MIMO early interval and ear guard, "
          "tooltips, defaults, both skins OK", flush=True)


def room_file_checks(browser):
    """Room measurements from a REW or Dirac Live file (ADR 0010): the file row
    shows only for the file source, Browse asks for the measurement kind, the
    table starts from the file's automatic assignment, a slot and ear claimed
    twice turns both rows red and keeps the job from starting, the request
    carries the table's map, Dirac's ear positions apply to every speaker, a
    missing or unreadable file says so (the reader's message only under
    Details), and the folder source sends no file. Both skins send the same."""
    def request_brir(page):
        request = brir_request(page)
        if page.locator("#job-modal").is_visible():  # Stable shows the job in its dialog
            page.locator("#job-modal-close").click()
        return request

    for skin in SKINS:
        context, page, errors = open_page(browser, skin, "dark", "en", "idle")
        navigate(page, "processing")
        disclosure(page, "room", True)
        assert not page.locator("#bf-room-file-block").is_visible()
        request = request_brir(page)
        assert "room_measurement_file" not in request, request

        page.locator("#bf-room-source").select_option("file")
        assert page.locator("#bf-room-file-block").is_visible()
        assert page.locator("#bf-room-file-result").inner_text() == "Choose a .mdat or .liveproject file."
        page.locator("#bf-room-file-block .btn-link").click()
        page.wait_for_function("galleryCalls.some(c => c.method === 'select_file' && c.args[0] === 'measurement')")
        rows = page.locator("#bf-room-file-result tbody tr")
        rows.nth(3).wait_for()
        assert rows.count() == 4
        assert page.locator("#bf-room-file-result .room-file-summary").inner_text().endswith("3 measurements with an impulse response, 48000 Hz")

        def slot(i, rows=rows):
            return rows.nth(i).locator('select[data-room-field="slot"]')

        def side(i, rows=rows):
            return rows.nth(i).locator('select[data-room-field="side"]')

        assert [slot(i).input_value() for i in range(4)] == ["FL", "FL", "", ""]
        assert [side(i).input_value() for i in range(2)] == ["left", "right"]
        assert rows.nth(2).get_attribute("data-state") == "skipped" and slot(2).is_disabled()
        assert side(3).is_disabled()
        assert page.locator("#bf-room-file-status").inner_text().startswith("Only the measurements assigned here")

        side(1).select_option("left")
        assert rows.nth(0).get_attribute("data-state") == "error" == rows.nth(1).get_attribute("data-state")
        assert "room-file-error" in (page.locator("#bf-room-file-status").get_attribute("class") or "")
        before = page.evaluate("galleryCalls.filter(c => c.method === 'start_brir').length")
        page.locator("#btn-generate-brir").click()
        page.wait_for_timeout(100)
        assert page.evaluate("galleryCalls.filter(c => c.method === 'start_brir').length") == before
        side(1).select_option("right")
        slot(3).select_option("FR")
        assert not side(3).is_disabled()
        request = request_brir(page)
        assert request["room_measurement_file"] == "D:/Measurements/living room.mdat", request
        assert request["room_measurement_map"] == "FL.left=1,FL.right=2,FR=4", request

        # Until the new file's inspection answers, the REW table is gone and
        # Generate starts nothing: it must not send the new path with the old map.
        page.locator("#bf-room-file").fill("D:/Dirac/slow living room.liveproject")
        assert rows.count() == 0
        before = page.evaluate("galleryCalls.filter(c => c.method === 'start_brir').length")
        page.locator("#btn-generate-brir").click()
        page.wait_for_timeout(300)
        assert page.evaluate("galleryCalls.filter(c => c.method === 'start_brir').length") == before
        page.locator("#bf-room-file-result .room-file-positions select").first.wait_for()
        assert rows.count() == 6
        assert [slot(i).input_value() for i in range(6)] == ["FL", "FR", "SL", "SR", "", ""]
        assert "Subwoofer" in rows.nth(4).inner_text()
        assert "calibration" in page.locator("#bf-room-file-result").inner_text()
        page.locator('#bf-room-file-result .room-file-positions select[data-room-field="right"]').select_option("2")
        request = request_brir(page)
        assert request["room_measurement_map"] == ("FL.left=1@1,FL.right=1@2,FR.left=2@1,FR.right=2@2,"
                                                   "SL.left=3@1,SL.right=3@2,SR.left=4@1,SR.right=4@2"), request

        page.locator("#bf-room-file").fill("D:/Dirac/partial living room.liveproject")
        page.wait_for_function("document.querySelector('#bf-room-file-status')?.textContent === t('hint_room_file')")
        page.locator('#bf-room-file-result .room-file-positions select[data-room-field="right"]').select_option("2")
        assert page.locator("#bf-room-file-status").inner_text() == page.evaluate(
            "fmt(t('room_file_not_measured'), {speakers: 'FR'})")
        assert _blocked(page)

        page.locator("#bf-room-file").fill("D:/Dirac/missing.liveproject")
        page.locator("#bf-room-file-result .room-file-error", has_text="The file does not exist.").wait_for()
        page.locator("#bf-room-file").fill("D:/Dirac/broken.mdat")
        page.locator("#bf-room-file-result .room-file-details").wait_for()
        assert "not a REW measurement file" not in page.locator("#bf-room-file-result .room-file-error").inner_text()
        assert not page.locator("#bf-room-file-result .room-file-details code").is_visible()

        page.locator("#bf-room-source").select_option("folder")
        assert not page.locator("#bf-room-file-block").is_visible()
        request = request_brir(page)
        assert "room_measurement_file" not in request and "room_measurement_map" not in request, request
        assert not errors, errors
        context.close()
    print("Gallery room file checks: source toggle, Browse kind, automatic table, duplicate block, REW and Dirac maps, missing/unreadable files, folder source OK", flush=True)


def _starts(page, method):
    return page.evaluate(f"galleryCalls.filter(c => c.method === '{method}').length")


def _blocked(page):
    """Generate starts no job while a measurement file's screen has a problem."""
    before = _starts(page, "start_brir")
    page.locator("#btn-generate-brir").click()
    page.wait_for_timeout(100)
    return _starts(page, "start_brir") == before


def brir_file_checks(browser):
    """BRIR measurements from a REW or Dirac Live file (ADR 0011): the row sits
    in Input Files and shows the file block only for the file source; the
    inspection asks for the brir purpose; a REW measurement is one ear (no
    "both"), a speaker with one ear or a missing FL keeps the job from
    starting; Dirac's two ears start at positions 1 and 2, one position for
    both ears or a one-position project blocks; the project's calibration is
    not offered; the folder source sends no file. Both skins send the same."""
    def request_brir(page):
        request = brir_request(page)
        if page.locator("#job-modal").is_visible():
            page.locator("#job-modal-close").click()
        return request

    for skin in SKINS:
        context, page, errors = open_page(browser, skin, "dark", "en", "idle")
        navigate(page, "processing")
        assert page.locator("#bf-brir-source-row").is_visible()
        assert not page.locator("#bf-brir-file-block").is_visible()
        request = request_brir(page)
        assert "brir_measurement_file" not in request and "brir_measurement_map" not in request, request

        page.locator("#bf-brir-source").select_option("file")
        assert page.locator("#bf-brir-file-block").is_visible()
        assert page.locator("#bf-brir-file-result").inner_text() == page.evaluate("t('room_file_choose')")
        assert _blocked(page)
        page.locator("#bf-brir-file-block .btn-link").click()
        rows = page.locator("#bf-brir-file-result tbody tr")
        rows.nth(3).wait_for()
        assert page.evaluate("galleryCalls.some(c => c.method === 'inspect_room_measurements' && c.args[0].purpose === 'brir')")
        status = page.locator("#bf-brir-file-status")

        def slot(i, rows=rows):
            return rows.nth(i).locator('select[data-room-field="slot"]')

        def side(i, rows=rows):
            return rows.nth(i).locator('select[data-room-field="side"]')

        assert [slot(i).input_value() for i in range(4)] == ["FL", "FL", "", ""]
        assert [side(i).input_value() for i in range(2)] == ["left", "right"]
        assert side(0).locator("option").count() == 2, "a BRIR measurement is one ear"
        assert status.inner_text() == page.evaluate("t('hint_brir_file')")

        slot(3).select_option("FR")
        assert side(3).input_value() == "left"
        assert status.inner_text() == page.evaluate("fmt(t('brir_file_incomplete'), {speakers: 'FR'})")
        assert rows.nth(3).get_attribute("data-state") == "error"
        assert _blocked(page)
        slot(3).select_option("")
        slot(0).select_option("FR")
        slot(1).select_option("FR")
        assert status.inner_text() == page.evaluate("t('brir_file_needs_fl')")
        assert _blocked(page)
        slot(0).select_option("FL")
        slot(1).select_option("FL")
        side(1).select_option("left")
        assert status.inner_text() == page.evaluate("t('room_file_duplicate')")
        side(1).select_option("right")
        request = request_brir(page)
        assert request["brir_measurement_file"] == "D:/Measurements/living room.mdat", request
        assert request["brir_measurement_map"] == "FL.left=1,FL.right=2", request

        page.locator("#bf-brir-file").fill("D:/Dirac/living room.liveproject")
        positions = page.locator("#bf-brir-file-result .room-file-positions select")
        positions.first.wait_for()
        assert [positions.nth(i).input_value() for i in range(2)] == ["1", "2"]
        text = " ".join(page.locator("#bf-brir-file-result").inner_text().split())  # rendered spaces collapse
        assert page.evaluate("t('hint_brir_file_dirac')") in text
        assert " ".join(page.evaluate("fmt(t('room_file_microphone'), {name: 'Umik-1  Gain: 18dB'})").split()) in text
        assert page.evaluate("t('room_file_microphone_calibrated').split('.')[1].trim()") not in text
        request = request_brir(page)
        assert request["brir_measurement_map"] == ("FL.left=1@1,FL.right=1@2,FR.left=2@1,FR.right=2@2,"
                                                   "SL.left=3@1,SL.right=3@2,SR.left=4@1,SR.right=4@2"), request
        positions.nth(1).select_option("1")
        assert status.inner_text() == page.evaluate("t('brir_file_same_position')")
        assert _blocked(page)

        # A speaker missing at a chosen position is named before the job starts (the
        # automatic map leaves FR with its left ear only).
        page.locator("#bf-brir-file").fill("D:/Dirac/partial living room.liveproject")
        page.wait_for_function("document.querySelector('#bf-brir-file-status')?.textContent === "
                               "fmt(t('room_file_not_measured'), {speakers: 'FR'})")
        assert page.locator("#bf-brir-file-result tbody tr").nth(1).get_attribute("data-state") == "error"
        assert _blocked(page)
        page.locator("#bf-brir-file-result tbody tr").nth(1).locator('select[data-room-field="slot"]').select_option("")
        assert page.locator("#bf-brir-file-status").inner_text() == page.evaluate("t('hint_brir_file')")

        page.locator("#bf-brir-file").fill("D:/Dirac/single position.liveproject")
        page.locator("#bf-brir-file-result .room-file-positions select").first.wait_for()
        page.wait_for_function("document.querySelector('#bf-brir-file-status')?.textContent === t('brir_file_one_position')")
        assert _blocked(page)

        page.locator("#bf-brir-source").select_option("folder")
        assert not page.locator("#bf-brir-file-block").is_visible()
        request = request_brir(page)
        assert "brir_measurement_file" not in request and "brir_measurement_map" not in request, request
        assert not errors, errors
        context.close()
    print("Gallery BRIR file checks: source row, brir purpose, one ear per REW measurement, incomplete/FL/duplicate "
          "blocks, Dirac ear positions, one-position block, no calibration line, folder source OK", flush=True)


def headphone_file_checks(browser):
    """Headphone measurement from a REW or Dirac Live file (ADR 0011): the file
    source hides the headphone file row and shows two earcup selects that
    start from the file's automatic choice; the request carries left=,right=
    and no headphone_compensation_file; one measurement for both earcups or a
    missing one blocks the job, but only while compensation is on; Dirac
    options name speaker and position. Both skins send the same."""
    def request_brir(page):
        request = brir_request(page)
        if page.locator("#job-modal").is_visible():
            page.locator("#job-modal-close").click()
        return request

    for skin in SKINS:
        context, page, errors = open_page(browser, skin, "dark", "en", "idle")
        navigate(page, "processing")
        disclosure(page, "headphone", True)
        assert page.locator("#bf-hp-folder-row").is_visible() and not page.locator("#bf-hp-file-block").is_visible()
        page.locator("#bf-headphone-file").fill("my headphones.wav")
        request = request_brir(page)
        assert request["headphone_compensation_file"] == "my headphones.wav", request
        assert "headphone_measurement_file" not in request, request

        page.locator("#bf-hp-source").select_option("file")
        assert not page.locator("#bf-hp-folder-row").is_visible() and page.locator("#bf-hp-file-block").is_visible()
        page.locator("#bf-hp-file-block .btn-link").click()
        left = page.locator('#bf-hp-file-result select[data-room-field="hp-left"]')
        right = page.locator('#bf-hp-file-result select[data-room-field="hp-right"]')
        left.wait_for()
        assert page.evaluate("galleryCalls.some(c => c.method === 'inspect_room_measurements' && c.args[0].purpose === 'headphones')")
        status = page.locator("#bf-hp-file-status")
        assert (left.input_value(), right.input_value()) == ("1", "2")
        assert left.locator("option").count() == 4, "a choice prompt and the three measurements with an impulse response"
        assert status.inner_text() == page.evaluate("t('hint_headphone_file')")
        request = request_brir(page)
        assert request["headphone_measurement_file"] == "D:/Measurements/living room.mdat", request
        assert request["headphone_measurement_map"] == "left=1,right=2", request
        assert "headphone_compensation_file" not in request, request

        right.select_option("1")
        assert status.inner_text() == page.evaluate("t('headphone_file_same')")
        assert page.locator('#bf-hp-file-result label[data-room-key="hp-right"]').get_attribute("data-state") == "error"
        assert _blocked(page)
        right.select_option("")
        assert status.inner_text() == page.evaluate("t('headphone_file_incomplete')")
        assert _blocked(page)
        disclosure(page, "headphone", False)
        request = request_brir(page)
        assert "headphone_measurement_file" not in request and request["do_headphone_compensation"] is False, request
        disclosure(page, "headphone", True)
        right.select_option("4")
        request = request_brir(page)
        assert request["headphone_measurement_map"] == "left=1,right=4", request

        page.locator("#bf-hp-file").fill("D:/Dirac/living room.liveproject")
        page.wait_for_function("document.querySelector('#bf-hp-file-result select[data-room-field=\"hp-left\"]')?.value === '1@1'")
        assert right.input_value() == "2@2"
        assert left.locator("option[value='1@1']").inner_text() == page.evaluate(
            "fmt(t('option_measurement_at_position'), {name: 'Left', n: 1})")
        request = request_brir(page)
        assert request["headphone_measurement_map"] == "left=1@1,right=2@2", request

        page.locator("#bf-hp-source").select_option("folder")
        assert page.locator("#bf-hp-folder-row").is_visible()
        request = request_brir(page)
        assert "headphone_measurement_file" not in request and "headphone_measurement_map" not in request, request
        assert not errors, errors
        context.close()
    print("Gallery headphone file checks: source row, earcup selects from the automatic choice, map without the "
          "recording file, same/missing blocks only while on, Dirac option names, folder source OK", flush=True)


def vbass_checks(browser):
    """Virtual bass crossover modes: Automatic hides the frequency and sends no
    vbass_freq, Manual and Legacy show and send it, the tooltip follows the
    mode, bootstrap defaults apply, and Stable sends the same request."""
    context, page, errors = open_page(browser, "studio", "dark", "en", "idle")
    navigate(page, "processing")
    disclosure(page, "vbass", True)
    mode = page.locator("#bf-vbass-mode")
    freq_row = page.locator("#bf-vbass-freq-row")
    tip = page.locator("#bf-vbass-mode-tip")
    assert mode.input_value() == "auto" and not freq_row.is_visible()
    assert tip.text_content() == page.evaluate("t('tooltip_vbass_mode_auto')")
    request = brir_request(page)
    assert request["vbass"] is True and request["vbass_mode"] == "auto" and "vbass_freq" not in request, request
    for name in ("manual", "legacy"):
        mode.select_option(name)
        assert freq_row.is_visible(), name
        assert tip.text_content() == page.evaluate(f"t('tooltip_vbass_mode_{name}')"), name
        page.locator("#bf-vbass-freq").fill("120")
        request = brir_request(page)
        assert request["vbass_mode"] == name and request["vbass_freq"] == 120, request
    mode.select_option("auto")
    assert not freq_row.is_visible()
    page.evaluate("state.brirDefaults = {vbass_mode: 'manual', vbass_freq: 90}; applyVbassDefaults()")
    assert mode.input_value() == "manual" and freq_row.is_visible()
    assert page.locator("#bf-vbass-freq").input_value() == "90"
    page.evaluate("state.brirDefaults = {}; applyVbassDefaults()")
    assert mode.input_value() == "auto" and page.locator("#bf-vbass-freq").input_value() == "250"
    navigate(page, "settings")
    page.locator("#sf-skin").select_option("stable")
    page.wait_for_function("document.documentElement.dataset.skin === 'stable'")
    if page.locator("#job-modal").is_visible():  # Stable shows the finished job in its dialog
        page.locator("#job-modal-close").click()
    navigate(page, "processing")
    assert mode.is_visible() and not freq_row.is_visible()
    mode.select_option("manual")
    request = page.evaluate("gatherBrirPayload()")
    assert request["vbass_mode"] == "manual" and request["vbass_freq"] == 250, request
    assert not errors, errors
    context.close()
    print("Gallery virtual bass checks: modes, frequency row, tooltips, requests, defaults, Stable OK", flush=True)


def tooltip_checks(browser):
    """Every info tooltip in Room Correction and Virtual Bass, in both skins and
    both room modes: the control and the button name the bubble, keyboard focus
    opens it inside the window, Escape closes it and keeps the focus; hover
    opens it, a click pins it and a click elsewhere closes it."""
    for skin in SKINS:
        context, page, errors = open_page(browser, skin, "dark", "en", "idle")
        navigate(page, "processing")
        disclosure(page, "room", True)
        disclosure(page, "vbass", True)
        page.locator("#bf-vbass-mode").select_option("manual")  # shows the crossover frequency and its tooltip
        seen = set()
        for mode in ("eq", "tuning", "tuning2"):
            page.locator(f"#bf-room-mode-{mode}").click()
            if mode == "tuning2":
                page.locator("#bf-room-tuning2-mimo").check()  # shows MIMO's tooltip
            for button in page.locator("#dis-room button.info-tip, #dis-vbass button.info-tip").all():
                if not button.is_visible():
                    continue
                tip = button.get_attribute("aria-describedby")
                seen.add(tip)
                bubble = page.locator(f"#{tip}")
                assert bubble.get_attribute("role") == "tooltip", tip
                assert page.locator(f'[aria-describedby~="{tip}"]:not(.info-tip)').count() == 1, tip
                button.scroll_into_view_if_needed()
                button.focus()  # reach it by Tab so :focus-visible applies, as for a keyboard user
                page.keyboard.press("Shift+Tab")
                page.keyboard.press("Tab")
                page.wait_for_function(f"document.getElementById('{tip}').hasAttribute('data-open')")
                inside = page.evaluate(f"""() => {{ const r = document.getElementById('{tip}').getBoundingClientRect();
                    return r.left >= 8 && r.right <= innerWidth - 8 && r.width > 0 && r.width <= 320; }}""")
                assert inside, (skin, tip)
                page.keyboard.press("Escape")
                assert bubble.get_attribute("data-open") is None, (skin, tip)
                assert page.evaluate("document.activeElement.getAttribute('aria-describedby')") == tip, (skin, tip)
        expected = {"bf-room-mode-tip", "bf-room-range-tip", "bf-room-tuning-curtain-tip", "bf-room-tuning-level-match-tip",
                    "bf-room-tuning2-mode-tip", "bf-room-tuning2-high-tip", "bf-room-tuning2-mimo-tip",
                    "bf-room-tuning2-mimo-early-tip", "bf-room-tuning2-mimo-ear-guard-tip",
                    "bf-room-tuning2-bands-tip", "bf-room-tuning2-dip-limit-tip", "bf-room-tuning2-low-protect-tip",
                    "bf-room-tuning2-mode-decay-tip", "bf-room-tuning2-lf-align-tip", "bf-room-tuning2-hf-phase-tip",
                    "bf-room-tuning2-high-basis-tip", "bf-room-tuning2-high-transition-tip", "bf-room-tuning2-input-hp-tip",
                    "bf-vbass-mode-tip", "bf-vbass-freq-tip",
                    "bf-room-phase-limit-tip" if skin == "studio" else "bf-room-phase-schroeder-tip"}
        assert seen == expected, (skin, seen)
        # Hover opens, a click pins it past the pointer leaving, a click elsewhere closes it.
        button = page.locator('button.info-tip[aria-describedby="bf-room-tuning-level-match-tip"]')
        bubble = page.locator("#bf-room-tuning-level-match-tip")
        button.hover()
        page.wait_for_function("document.getElementById('bf-room-tuning-level-match-tip').hasAttribute('data-open')")
        button.click()
        page.mouse.move(2, 2)
        page.wait_for_timeout(300)
        assert bubble.get_attribute("data-open") is not None
        page.mouse.click(2, 2)
        assert bubble.get_attribute("data-open") is None
        assert not errors, errors
        context.close()
    print("Gallery tooltip checks: every Room Correction and Virtual Bass tooltip opens on focus inside the window, "
          "closes on Escape; hover, click pin and outside click OK", flush=True)


def shell_checks(browser):
    """Only #content scrolls, never the window: every view in both skins, with
    every Processing card open and each advanced tab shown, keeps the document
    exactly the window's size from the default window down to a small one. An
    absolutely positioned element that escapes the scroller (a .sr-only label
    did on Studio's Processing view) gives the window a second scrollbar and
    an empty band below the app."""
    size = "() => { const d = document.documentElement; return [d.scrollWidth, d.scrollHeight, d.clientWidth, d.clientHeight]; }"

    def window_fixed(page, where):
        for width, height in ((1180, 820), (960, 640), (760, 560)):
            page.set_viewport_size({"width": width, "height": height})
            doc_w, doc_h, view_w, view_h = page.evaluate(size)
            assert doc_w <= view_w and doc_h <= view_h, (where, (width, height), (doc_w, doc_h))
        page.set_viewport_size({"width": 1280, "height": 860})

    for skin in SKINS:
        context, page, errors = open_page(browser, skin, "dark", "ko", "idle")
        for view in ("recorder", "processing", "recovery", "settings", "info"):
            navigate(page, view)
            window_fixed(page, (skin, view))
        navigate(page, "processing")
        for name in ("room", "headphone", "eq", "advanced", "vbass"):
            if page.locator(f"#dis-{name} > .disclosure-head").is_visible():
                disclosure(page, name, True)
        window_fixed(page, (skin, "processing", "open"))
        for tab in ("tone", "time", "output", "correction"):
            if page.locator(f"#adv-tab-{tab}").is_visible():
                page.locator(f"#adv-tab-{tab}").click()
                window_fixed(page, (skin, "processing", tab))
        assert not errors, errors
        context.close()
    print("Gallery shell checks: the window never scrolls, only the content does, in every view of both skins OK", flush=True)


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
        dialog_checks(browser)
        channel_order_checks(browser)
        short_output_checks(browser)
        recording_cancel_checks(browser)
        fill_lfe_checks(browser)
        itd_checks(browser)
        advanced_checks(browser)
        layout_checks(browser)
        guided_checks(browser)
        polarity_checks(browser)
        room_phase_checks(browser)
        room_tuning2_checks(browser)
        room_file_checks(browser)
        brir_file_checks(browser)
        headphone_file_checks(browser)
        vbass_checks(browser)
        tooltip_checks(browser)
        shell_checks(browser)
        for skin, theme, language in itertools.product(SKINS, THEMES, LANGUAGES):
            context, page, errors = open_page(browser, skin, theme, language, "idle")
            for view in VIEWS:
                navigate(page, view)
                shots.append(shoot(page, output, f"{view}-{skin}-{language}-{theme}", errors))
            context.close()
        for theme, language in itertools.product(THEMES, LANGUAGES):
            context, page, errors = open_page(browser, "studio", theme, language, "idle")
            navigate(page, "processing")
            page.locator("#btn-generate-brir").click()
            page.locator("#brir-alignment:not([hidden])").wait_for()
            assert page.locator("#brir-alignment-rows tr").count() == 7
            assert page.locator("#brir-alignment .align-flag.warn").count() == 3
            shots.append(shoot(page, output, f"processing-itd-report-studio-{language}-{theme}", errors))
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
                answer(page)
                page.wait_for_function("document.querySelector('[data-rec-detail]').textContent.includes('AUDCLNT_E_DEVICE_IN_USE')")
                if skin == "stable":
                    # Stable runs jobs in a dialog, so the refusal is also told in a notice.
                    page.locator("#ask-modal:not([hidden])").wait_for()
                    assert "AUDCLNT_E_DEVICE_IN_USE" in page.locator("#ask-modal-message").inner_text()
                    assert page.locator("#ask-modal-cancel").is_hidden()
                assert native_dialogs(page) == 0
            shots.append(shoot(page, output, f"recorder-{skin}-{language}-{theme}-{scenario}", errors))
            context.close()
        for skin, theme, language in itertools.product(SKINS, THEMES, LANGUAGES):
            context, page, errors = open_page(browser, skin, theme, language, "idle")
            navigate(page, "recorder")
            page.locator("#rf-sweep-layout").select_option("22.2")
            page.locator("#rf-layout-use-all").click()
            page.locator("#rf-layout-row").scroll_into_view_if_needed()
            shots.append(shoot(page, output, f"recorder-{skin}-{language}-{theme}-layout-22.2", errors))
            context.close()
        for skin, theme, language in itertools.product(SKINS, THEMES, LANGUAGES):
            context, page, errors = open_page(browser, skin, theme, language, "darwin")
            navigate(page, "recorder")
            page.locator("#rf-sweep-layout").select_option("7.1")
            page.locator("#rf-channel-order-row").scroll_into_view_if_needed()
            shots.append(shoot(page, output, f"recorder-{skin}-{language}-{theme}-hdmi-order", errors))
            page.locator("#btn-start-recording").click()
            page.locator("#ask-modal:not([hidden])").wait_for()
            shots.append(shoot(page, output, f"recorder-{skin}-{language}-{theme}-confirm", errors))
            context.close()
        for skin, theme, language in itertools.product(SKINS, THEMES, LANGUAGES):
            context, page, errors = open_page(browser, skin, theme, language, "darwin")
            navigate(page, "recorder")
            page.locator("#rf-sweep-layout").select_option("7.1.4")
            page.locator("#rf-channel-shortage").scroll_into_view_if_needed()
            shots.append(shoot(page, output, f"recorder-{skin}-{language}-{theme}-short-output", errors))
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
            page.locator("#adv-tab-output").click()
            page.locator("#adv-studio").scroll_into_view_if_needed()
            shots.append(shoot(page, output, f"processing-advanced-studio-{language}-{theme}-output-auto", errors))
            page.locator("#ba-layout-files").select_option("listed")
            page.locator("#ba-layout-force-2").check()
            for tab in ADV_TABS:
                page.locator(f"#adv-tab-{tab}").click()
                page.locator("#adv-studio").scroll_into_view_if_needed()
                shots.append(shoot(page, output, f"processing-advanced-studio-{language}-{theme}-{tab}", errors))
            context.close()
        for skin, theme, language in itertools.product(SKINS, THEMES, LANGUAGES):
            context, page, errors = open_page(browser, skin, theme, language, "idle")
            navigate(page, "processing")
            disclosure(page, "room", True)
            page.locator("#bf-room-mode-tuning2").click()
            page.locator("#bf-room-tuning2-mimo").check()
            shots.append(shoot(page, output, f"processing-room-tuning2-{skin}-{language}-{theme}", errors))
            if skin == "studio":
                page.locator("#bf-room-tuning2-mimo").uncheck()
                page.locator("#bf-room-tuning2-bands").fill("1000-3000, 6000-9000")
                page.locator("#bf-room-tuning2-dip-limit").check()
                page.locator("#bf-room-tuning2-low-protect").check()
                page.locator("#bf-room-tuning2-bands-row").scroll_into_view_if_needed()
                shots.append(shoot(page, output, f"processing-tuning2-bundle1-studio-{language}-{theme}", errors))
                for option in ("mode-decay", "lf-align", "hf-phase"):
                    page.locator(f"#bf-room-tuning2-{option}").check()
                page.locator("#bf-room-tuning2-mode-row").scroll_into_view_if_needed()
                shots.append(shoot(page, output, f"processing-tuning2-bundle2-studio-{language}-{theme}", errors))
                page.locator("#bf-room-tuning2-mode").select_option("zero")
                page.locator("#bf-room-tuning2-fir-hint").scroll_into_view_if_needed()
                shots.append(shoot(page, output, f"processing-tuning2-bundle2-zero-studio-{language}-{theme}", errors))
                page.locator("#bf-room-tuning2-mode").select_option("mixed")
                page.locator("#bf-room-tuning2-high-basis").select_option("lenient")
                page.locator("#bf-room-tuning2-high-transition").select_option("2")
                page.locator("#bf-room-tuning2-input-hp").fill("25")
                assert page.locator("#bf-room-tuning2-high-volume-hint").is_visible()
                page.locator("#bf-room-tuning2-high-basis-row").scroll_into_view_if_needed()
                shots.append(shoot(page, output, f"processing-tuning2-bundle3-studio-{language}-{theme}", errors))
                page.locator("#bf-room-tuning2-mimo").check()
                page.locator("#bf-room-tuning2-mimo-early").fill("20")
                page.locator("#bf-room-tuning2-mimo-ear-guard").check()
                page.locator("#bf-room-tuning2-mimo-ear-guard-row").scroll_into_view_if_needed()
                shots.append(shoot(page, output, f"processing-tuning2-bundle4-studio-{language}-{theme}", errors))
            context.close()
        for skin, theme, language in itertools.product(SKINS, THEMES, LANGUAGES):
            context, page, errors = open_page(browser, skin, theme, language, "idle")
            navigate(page, "processing")
            disclosure(page, "room", True)
            page.locator("#bf-room-source").select_option("file")
            for kind, path, rows in (("rew", "D:/Measurements/living room.mdat", 4),
                                     ("dirac", "D:/Dirac/living room.liveproject", 6)):
                page.locator("#bf-room-file").fill(path)
                page.locator("#bf-room-file-result tbody tr").nth(rows - 1).wait_for()
                if kind == "rew":
                    page.locator("#bf-room-file-result tbody tr").nth(3).locator('select[data-room-field="slot"]').select_option("FR")
                page.locator("#bf-room-file-block").scroll_into_view_if_needed()
                shots.append(shoot(page, output, f"processing-room-file-{skin}-{language}-{theme}-{kind}", errors))
            context.close()
        for skin, theme, language in itertools.product(SKINS, THEMES, LANGUAGES):
            context, page, errors = open_page(browser, skin, theme, language, "idle")
            navigate(page, "processing")
            page.locator("#bf-brir-source").select_option("file")
            for kind, path, rows in (("rew", "D:/Measurements/living room.mdat", 4),
                                     ("dirac", "D:/Dirac/living room.liveproject", 6)):
                page.locator("#bf-brir-file").fill(path)
                page.locator("#bf-brir-file-result tbody tr").nth(rows - 1).wait_for()
                if kind == "rew":  # a speaker with one ear, as the screen reports it
                    page.locator("#bf-brir-file-result tbody tr").nth(3).locator('select[data-room-field="slot"]').select_option("FR")
                page.locator("#bf-brir-file-block").scroll_into_view_if_needed()
                shots.append(shoot(page, output, f"processing-brir-file-{skin}-{language}-{theme}-{kind}", errors))
            context.close()
        for skin, theme, language in itertools.product(SKINS, THEMES, LANGUAGES):
            context, page, errors = open_page(browser, skin, theme, language, "idle")
            navigate(page, "processing")
            disclosure(page, "headphone", True)
            page.locator("#bf-hp-source").select_option("file")
            for kind, path, value in (("rew", "D:/Measurements/living room.mdat", "1"),
                                      ("dirac", "D:/Dirac/living room.liveproject", "1@1")):
                page.locator("#bf-hp-file").fill(path)
                page.wait_for_function(f"document.querySelector('#bf-hp-file-result select[data-room-field=\"hp-left\"]')?.value === '{value}'")
                page.locator("#bf-hp-file-block").scroll_into_view_if_needed()
                shots.append(shoot(page, output, f"processing-headphone-file-{skin}-{language}-{theme}-{kind}", errors))
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
        for theme, language in itertools.product(THEMES, LANGUAGES):
            context, page, errors = open_page(browser, "studio", theme, language, "idle")
            navigate(page, "recorder")
            page.locator("#gd-setup").select_option("stereo")
            page.locator("#gd-steps .gd-step").nth(3).click()
            shots.append(shoot(page, output, f"recorder-guided-{language}-{theme}", errors))
            context.close()
        for language in LANGUAGES:
            context, page, errors = open_page(browser, "studio", "dark", language, "polarity")
            polarity_take(page)
            shots.append(shoot(page, output, f"recorder-polarity-studio-{language}-dark", errors))
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
