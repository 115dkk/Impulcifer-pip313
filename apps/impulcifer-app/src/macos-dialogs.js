// @ts-check
// macOS only (dialog.rs injects it there). WKWebView shows a confirm or alert panel only through
// a UI delegate, and wry implements neither: window.confirm returns false at once (measured 0-1 ms),
// so every recording start ended there, and window.alert showed nothing. Both go to the dialog
// plugin's `message` command instead, the way the plugin's own JS `confirm()` does it.
// confirm resolves to a boolean, so ui/app.js awaits it; on WebView2 and WebKitGTK the browser's
// synchronous boolean stays, and awaiting it changes nothing.
// The plugin's bundled shim is not reused: it still invokes the plugin's `confirm` command, which
// the plugin no longer has (2.7 folded ask/confirm into `message`).
(function () {
  /** @param {Record<string, unknown>} payload */
  const message = payload => {
    const core = window.__TAURI__ && window.__TAURI__.core;
    if (!core) {
      return Promise.reject(new Error("impulcifer dialogs: Tauri invoke is unavailable (withGlobalTauri must be true)"));
    }
    return core.invoke("plugin:dialog|message", payload);
  };
  window.alert = text => {
    message({ message: String(text) }).catch(error => console.error("impulcifer dialogs: alert failed", error));
  };
  window.confirm = /** @type {(text?: string) => boolean} */ (/** @type {unknown} */ (
    async (/** @type {unknown} */ text) => (await message({ message: String(text), buttons: "OkCancel" })) === "Ok"
  ));
})();
