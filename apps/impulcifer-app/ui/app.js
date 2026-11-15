// @ts-check
// The parts of the screen (wiring, language, BRIR request), the shell's listeners, boot and the calls that start
// the page; loads last.
"use strict";

/* ----------------------------------------------------------------- parts */

/* Each part of the screen, in its own file, wires its controls (`wire`),
   draws again what follows the language or the skin (`render`) and adds its
   fields to the BRIR request behind its own section switch (`addArgs`). The
   table is read only at run time, after every script has loaded. Parts are
   wired in this order, so two parts that listen to the same element and
   event keep their order (the test signal check, then the EQ inspection, on
   the folder input). */
/** @typedef {{ name: string, wire?: () => void, render?: () => void,
 *   addArgs?: (args: ProcessingRequest) => void }} Feature */
/** @type {readonly Feature[]} */
const FEATURES = [
  { name: "shell", wire: wireShell },
  { name: "recorder", wire: wireRecorder, render: renderRecorder },
  { name: "dialogs", wire: wireDialogs },
  { name: "guided", wire: wireGuided, render: renderGuided },
  { name: "advanced", wire: wireAdvancedForms, render: renderAdvancedForms, addArgs: addAdvancedArgs },
  { name: "test signal", wire: wireTestSignal, addArgs: addTestSignalArgs },
  { name: "room", wire: wireRoom, render: renderRoom, addArgs: addRoomAndVbassArgs },
  { name: "measurement files", wire: wireMfile, render: renderMfiles, addArgs: addMeasurementFileArgs },
  { name: "eq", wire: wireEq, render: renderEq, addArgs: addEqArgs },
  { name: "jobs", wire: wireJobs, render: renderJobs },
  { name: "recovery", wire: wireRecovery, render: renderRecovery },
  { name: "settings", wire: wireSettings, render: renderSettings },
  { name: "update", wire: wireUpdate },
];

/* ---------------------------------------------------------------- wiring */

function wireEvents() {
  for (const feature of FEATURES) feature.wire?.();
}

/* The navigation, the section switches, the browse buttons and the links. */
function wireShell() {
  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll(".nav-item")).forEach((item) => {
    item.addEventListener("click", () => {
      /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll(".nav-item")).forEach((node) => node.classList.remove("active"));
      /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll(".view")).forEach((node) => node.classList.remove("active"));
      item.classList.add("active");
      $(`view-${item.dataset.view}`).classList.add("active");
    });
  });

  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-disclosure]")).forEach((head) => {
    const toggle = () => {
      head.parentElement?.classList.toggle("open");
      for (const p of ADV_PREFIXES) refreshAdvanced(p);
      scheduleTestSignalCheck(0);
    };
    head.addEventListener("click", toggle);
    head.querySelector(".switch")?.addEventListener("keydown", (event) => {
      if (event instanceof KeyboardEvent && (event.key === "Enter" || event.key === " ")) {
        event.preventDefault();
        toggle();
      }
    });
  });

  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-browse]")).forEach((button) => {
    button.addEventListener("click", async () => {
      const kind = button.dataset.browse;
      const response = kind === "dir"
        ? await api().select_directory()
        : await api().select_file(kind);
      if (response.ok && response.data.path) {
        const target = el(button.dataset.target || "", HTMLInputElement);
        target.value = response.data.path;
        target.dispatchEvent(new Event("input"));
      }
    });
  });

  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-open-url]")).forEach((button) => {
    button.addEventListener("click", () => api().open_url(button.dataset.openUrl || ""));
  });
}

/* -------------------------------------------------------------- language */

function applyStrings() {
  document.documentElement.lang = state.language;
  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-i18n]")).forEach((node) => {
    node.textContent = t(node.dataset.i18n || "");
  });
  // Every info button has the same name; its description is the bubble it opens.
  document.querySelectorAll(".info-tip").forEach((button) => button.setAttribute("aria-label", t("button_more_info")));
  for (const feature of FEATURES) feature.render?.();
  if (state.version) $("runtime-status").textContent = `v${state.version} · ${state.platform} · ${t("webview_bridge_connected")}`;
}

/* ---------------------------------------------------------- BRIR request */

function gatherBrirPayload() {
  /** @type {ProcessingRequest} */
  const args = {
    dir_path: val("bf-dir-path"),
    plot: checked("bf-plot"),
    do_room_correction: isOpen("dis-room"),
    do_headphone_compensation: isOpen("dis-headphone"),
    do_equalization: isOpen("dis-eq"),
  };
  for (const feature of FEATURES) feature.addArgs?.(args);
  return args;
}

/* ------------------------------------------------------------------ boot */

async function boot() {
  if (state.booted) return;
  state.booted = true;

  let response;
  try {
    response = await api().bootstrap();
  } catch (error) {
    $("runtime-status").textContent = tPreboot("webview_bridge_failed");
    appendLog(String(error));
    return;
  }
  if (!response.ok) {
    $("runtime-status").textContent = tPreboot("webview_bridge_failed");
    appendLog(errorText(response));
    return;
  }
  const data = response.data;
  state.version = data.version;
  state.platform = data.platform;
  state.shareModes = data.capabilities.share_modes;
  state.brirDefaults = data.brir_defaults || {};
  applyRoomDefaults();
  applyVbassDefaults();
  applyTimeDefaults();
  state.sweepDefaults = data.sweep || {};
  populateSweepLayouts(
    (data.sweep && data.sweep.layouts) || ["mono", "stereo", "5.1", "7.1", "7.1.4", "7.1.6"]
  );
  populateGuidedTargets();
  updateSweepSourceVisibility();
  updateTestSignalVisibility();
  if (data.ui) {
    state.strings = data.ui.strings || {};
    state.language = data.ui.language || "en";
    populateLanguages(data.ui.languages || []);
    el("sf-theme", HTMLSelectElement).value = data.ui.theme || "dark";
    applyTheme(data.ui.theme || "dark");
    el("sf-skin", HTMLSelectElement).value = data.ui.skin === "stable" ? "stable" : "studio";
    applySkin(data.ui.skin);
  }
  applyStrings();
  /** @type {Record<string, string>} */
  const backendLabels = { edgechromium: "WebView2", cocoa: "WKWebView", gtk: "WebKitGTK" };
  const backendLabel = backendLabels[data.webview_backend] || "WebView";
  $("brand-version").textContent = `v${data.version}`;
  $("runtime-status").textContent = `v${data.version} · ${data.platform} · ${backendLabel}`;
  appendLog(t("webview_bridge_connected"));

  // A job still running from before a reload is followed again.
  const activeJob = data.active_job;
  if (activeJob && activeJob.kind === "update") {
    if (!isTerminal(activeJob.status)) {
      showUpdateProgressOnly();
      followJob(updateState, activeJob, UPDATE_JOB);
    }
  } else {
    renderJobState(activeJob);
    if (activeJob && !isTerminal(activeJob.status)) followMainJob(activeJob);
  }

  await Promise.all([loadDevices(), loadSystemInfo()]);
  refreshResolvedPath();
  scheduleRecoveryPlan();
  scheduleEqInspection(0);

  // First run: ask for the language before anything else (CTk parity).
  if (data.ui && data.ui.first_run) {
    showFirstRunLanguageModal(data.ui.languages || []);
  }

  // Mirror the CTk root.after(2000) startup update check; failures stay
  // silent and never block the UI.
  window.setTimeout(() => checkForUpdates(false), 2000);
}

/** @param {Language[]} languages */
function showFirstRunLanguageModal(languages) {
  const list = $("language-modal-list");
  list.replaceChildren();
  languages.forEach(({ code, name }) => {
    const button = document.createElement("button");
    button.className = "btn btn-secondary";
    button.type = "button";
    button.textContent = name;
    button.addEventListener("click", async () => {
      // set_language persists the choice and marks language_selected.
      await changeLanguage(code);
      el("sf-language", HTMLSelectElement).value = code;
      $("language-modal").hidden = true;
    });
    list.appendChild(button);
  });
  if (list.childElementCount > 0) $("language-modal").hidden = false;
}

for (const p of ADV_PREFIXES) buildDecayGrid(p);
wireEvents();
wireTips();
// Studio's locks follow the page from the start; Stable's markup already matches its defaults.
refreshAdvanced("ba");
updateChannelGuidance();

window.addEventListener("pywebviewready", boot);
if (window.pywebview && window.pywebview.api) boot();
