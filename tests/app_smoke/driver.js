// @ts-check
// Opt-in initialization script. Drive the frozen DOM, never replace service calls.
(function () {
  const params = window.__impulciferSmokeParams;
  const observer = window.__impulciferSmoke;
  if (!params || !observer) throw new Error("Smoke initialization missing");
  let ready = false;
  window.addEventListener("pywebviewready", () => { ready = true; }, { once: true });
  /** @param {number} ms */
  const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
  /** @param {unknown} value @param {string | null} message @returns {asserts value} */
  function assert(value, message) { if (!value) throw new Error(message || "assertion failed"); }
  /** @param {string} selector @returns {HTMLElement} */
  const node = selector => { const value = document.querySelector(selector); assert(value instanceof HTMLElement, `missing ${selector}`); return value; };
  /** @param {string} selector */
  const select = selector => { const value = node(selector); assert(value instanceof HTMLSelectElement, `not a select ${selector}`); return value; };
  /** @param {string} selector */
  const enabled = selector => { const value = node(selector); return !("disabled" in value) || !value.disabled; };
  /** @param {() => unknown} test @param {string} message @param {number} timeout */
  const wait = async (test, message, timeout = 15000) => {
    const end = Date.now() + timeout;
    while (!test()) { assert(Date.now() < end, `timeout: ${message}`); await delay(50); }
  };
  /** @param {Record<string, unknown>} record */
  const report = async record => {
    const tauri = window.__TAURI__;
    assert(tauri, "Tauri missing");
    const response = await /** @type {Promise<Envelope<null>>} */ (tauri.core.invoke("pywebview_api", { method: "smoke_report", args: [record] }));
    assert(response.ok, `report sink: ${JSON.stringify(response)}`);
  };
  let sequence = 0;
  /** @param {string} step @param {Record<string, unknown>} detail */
  const checkpoint = async (step, detail = {}) => {
    // The smoke sink waits on the harness's atomic acknowledgment file only
    // after appending this record. Python verifies disk state and PrintWindow
    // before permitting navigation; no network permissions or extra command.
    const id = ++sequence;
    await delay(150);
    await report({ step, checkpoint: id, ...detail, events: observer.events.slice() });
  };
  /** @param {string} selector @param {string} value */
  const input = (selector, value) => {
    const element = node(selector);
    assert(element instanceof HTMLInputElement || element instanceof HTMLSelectElement, `not a field ${selector}`);
    assert(!element.disabled, `disabled ${selector}`);
    element.value = value;
    assert(element.value === value, `unavailable value ${value}`);
    element.dispatchEvent(new Event("input", { bubbles: true }));
    element.dispatchEvent(new Event("change", { bubbles: true }));
  };
  /** @param {string} selector */
  const click = selector => { const element = node(selector); assert(enabled(selector), `disabled ${selector}`); element.click(); };
  /** @param {string} view */
  const navigate = async view => {
    click(`#nav [data-view="${view}"]`);
    await wait(() => node(`#view-${view}`).classList.contains("active"), `view ${view}`);
    node("#content").scrollTop = 0;
  };
  /** @template {IpcMethod} M @param {M} method @param {number} start */
  const responses = (method, start = 0) => observer.events.slice(start).filter(
    /** @returns {e is Extract<SmokeIpcResponse, {method: M}>} */
    e => e.kind === "ipc-response" && e.method === method);
  /** @param {string} view @param {string} label @param {IpcMethod} method @param {number} start */
  const job = async (view, label, method, start) => {
    const prefix = params.en[label] + " · ";
    await wait(() => {
      const rejected = responses(method, start).find(e => !e.response.ok && e.response.error.code !== "CONFIRMATION_REQUIRED");
      assert(!rejected, `start rejected: ${JSON.stringify(rejected)}`);
      return ["succeeded", "failed", "cancelled"].some(s => node(`#view-${view} [data-job-state]`).textContent === prefix + params.en[`webview_status_${s}`]);
    }, method, 120000);
    const status = node(`#view-${view} [data-job-state]`).textContent;
    assert(status === prefix + params.en.webview_status_succeeded, `${status}: ${node('[data-log]').textContent}`);
    return status;
  };
  /** @type {SmokeStep[]} */
  const steps = [];
  /** @param {string} name @param {() => Promise<Record<string, unknown>>} action */
  const step = async (name, action) => {
    const start = Date.now();
    observer.currentStep = name;
    /** @type {SmokeStep} */
    const record = { step: name, status: "ok", details: {} };
    try { record.details = await action() || {}; }
    catch (error) { record.status = "fail"; record.error = String(error); record.stack = error instanceof Error ? error.stack : undefined; }
    record.duration_seconds = (Date.now() - start) / 1000;
    record.ui_log = document.querySelector('[data-log]')?.textContent || "";
    try { await checkpoint(name, { ...record }); }
    catch (error) { record.status = "fail"; record.evidence_error = String(error); }
    steps.push(record);
    await report({ ...record, result: true });
    return record.status === "ok";
  };
  (async () => {
    try {
      const booted = await step("bootstrap", async () => {
        await wait(() => ready && responses("bootstrap").length > 0, "ready and bootstrap");
        const boots = responses("bootstrap");
        assert(boots.length === 1 && boots[0].response.ok && boots[0].response.data.ui.first_run, "fresh bootstrap must succeed once");
        await wait(() => !node("#language-modal").hidden, "first-run language modal");
        await checkpoint("first-run");
        const english = [...document.querySelectorAll("#language-modal-list button")].find(b => b.textContent === "English");
        assert(english instanceof HTMLElement, "English first-run choice absent"); english.click();
        await wait(() => node("#language-modal").hidden && document.documentElement.lang === "en", "English selected");
        await navigate("settings"); input("#sf-skin", "studio");
        await wait(() => document.documentElement.dataset.skin === "studio", "Studio skin");
        await navigate("processing");
        await wait(() => enabled("#btn-generate-brir"), "enabled generate");
        assert([...select("#rf-share-mode").options].some(o => o.value === "auto"), "Auto device access missing");
        return { runtime: node("#runtime-status").textContent, bootstrap_observed: true, reloads: 0 };
      });
      if (booted) {
        const brir = await step("brir", async () => {
          await navigate("processing"); input("#bf-dir-path", params.demo); input("#bf-test-signal-source", "auto");
          const start = observer.events.length; click("#btn-generate-brir");
          const status = await job("processing", "sidebar_processing", "start_brir", start);
          assert(document.querySelectorAll("#brir-steps li.done").length === 10, "incomplete BRIR checklist");
          node("#brir-steps").scrollIntoView({ block: "center" });
          return { status, test_signal: "auto", completed_steps: 10 };
        });
        await step("recovery", async () => {
          assert(brir, "BRIR failed; no substitute recovery input");
          await navigate("recovery"); input("#recovery-dir-path", params.recovery);
          await wait(() => enabled("#btn-start-recovery"), "recovery plan ready");
          click("#btn-start-recovery");
          await wait(() => ["succeeded", "failed", "cancelled"].includes(node(".recovery-result").dataset.status || ""), "recovery terminal", 120000);
          assert(node(".recovery-result").dataset.status === "succeeded", node("#recovery-status-detail").textContent);
          assert(node("#recovery-status-title").textContent === params.en.webview_status_succeeded, "recovery title");
          const created = node('#recovery-ledger [data-file="hrir.wav"]').dataset.status;
          assert(created === "created", "hrir.wav ledger must be created");
          return { title: node("#recovery-status-title").textContent, ledger_hrir: created };
        });
        await step("settings", async () => {
          await navigate("settings");
          for (const code of ["ko", "en"]) {
            input("#sf-language", code);
            await wait(() => node('label[for="sf-language"]').textContent === params.labels[code], `${code} label`);
            await checkpoint(`settings-${code}`, { language: code, label: node('label[for="sf-language"]').textContent });
          }
          return { languages: ["ko", "en"] };
        });
        await step("updates", async () => {
          await navigate("info"); const start = observer.events.length; click("#btn-check-updates");
          // The button sets the "checking" line synchronously; the check itself is real since P20
          // (GitHub releases): it settles as "up to date", as the update modal, or as an error
          // line when the network is unavailable. Any of those passes; the envelope must be
          // well-formed and the UI must leave the checking state.
          const status = node("#update-check-status");
          const checking = status.textContent;
          await wait(() => responses("check_for_updates", start).length === 1, "update check response");
          const result = responses("check_for_updates", start);
          const actual = result[0]?.response;
          const okShape = !!actual && actual.ok === true && !!actual.data
            && typeof actual.data.update_available === "boolean" && typeof actual.data.current_version === "string";
          const errShape = !!actual && actual.ok === false && !!actual.error
            && typeof actual.error.message === "string" && actual.error.message.length > 0;
          assert(okShape || errShape, `update envelope: ${JSON.stringify(result)}`);
          await wait(() => status.hidden || status.textContent !== checking
            || !node("#update-modal").hidden, "update status settled");
          const modal = !node("#update-modal").hidden;
          assert(!actual?.ok || actual.data.update_available === modal,
            `update modal ${modal} does not match envelope ${JSON.stringify(actual)}`);
          return { ui_text: modal ? "modal" : status.textContent, modal, envelope: actual };
        });
        if (params.hardware) await step("recording", async () => {
          await navigate("recorder");
          /** @type {Record<string, string>} */
          const devices = {};
          for (const [id, prefix] of [["rf-output-device", "CABLE-A Input"], ["rf-input-device", "CABLE-A Output"]]) {
            const matches = [...select(`#${id}`).options].filter(o => o.value.startsWith(prefix) && o.value.includes("VB-Audio Cable A"));
            assert(matches.length === 1, `documented CABLE-A missing/ambiguous: ${id}`);
            input(`#${id}`, matches[0].value); devices[id] = matches[0].value;
          }
          input("#rf-record-dir", params.recording); input("#rf-sweep-source", "default");
          await checkpoint("recording-ready");
          const start = observer.events.length;
          click("#btn-record-headphones");
          const status = await job("recorder", "sidebar_recorder", "start_recording", start);
          node('#view-recorder [data-activity-card]').scrollIntoView({ block: "center" });
          return { status, devices };
        });
        else { steps.push({ step: "recording", status: "skipped", duration_seconds: 0, details: { reason: "requires --hardware" } }); await report({ ...steps.at(-1), result: true }); }
      }
    } catch (error) {
      steps.push({ step: "driver", status: "fail", error: String(error), stack: error instanceof Error ? error.stack : undefined });
    } finally {
      await report({ step: "done", steps, events: observer.events.slice(), url: location.href });
    }
  })().catch(error => console.error("smoke driver could not report done", error));
})();
