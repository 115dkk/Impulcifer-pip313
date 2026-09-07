#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

//! The 23 pywebview-compatible IPC methods. Every method returns an envelope
//! and never panics across the boundary. Host capabilities (dialogs, open
//! path/url, title-bar theme) are injected through `HostAdapter`.

use impulcifer_types::ipc::{self, ErrorCode, IpcMethod};
use serde_json::{Value, json};

pub trait HostAdapter: Send + Sync {
    fn select_file(&self, kind: &str) -> Option<String>;
    fn select_directory(&self) -> Option<String>;
    fn open_path(&self, path: &str) -> Result<(), String>;
    fn open_url(&self, url: &str) -> Result<(), String>;
    fn apply_title_theme(&self, theme: &str);
}

/// Host adapter for headless contexts (CLI, tests): dialogs return None.
pub struct NoopHost;

impl HostAdapter for NoopHost {
    fn select_file(&self, _kind: &str) -> Option<String> {
        None
    }
    fn select_directory(&self) -> Option<String> {
        None
    }
    fn open_path(&self, _path: &str) -> Result<(), String> {
        Ok(())
    }
    fn open_url(&self, _url: &str) -> Result<(), String> {
        Ok(())
    }
    fn apply_title_theme(&self, _theme: &str) {}
}

pub struct ImpulciferService {
    host: Box<dyn HostAdapter>,
}

impl ImpulciferService {
    pub fn new(host: Box<dyn HostAdapter>) -> Self {
        ImpulciferService { host }
    }

    /// Dispatch a pywebview-style call: positional `args` as sent by the
    /// frontend. Unknown methods and internal failures become envelopes.
    pub fn call(&self, method: &str, args: Vec<Value>) -> Value {
        let Some(method) = IpcMethod::from_wire_name(method) else {
            return ipc::error(
                ErrorCode::InvalidRequest,
                format!("unknown method: {method}"),
                json!({}),
                false,
            );
        };
        let _ = &self.host;
        let _ = args;
        ipc::error(
            ErrorCode::InternalError,
            format!("{} not implemented", method.wire_name()),
            json!({}),
            false,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_method_is_invalid_request_envelope() {
        let service = ImpulciferService::new(Box::new(NoopHost));
        let out = service.call("does_not_exist", vec![]);
        assert_eq!(out["ok"], json!(false));
        assert_eq!(out["error"]["code"], json!("INVALID_REQUEST"));
    }
}
