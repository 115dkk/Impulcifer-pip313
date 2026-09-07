// A failed startup still reports diagnostics and done, without calling services.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const source = fs.readFileSync(process.argv[2], "utf8");
const calls = [];
let clock = 0;
let done;
const finished = new Promise(resolve => { done = resolve; });
const events = [{ kind: "pageerror", text: "original boot failure" }];
const window = {
  __impulciferSmokeParams: {},
  __impulciferSmoke: { events },
  addEventListener() {},
  __TAURI__: { core: { invoke(command, payload) {
    calls.push({ command, payload });
    if (payload.args[0].step === "done") done(payload.args[0]);
    return Promise.resolve({ ok: true, data: null });
  } } },
};
vm.runInNewContext(source, {
  window, document: { querySelector: () => null }, console,
  location: { href: "http://tauri.localhost/" },
  Date: { now: () => { clock += 10000; return clock; } },
  setTimeout: callback => setImmediate(callback),
});
(async () => {
  const result = await finished;
  assert.equal(result.steps[0].step, "bootstrap");
  assert.equal(result.steps[0].status, "fail");
  assert.match(result.steps[0].error, /ready and bootstrap/);
  assert.equal(result.events[0], events[0], "done must preserve original page errors");
  assert.ok(calls.every(c => c.command === "pywebview_api" && c.payload.method === "smoke_report"));
  assert.ok(calls.some(c => c.payload.args[0].checkpoint), "failed step still requests evidence");
  console.log("driver contract: boot timeout preserves diagnostics, checkpoint and done; no substitute IPC OK");
})().catch(error => { console.error(error); process.exitCode = 1; });
