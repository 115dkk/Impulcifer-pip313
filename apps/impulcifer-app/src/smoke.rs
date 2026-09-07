//! Opt-in startup configuration and serialized report sink; no extra IPC command.
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::Mutex,
};

use impulcifer_types::ipc::{self, ErrorCode};
use serde_json::{Value, json};

pub struct SmokeConfig {
    sink: Mutex<Result<File, String>>,
    ack_directory: Option<PathBuf>,
    pub driver: Option<String>,
    pub data_directory: Option<PathBuf>,
    pub cdp_port: Option<u16>,
}

impl SmokeConfig {
    pub fn from_environment() -> Result<Option<Self>, Box<dyn std::error::Error>> {
        if std::env::var("IMPULCIFER_APP_SMOKE").as_deref() != Ok("1") {
            return Ok(None);
        }
        let sink = std::env::var_os("IMPULCIFER_APP_SMOKE_REPORT")
            .ok_or_else(|| "IMPULCIFER_APP_SMOKE_REPORT is missing".to_owned())
            .and_then(|path| {
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .map_err(|e| e.to_string())
            });
        let driver = if let Some(path) = std::env::var_os("IMPULCIFER_APP_SMOKE_DRIVER") {
            let path = PathBuf::from(path);
            if !path.is_absolute() {
                return Err("smoke driver path must be absolute".into());
            }
            let params: Value = serde_json::from_slice(&std::fs::read(std::env::var(
                "IMPULCIFER_APP_SMOKE_PARAMS",
            )?)?)?;
            Some(format!(
                "window.__impulciferSmokeParams = {};\n{}",
                serde_json::to_string(&params)?,
                std::fs::read_to_string(path)?
            ))
        } else {
            None
        };
        Ok(Some(Self {
            sink: Mutex::new(sink),
            ack_directory: std::env::var_os("IMPULCIFER_APP_SMOKE_ACK_DIR").map(PathBuf::from),
            driver,
            data_directory: std::env::var_os("WEBVIEW2_USER_DATA_FOLDER").map(PathBuf::from),
            cdp_port: std::env::var("IMPULCIFER_APP_SMOKE_CDP_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .filter(|p| *p != 0),
        }))
    }

    pub fn report(&self, args: &[Value]) -> Value {
        let Some(record) = args.first().filter(|_| args.len() == 1) else {
            return ipc::error(
                ErrorCode::InvalidRequest,
                "smoke_report requires one record",
                json!({}),
                false,
            );
        };
        let result = (|| -> Result<(), String> {
            let mut sink = self.sink.lock().map_err(|e| e.to_string())?;
            let file = sink.as_mut().map_err(|e| e.clone())?;
            let mut line = serde_json::to_vec(record).map_err(|e| e.to_string())?;
            line.push(b'\n');
            file.write_all(&line)
                .and_then(|()| file.flush())
                .map_err(|e| e.to_string())?;
            drop(sink);
            if let Some(id) = record.get("checkpoint").and_then(Value::as_u64) {
                let directory = self
                    .ack_directory
                    .as_ref()
                    .ok_or("smoke acknowledgment directory missing")?;
                let path = directory.join(format!("{id}.json"));
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
                loop {
                    match std::fs::read(&path) {
                        Ok(bytes) => {
                            let ack: Value =
                                serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                            if ack["ok"] != true {
                                return Err(ack["error"]
                                    .as_str()
                                    .unwrap_or("checkpoint failed")
                                    .to_owned());
                            }
                            break;
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            if std::time::Instant::now() >= deadline {
                                return Err("smoke checkpoint timed out".into());
                            }
                            std::thread::sleep(std::time::Duration::from_millis(20));
                        }
                        Err(e) => return Err(e.to_string()),
                    }
                }
            }
            Ok(())
        })();
        match result {
            Ok(()) => json!({"ok":true,"data":null}),
            Err(message) => ipc::error(ErrorCode::InternalError, message, json!({}), false),
        }
    }
}

pub fn dispatch(
    smoke: Option<&SmokeConfig>,
    method: &str,
    args: Vec<Value>,
    forward: impl FnOnce(&str, Vec<Value>) -> Value,
) -> Value {
    if method == "smoke_report"
        && let Some(smoke) = smoke
    {
        smoke.report(&args)
    } else {
        forward(method, args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn smoke_report_contract() {
        let path = std::env::temp_dir().join(format!("p15-report-{}.ndjson", std::process::id()));
        let config = SmokeConfig {
            sink: Mutex::new(Ok(File::create(&path).unwrap())),
            ack_directory: None,
            driver: None,
            data_directory: None,
            cdp_port: None,
        };
        let record = json!({"step":"test", "text":"line\n한글"});
        assert_eq!(
            dispatch(
                None,
                "smoke_report",
                vec![record.clone()],
                |method, args| json!({"method":method,"args":args})
            ),
            json!({"method":"smoke_report","args":[record]})
        );
        assert_eq!(
            dispatch(Some(&config), "bootstrap", vec![], |method, _| json!(
                method
            )),
            json!("bootstrap")
        );
        std::thread::scope(|scope| {
            for _ in 0..16 {
                scope.spawn(|| {
                    assert_eq!(
                        config.report(std::slice::from_ref(&record)),
                        json!({"ok":true,"data":null})
                    );
                });
            }
        });
        assert_eq!(config.report(&[])["error"]["code"], "INVALID_REQUEST");
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 16);
        for line in text.lines() {
            assert_eq!(serde_json::from_str::<Value>(line).unwrap(), record);
        }
        drop(config);
        std::fs::remove_file(path).unwrap();
        let broken = SmokeConfig {
            sink: Mutex::new(Err("cannot open sink".into())),
            ack_directory: None,
            driver: None,
            data_directory: None,
            cdp_port: None,
        };
        assert_eq!(broken.report(&[record])["error"]["code"], "INTERNAL_ERROR");
    }
}
