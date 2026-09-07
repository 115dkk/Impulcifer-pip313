const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const source = fs.readFileSync(process.argv[2], "utf8");
const listeners = new Map();
const envelope = { ok: true, data: { ui: {} } };
const core = Object.freeze({ invoke: () => Promise.resolve(envelope) });
const window = {
  __TAURI__: { core },
  addEventListener: (name, handler) => {
    if (!listeners.has(name)) listeners.set(name, []);
    listeners.get(name).push(handler);
  },
};
const consoleStub = Object.fromEntries(
  ["debug", "log", "info", "warn", "error", "trace", "assert"].map(name => [name, () => {}]),
);
vm.runInNewContext(source, { window, console: consoleStub });
(async () => {
  const actual = await window.__TAURI__.core.invoke("pywebview_api", { method: "bootstrap", args: [] });
  assert.equal(actual, envelope);
  assert.equal(core.invoke instanceof Function, true);
  assert.deepEqual(Array.from(window.__impulciferSmoke.events, e => e.kind), ["ipc-request", "ipc-response"]);
  const beforeReports = window.__impulciferSmoke.events.length;
  for (let i = 0; i < 100; i++) {
    assert.equal(await window.__TAURI__.core.invoke("pywebview_api", { method: "smoke_report", args: [{ step: "done" }] }), envelope);
  }
  assert.equal(window.__impulciferSmoke.events.length, beforeReports, "reports must never record themselves");
  consoleStub.log("normal message");
  assert.equal(window.__impulciferSmoke.events.at(-1).text, "normal message");
  consoleStub.assert(true, "passing assertion");
  assert.equal(window.__impulciferSmoke.events.at(-1).failed, false);
  consoleStub.error("startup failure");
  for (const handler of listeners.get("unhandledrejection")) handler({ reason: new Error("uncaught") });
  assert.equal(window.__impulciferSmoke.events.at(-1).kind, "pageerror");
  assert.equal(window.__impulciferSmoke.events.at(-2).text, "startup failure");
  console.log("observer contract: frozen Tauri namespace, real envelope identity, startup console/rejection OK");
})().catch(error => { console.error(error); process.exitCode = 1; });
