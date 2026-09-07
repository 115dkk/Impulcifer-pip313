// Injected as a Tauri initialization script before webview_ui/app.js runs.
// It recreates the pywebview contract so the 2.x frontend stays untouched:
//   window.pywebview.api.<method>(...positionalArgs) -> Promise<envelope>
//   window.dispatchEvent(new Event("pywebviewready")) once the DOM is ready.
(function () {
  if (window.pywebview && window.pywebview.api) {
    return;
  }
  const invoke = window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke;
  if (!invoke) {
    console.error("impulcifer bridge: Tauri invoke is unavailable (withGlobalTauri must be true)");
    return;
  }
  const api = new Proxy({}, {
    get(_target, name) {
      if (typeof name !== "string") {
        return undefined;
      }
      return (...args) => invoke("pywebview_api", { method: name, args });
    },
  });
  window.pywebview = { api };
  const fire = () => window.dispatchEvent(new Event("pywebviewready"));
  if (document.readyState === "loading") {
    window.addEventListener("DOMContentLoaded", fire, { once: true });
  } else {
    fire();
  }
})();
