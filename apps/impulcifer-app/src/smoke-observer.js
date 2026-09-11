// @ts-check
// Enabled only by the local smoke harness. Never replace service envelopes.
(function () {
  /** @type {SmokeObserver} */
  const observer = { events: [], startedAt: Date.now() };
  window.__impulciferSmoke = observer;
  /** @param {SmokeEvent["kind"]} kind @param {Record<string, unknown>} detail */
  const record = (kind, detail) => {
    // This observer is the transport boundary. The method determines the wire
    // response variant; consumers narrow it by kind and method, never by casts.
    observer.events.push(/** @type {SmokeEvent} */ ({ kind, time: Date.now(),
      step: observer.currentStep || "launch", ...detail }));
  };
  /** @type {(keyof Console)[]} */
  const levels = ["debug", "log", "info", "warn", "error", "trace", "assert",
    "dir", "dirxml", "table", "count", "countReset", "time", "timeLog", "timeEnd",
    "group", "groupCollapsed", "groupEnd", "clear"];
  for (const level of levels) {
    if (typeof console[level] !== "function") continue;
    const original = /** @type {(...args: unknown[]) => void} */ (console[level].bind(console));
    console[level] = (/** @type {unknown[]} */ ...args) => {
      record("console", { level, text: args.map(String).join(" "),
        failed: level === "assert" ? !args[0] : level === "error" });
      return original(...args);
    };
  }
  window.addEventListener("error", (event) => {
    record("pageerror", { text: event.message, stack: event.error instanceof Error ? event.error.stack : "",
      filename: event.filename, line: event.lineno });
  });
  window.addEventListener("error", (event) => {
    const target = event.target;
    if (target instanceof Element) {
      record("resource-error", { text: target.getAttribute("src") || target.getAttribute("href") || target.tagName });
    }
  }, true);
  window.addEventListener("unhandledrejection", (event) => {
    record("pageerror", { text: String(event.reason), stack: event.reason instanceof Error ? event.reason.stack : "" });
  });
  const tauri = window.__TAURI__;
  const core = tauri?.core;
  if (!tauri || !core?.invoke) {
    record("pageerror", { text: "smoke observer: Tauri invoke is unavailable" });
    return;
  }
  const invoke = core.invoke;
  // Tauri freezes core; replace its parent with a shallow copy.
  tauri.core = { ...core, invoke:
    /** @template T @param {string} command @param {Record<string, unknown>} [payload] @param {unknown} [options] @returns {Promise<T>} */
    function (command, payload, options) {
      const result = invoke.call(this, command, payload, options);
      if (command !== "pywebview_api" || payload?.method === "smoke_report") return /** @type {Promise<T>} */ (result);
      record("ipc-request", { method: payload?.method, args: payload?.args });
      return /** @type {Promise<T>} */ (result.then((response) => {
        record("ipc-response", { method: payload?.method, response });
        return response;
      }));
    }
  };
})();
