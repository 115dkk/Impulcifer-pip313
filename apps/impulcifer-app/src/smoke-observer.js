// Enabled only by the local smoke harness, before the bridge and page scripts.
// Observe real calls and errors; never replace an envelope or consume a rejection.
(function () {
  const events = [];
  window.__impulciferSmoke = { events, startedAt: Date.now() };
  const record = (kind, detail) => events.push({ kind, time: Date.now(),
    step: window.__impulciferSmoke.currentStep || "launch", ...detail });
  for (const level of ["debug", "log", "info", "warn", "error", "trace", "assert",
    "dir", "dirxml", "table", "count", "countReset", "time", "timeLog", "timeEnd",
    "group", "groupCollapsed", "groupEnd", "clear"]) {
    if (typeof console[level] !== "function") continue;
    const original = console[level].bind(console);
    console[level] = (...args) => {
      record("console", { level, text: args.map(String).join(" "),
        failed: level === "assert" ? !args[0] : level === "error" });
      return original(...args);
    };
  }
  window.addEventListener("error", (event) => {
    record("pageerror", { text: event.message, stack: event.error?.stack || "",
      filename: event.filename, line: event.lineno });
  });
  window.addEventListener("error", (event) => {
    if (event.target !== window) {
      record("resource-error", { text: event.target.src || event.target.href || event.target.tagName });
    }
  }, true);
  window.addEventListener("unhandledrejection", (event) => {
    record("pageerror", { text: String(event.reason), stack: event.reason?.stack || "" });
  });
  const core = window.__TAURI__?.core;
  if (!core?.invoke) {
    record("pageerror", { text: "smoke observer: Tauri invoke is unavailable" });
    return;
  }
  const invoke = core.invoke;
  // Tauri freezes the core export namespace; replace its parent property
  // with a shallow copy, retaining every API and the original invoke function.
  window.__TAURI__.core = { ...core, invoke: function (command, payload, ...rest) {
    const result = invoke.call(this, command, payload, ...rest);
    if (command !== "pywebview_api" || payload.method === "smoke_report") return result;
    record("ipc-request", { method: payload.method, args: payload.args });
    return result.then((response) => {
      record("ipc-response", { method: payload.method, response });
      return response;
    });
  } };
})();
