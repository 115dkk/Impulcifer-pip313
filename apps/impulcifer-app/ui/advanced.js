// @ts-check
// The advanced options of both skins: their rules, Studio's tabs, the carry-over and the request fields.
"use strict";

/* Advanced options are read from one of two forms with the same field
   suffixes: Stable's single list ("bf-") and Studio's four tabs ("ba-").
   Every function below that takes `p` works on either form. */
/** @typedef {"bf" | "ba"} AdvPrefix */
/** @type {readonly AdvPrefix[]} */
const ADV_PREFIXES = ["bf", "ba"];

/* The forms this page has: the Android page has Studio's alone. A form is
   there when its per-channel decay grid is. */
/** @returns {AdvPrefix[]} */
function advPrefixes() {
  return ADV_PREFIXES.filter((p) => document.getElementById(`${p}-decay-grid`) !== null);
}

/** @param {ProcessingRequest} args @param {string} p @param {boolean} equalizing */
function addToneArgs(args, p, equalizing) {
  // Bass boost and tilt shape the equalization target, so they only apply
  // when an equalization stage runs; Studio locks them otherwise.
  if (equalizing) {
    const bassGain = numOr(`${p}-bass-gain`, 0);
    if (bassGain) {
      args.bass_boost_gain = bassGain;
      args.bass_boost_fc = numOr(`${p}-bass-fc`, brirDefault("bass_boost_fc", 105));
      args.bass_boost_q = numOr(`${p}-bass-q`, brirDefault("bass_boost_q", 0.76));
    }
    const tilt = numOr(`${p}-tilt`, 0);
    if (tilt) args.tilt = tilt;
  }
  args.target_level = numOrNull(`${p}-target-level`);
  const balance = val(`${p}-balance`);
  if (balance === "number") args.channel_balance = Math.trunc(numOr(`${p}-balance-db`, 0));
  else if (balance !== "none") args.channel_balance = balance;
}

/** @param {ProcessingRequest} args @param {string} p */
function addTimeArgs(args, p) {
  if (checked(`${p}-decay-per-channel`)) {
    /** @type {Record<string, number>} */
    const decay = {};
    for (const channel of DECAY_CHANNELS) {
      const value = numOrNull(`${p}-decay-${channel}`);
      if (value !== null && value > 0) decay[channel] = value / 1000;
    }
    if (Object.keys(decay).length) args.decay = decay;
  } else {
    const decayMs = numOrNull(`${p}-decay`);
    if (decayMs !== null && decayMs > 0) args.decay = decayMs / 1000;
  }
  args.head_ms = numOr(`${p}-head-ms`, brirDefault("head_ms", 1.0));
  args.itd_mode = /** @type {ItdMode} */ (val(`${p}-itd-mode`) || "off");
  args.head_radius = numOrNull(`${p}-head-radius`);
}

function applyTimeDefaults() {
  for (const p of advPrefixes()) {
    const mode = el(`${p}-itd-mode`, HTMLSelectElement);
    mode.value = brirDefault("itd_mode", "off");
    for (const option of mode.options) option.defaultSelected = option.value === mode.value;
    const radius = el(`${p}-head-radius`, HTMLInputElement);
    radius.value = String(brirDefault("head_radius", null) ?? "");
    radius.defaultValue = radius.value;
  }
}

/** @param {ProcessingRequest} args @param {string} p */
function addOutputArgs(args, p) {
  args.fs = checked(`${p}-resample`) ? Math.trunc(numOr(`${p}-fs`, 48000)) : null;
  args.jamesdsp = checked(`${p}-jamesdsp`);
  args.hangloose = checked(`${p}-hangloose`);
  args.remove_silent_channels = checked(`${p}-remove-silent-channels`);
  args.output_truehd_layouts = checked(`${p}-truehd`);
  args.layout_files = layoutFilesValue(p);
  args.missing_speakers = /** @type {MissingSpeakers} */ (val(`${p}-missing-speakers`) || "silent");
  args.lfe_mode = /** @type {LfeMode} */ (val(`${p}-lfe-mode`) || "none");
  args.sofa = checked(`${p}-sofa`);
  args.sofa_split_headphone_eq = args.sofa && !el(`${p}-sofa-split`, HTMLInputElement).disabled && checked(`${p}-sofa-split`);
}

/* ADR 0019: the headphone EQ can only be split off a SOFA file of a job that
   uses headphone compensation. A locked box keeps its tick for when it unlocks;
   addOutputArgs sends it only while unlocked. */
/** @param {string} p */
function updateSofaSplit(p) {
  const locked = !checked(`${p}-sofa`) || !isOpen("dis-headphone");
  el(`${p}-sofa-split`, HTMLInputElement).disabled = locked;
  $(`${p}-sofa-split-row`).title = locked ? t("tooltip_sofa_split_locked")
    : p === "ba" ? t("tooltip_sofa_split_headphone_eq") : "";
  if (p === "ba") $("ba-sofa-row").title = t("tooltip_sofa");
}

/* ADR 0014: each ITD choice shows its own explanation, and the head radius
   only matters when a model is involved (it still feeds the report's model
   column, so the row stays visible; the hint says what "auto" does). */
/** @param {string} p */
function updateItdHint(p) {
  const mode = val(`${p}-itd-mode`) || "off";
  for (const node of $(`${p}-itd-mode-row`).querySelectorAll("[data-itd-mode]")) {
    if (node instanceof HTMLElement) node.hidden = node.dataset.itdMode !== mode;
  }
}

/* ADR 0014: the alignment report of a finished BRIR job. A pure view: it
   reads the report and the locale table and writes the activity card's
   report block. Hidden again with `null` (a new job, a failure). */
/** @param {AlignmentReport | null | undefined} report */
function renderAlignment(report) {
  const block = $("brir-alignment");
  if (!report || !Array.isArray(report.speakers) || report.speakers.length === 0) {
    block.hidden = true;
    return;
  }
  /** @param {number | null | undefined} value @param {number} decimals @param {string} unit */
  const num = (value, decimals, unit) =>
    value === null || value === undefined || !Number.isFinite(value) ? "–" : `${value.toFixed(decimals)}${unit}`;
  /** @param {number | null | undefined} value @param {number} decimals @param {string} unit */
  const signed = (value, decimals, unit) =>
    value === null || value === undefined || !Number.isFinite(value)
      ? "–"
      : `${value > 0 ? "+" : ""}${value.toFixed(decimals)}${unit}`;

  $("brir-alignment-mode").textContent = `itd_mode=${report.mode}`;

  const summary = $("brir-alignment-summary");
  summary.replaceChildren();
  /** @param {string} key @param {string} text @param {"warn" | "ok" | null} flag @param {string} flagText */
  const row = (key, text, flag, flagText) => {
    const dt = document.createElement("dt");
    dt.className = "kv-key";
    dt.textContent = t(key);
    const dd = document.createElement("dd");
    dd.className = "kv-val";
    dd.textContent = text;
    if (flag) {
      const chip = document.createElement("span");
      chip.className = `align-flag ${flag}`;
      chip.textContent = flagText;
      dd.append(chip);
    }
    summary.append(dt, dd);
  };
  const yaw = report.yaw_deg;
  row(
    "label_itd_yaw",
    yaw === null || yaw === undefined ? "–" : fmt(t("value_itd_yaw"), { deg: Math.abs(yaw).toFixed(1), side: t(yaw >= 0 ? "value_itd_left" : "value_itd_right") }),
    yaw !== null && yaw !== undefined && Math.abs(yaw) >= 3 ? "warn" : null,
    t("flag_itd_head_turned"),
  );
  row("label_itd_offset", signed(report.common_offset_us, 0, " µs"), null, "");
  row(
    "label_itd_input_balance",
    signed(report.input_balance_db, 1, " dB"),
    report.input_balance_db !== null && Math.abs(report.input_balance_db) >= 3 ? "warn" : null,
    t("flag_itd_input_unbalanced"),
  );
  if (report.headphone_balance_db !== null || report.predicted_balance_db !== null) {
    row("label_itd_headphone_balance", signed(report.headphone_balance_db, 1, " dB"), null, "");
    const predicted = report.predicted_balance_db;
    const bad = predicted !== null && Math.abs(predicted) >= 2;
    row("label_itd_predicted_balance", signed(predicted, 1, " dB"), bad ? "warn" : "ok", t(bad ? "flag_itd_pairing_warning" : "flag_itd_pairing_ok"));
  }
  row("label_itd_hf_asymmetry", signed(report.hf_asymmetry_db, 1, " dB"), null, "");
  const radius = report.head_radius_cm;
  row(
    "label_itd_head_radius",
    radius === null ? "–" : `${num(radius, 1, " cm")} (${t(`value_itd_radius_${report.head_radius_source || "default"}`)})`,
    null,
    "",
  );

  const rows = $("brir-alignment-rows");
  rows.replaceChildren();
  for (const speaker of report.speakers) {
    const tr = document.createElement("tr");
    if (speaker.skipped) tr.className = "skipped";
    const cells = [
      ["speaker", speaker.speaker],
      ["num", signed(speaker.measured_us, 0, "")],
      ["num", signed(speaker.model_us, 0, "")],
      ["num", signed(speaker.after_us, 0, "")],
      ["num", signed(speaker.lf_ild_db, 1, "")],
    ];
    cells.forEach(([className, text], index) => {
      const td = document.createElement("td");
      td.className = className;
      if (index === 3 && speaker.applied_us !== null && speaker.applied_us !== undefined && Math.abs(speaker.applied_us) >= 1) {
        td.classList.add("changed");
      }
      td.textContent = text;
      tr.append(td);
    });
    rows.append(tr);
  }
  block.hidden = false;
}

/* ADR 0013: each LFE choice has its own explanation, in the row's tooltip. A
   second paragraph says that a measured subwoofer (assigned to LFE in a BRIR
   measurement file) is used whatever the choice. */
/** @param {string} p */
function updateLfeHint(p) {
  const mode = val(`${p}-lfe-mode`);
  const choice = document.createElement("span");
  choice.className = "tip-para";
  choice.textContent = mode === "center" ? t("hint_lfe_mode_center")
    : mode === "all" ? t("hint_lfe_mode_all") : t("hint_lfe_mode_none");
  const measured = document.createElement("span");
  measured.className = "tip-para";
  measured.textContent = t("hint_lfe_measured");
  $(`${p}-lfe-mode-tip`).replaceChildren(choice, measured);
}

/* The seven formats a channel-order file can be forced for, in page order:
   checkboxes `${p}-layout-force-0..6`, whose data-layout holds the id. */
const LAYOUT_FORCE_COUNT = 7;

/** @param {string} p */
function layoutForceBoxes(p) {
  return Array.from({ length: LAYOUT_FORCE_COUNT }, (_, index) => el(`${p}-layout-force-${index}`, HTMLInputElement));
}

/* The mode says what is written: complete formats only ("auto"), those plus
   the picked formats with silent missing speakers ("listed"), or nothing.
   Picks only count in "listed"; picking none there is "auto". */
/** @param {string} p */
function layoutFilesValue(p) {
  const mode = val(`${p}-layout-files`);
  if (mode !== "listed") return mode === "none" ? "none" : "auto";
  const forced = layoutForceBoxes(p).filter((box) => box.checked).map((box) => box.dataset.layout || "");
  return forced.length ? forced.join(",") : "auto";
}

/* Each mode has its own explanation in the row's tooltip, and the format list
   opens only when the user asks for formats with speakers missing. */
/** @param {string} p */
function updateLayoutForce(p) {
  const mode = val(`${p}-layout-files`);
  for (const node of $(`${p}-layout-files-row`).querySelectorAll("[data-layout-mode]")) {
    if (node instanceof HTMLElement) node.hidden = node.dataset.layoutMode !== mode;
  }
  for (const box of layoutForceBoxes(p)) box.disabled = mode !== "listed";
  $(`${p}-layout-files-tip`).textContent = mode === "listed" ? t("hint_layout_files")
    : mode === "none" ? t("hint_layout_files_none") : t("hint_layout_files_auto");
}

/** @param {ProcessingRequest} args @param {string} p */
function addCorrectionArgs(args, p) {
  args.interactive_plots = checked(`${p}-interactive-plots`);
  args.microphone_deviation_correction = checked(`${p}-mic-deviation`);
  args.mic_deviation_strength = numOr(`${p}-mic-strength`, brirDefault("mic_deviation_strength", 0.7));
  args.mic_deviation_debug_plots = checked(`${p}-mic-debug`);
}

/** @param {ProcessingRequest} args */
function addStudioAdvancedArgs(args) {
  if (advOn("tone")) addToneArgs(args, "ba", equalizationStageOn());
  if (advOn("time")) addTimeArgs(args, "ba");
  if (advOn("output")) addOutputArgs(args, "ba");
  if (advOn("correction")) addCorrectionArgs(args, "ba");
}

/* The advanced block of the request: Studio's tabs that are on, or Stable's
   list while its section is open. */
/** @param {ProcessingRequest} args */
function addAdvancedArgs(args) {
  if (state.skin === "studio") addStudioAdvancedArgs(args);
  else if (isOpen("dis-advanced")) {
    addToneArgs(args, "bf", true);
    addTimeArgs(args, "bf");
    addOutputArgs(args, "bf");
    addCorrectionArgs(args, "bf");
  }
}

/* --------------------------------------------------- Studio advanced tabs */

const ADV_TABS = ["tone", "time", "output", "correction"];

/* Which fields each tab owns, by suffix after "bf-"/"ba-". The per-channel
   decay inputs belong to "time" as well. */
/** @type {Record<string, string[]>} */
const ADV_FIELDS = {
  tone: ["bass-gain", "bass-fc", "bass-q", "tilt", "target-level", "balance", "balance-db"],
  time: ["head-ms", "itd-mode", "head-radius", "decay", "decay-per-channel", ...DECAY_CHANNELS.map((channel) => `decay-${channel}`)],
  output: ["resample", "fs", "jamesdsp", "hangloose", "truehd", "remove-silent-channels", "layout-files",
    ...Array.from({ length: LAYOUT_FORCE_COUNT }, (_, index) => `layout-force-${index}`),
    "missing-speakers", "lfe-mode", "sofa", "sofa-split"],
  correction: ["mic-deviation", "mic-strength", "mic-debug", "interactive-plots"],
};

/** @param {string} tab */
function advOn(tab) {
  return $(`adv-on-${tab}`).getAttribute("aria-checked") === "true";
}

/** @param {string} tab @param {boolean} on */
function setAdvOn(tab, on) {
  $(`adv-on-${tab}`).setAttribute("aria-checked", String(on));
  el(`adv-fields-${tab}`, HTMLFieldSetElement).disabled = !on;
  $(`adv-tab-${tab}`).classList.toggle("is-off", !on);
}

/** @param {string} tab @param {boolean} [focus] */
function selectAdvTab(tab, focus = false) {
  for (const name of ADV_TABS) {
    const selected = name === tab;
    const button = $(`adv-tab-${name}`);
    button.setAttribute("aria-selected", String(selected));
    button.tabIndex = selected ? 0 : -1;
    $(`adv-panel-${name}`).hidden = !selected;
  }
  if (focus) $(`adv-tab-${tab}`).focus();
}

function equalizationStageOn() {
  return isOpen("dis-room") || isOpen("dis-headphone") || isOpen("dis-eq");
}

/* -------------------------------------------------- rules for both forms */

/* What one choice in a form enables, disables or shows in the same form, by
   suffix. A rule reads its `when` field: a ticked checkbox, the "number"
   choice of a select (`is: "number"`), or a number other than zero
   (`is: "nonzero"`). A lock disables a part while the page cannot use it and
   puts `title` (a literal t() call, so ui_catalog.rs sees the key) in the
   tooltip of `titleOn`, the part itself by default; with `keep`, a locked
   checkbox shows what will happen (unchecked) and gets its own choice back
   when it unlocks. `only` keeps a rule to one form. Rules run in order, so a
   lock comes before the rules that read the part it locks. */
/** @typedef {{ when: string, is?: "number" | "nonzero", enables?: string[], disables?: string[],
 *   shows?: string[], only?: AdvPrefix }} AdvRule */
/** @typedef {{ lock: string, lockedWhen: () => boolean, title: () => string, titleOn?: string, keep?: boolean,
 *   only?: AdvPrefix }} AdvLock */

/* Studio locks what cannot apply instead of explaining it: the bass boost and
   tilt need an equalization stage, and headphone compensation already cancels
   the microphones, so mic deviation correction is skipped while it is on.
   Stable leaves those fields open. */
/** @type {(AdvRule | AdvLock)[]} */
const ADV_RULES = [
  { lock: "tone-eq", lockedWhen: () => !equalizationStageOn(), title: () => t("studio_adv_locked_no_eq"), only: "ba" },
  { when: "bass-gain", is: "nonzero", enables: ["bass-fc", "bass-q"], only: "ba" },
  { when: "balance", is: "number", enables: ["balance-db"] },
  { when: "decay-per-channel", disables: ["decay"], shows: ["decay-channels"] },
  { when: "resample", enables: ["fs"] },
  { lock: "mic-deviation", lockedWhen: () => isOpen("dis-headphone"), title: () => t("studio_adv_locked_headphone"),
    titleOn: "mic-row", keep: true, only: "ba" },
  { when: "mic-deviation", enables: ["mic-strength", "mic-debug"] },
];

/* Tooltips and rows that follow one choice; each runs with the rules and on
   its own when that choice changes. */
/** @type {{ on: string, update: (p: AdvPrefix) => void }[]} */
const ADV_HINTS = [
  { on: "layout-files", update: updateLayoutForce },
  { on: "lfe-mode", update: updateLfeHint },
  { on: "itd-mode", update: updateItdHint },
  { on: "sofa", update: updateSofaSplit },
];

/** @param {string} id */
function advField(id) {
  const node = $(id);
  if (!(node instanceof HTMLInputElement || node instanceof HTMLSelectElement)) throw new Error(`Not a field: ${id}`);
  return node;
}

/** @param {string} id */
function advControl(id) {
  const node = $(id);
  if (!(node instanceof HTMLInputElement || node instanceof HTMLSelectElement || node instanceof HTMLFieldSetElement)) {
    throw new Error(`Not a control: ${id}`);
  }
  return node;
}

/** @param {AdvPrefix} p @param {AdvRule} rule */
function advRuleHolds(p, rule) {
  const id = `${p}-${rule.when}`;
  if (rule.is === "number") return val(id) === "number";
  if (rule.is === "nonzero") return numOr(id, 0) !== 0;
  return checked(id);
}

/** @param {AdvPrefix} p @param {AdvLock} rule */
function applyAdvLock(p, rule) {
  const node = advControl(`${p}-${rule.lock}`);
  const locked = rule.lockedWhen();
  if (rule.keep) {
    if (!(node instanceof HTMLInputElement && node.type === "checkbox")) throw new Error(`Not a checkbox: ${node.id}`);
    if (locked && !node.disabled) {
      node.dataset.kept = String(node.checked);
      node.checked = false;
    } else if (!locked && node.disabled) {
      node.checked = node.dataset.kept === "true";
      delete node.dataset.kept;
    }
  }
  node.disabled = locked;
  $(`${p}-${rule.titleOn ?? rule.lock}`).title = locked ? rule.title() : "";
}

/** @param {AdvPrefix} p */
function refreshAdvanced(p) {
  for (const rule of ADV_RULES) {
    if (rule.only && rule.only !== p) continue;
    if ("lock" in rule) {
      applyAdvLock(p, rule);
      continue;
    }
    const holds = advRuleHolds(p, rule);
    for (const part of rule.enables ?? []) advField(`${p}-${part}`).disabled = !holds;
    for (const part of rule.disables ?? []) advField(`${p}-${part}`).disabled = holds;
    for (const part of rule.shows ?? []) $(`${p}-${part}`).hidden = !holds;
  }
  for (const hint of ADV_HINTS) hint.update(p);
}

/* Both forms again, after the language changes. */
function renderAdvancedForms() {
  for (const p of advPrefixes()) refreshAdvanced(p);
}

/* The field a rule reads runs the rules again (a number field as it is
   typed, anything else when it changes); the choice a hint follows runs that
   hint. Locks follow the page, which runs the rules of both forms itself. */
/** @param {AdvPrefix} p */
function wireAdvanced(p) {
  for (const rule of ADV_RULES) {
    if ("lock" in rule || (rule.only && rule.only !== p)) continue;
    const field = advField(`${p}-${rule.when}`);
    const typed = field instanceof HTMLInputElement && field.type === "number";
    field.addEventListener(typed ? "input" : "change", () => refreshAdvanced(p));
  }
  for (const hint of ADV_HINTS) advField(`${p}-${hint.on}`).addEventListener("change", () => hint.update(p));
}

/* The advanced listeners: both forms' rules and hints, then Studio's tabs
   (a tab is picked by a click or the arrow keys, and has a switch and a reset). */
function wireAdvancedForms() {
  for (const p of advPrefixes()) wireAdvanced(p);
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
}

/* Each form lays out a channel's decay field its own way: Stable puts "FL:"
   and a bare field straight in the grid, Studio a cell with the code and a
   unit. */
/** @type {Record<AdvPrefix, { placeholder?: string, className?: string,
 *   cell: (channel: string, input: HTMLInputElement) => HTMLElement[] }>} */
const DECAY_CELLS = {
  bf: {
    className: "num mono",
    cell: (channel, input) => {
      const label = document.createElement("span");
      label.className = "mini-label mono";
      label.textContent = `${channel}:`;
      return [label, input];
    },
  },
  ba: {
    placeholder: "—",
    cell: (channel, input) => {
      const cell = document.createElement("label");
      cell.className = "ch-cell";
      const code = document.createElement("span");
      code.className = "ch-code mono";
      code.textContent = channel;
      const field = document.createElement("span");
      field.className = "unit-field";
      const unit = document.createElement("span");
      unit.className = "unit";
      unit.dataset.i18n = "studio_adv_unit_ms";
      unit.textContent = t("studio_adv_unit_ms");
      field.append(input, unit);
      cell.append(code, field);
      return [cell];
    },
  },
};

/** @param {AdvPrefix} p */
function buildDecayGrid(p) {
  const layout = DECAY_CELLS[p];
  const grid = $(`${p}-decay-grid`);
  for (const channel of DECAY_CHANNELS) {
    const input = document.createElement("input");
    input.type = "number";
    input.min = "0";
    if (layout.placeholder) input.placeholder = layout.placeholder;
    input.id = `${p}-decay-${channel}`;
    if (layout.className) input.className = layout.className;
    grid.append(...layout.cell(channel, input));
  }
}

/* -------------------------------------------------- reset and carry-over */

/** @param {HTMLInputElement | HTMLSelectElement} node */
function resetField(node) {
  if (node instanceof HTMLSelectElement) {
    const initial = Array.from(node.options).findIndex((option) => option.defaultSelected);
    node.selectedIndex = Math.max(0, initial);
  } else if (node.type === "checkbox") {
    node.checked = node.defaultChecked;
    delete node.dataset.kept;
  } else {
    node.value = node.defaultValue;
  }
}

/** @param {string} tab */
function resetAdvTab(tab) {
  for (const suffix of ADV_FIELDS[tab]) resetField(advField(`ba-${suffix}`));
  refreshAdvanced("ba");
}

/* The skins keep separate forms, so a switch in the middle of a session
   carries the values across. Stable -> Studio turns every tab on or off with
   the Advanced switch. Studio -> Stable opens the list when any tab is on and
   carries the defaults for tabs that are off, so nothing a tab held back
   becomes active. */
/** @param {string} skin */
function syncAdvancedForms(skin) {
  const toStudio = skin === "studio";
  const stableOpen = isOpen("dis-advanced");
  for (const tab of ADV_TABS) {
    const active = toStudio ? stableOpen : advOn(tab);
    for (const suffix of ADV_FIELDS[tab]) {
      const from = advField(`${toStudio ? "bf" : "ba"}-${suffix}`);
      const to = advField(`${toStudio ? "ba" : "bf"}-${suffix}`);
      if (!toStudio && !active) {
        resetField(to);
      } else if (to instanceof HTMLInputElement && to.type === "checkbox" && from instanceof HTMLInputElement) {
        // A locked Studio checkbox keeps the carried choice until it unlocks.
        if (toStudio && to.disabled && to.dataset.kept !== undefined) to.dataset.kept = String(from.checked);
        else to.checked = from.checked;
      } else {
        to.value = from.value;
      }
    }
    if (toStudio) setAdvOn(tab, stableOpen);
  }
  if (!toStudio) $("dis-advanced").classList.toggle("open", ADV_TABS.some(advOn));
  for (const p of ADV_PREFIXES) refreshAdvanced(p);
}
