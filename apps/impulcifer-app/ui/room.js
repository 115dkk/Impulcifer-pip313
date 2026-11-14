// @ts-check
// Room correction and virtual bass: modes, rows, defaults and request arguments.
"use strict";

/* ------------------------------------------- room correction mode & rows
   Room EQ (ADR 0004) corrects the magnitude at each ear position. Virtual
   room tuning (ADR 0005) and virtual room tuning 2 (ADR 0008) are separate
   modes with their own settings, so while one is chosen the Room EQ rows
   (range, boost cap, legacy limits, FR combination) are hidden and not sent.
   Of the room facts, tuning reads the volume and the Schroeder frequency
   only at the Schroeder timing limit, tuning 2 only for an automatic band.
   Rows are hidden, not cleared, so switching the mode, the range or the
   limit keeps what was typed. */

/** @type {readonly RoomMode[]} */
const ROOM_MODES = ["eq", "tuning", "tuning2"];

/** @param {unknown} value @returns {value is RoomMode} */
function isRoomMode(value) {
  return typeof value === "string" && ROOM_MODES.some((mode) => mode === value);
}

/** @returns {RoomMode} */
function roomMode() {
  return ROOM_MODES.find((mode) => $(`bf-room-mode-${mode}`).getAttribute("aria-checked") === "true") ?? "eq";
}

/** @param {RoomMode} mode @param {boolean} [focus] */
function setRoomMode(mode, focus = false) {
  for (const name of ROOM_MODES) {
    const button = $(`bf-room-mode-${name}`);
    button.setAttribute("aria-checked", String(name === mode));
    button.tabIndex = name === mode ? 0 : -1;
  }
  if (focus) $(`bf-room-mode-${mode}`).focus();
  updateRoomRows();
}

/** @param {RoomMode} mode */
function roomModeHint(mode) {
  switch (mode) {
    case "tuning": return t("tooltip_room_mode_tuning");
    case "tuning2": return t("tooltip_room_mode_tuning2");
    default: return t("tooltip_room_mode_eq");
  }
}

/* Tuning 2's filter is one of SECS 2's four structures or a latency of
   30–200 ms ("custom", sent as the number). Its tooltip names the choice. */

/** @type {readonly Tuning2Filter[]} */
const TUNING2_FILTERS = ["zero", "mixed", "linear", "long", "custom"];

/** @param {unknown} value @returns {value is Tuning2Filter} */
function isTuning2Filter(value) {
  return typeof value === "string" && TUNING2_FILTERS.some((filter) => filter === value);
}

/** @returns {Tuning2Filter} */
function tuning2Filter() {
  const value = val("bf-room-tuning2-mode");
  return isTuning2Filter(value) ? value : "mixed";
}

/** @param {Tuning2Filter} filter */
function tuning2FilterHint(filter) {
  return t(`tooltip_room_tuning2_mode_${filter}`);
}

/** @returns {Exclude<Tuning2Filter, "custom"> | number} */
function tuning2Mode() {
  const filter = tuning2Filter();
  return filter === "custom" ? numOr("bf-room-tuning2-latency", 200) : filter;
}

/* Sets the filter from bootstrap's spec: a structure name, or a latency in
   ms (number or numeric string), which selects "custom" with that value. */
/** @param {unknown} spec */
function setTuning2Mode(spec) {
  const ms = typeof spec === "number" || (typeof spec === "string" && spec.trim() !== "") ? Number(spec) : NaN;
  if (isTuning2Filter(spec) && spec !== "custom") {
    el("bf-room-tuning2-mode", HTMLSelectElement).value = spec;
  } else if (Number.isFinite(ms)) {
    el("bf-room-tuning2-mode", HTMLSelectElement).value = "custom";
    el("bf-room-tuning2-latency", HTMLInputElement).value = String(ms);
  } else {
    el("bf-room-tuning2-mode", HTMLSelectElement).value = "mixed";
  }
}

/** A numeric JSON spec as input text; "auto", "off" and missing values stay empty. @param {unknown} spec */
function numberSpecText(spec) {
  const value = typeof spec === "number" || (typeof spec === "string" && spec.trim() !== "") ? Number(spec) : NaN;
  return Number.isFinite(value) ? String(value) : "";
}

/* How far up tuning corrects timing. "off" corrects the magnitude only and
   adds no delay, "full" is the whole band (the default, as the processor
   tuning follows). Studio picks one of eight stops on a slider; Stable only
   asks whether to stop at the Schroeder frequency. The slider holds the
   limit in both skins: a limit Stable's checkbox cannot show (Off, a Hz
   stop) is kept and named on the line under it (updateRoomRows), and
   ticking or clearing the box replaces it (onRoomPhaseCheckbox). */

/** @type {readonly RoomPhaseLimit[]} */
const ROOM_PHASE_STOPS = ["off", "schroeder", 500, 1000, 2000, 5000, 10000, "full"];
const ROOM_PHASE_SCHROEDER = ROOM_PHASE_STOPS.indexOf("schroeder");
const ROOM_PHASE_FULL = ROOM_PHASE_STOPS.length - 1;

/** @returns {number} the slider's stop, 0 to ROOM_PHASE_FULL */
function roomPhaseStop() {
  const stop = Math.round(Number(el("bf-room-phase-limit", HTMLInputElement).value));
  return Number.isFinite(stop) ? Math.max(0, Math.min(ROOM_PHASE_FULL, stop)) : ROOM_PHASE_FULL;
}

/** @param {RoomPhaseLimit} limit */
function roomPhaseText(limit) {
  switch (limit) {
    case "off": return t("option_room_tuning_phase_off");
    case "schroeder": return t("option_room_tuning_phase_schroeder");
    case "full": return t("option_room_tuning_phase_full");
    default:
      return limit < 1000
        ? `${limit} ${t("studio_adv_unit_hz")}`
        : `${limit / 1000} ${t("studio_adv_unit_khz")}`;
  }
}

/* The readout and aria-valuetext name the stop. --range-p (0–1) and
   --range-gaps on the slider's wrapper place the accent fill, which WebKit
   cannot draw from the value on its own, and the stop ticks. */
function updateRoomPhaseReadout() {
  const slider = el("bf-room-phase-limit", HTMLInputElement);
  const stop = roomPhaseStop();
  const text = roomPhaseText(ROOM_PHASE_STOPS[stop]);
  el("bf-room-phase-limit-value", HTMLOutputElement).value = text;
  slider.setAttribute("aria-valuetext", text);
  slider.parentElement?.style.setProperty("--range-p", String(stop / ROOM_PHASE_FULL));
  slider.parentElement?.style.setProperty("--range-gaps", String(ROOM_PHASE_FULL));
}

/* The Studio slider holds the limit in both skins. Stable's checkbox only
   shows whether it is the Schroeder stop, and changes the slider when the
   user ticks or clears it, so a limit Stable cannot show (Off, a Hz stop)
   survives a skin switch instead of silently becoming the full band. */
/** @returns {RoomPhaseLimit} */
function roomPhaseLimit() {
  return ROOM_PHASE_STOPS[roomPhaseStop()];
}

/* Sets both controls from a limit spec as bootstrap sends it: "off",
   "schroeder", "full", or Hz (a number or a numeric string), which lands
   on the nearest stop by octaves, 20000 Hz being the full band. Anything
   else is the full band. */
/** @param {unknown} spec */
function setRoomPhaseLimit(spec) {
  const named = typeof spec === "string" ? ROOM_PHASE_STOPS.indexOf(/** @type {RoomPhaseLimit} */ (spec)) : -1;
  const hz = typeof spec === "number" || (typeof spec === "string" && spec.trim() !== "") ? Number(spec) : NaN;
  let stop = ROOM_PHASE_FULL;
  if (named >= 0) {
    stop = named;
  } else if (Number.isFinite(hz) && hz > 0) {
    let best = Infinity;
    ROOM_PHASE_STOPS.forEach((limit, index) => {
      const at = limit === "full" ? 20000 : limit;
      if (typeof at !== "number") return;
      const distance = Math.abs(Math.log2(hz / at));
      if (distance < best) {
        best = distance;
        stop = index;
      }
    });
  }
  el("bf-room-phase-limit", HTMLInputElement).value = String(stop);
  el("bf-room-phase-schroeder", HTMLInputElement).checked = stop === ROOM_PHASE_SCHROEDER;
  updateRoomPhaseReadout();
}

/* Studio -> Stable ticks the box for the Schroeder stop alone. The slider
   keeps every other stop, so Off or a Hz stop still applies in Stable (the
   line under the box names it) and is back on the slider in Studio. */
/** @param {string} skin */
function syncRoomPhase(skin) {
  if (skin === "stable") {
    el("bf-room-phase-schroeder", HTMLInputElement).checked = roomPhaseStop() === ROOM_PHASE_SCHROEDER;
  }
  updateRoomRows();
}

/* Stable's checkbox is a direct choice: Schroeder when ticked, the full band when cleared. */
function onRoomPhaseCheckbox() {
  el("bf-room-phase-limit", HTMLInputElement).value = String(checked("bf-room-phase-schroeder") ? ROOM_PHASE_SCHROEDER : ROOM_PHASE_FULL);
  updateRoomPhaseReadout();
  updateRoomRows();
}

/* The tuning delay is "auto" or a fixed number of ms; bootstrap sends its
   default as the CLI spec ("10" or "auto"). */
function defaultTuningDelay() {
  const spec = Number(state.brirDefaults.room_tuning_delay);
  return Number.isFinite(spec) && spec > 0 ? spec : 10;
}

/** @returns {"auto" | number} */
function roomTuningDelay() {
  return checked("bf-room-tuning-delay-auto") ? "auto" : numOr("bf-room-tuning-delay", defaultTuningDelay());
}

/** @type {readonly RoomRange[]} */
const ROOM_RANGES = ["modes", "schroeder", "extreme", "legacy"];

/** @param {unknown} value @returns {value is RoomRange} */
function isRoomRange(value) {
  return typeof value === "string" && ROOM_RANGES.some((range) => range === value);
}

/** @returns {RoomRange} */
function roomRange() {
  const value = val("bf-room-range");
  return isRoomRange(value) ? value : "schroeder";
}

/** @param {RoomRange} range */
function roomRangeHint(range) {
  switch (range) {
    case "modes": return t("tooltip_room_range_modes");
    case "extreme": return t("tooltip_room_range_extreme");
    case "legacy": return t("tooltip_room_range_legacy");
    default: return t("tooltip_room_range_schroeder");
  }
}

/* The room volume and Schroeder rows serve every mode. Tuning 2 shows them in
   its correction-band group, right under the reach they feed; the other modes
   keep them where the markup puts them. Moving the nodes, not reordering them
   with styles, keeps the tab order the same as the order on screen. A row moves
   only when it is out of place, so typing in it (its input event comes here)
   never re-inserts the focused field. */
/** @type {Comment | null} */
let roomFactsHome = null;

/** @param {boolean} tuning2 */
function placeRoomFacts(tuning2) {
  const volume = $("bf-room-volume-row");
  const schroeder = $("bf-schroeder-freq-row");
  if (!roomFactsHome) {
    roomFactsHome = document.createComment("room volume and Schroeder rows outside tuning 2");
    volume.before(roomFactsHome);
  }
  const hint = $("bf-room-tuning2-high-volume-hint");
  if (tuning2) {
    if (volume.nextElementSibling !== schroeder || schroeder.nextElementSibling !== hint) hint.before(volume, schroeder);
  } else if (roomFactsHome.nextSibling !== volume) {
    roomFactsHome.after(volume, schroeder);
  }
}

/* Applies the mode, the range and the timing limit together: which rows
   show, the delay's lock, the readout and the mode and range tooltips. */
function updateRoomRows() {
  const mode = roomMode();
  const tuning = mode === "tuning";
  const tuning2 = mode === "tuning2";
  const eq = mode === "eq";
  const range = roomRange();
  const legacy = range === "legacy";
  const limit = roomPhaseLimit();
  const mimo = checked("bf-room-tuning2-mimo");
  const roomFacts = tuning ? limit === "schroeder" : tuning2 ? val("bf-room-tuning2-high").trim() === "" : !legacy;
  /** @type {Record<string, boolean>} row id -> shown */
  const rows = {
    "bf-room-range-row": eq,
    "bf-room-volume-row": roomFacts,
    "bf-schroeder-freq-row": roomFacts,
    "bf-room-max-boost-row": eq && !legacy,
    "bf-specific-limit-row": eq && legacy,
    "bf-generic-limit-row": eq && legacy,
    "bf-fr-combination-row": eq,
    // Both phase-limit controls follow the mode; the styles show each in its own skin.
    "bf-room-tuning-max-boost-row": tuning,
    "bf-room-tuning-curtain-row": tuning,
    "bf-room-tuning-level-match-row": tuning || tuning2,
    "bf-room-phase-limit-row": tuning,
    "bf-room-phase-schroeder-row": tuning,
    "bf-room-tuning-delay-row": tuning && limit !== "off",
    "bf-room-tuning2-mode-row": tuning2,
    "bf-room-tuning2-latency-row": tuning2 && tuning2Filter() === "custom",
    "bf-room-tuning2-max-boost-row": tuning2,
    // Tuning 2's group headings and the wrappers that pair or indent its rows.
    "bf-room-tuning2-head-filter": tuning2,
    "bf-room-tuning2-head-band": tuning2,
    "bf-room-tuning2-head-protect": tuning2,
    "bf-room-tuning2-head-timing": tuning2,
    "bf-room-tuning2-head-speakers": tuning2,
    "bf-room-tuning2-band-pair": tuning2,
    "bf-room-tuning2-mimo-detail": tuning2 && mimo,
    "bf-room-tuning2-low-row": tuning2,
    "bf-room-tuning2-high-row": tuning2,
    // A typed upper end leaves the basis and the reach nothing to do: they stay in view, disabled.
    "bf-room-tuning2-high-basis-row": tuning2,
    "bf-room-tuning2-high-transition-row": tuning2,
    "bf-room-tuning2-high-volume-hint": tuning2 && roomFacts
      && (val("bf-room-tuning2-high-basis") === "lenient" || val("bf-room-tuning2-high-transition") !== "1")
      && val("bf-room-volume").trim() === "" && val("bf-schroeder-freq").trim() === "",
    "bf-room-tuning2-input-hp-row": tuning2,
    "bf-room-tuning2-bands-row": tuning2,
    "bf-room-tuning2-dip-limit-row": tuning2,
    "bf-room-tuning2-low-protect-row": tuning2,
    "bf-room-tuning2-mode-decay-row": tuning2,
    "bf-room-tuning2-lf-align-row": tuning2,
    "bf-room-tuning2-hf-phase-row": tuning2,
    "bf-room-tuning2-fir-hint": tuning2 && tuning2Filter() === "zero",
    "bf-room-tuning2-mimo-row": tuning2,
    "bf-room-tuning2-fsiso-row": tuning2 && mimo,
    "bf-room-tuning2-mimo-low-row": tuning2 && mimo,
    "bf-room-tuning2-mimo-early-row": tuning2 && mimo,
    "bf-room-tuning2-mimo-ear-guard-row": tuning2 && mimo,
  };
  for (const [id, shown] of Object.entries(rows)) $(id).hidden = !shown;
  placeRoomFacts(tuning2);
  for (const id of ["bf-room-tuning2-high-basis", "bf-room-tuning2-high-transition"]) {
    el(id, HTMLSelectElement).disabled = tuning2 && !roomFacts;
  }
  for (const id of ["bf-room-tuning2-lf-align", "bf-room-tuning2-hf-phase"]) {
    el(id, HTMLInputElement).disabled = tuning2Filter() === "zero";
  }
  // Stable's checkbox shows only Schroeder or the full band; name any other
  // limit the slider keeps. The styles hide this line in Studio.
  const current = $("bf-room-phase-current");
  const named = tuning && limit !== "schroeder" && limit !== "full";
  current.hidden = !named;
  current.textContent = named ? fmt(t("label_room_tuning_phase_current"), { limit: roomPhaseText(limit) }) : "";
  el("bf-room-tuning-delay", HTMLInputElement).disabled = checked("bf-room-tuning-delay-auto");
  updateRoomPhaseReadout();
  $("bf-room-mode-tip").textContent = roomModeHint(mode);
  $("bf-room-range-tip").textContent = roomRangeHint(range);
  $("bf-room-tuning2-mode-tip").textContent = tuning2FilterHint(tuning2Filter());
  // The two tunings match levels differently (±6 dB around the median, or down to the quietest).
  $("bf-room-tuning-level-match-tip").textContent = t(tuning2 ? "tooltip_room_tuning2_level_match" : "tooltip_room_tuning_level_match");
  el("bf-schroeder-freq", HTMLInputElement).placeholder = t("placeholder_schroeder_freq");
  el("bf-room-tuning2-input-hp", HTMLInputElement).placeholder = t("placeholder_room_tuning2_off");
  for (const id of ["bf-room-tuning2-high", "bf-room-tuning2-mimo-low"]) {
    el(id, HTMLInputElement).placeholder = t("placeholder_room_tuning2_auto");
  }
}

/* Every room setting starts from the service's defaults. The room volume
   and the Schroeder frequency start empty and are sent as null: the service
   then assumes 50 m³ and estimates the frequency. */
function applyRoomDefaults() {
  const defaults = state.brirDefaults;
  el("bf-room-range", HTMLSelectElement).value = isRoomRange(defaults.room_range) ? defaults.room_range : "schroeder";
  el("bf-room-max-boost", HTMLInputElement).value = String(brirDefault("room_max_boost", 12));
  el("bf-room-tuning-max-boost", HTMLInputElement).value = String(brirDefault("room_tuning_max_boost", 6));
  el("bf-room-tuning-curtain", HTMLInputElement).value = String(brirDefault("room_tuning_curtain", 300));
  el("bf-room-tuning-level-match", HTMLInputElement).checked = defaults.room_tuning_level_match !== false;
  el("bf-room-tuning-delay", HTMLInputElement).value = String(defaultTuningDelay());
  el("bf-room-tuning-delay-auto", HTMLInputElement).checked = defaults.room_tuning_delay === "auto";
  setRoomPhaseLimit(defaults.room_tuning_phase_limit);
  setTuning2Mode(defaults.room_tuning2_mode);
  el("bf-room-tuning2-max-boost", HTMLInputElement).value = String(brirDefault("room_tuning2_max_boost", 10));
  el("bf-room-tuning2-low", HTMLInputElement).value = String(brirDefault("room_tuning2_low", 20));
  el("bf-room-tuning2-high", HTMLInputElement).value = numberSpecText(defaults.room_tuning2_high);
  el("bf-room-tuning2-high-basis", HTMLSelectElement).value = defaults.room_tuning2_high_basis === "lenient" ? "lenient" : "strict";
  const transition = String(defaults.room_tuning2_high_transition ?? 1);
  el("bf-room-tuning2-high-transition", HTMLSelectElement).value = ["1", "2", "4"].includes(transition) ? transition : "1";
  el("bf-room-tuning2-input-hp", HTMLInputElement).value = numberSpecText(defaults.room_tuning2_input_hp);
  el("bf-room-tuning2-bands", HTMLInputElement).value = typeof defaults.room_tuning2_bands === "string" ? defaults.room_tuning2_bands : "";
  el("bf-room-tuning2-dip-limit", HTMLInputElement).checked = defaults.room_tuning2_dip_limit === true;
  el("bf-room-tuning2-low-protect", HTMLInputElement).checked = defaults.room_tuning2_low_protect === true;
  el("bf-room-tuning2-mode-decay", HTMLInputElement).checked = defaults.room_tuning2_mode_decay === true;
  el("bf-room-tuning2-lf-align", HTMLInputElement).checked = defaults.room_tuning2_lf_align === true;
  el("bf-room-tuning2-hf-phase", HTMLInputElement).checked = defaults.room_tuning2_hf_phase === true;
  el("bf-room-tuning2-mimo", HTMLInputElement).checked = defaults.room_tuning2_mimo === true;
  el("bf-room-tuning2-fsiso", HTMLInputElement).value = String(brirDefault("room_tuning2_fsiso", 150));
  el("bf-room-tuning2-mimo-low", HTMLInputElement).value = numberSpecText(defaults.room_tuning2_mimo_low);
  el("bf-room-tuning2-mimo-early", HTMLInputElement).value = String(brirDefault("room_tuning2_mimo_early", 60));
  el("bf-room-tuning2-mimo-ear-guard", HTMLInputElement).checked = defaults.room_tuning2_mimo_ear_guard === true;
  setRoomMode(isRoomMode(defaults.room_mode) ? defaults.room_mode : "eq");
}

/** @type {readonly VbassMode[]} */
const VBASS_MODES = ["auto", "manual", "legacy"];

/** @param {unknown} value @returns {value is VbassMode} */
function isVbassMode(value) {
  return typeof value === "string" && VBASS_MODES.some((mode) => mode === value);
}

/** @returns {VbassMode} */
function vbassMode() {
  const value = val("bf-vbass-mode");
  return isVbassMode(value) ? value : "auto";
}

/** @param {VbassMode} mode */
function vbassModeHint(mode) {
  switch (mode) {
    case "manual": return t("tooltip_vbass_mode_manual");
    case "legacy": return t("tooltip_vbass_mode_legacy");
    default: return t("tooltip_vbass_mode_auto");
  }
}

/* Automatic picks the crossover itself, so the frequency row shows only for
   the other two; the tooltip follows the choice. */
function updateVbassRows() {
  const mode = vbassMode();
  $("bf-vbass-freq-row").hidden = mode === "auto";
  $("bf-vbass-mode-tip").textContent = vbassModeHint(mode);
}

function applyVbassDefaults() {
  const defaults = state.brirDefaults;
  el("bf-vbass-mode", HTMLSelectElement).value = isVbassMode(defaults.vbass_mode) ? defaults.vbass_mode : "auto";
  el("bf-vbass-freq", HTMLInputElement).value = String(brirDefault("vbass_freq", 250));
  updateVbassRows();
}

/** @param {ProcessingRequest} args */
function addRoomEqArgs(args) {
  const range = roomRange();
  args.room_range = range;
  if (range === "legacy") {
    args.specific_limit = numOr("bf-specific-limit", brirDefault("specific_limit", 400));
    args.generic_limit = numOr("bf-generic-limit", brirDefault("generic_limit", 300));
  } else {
    args.room_volume = numOrNull("bf-room-volume");
    args.schroeder_freq = numOrNull("bf-schroeder-freq");
    args.room_max_boost = numOr("bf-room-max-boost", brirDefault("room_max_boost", 12));
  }
  args.fr_combination_method = val("bf-fr-combination");
}

/** @param {ProcessingRequest} args */
function addRoomTuningArgs(args) {
  const limit = roomPhaseLimit();
  args.room_tuning_phase_limit = limit;
  // Off adds no delay, so there is none to send.
  if (limit !== "off") args.room_tuning_delay = roomTuningDelay();
  if (limit === "schroeder") {
    args.room_volume = numOrNull("bf-room-volume");
    args.schroeder_freq = numOrNull("bf-schroeder-freq");
  }
  args.room_tuning_max_boost = numOr("bf-room-tuning-max-boost", brirDefault("room_tuning_max_boost", 6));
  args.room_tuning_curtain = numOr("bf-room-tuning-curtain", brirDefault("room_tuning_curtain", 300));
  args.room_tuning_level_match = checked("bf-room-tuning-level-match");
}

/* Tuning 2 sends its own fields and the shared level match. An empty upper
   band end or support floor is "auto"; the room facts go only with an
   automatic band, the MIMO limits only with MIMO on. */
/** @param {ProcessingRequest} args */
function addRoomTuning2Args(args) {
  args.room_tuning2_mode = tuning2Mode();
  args.room_tuning2_max_boost = numOr("bf-room-tuning2-max-boost", brirDefault("room_tuning2_max_boost", 10));
  args.room_tuning2_low = numOr("bf-room-tuning2-low", brirDefault("room_tuning2_low", 20));
  const high = numOrNull("bf-room-tuning2-high");
  args.room_tuning2_high = high ?? "auto";
  args.room_tuning2_high_basis = val("bf-room-tuning2-high-basis") === "lenient" ? "lenient" : "strict";
  const transition = Number(val("bf-room-tuning2-high-transition"));
  args.room_tuning2_high_transition = transition === 2 || transition === 4 ? transition : 1;
  args.room_tuning2_input_hp = numOrNull("bf-room-tuning2-input-hp") ?? "off";
  args.room_tuning2_bands = val("bf-room-tuning2-bands").trim();
  args.room_tuning2_dip_limit = checked("bf-room-tuning2-dip-limit");
  args.room_tuning2_low_protect = checked("bf-room-tuning2-low-protect");
  args.room_tuning2_mode_decay = checked("bf-room-tuning2-mode-decay");
  args.room_tuning2_lf_align = tuning2Filter() !== "zero" && checked("bf-room-tuning2-lf-align");
  args.room_tuning2_hf_phase = tuning2Filter() !== "zero" && checked("bf-room-tuning2-hf-phase");
  if (high === null) {
    args.room_volume = numOrNull("bf-room-volume");
    args.schroeder_freq = numOrNull("bf-schroeder-freq");
  }
  args.room_tuning_level_match = checked("bf-room-tuning-level-match");
  const mimo = checked("bf-room-tuning2-mimo");
  args.room_tuning2_mimo = mimo;
  if (mimo) {
    args.room_tuning2_fsiso = numOr("bf-room-tuning2-fsiso", brirDefault("room_tuning2_fsiso", 150));
    args.room_tuning2_mimo_low = numOrNull("bf-room-tuning2-mimo-low") ?? "auto";
    args.room_tuning2_mimo_early = numOr("bf-room-tuning2-mimo-early", brirDefault("room_tuning2_mimo_early", 60));
    args.room_tuning2_mimo_ear_guard = checked("bf-room-tuning2-mimo-ear-guard");
  }
}
