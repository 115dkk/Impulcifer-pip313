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

/* ------------------------------------------------- room options as a table
   One entry per room setting the request carries, and one per row that is
   not a setting. A setting's row shows exactly when its value is sent: in
   the modes it lists, while its `when` holds. updateRoomRows shows the rows
   and locks the inputs from these tables, applyRoomDefaults sets the start
   values and addRoomArgs sends the values, so an option is one entry here. */

/** The facts every rule reads, computed once per pass (roomContext).
 * @typedef {{ mode: RoomMode, range: RoomRange, limit: RoomPhaseLimit, filter: Tuning2Filter, mimo: boolean,
 *   roomFacts: boolean }} RoomContext */
/** Shown in these modes while `when` (default: always) holds.
 * @typedef {{ modes: readonly RoomMode[], when?: (context: RoomContext) => boolean }} RoomShown */
/** A row that is not a sent setting; `id` is the element itself.
 * @typedef {RoomShown & { id: string }} RoomRow */
/** How a setting is read and where its start value comes from:
 *   number    numOr(id, the service default or `fallback`); starts at that default,
 *             or as the markup has it with `fromMarkup`
 *   numberOr  the number, or `word` when empty; starts from the default's spec ("auto" stays empty)
 *   optional  the number or null; starts empty (no default)
 *   check     checked; starts at the default when it is a boolean, else `fallback` (false).
 *             A disabled check sends false and keeps its tick for when it unlocks.
 *   text      the trimmed text; starts at the default string or empty
 *   choice    the value if in `choices`, else `fallback` (as a number with `number`); starts likewise
 *   select    the selected value; starts as the markup has it (no default)
 *   custom    `read` and `reset`, for a setting with two controls or a spec
 * @typedef {{ kind: "number", fallback: number, fromMarkup?: true } | { kind: "numberOr", word: "auto" | "off" }
 *   | { kind: "optional" } | { kind: "check", fallback?: boolean } | { kind: "text" }
 *   | { kind: "choice", choices: readonly string[], fallback: string, number?: true } | { kind: "select" }
 *   | { kind: "custom", read: (context: RoomContext) => unknown, reset: (defaults: BrirDefaults) => void }
 *   } RoomFieldKind */
/** A sent setting: its input `id` (its row is `${id}-row`), the request field `arg` (also the brir_defaults
 * key), when the input is `disabled`, and whether its input re-applies the rows (`refresh`).
 * @typedef {RoomShown & RoomFieldKind & { id: string, arg: keyof ProcessingRequest,
 *   disabled?: (context: RoomContext) => boolean, refresh?: true }} RoomField */

/** @type {readonly RoomField[]} */
const ROOM_FIELDS = [
  // Room EQ (ADR 0004). The legacy range has its own limits instead of the boost cap; they keep
  // the markup's values (400 and 300, the service's defaults too), as they always have.
  { id: "bf-room-range", arg: "room_range", modes: ["eq"], kind: "choice", choices: ROOM_RANGES, fallback: "schroeder",
    refresh: true },
  { id: "bf-room-max-boost", arg: "room_max_boost", modes: ["eq"], when: (c) => c.range !== "legacy",
    kind: "number", fallback: 12 },
  { id: "bf-specific-limit", arg: "specific_limit", modes: ["eq"], when: (c) => c.range === "legacy",
    kind: "number", fallback: 400, fromMarkup: true },
  { id: "bf-generic-limit", arg: "generic_limit", modes: ["eq"], when: (c) => c.range === "legacy",
    kind: "number", fallback: 300, fromMarkup: true },
  { id: "bf-fr-combination", arg: "fr_combination_method", modes: ["eq"], kind: "select" },
  // The room facts serve every mode that reads them (roomContext). Empty is sent as null: the
  // service then assumes 50 m³ and estimates the frequency.
  { id: "bf-room-volume", arg: "room_volume", modes: ROOM_MODES, when: (c) => c.roomFacts, kind: "optional",
    refresh: true },
  { id: "bf-schroeder-freq", arg: "schroeder_freq", modes: ROOM_MODES, when: (c) => c.roomFacts,
    kind: "optional", refresh: true },
  // Virtual room tuning (ADR 0005). Off adds no delay, so there is none to send.
  { id: "bf-room-phase-limit", arg: "room_tuning_phase_limit", modes: ["tuning"], kind: "custom",
    read: (c) => c.limit, reset: (defaults) => setRoomPhaseLimit(defaults.room_tuning_phase_limit), refresh: true },
  { id: "bf-room-tuning-delay", arg: "room_tuning_delay", modes: ["tuning"], when: (c) => c.limit !== "off",
    kind: "custom", read: roomTuningDelay, reset: (defaults) => {
      el("bf-room-tuning-delay", HTMLInputElement).value = String(defaultTuningDelay());
      el("bf-room-tuning-delay-auto", HTMLInputElement).checked = defaults.room_tuning_delay === "auto";
    }, disabled: () => checked("bf-room-tuning-delay-auto") },
  { id: "bf-room-tuning-max-boost", arg: "room_tuning_max_boost", modes: ["tuning"], kind: "number", fallback: 6 },
  { id: "bf-room-tuning-curtain", arg: "room_tuning_curtain", modes: ["tuning"], kind: "number", fallback: 300 },
  // Both tunings match the speaker levels, each its own way (the tooltip says which).
  { id: "bf-room-tuning-level-match", arg: "room_tuning_level_match", modes: ["tuning", "tuning2"], kind: "check",
    fallback: true },
  // Virtual room tuning 2 (ADR 0008, 0015). An empty upper band end or support floor is "auto".
  { id: "bf-room-tuning2-mode", arg: "room_tuning2_mode", modes: ["tuning2"], kind: "custom", read: tuning2Mode,
    reset: (defaults) => setTuning2Mode(defaults.room_tuning2_mode), refresh: true },
  { id: "bf-room-tuning2-max-boost", arg: "room_tuning2_max_boost", modes: ["tuning2"], kind: "number", fallback: 10 },
  { id: "bf-room-tuning2-low", arg: "room_tuning2_low", modes: ["tuning2"], kind: "number", fallback: 20 },
  { id: "bf-room-tuning2-high", arg: "room_tuning2_high", modes: ["tuning2"], kind: "numberOr", word: "auto",
    refresh: true },
  // A typed upper end leaves the basis and the reach nothing to do: they stay in view, disabled, and are sent.
  { id: "bf-room-tuning2-high-basis", arg: "room_tuning2_high_basis", modes: ["tuning2"], kind: "choice",
    choices: ["strict", "lenient"], fallback: "strict", disabled: (c) => c.mode === "tuning2" && !c.roomFacts,
    refresh: true },
  { id: "bf-room-tuning2-high-transition", arg: "room_tuning2_high_transition", modes: ["tuning2"], kind: "choice",
    choices: ["1", "2", "4"], fallback: "1", number: true, disabled: (c) => c.mode === "tuning2" && !c.roomFacts,
    refresh: true },
  { id: "bf-room-tuning2-input-hp", arg: "room_tuning2_input_hp", modes: ["tuning2"], kind: "numberOr", word: "off" },
  { id: "bf-room-tuning2-bands", arg: "room_tuning2_bands", modes: ["tuning2"], kind: "text" },
  { id: "bf-room-tuning2-dip-limit", arg: "room_tuning2_dip_limit", modes: ["tuning2"], kind: "check" },
  { id: "bf-room-tuning2-low-protect", arg: "room_tuning2_low_protect", modes: ["tuning2"], kind: "check" },
  { id: "bf-room-tuning2-mode-decay", arg: "room_tuning2_mode_decay", modes: ["tuning2"], kind: "check" },
  // FIR only: PEQ only (the zero filter) locks them, and a locked check sends false.
  { id: "bf-room-tuning2-lf-align", arg: "room_tuning2_lf_align", modes: ["tuning2"], kind: "check",
    disabled: (c) => c.filter === "zero" },
  { id: "bf-room-tuning2-hf-phase", arg: "room_tuning2_hf_phase", modes: ["tuning2"], kind: "check",
    disabled: (c) => c.filter === "zero" },
  // MIMO's limits show and go only with MIMO on.
  { id: "bf-room-tuning2-mimo", arg: "room_tuning2_mimo", modes: ["tuning2"], kind: "check", refresh: true },
  { id: "bf-room-tuning2-fsiso", arg: "room_tuning2_fsiso", modes: ["tuning2"], when: (c) => c.mimo,
    kind: "number", fallback: 150 },
  { id: "bf-room-tuning2-mimo-low", arg: "room_tuning2_mimo_low", modes: ["tuning2"], when: (c) => c.mimo,
    kind: "numberOr", word: "auto" },
  { id: "bf-room-tuning2-mimo-early", arg: "room_tuning2_mimo_early", modes: ["tuning2"], when: (c) => c.mimo,
    kind: "number", fallback: 60 },
  { id: "bf-room-tuning2-mimo-ear-guard", arg: "room_tuning2_mimo_ear_guard", modes: ["tuning2"],
    when: (c) => c.mimo, kind: "check" },
];

/** @type {readonly RoomRow[]} */
const ROOM_ROWS = [
  // Both timing-limit controls follow the mode; the styles show the slider or this checkbox by skin.
  { id: "bf-room-phase-schroeder-row", modes: ["tuning"] },
  // Tuning 2's group headings, the wrappers that pair or indent its rows, and its hints.
  { id: "bf-room-tuning2-head-filter", modes: ["tuning2"] },
  { id: "bf-room-tuning2-latency-row", modes: ["tuning2"], when: (c) => c.filter === "custom" },
  { id: "bf-room-tuning2-head-band", modes: ["tuning2"] },
  { id: "bf-room-tuning2-band-pair", modes: ["tuning2"] },
  // The basis and the reach need a room volume or a Schroeder frequency to move the automatic upper end.
  { id: "bf-room-tuning2-high-volume-hint", modes: ["tuning2"], when: (c) => c.roomFacts
      && (val("bf-room-tuning2-high-basis") === "lenient" || val("bf-room-tuning2-high-transition") !== "1")
      && val("bf-room-volume") === "" && val("bf-schroeder-freq") === "" },
  { id: "bf-room-tuning2-head-protect", modes: ["tuning2"] },
  { id: "bf-room-tuning2-head-timing", modes: ["tuning2"] },
  { id: "bf-room-tuning2-fir-hint", modes: ["tuning2"], when: (c) => c.filter === "zero" },
  { id: "bf-room-tuning2-head-speakers", modes: ["tuning2"] },
  { id: "bf-room-tuning2-mimo-detail", modes: ["tuning2"], when: (c) => c.mimo },
];

/* Of the room facts, Room EQ reads them unless the range is legacy, tuning
   only at the Schroeder timing limit, tuning 2 only for an automatic band. */
/** @returns {RoomContext} */
function roomContext() {
  const mode = roomMode();
  const range = roomRange();
  const limit = roomPhaseLimit();
  const roomFacts = mode === "tuning" ? limit === "schroeder"
    : mode === "tuning2" ? val("bf-room-tuning2-high") === "" : range !== "legacy";
  return { mode, range, limit, filter: tuning2Filter(), mimo: checked("bf-room-tuning2-mimo"), roomFacts };
}

/** @param {RoomShown} row @param {RoomContext} context */
function roomShown(row, context) {
  return row.modes.includes(context.mode) && (row.when?.(context) ?? true);
}

/** @param {string} id */
function roomInput(id) {
  const node = $(id);
  if (!(node instanceof HTMLInputElement || node instanceof HTMLSelectElement)) throw new Error(`Not a field: ${id}`);
  return node;
}

/** @param {RoomField} field @param {RoomContext} context */
function roomFieldValue(field, context) {
  switch (field.kind) {
    case "number": return numOr(field.id, brirDefault(field.arg, field.fallback));
    case "numberOr": return numOrNull(field.id) ?? field.word;
    case "optional": return numOrNull(field.id);
    case "check": return !field.disabled?.(context) && checked(field.id);
    case "text":
    case "select": return val(field.id);
    case "choice": {
      const value = val(field.id);
      const chosen = field.choices.includes(value) ? value : field.fallback;
      return field.number ? Number(chosen) : chosen;
    }
    case "custom": return field.read(context);
  }
}

/** @param {RoomField} field */
function resetRoomField(field) {
  const defaults = state.brirDefaults;
  const spec = defaults[field.arg];
  switch (field.kind) {
    case "number":
      if (!field.fromMarkup) roomInput(field.id).value = String(brirDefault(field.arg, field.fallback));
      break;
    case "numberOr":
      roomInput(field.id).value = numberSpecText(spec);
      break;
    case "check":
      el(field.id, HTMLInputElement).checked = typeof spec === "boolean" ? spec : field.fallback ?? false;
      break;
    case "text":
      roomInput(field.id).value = typeof spec === "string" ? spec : "";
      break;
    case "choice": {
      const value = String(spec ?? field.fallback);
      roomInput(field.id).value = field.choices.includes(value) ? value : field.fallback;
      break;
    }
    case "custom":
      field.reset(defaults);
      break;
    case "optional":
    case "select":
      break;
  }
}

/* Applies the mode, the range, the timing limit and the tuning 2 choices
   together: which rows show and which inputs are locked (from the tables),
   where the room facts sit, the named limit, the readout and the tooltips. */
function updateRoomRows() {
  const context = roomContext();
  const { mode, range, limit } = context;
  for (const field of ROOM_FIELDS) {
    $(`${field.id}-row`).hidden = !roomShown(field, context);
    if (field.disabled) roomInput(field.id).disabled = field.disabled(context);
  }
  for (const row of ROOM_ROWS) $(row.id).hidden = !roomShown(row, context);
  placeRoomFacts(mode === "tuning2");
  // Stable's checkbox shows only Schroeder or the full band; name any other
  // limit the slider keeps. The styles hide this line in Studio.
  const current = $("bf-room-phase-current");
  const named = mode === "tuning" && limit !== "schroeder" && limit !== "full";
  current.hidden = !named;
  current.textContent = named ? fmt(t("label_room_tuning_phase_current"), { limit: roomPhaseText(limit) }) : "";
  updateRoomPhaseReadout();
  $("bf-room-mode-tip").textContent = roomModeHint(mode);
  $("bf-room-range-tip").textContent = roomRangeHint(range);
  $("bf-room-tuning2-mode-tip").textContent = tuning2FilterHint(context.filter);
  // The two tunings match levels differently (±6 dB around the median, or down to the quietest).
  $("bf-room-tuning-level-match-tip").textContent = t(mode === "tuning2" ? "tooltip_room_tuning2_level_match" : "tooltip_room_tuning_level_match");
  el("bf-schroeder-freq", HTMLInputElement).placeholder = t("placeholder_schroeder_freq");
  el("bf-room-tuning2-input-hp", HTMLInputElement).placeholder = t("placeholder_room_tuning2_off");
  for (const id of ["bf-room-tuning2-high", "bf-room-tuning2-mimo-low"]) {
    el(id, HTMLInputElement).placeholder = t("placeholder_room_tuning2_auto");
  }
}

/* Every room setting starts from the service's defaults (see the kinds
   above); the mode comes last and applies the rows. */
function applyRoomDefaults() {
  for (const field of ROOM_FIELDS) resetRoomField(field);
  const mode = state.brirDefaults.room_mode;
  setRoomMode(isRoomMode(mode) ? mode : "eq");
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

/* The room block of the request: the target, the microphone calibration,
   the measurement file, the mode, then every table setting whose row shows. */
/** @param {ProcessingRequest} args */
function addRoomArgs(args) {
  const context = roomContext();
  args.room_target = val("bf-room-target") || null;
  args.room_mic_calibration = val("bf-mic-calibration") || null;
  if (mfileMode("room")) addMfileArgs(args, "room");
  args.room_mode = context.mode;
  const fields = /** @type {Record<string, unknown>} */ (args);
  for (const field of ROOM_FIELDS) {
    if (roomShown(field, context)) fields[field.arg] = roomFieldValue(field, context);
  }
}

/* The room and virtual bass listeners. A table input with `refresh`
   re-applies the rows: a select or checkbox when it changes, any other input
   as it is typed or dragged. */
function wireRoom() {
  for (const field of ROOM_FIELDS) {
    if (!field.refresh) continue;
    const input = roomInput(field.id);
    const changes = input instanceof HTMLSelectElement || input.type === "checkbox";
    input.addEventListener(changes ? "change" : "input", updateRoomRows);
  }
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
  // The second controls of two settings: Stable's limit checkbox replaces the limit, the auto box locks the delay.
  el("bf-room-phase-schroeder", HTMLInputElement).addEventListener("change", onRoomPhaseCheckbox);
  el("bf-room-tuning-delay-auto", HTMLInputElement).addEventListener("change", updateRoomRows);
  el("bf-vbass-mode", HTMLSelectElement).addEventListener("change", updateVbassRows);
}
