// Contract for src/macos-dialogs.js: confirm resolves to the user's answer from the dialog
// plugin's `message` command, alert shows a message, and nothing else is touched.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const source = fs.readFileSync(process.argv[2], "utf8");
const calls = [];
let answer = "Ok";
const window = {
  __TAURI__: { core: { invoke: (command, payload) => {
    // The payload comes from the script's realm; copy it so deepEqual compares values only.
    calls.push([command, JSON.parse(JSON.stringify(payload))]);
    return Promise.resolve(answer);
  } } },
};
vm.runInNewContext(source, { window, console: { error: () => {} }, Promise, Error, String });
(async () => {
  assert.equal(await window.confirm("start?"), true);
  assert.deepEqual(calls.at(-1), ["plugin:dialog|message", { message: "start?", buttons: "OkCancel" }]);
  answer = "Cancel";
  assert.equal(await window.confirm("start?"), false);
  answer = "Ok";
  assert.equal(window.alert("done"), undefined);
  assert.deepEqual(calls.at(-1), ["plugin:dialog|message", { message: "done" }]);
  assert.deepEqual(calls.map(([command]) => command), Array(3).fill("plugin:dialog|message"));
  // Without the Tauri global the confirm rejects instead of answering "no" silently.
  delete window.__TAURI__;
  await assert.rejects(window.confirm("start?"), /Tauri invoke is unavailable/);
  console.log("macos dialogs contract: confirm answers Ok/Cancel through message, alert shows, missing invoke rejects OK");
})().catch(error => {
  console.error(error);
  process.exit(1);
});
