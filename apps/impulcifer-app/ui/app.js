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
   the folder input).
   Both pages (index.html and android.html) share this table. A part with a
   `root` runs only on a page that has an element with that id, so a part
   whose markup is not on the page does nothing at all.
   boot() runs the start-up hooks in table order: `boot` with bootstrap's
   answer before the page's words are drawn, `load` (awaited together) once
   they are, then `ready`. `job` shows the main channel's job (renderJobState). */
/** @typedef {{ name: string, root?: string, wire?: () => void, render?: () => void,
 *   addArgs?: (args: ProcessingRequest) => void, boot?: (data: Bootstrap) => void,
 *   load?: () => Promise<void>, ready?: (data: Bootstrap) => void,
 *   job?: (job: Job | null) => void }} Feature */
/** @type {readonly Feature[]} */
const FEATURES = [
  { name: "shell", wire: wireShell },
  // A page loads only the scripts of the parts it has: the desktop page has no chrome.js,
  // storage.js, chooser.js, sheet.js, tipsheet.js and optionsheet.js, and the Android page no
  // recorder.js and guide.js. Those parts name their functions inside arrows, so a name is
  // looked up only on a page that has the markup (and so the script).
  { name: "android shell", root: "app-bar", wire: () => wireAndroidShell(), render: () => renderAndroidShell(),
    job: () => syncBackButton() },
  { name: "recorder", root: "view-recorder", wire: () => wireRecorder(), render: () => renderRecorder(),
    boot: (data) => bootRecorder(data), load: () => loadDevices(), ready: () => { refreshResolvedPath(); },
    job: (job) => showRecordingCancel(job) },
  { name: "dialogs", root: "ask-modal", wire: wireDialogs },
  { name: "guided", root: "guided-card", wire: () => wireGuided(), render: () => renderGuided(),
    boot: () => populateGuidedTargets() },
  { name: "advanced", root: "adv-studio", wire: wireAdvancedForms, render: renderAdvancedForms,
    addArgs: addAdvancedArgs, boot: applyTimeDefaults },
  { name: "test signal", root: "bf-test-signal-source", wire: wireTestSignal, addArgs: addTestSignalArgs,
    boot: () => { updateTestSignalVisibility(); updateTestSignalScope(); } },
  { name: "room", root: "dis-room", wire: wireRoom, render: renderRoom, addArgs: addRoomAndVbassArgs, boot: bootRoom },
  { name: "measurement files", root: "bf-brir-source", wire: wireMfile, render: renderMfiles,
    addArgs: addMeasurementFileArgs },
  { name: "eq", root: "dis-eq", wire: wireEq, render: renderEq, addArgs: addEqArgs, ready: () => scheduleEqInspection(0) },
  { name: "jobs", root: "btn-generate-brir", wire: wireJobs, render: renderJobs },
  { name: "job dialog", root: "job-modal", wire: wireJobModal, job: updateJobModal },
  { name: "job sheet", root: "job-sheet", wire: () => wireJobSheet(), render: () => renderJobSheetWords(),
    job: (job) => showJobInSheet(job) },
  { name: "recovery", root: "view-recovery", wire: wireRecovery, render: renderRecovery, ready: scheduleRecoveryPlan },
  { name: "storage", root: "storage-notice", wire: () => wireStorage(), render: () => renderStorage(),
    ready: () => { refreshStorageAccess(); } },
  { name: "chooser", root: "chooser", wire: () => wireChooser(), render: () => renderChooserWords() },
  { name: "tip sheet", root: "tip-sheet", render: () => fillTipSheet() },
  { name: "option sheet", root: "option-sheet", wire: () => wireOptionSheet() },
  { name: "settings", root: "view-settings", wire: wireSettings, render: renderSettings, boot: bootSettings,
    load: loadSystemInfo, ready: readySettings },
  { name: "update", root: "update-modal", wire: wireUpdate, ready: () => window.setTimeout(() => checkForUpdates(false), 2000) },
];

/** @param {Feature} feature */
function featurePresent(feature) {
  return !feature.root || document.getElementById(feature.root) !== null;
}

/* The parts whose markup is on this page, in table order. */
function presentFeatures() {
  return FEATURES.filter(featurePresent);
}

/* ---------------------------------------------------------------- wiring */

function wireEvents() {
  for (const feature of presentFeatures()) feature.wire?.();
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
      head.parentElement?.dispatchEvent(new Event("disclosure-toggle", { bubbles: true }));
      for (const p of advPrefixes()) refreshAdvanced(p);
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
      const target = el(button.dataset.target || "", HTMLInputElement);
      const path = await pickPath(button.dataset.browse || "dir", target.value);
      if (path) {
        target.value = path;
        target.dispatchEvent(new Event("input"));
      }
    });
  });

  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-open-url]")).forEach((button) => {
    button.addEventListener("click", () => api().open_url(button.dataset.openUrl || ""));
  });
}

/* A folder ("dir") or a file of a kind ("audio", "text", "wav", "measurement")
   for a browse button or the EQ slots. A page with its own chooser (#chooser,
   the Android page, where the system dialogs cannot hand over a path) opens
   it at `start`; otherwise the native dialog asks. Null when nothing was chosen. */
/** @param {string} kind @param {string} start @returns {Promise<string | null>} */
async function pickPath(kind, start) {
  if (document.getElementById("chooser")) return openChooser(kind, start);
  const response = kind === "dir" ? await api().select_directory() : await api().select_file(kind);
  return response.ok && response.data.path ? response.data.path : null;
}

/* -------------------------------------------------------------- language */

function applyStrings() {
  document.documentElement.lang = state.language;
  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-i18n]")).forEach((node) => {
    node.textContent = t(node.dataset.i18n || "");
  });
  // Every info button has the same name; its description is the bubble it opens.
  document.querySelectorAll(".info-tip").forEach((button) => button.setAttribute("aria-label", t("button_more_info")));
  for (const feature of presentFeatures()) feature.render?.();
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
  for (const feature of presentFeatures()) feature.addArgs?.(args);
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
  state.sweepDefaults = data.sweep || {};
  if (data.ui) {
    state.strings = data.ui.strings || {};
    state.language = data.ui.language || "en";
  }
  for (const feature of presentFeatures()) feature.boot?.(data);
  applyStrings();
  /** @type {Record<string, string>} */
  const backendLabels = { edgechromium: "WebView2", cocoa: "WKWebView", gtk: "WebKitGTK" };
  const backendLabel = backendLabels[data.webview_backend] || "WebView";
  const brand = document.getElementById("brand-version");
  if (brand) brand.textContent = `v${data.version}`;
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

  // The devices and the system information, then the checks that need them;
  // the first-run language choice and, 2 s later as CTk's root.after(2000),
  // the silent update check.
  await Promise.all(presentFeatures().map((feature) => feature.load?.()));
  for (const feature of presentFeatures()) feature.ready?.(data);
  document.documentElement.dataset.booted = "true";
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

for (const p of advPrefixes()) buildDecayGrid(p);
wireEvents();
// An info button opens a bubble beside it, or on a page with a tip sheet (Android) a sheet from the bottom.
if (document.getElementById("tip-sheet")) wireTipSheet();
else wireTips();
// Studio's locks follow the page from the start; Stable's markup already matches its defaults.
refreshAdvanced("ba");

window.addEventListener("pywebviewready", boot);
if (window.pywebview && window.pywebview.api) boot();
