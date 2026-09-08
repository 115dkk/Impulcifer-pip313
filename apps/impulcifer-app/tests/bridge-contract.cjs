// Execute the actual injected script in a minimal DOM/event environment.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const source = fs.readFileSync(process.argv[2], "utf8");

async function check(readyState) {
  const listeners = new Map();
  const calls = [];
  const delivered = [];
  const envelope = { ok: false, error: { code: "INTERNAL_ERROR", message: "sample" } };
  const window = {
    __TAURI__: { core: { invoke: (command, payload) => {
      calls.push({ command, payload });
      return Promise.resolve(envelope);
    } } },
    addEventListener(name, handler, options) {
      listeners.set(name, { handler, options });
    },
    dispatchEvent(event) {
      assert.ok(window.pywebview.api, "API must exist before ready dispatch");
      delivered.push(event.type);
      const listener = listeners.get(event.type);
      if (listener) {
        if (listener.options?.once) listeners.delete(event.type);
        listener.handler(event);
      }
    },
  };
  const context = vm.createContext({ window, document: { readyState }, Event, console });
  vm.runInContext(source, context);
  const api = window.pywebview.api;
  assert.equal(typeof api.bootstrap, "function");
  assert.equal(api[Symbol.iterator], undefined);
  assert.equal(await api.bootstrap(), envelope, "envelope identity must be preserved");
  const args = [{ dir_path: "temp", enabled: true }, null, 4, "literal"];
  assert.equal(await api.start_brir(...args), envelope);
  assert.deepEqual(JSON.parse(JSON.stringify(calls)), [
    { command: "pywebview_api", payload: { method: "bootstrap", args: [] } },
    { command: "pywebview_api", payload: { method: "start_brir", args } },
  ]);
  if (readyState === "loading") {
    assert.deepEqual(delivered, [], "ready must wait for DOMContentLoaded");
    assert.equal(listeners.get("DOMContentLoaded").options.once, true);
    window.dispatchEvent(new Event("DOMContentLoaded"));
    window.dispatchEvent(new Event("DOMContentLoaded"));
  }
  assert.equal(delivered.filter(name => name === "pywebviewready").length, 1);
  vm.runInContext(source, context);
  assert.equal(window.pywebview.api, api, "re-injection must retain existing API");
  assert.equal(delivered.filter(name => name === "pywebviewready").length, 1);
}

(async () => {
  for (const readyState of ["loading", "interactive", "complete"]) await check(readyState);
  const errors = [];
  const window = {};
  vm.runInNewContext(source, { window, console: { error: text => errors.push(text) } });
  assert.equal(window.pywebview, undefined);
  assert.equal(errors.length, 1);
  assert.match(errors[0], /Tauri invoke is unavailable/);
  console.log("bridge contract: loading/interactive/complete, positional IPC, envelope identity, once and missing invoke OK");
})().catch(error => { console.error(error); process.exitCode = 1; });
