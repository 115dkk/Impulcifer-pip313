const state = {
  jobId: null,
  nextSeq: 0,
  pollTimer: null,
};

const runtimeStatus = document.querySelector("#runtime-status");
const jobState = document.querySelector("#job-state");
const progress = document.querySelector("#progress");
const eventLog = document.querySelector("#event-log");
const cancelButton = document.querySelector("#cancel-job");

function appendLog(message) {
  eventLog.textContent += `\n${message}`;
  eventLog.scrollTop = eventLog.scrollHeight;
}

function errorText(response) {
  if (!response || response.ok) return "Unknown error";
  const detail = response.error.details || {};
  return `${response.error.code}: ${response.error.message}${Object.keys(detail).length ? ` ${JSON.stringify(detail)}` : ""}`;
}

function setBusy(job) {
  const busy = Boolean(job && !["succeeded", "failed", "cancelled"].includes(job.status));
  document.querySelectorAll("[data-start]").forEach((button) => { button.disabled = busy; });
  cancelButton.disabled = !busy || !job.cancellable;
  jobState.textContent = job ? `${job.kind} · ${job.status}` : "Idle";
}

function formObject(form) {
  const data = Object.fromEntries(new FormData(form).entries());
  form.querySelectorAll('input[type="checkbox"]').forEach((input) => { data[input.name] = input.checked; });
  return data;
}

async function confirmAndRetry(start, payload, response) {
  if (response?.error?.code !== "CONFIRMATION_REQUIRED") return response;
  if (!window.confirm(response.error.message)) return null;
  payload.confirm_warnings = true;
  return start(payload);
}

async function begin(start, payload) {
  let response = await start(payload);
  response = await confirmAndRetry(start, payload, response);
  if (!response) return;
  if (!response.ok) {
    appendLog(errorText(response));
    return;
  }
  state.jobId = response.data.job.job_id;
  state.nextSeq = 0;
  progress.value = 0;
  setBusy(response.data.job);
  schedulePoll(0);
}

function schedulePoll(delay = 250) {
  window.clearTimeout(state.pollTimer);
  state.pollTimer = window.setTimeout(pollJob, delay);
}

async function pollJob() {
  if (!state.jobId) return;
  const response = await window.pywebview.api.poll_job(state.jobId, state.nextSeq);
  if (!response.ok) {
    appendLog(errorText(response));
    setBusy(null);
    return;
  }
  const { job, events, next_seq: nextSeq } = response.data;
  state.nextSeq = nextSeq;
  for (const event of events) {
    if (event.type === "progress" && typeof event.payload.progress === "number") {
      progress.value = event.payload.progress;
    }
    if (event.type === "log") appendLog(`[${event.payload.level}] ${event.payload.message}`);
    if (event.type === "progress" && event.payload.message) appendLog(event.payload.message);
    if (event.type === "status") appendLog(`Status: ${event.payload.status}`);
  }
  setBusy(job);
  if (["succeeded", "failed", "cancelled"].includes(job.status)) {
    if (job.result) appendLog(JSON.stringify(job.result, null, 2));
    if (job.error) appendLog(`${job.error.code}: ${job.error.message}`);
    state.jobId = null;
    return;
  }
  schedulePoll();
}

async function loadDevices(hostApi = "") {
  const response = await window.pywebview.api.list_audio_devices(hostApi || null);
  if (!response.ok) {
    appendLog(errorText(response));
    return;
  }
  const hostSelect = document.querySelector("#host-api");
  if (hostSelect.options.length === 1) {
    response.data.host_apis.forEach((name) => hostSelect.add(new Option(name, name)));
  }
  const inputs = response.data.devices.filter((item) => item.max_input_channels > 0);
  const outputs = response.data.devices.filter((item) => item.max_output_channels > 0);
  fillDevices(document.querySelector("#input-device"), inputs);
  fillDevices(document.querySelector("#output-device"), outputs);
}

function fillDevices(select, devices) {
  select.replaceChildren(new Option("Default", ""));
  devices.forEach((device) => select.add(new Option(device.name, device.name)));
}

document.querySelector("#host-api").addEventListener("change", (event) => loadDevices(event.target.value));

document.querySelector("#recorder-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const payload = formObject(event.currentTarget);
  payload.channels = Number(payload.channels);
  await begin((request) => window.pywebview.api.start_recording(request), payload);
});

document.querySelector("#brir-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  await begin(
    (request) => window.pywebview.api.start_brir(request),
    formObject(event.currentTarget),
  );
});

cancelButton.addEventListener("click", async () => {
  if (!state.jobId) return;
  const response = await window.pywebview.api.cancel_job(state.jobId);
  if (!response.ok) appendLog(errorText(response));
});

window.addEventListener("pywebviewready", async () => {
  const response = await window.pywebview.api.bootstrap();
  if (!response.ok) {
    runtimeStatus.textContent = errorText(response);
    return;
  }
  runtimeStatus.textContent = `v${response.data.version} · ${response.data.platform} · Edge WebView2`;
  eventLog.textContent = "Python bridge connected.";
  setBusy(response.data.active_job);
  if (response.data.active_job) {
    state.jobId = response.data.active_job.job_id;
    state.nextSeq = 0;
    schedulePoll(0);
  }
  await loadDevices();
});
