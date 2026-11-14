// @ts-check
// Event wiring, boot and the calls that start the page; loads last.
"use strict";

/* ---------------------------------------------------------------- wiring */

function buildDecayGrid() {
  const grid = $("bf-decay-grid");
  for (const channel of DECAY_CHANNELS) {
    const label = document.createElement("span");
    label.className = "mini-label mono";
    label.textContent = `${channel}:`;
    const input = document.createElement("input");
    input.type = "number";
    input.min = "0";
    input.id = `bf-decay-${channel}`;
    input.className = "num mono";
    grid.append(label, input);
  }
}

function buildStudioDecayGrid() {
  const grid = $("ba-decay-grid");
  for (const channel of DECAY_CHANNELS) {
    const cell = document.createElement("label");
    cell.className = "ch-cell";
    const code = document.createElement("span");
    code.className = "ch-code mono";
    code.textContent = channel;
    const field = document.createElement("span");
    field.className = "unit-field";
    const input = document.createElement("input");
    input.type = "number";
    input.min = "0";
    input.placeholder = "—";
    input.id = `ba-decay-${channel}`;
    const unit = document.createElement("span");
    unit.className = "unit";
    unit.dataset.i18n = "studio_adv_unit_ms";
    unit.textContent = t("studio_adv_unit_ms");
    field.append(input, unit);
    cell.append(code, field);
    grid.append(cell);
  }
}

function wireEvents() {
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
      refreshStudioAdvanced();
      updateSofaSplit("bf");
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

  el("btn-refresh-devices", HTMLButtonElement).addEventListener("click", () => loadDevices(val("rf-host-api")));
  el("rf-host-api", HTMLSelectElement).addEventListener("change", () => loadDevices(val("rf-host-api")));

  el("rf-play", HTMLInputElement).addEventListener("input", () => {
    refreshResolvedPath();
    updateChannelOrder();
  });
  el("rf-output-device", HTMLSelectElement).addEventListener("change", updateChannelOrder);
  el("rf-channel-order", HTMLSelectElement).addEventListener("change", updateChannelOrder);
  el("ask-modal-ok", HTMLButtonElement).addEventListener("click", () => closeAsk(true));
  el("ask-modal-cancel", HTMLButtonElement).addEventListener("click", () => closeAsk(false));
  document.addEventListener("keydown", onAskKey);
  el("rf-record-dir", HTMLInputElement).addEventListener("input", () => {
    refreshResolvedPath();
    renderGuided();
  });
  el("rf-append", HTMLInputElement).addEventListener("change", refreshResolvedPath);
  el("rf-sweep-source", HTMLSelectElement).addEventListener("change", () => {
    updateSweepSourceVisibility();
    refreshResolvedPath();
    updateChannelOrder();
  });
  el("rf-sweep-speakers", HTMLInputElement).addEventListener("input", () => {
    refreshResolvedPath();
    updateChannelOrder();
  });
  el("btn-cancel-recording", HTMLButtonElement).addEventListener("click", cancelActiveJob);
  el("rf-sweep-layout", HTMLSelectElement).addEventListener("change", () => {
    updateChannelOrder();
    useLayoutSpeakers();
  });
  el("rf-layout-use-all", HTMLButtonElement).addEventListener("click", useLayoutSpeakers);
  el("gd-setup", HTMLSelectElement).addEventListener("change", populateGuidedTargets);
  el("gd-target", HTMLSelectElement).addEventListener("change", () => showGuidedStep(0));
  el("btn-gd-record", HTMLButtonElement).addEventListener("click", recordGuidedStep);
  el("btn-gd-cancel", HTMLButtonElement).addEventListener("click", stopGuidedCountdown);
  el("btn-gd-prev", HTMLButtonElement).addEventListener("click", () => showGuidedStep(state.guided.index - 1));
  el("btn-gd-next", HTMLButtonElement).addEventListener("click", () => showGuidedStep(state.guided.index + 1));
  for (const p of ["bf", "ba"]) {
    el(`${p}-layout-files`, HTMLSelectElement).addEventListener("change", () => updateLayoutForce(p));
    el(`${p}-lfe-mode`, HTMLSelectElement).addEventListener("change", () => updateLfeHint(p));
    el(`${p}-itd-mode`, HTMLSelectElement).addEventListener("change", () => updateItdHint(p));
  }
  el("bf-test-signal-source", HTMLSelectElement).addEventListener("change", () => {
    updateTestSignalVisibility();
    scheduleTestSignalCheck(0);
  });
  for (const id of ["bf-test-signal", "bf-dir-path", "bf-headphone-file"]) {
    el(id, HTMLInputElement).addEventListener("input", () => scheduleTestSignalCheck());
  }
  el("bf-room-range", HTMLSelectElement).addEventListener("change", updateRoomRows);
  el("bf-vbass-mode", HTMLSelectElement).addEventListener("change", updateVbassRows);
  ROOM_MODES.forEach((mode, index) => {
    const button = $(`bf-room-mode-${mode}`);
    button.addEventListener("click", () => setRoomMode(mode));
    // Radio group: the arrows move the choice itself, wrapping at the ends.
    button.addEventListener("keydown", (event) => {
      const last = ROOM_MODES.length - 1;
      const next = index === last ? 0 : index + 1;
      const previous = index === 0 ? last : index - 1;
      /** @type {Record<string, number>} */
      const moves = { ArrowRight: next, ArrowDown: next, ArrowLeft: previous, ArrowUp: previous, Home: 0, End: last };
      if (!Object.prototype.hasOwnProperty.call(moves, event.key)) return;
      event.preventDefault();
      setRoomMode(ROOM_MODES[moves[event.key]], true);
    });
  });
  // The limit decides which rows below it show (the room facts, the delay).
  el("bf-room-phase-limit", HTMLInputElement).addEventListener("input", updateRoomRows);
  el("bf-room-phase-schroeder", HTMLInputElement).addEventListener("change", onRoomPhaseCheckbox);
  el("bf-room-tuning-delay-auto", HTMLInputElement).addEventListener("change", updateRoomRows);
  // Tuning 2: the filter shows the latency row, an empty upper end the room facts, MIMO its limits.
  el("bf-room-tuning2-mode", HTMLSelectElement).addEventListener("change", updateRoomRows);
  for (const id of ["bf-room-tuning2-high", "bf-room-volume", "bf-schroeder-freq"]) {
    el(id, HTMLInputElement).addEventListener("input", updateRoomRows);
  }
  for (const id of ["bf-room-tuning2-high-basis", "bf-room-tuning2-high-transition"]) {
    el(id, HTMLSelectElement).addEventListener("change", updateRoomRows);
  }
  el("bf-room-tuning2-mimo", HTMLInputElement).addEventListener("change", updateRoomRows);
  // The source select also renders the file block (below); this one shows the sweep file row.
  el("bf-brir-recording-sweep-source", HTMLSelectElement).addEventListener("change", updateBrirRecordingRows);
  for (const id of ["bf-brir-recording-file", "bf-brir-recording-speakers", "bf-brir-recording-sweep"]) {
    el(id, HTMLInputElement).addEventListener("input", updateBrirRecordingStatus);
  }
  el("bf-dir-path", HTMLInputElement).addEventListener("input", () => scheduleEqInspection());
  for (const purpose of MFILE_PURPOSES) {
    const ids = MFILE[purpose];
    el(ids.source, HTMLSelectElement).addEventListener("change", () => {
      renderMfile(purpose);
      scheduleMfileInspection(purpose, 0);
      scheduleTestSignalCheck(0);
    });
    el(ids.prefix, HTMLInputElement).addEventListener("input", () => scheduleMfileInspection(purpose));
    $(`${ids.prefix}-result`).addEventListener("change", (event) => onMfileChange(purpose, event));
  }
  // The preview is drawn at its rendered width; redraw when that changes
  // (window resize, or the disclosure opening from display: none).
  let eqPreviewWidth = 0;
  new ResizeObserver((entries) => {
    const width = Math.round(entries[0].contentRect.width);
    if (width && width !== eqPreviewWidth) {
      eqPreviewWidth = width;
      renderEqPreview();
    }
  }).observe($("bf-eq-result"));
  el("btn-detect-sweep", HTMLButtonElement).addEventListener("click", detectSweep);
  el("rf-force-channels", HTMLInputElement).addEventListener("change", updateChannelGuidance);
  el("rf-channels", HTMLInputElement).addEventListener("input", updateChannelGuidance);

  el("btn-start-recording", HTMLButtonElement).addEventListener("click", startSpeakersRecording);
  el("btn-record-headphones", HTMLButtonElement).addEventListener("click", startHeadphonesRecording);
  el("btn-sweep-set", HTMLButtonElement).addEventListener("click", generateSweepSet);

  for (const id of ["bf-resample", "bf-balance", "bf-decay-per-channel", "bf-mic-deviation", "bf-sofa"]) {
    $(id).addEventListener("change", refreshStableAdvanced);
  }

  ADV_TABS.forEach((tab, index) => {
    const button = $(`adv-tab-${tab}`);
    button.addEventListener("click", () => selectAdvTab(tab));
    button.addEventListener("keydown", (event) => {
      const last = ADV_TABS.length - 1;
      /** @type {Record<string, number>} */
      const moves = { ArrowRight: index === last ? 0 : index + 1, ArrowLeft: index === 0 ? last : index - 1, Home: 0, End: last };
      if (!Object.prototype.hasOwnProperty.call(moves, event.key)) return;
      event.preventDefault();
      selectAdvTab(ADV_TABS[moves[event.key]], true);
    });
    $(`adv-on-${tab}`).addEventListener("click", () => setAdvOn(tab, !advOn(tab)));
    $(`adv-reset-${tab}`).addEventListener("click", () => resetAdvTab(tab));
  });
  el("ba-bass-gain", HTMLInputElement).addEventListener("input", refreshStudioAdvanced);
  for (const id of ["ba-balance", "ba-resample", "ba-decay-per-channel", "ba-mic-deviation", "ba-sofa"]) {
    $(id).addEventListener("change", refreshStudioAdvanced);
  }

  el("btn-generate-brir", HTMLButtonElement).addEventListener("click", () => {
    if (brirRecordingProblem()) {
      updateBrirRecordingStatus();
      $("bf-brir-recording-block").scrollIntoView({ block: "center" });
      return;
    }
    const blocked = MFILE_PURPOSES.find((purpose) => mfileActive(purpose) && mfileBlocked(purpose));
    if (blocked) {
      renderMfile(blocked);
      $(`${MFILE[blocked].prefix}-block`).scrollIntoView({ block: "center" });
      return;
    }
    begin((request) => api().start_brir(request), gatherBrirPayload());
  });
  el("btn-start-recovery", HTMLButtonElement).addEventListener("click", () =>
    begin(
      (request) => api().start_output_recovery(request),
      recoveryRequest(),
      "output_recovery",
    ),
  );
  el("btn-cancel-brir", HTMLButtonElement).addEventListener("click", cancelActiveJob);
  el("job-modal-cancel", HTMLButtonElement).addEventListener("click", cancelActiveJob);
  el("job-modal-close", HTMLButtonElement).addEventListener("click", () => {
    state.modalDismissed = true;
    $("job-modal").hidden = true;
  });
  el("btn-open-output", HTMLButtonElement).addEventListener("click", () => {
    if (state.lastOutputDir) api().open_path(state.lastOutputDir);
  });
  el("btn-open-recovery-output", HTMLButtonElement).addEventListener("click", () => {
    if (state.lastRecoveryOutputDir) api().open_path(state.lastRecoveryOutputDir);
  });

  el("btn-open-data", HTMLButtonElement).addEventListener("click", () => api().open_path(state.systemInfo?.paths.data_dir));
  el("btn-open-settings", HTMLButtonElement).addEventListener("click", () => {
    const path = state.systemInfo?.paths.settings_path;
    if (path) api().open_path(parentFolder(path));
  });
  for (const id of ["recovery-dir-path", "recovery-include-hangloose", "recovery-remove-silent-channels"]) {
    $(id).addEventListener("input", scheduleRecoveryPlan);
    $(id).addEventListener("change", scheduleRecoveryPlan);
  }
  el("sf-skin", HTMLSelectElement).addEventListener("change", () => changeSkin(val("sf-skin")));
  el("sf-theme", HTMLSelectElement).addEventListener("change", () => changeTheme(val("sf-theme")));
  el("sf-language", HTMLSelectElement).addEventListener("change", () => changeLanguage(val("sf-language")));

  el("btn-check-updates", HTMLButtonElement).addEventListener("click", () => checkForUpdates(true));
  el("update-now", HTMLButtonElement).addEventListener("click", beginUpdate);
  el("update-remind", HTMLButtonElement).addEventListener("click", hideUpdateModal);
  el("update-skip", HTMLButtonElement).addEventListener("click", hideUpdateModal);
  el("update-close", HTMLButtonElement).addEventListener("click", hideUpdateModal);
  el("update-restart", HTMLButtonElement).addEventListener("click", applyStagedUpdate);
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

  const activeJob = data.active_job;
  if (activeJob && activeJob.kind === "update") {
    if (!["succeeded", "failed", "cancelled"].includes(activeJob.status)) {
      updateState.jobId = activeJob.job_id;
      updateState.nextSeq = 0;
      showUpdateProgressOnly();
      pollUpdateJob();
    }
  } else {
    renderJobState(activeJob);
    if (activeJob && !["succeeded", "failed", "cancelled"].includes(activeJob.status)) {
      state.jobId = activeJob.job_id;
      state.jobKind = activeJob.kind;
      state.nextSeq = 0;
      resetSteps(activeJob.kind === "brir");
      if (activeJob.kind === "brir") renderAlignment(null);
      resetRecorderStatus();
      schedulePoll(0);
    }
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

buildDecayGrid();
buildStudioDecayGrid();
wireEvents();
wireTips();
refreshStudioAdvanced();
updateChannelGuidance();

window.addEventListener("pywebviewready", boot);
if (window.pywebview && window.pywebview.api) boot();
