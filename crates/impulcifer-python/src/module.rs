//! Python objects never enter a service worker. Waits run detached; event
//! delivery attaches only on the calling thread and without a service lock.
use impulcifer_service::{ImpulciferService, NoopHost};
use impulcifer_types::config::FIELD_NAMES;
use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyDict};
use serde_json::{Value, json};
use std::{io::Write, time::Duration};

#[pyfunction]
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn invalid(error: impl std::fmt::Display) -> PyErr {
    PyRuntimeError::new_err(format!("INVALID_REQUEST: {error}"))
}
fn service_error(error: &Value) -> PyErr {
    PyRuntimeError::new_err(format!(
        "{}: {}",
        error["code"].as_str().unwrap_or("INTERNAL_ERROR"),
        error["message"]
            .as_str()
            .unwrap_or("Service request failed.")
    ))
}
fn data(response: Value) -> PyResult<Value> {
    if response["ok"] == true {
        Ok(response["data"].clone())
    } else {
        Err(service_error(&response["error"]))
    }
}
fn to_python(py: Python<'_>, value: &Value) -> PyResult<Py<PyAny>> {
    Ok(py
        .import("json")?
        .call_method1("loads", (value.to_string(),))?
        .unbind())
}
fn from_python(py: Python<'_>, value: &Bound<'_, PyDict>) -> PyResult<Value> {
    let options = PyDict::new(py);
    options.set_item("allow_nan", false)?;
    let text: String = py
        .import("json")?
        .call_method("dumps", (value,), Some(&options))?
        .extract()?;
    serde_json::from_str(&text).map_err(invalid)
}

#[pyfunction]
#[pyo3(signature = (config, progress=None, log=None))]
fn run(
    py: Python<'_>,
    config: &Bound<'_, PyDict>,
    progress: Option<Py<PyAny>>,
    log: Option<Py<PyAny>>,
) -> PyResult<Py<PyAny>> {
    for callback in [&progress, &log].into_iter().flatten() {
        if !callback.bind(py).is_callable() {
            return Err(invalid("progress and log must be callable or None"));
        }
    }
    // Filter before serialization: retired kwargs may contain arbitrary Python
    // objects, circular containers, or other values JSON cannot represent.
    let filtered = PyDict::new(py);
    for name in FIELD_NAMES {
        if let Some(value) = config.get_item(name)? {
            filtered.set_item(name, value)?;
        }
    }
    let kwargs = from_python(py, &filtered).map_err(invalid)?;
    let config =
        crate::run_config_dict(kwargs.as_object().ok_or_else(|| invalid("expected dict"))?)
            .map_err(invalid)?;
    let request = serde_json::to_value(config).map_err(invalid)?;
    let result = py.detach(move || run_job("start_brir", request, progress, log))?;
    to_python(py, &result)
}

fn run_job(
    method: &str,
    request: Value,
    progress: Option<Py<PyAny>>,
    log: Option<Py<PyAny>>,
) -> PyResult<Value> {
    // Service owns the JobRegistry and starts the same BRIR/recovery worker
    // used by IPC. Each invocation has an independent registry.
    let service = ImpulciferService::new(Box::new(NoopHost));
    let started = data(service.call(method, vec![request]))?;
    let id = started["job"]["job_id"].clone();
    let mut after_seq = json!(0);
    let mut callback_error = None;
    loop {
        let poll = data(service.call("poll_job", vec![id.clone(), after_seq]))?;
        if callback_error.is_none() {
            let delivered = (|| -> PyResult<()> {
                Python::attach(|py| py.check_signals())?;
                if let Some(events) = poll["events"].as_array() {
                    for event in events {
                        let callback = match event["type"].as_str() {
                            Some("progress") => &progress,
                            Some("log") => &log,
                            _ => continue,
                        };
                        if let Some(callback) = callback {
                            Python::attach(|py| {
                                callback.call1(py, (to_python(py, &event["payload"])?,))
                            })?;
                        }
                    }
                }
                Ok(())
            })();
            if let Err(error) = delivered {
                callback_error = Some(error);
                // Never leave a running job behind after callback/signal errors.
                // Recovery is not cancellable; it is still drained to completion.
                service.call("cancel_job", vec![id.clone()]);
            }
        }
        after_seq = poll["next_seq"].clone();
        match poll["job"]["status"].as_str() {
            Some("succeeded" | "failed" | "cancelled") => {
                if let Some(error) = callback_error {
                    return Err(error);
                }
                return match poll["job"]["status"].as_str() {
                    Some("succeeded") => Ok(poll["job"]["result"].clone()),
                    Some("cancelled") => Err(PyRuntimeError::new_err("CANCELLED: Job cancelled.")),
                    _ => Err(service_error(&poll["job"]["error"])),
                };
            }
            _ => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

fn directory_call(py: Python<'_>, method: &str, dir: String) -> PyResult<Py<PyAny>> {
    let result = py.detach(move || {
        data(ImpulciferService::new(Box::new(NoopHost)).call(method, vec![json!(dir)]))
    })?;
    to_python(py, &result)
}
#[pyfunction]
fn detect_sweep(py: Python<'_>, dir: String) -> PyResult<Py<PyAny>> {
    directory_call(py, "detect_sweep", dir)
}
#[pyfunction]
fn generate_sweep_set(py: Python<'_>, dir: String) -> PyResult<Py<PyAny>> {
    directory_call(py, "generate_sweep_set", dir)
}
#[pyfunction]
#[pyo3(signature = (dir, **options))]
fn recover_brir_outputs(
    py: Python<'_>,
    dir: String,
    options: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let request = match options {
        Some(options) => options.copy()?,
        None => PyDict::new(py),
    };
    request.set_item("dir_path", dir)?;
    let request = from_python(py, &request).map_err(invalid)?;
    let result = py.detach(move || run_job("start_output_recovery", request, None, None))?;
    to_python(py, &result)
}

struct PythonWriter {
    stream: Py<PyAny>,
    pending: Vec<u8>,
    error: Option<PyErr>,
}
impl PythonWriter {
    fn new(stream: Py<PyAny>) -> Self {
        Self {
            stream,
            pending: Vec::new(),
            error: None,
        }
    }
    fn send(&mut self) -> std::io::Result<()> {
        if self.error.is_some() {
            return Err(std::io::Error::other("Python stream write failed"));
        }
        if self.pending.is_empty() {
            return Ok(());
        }
        let text = std::str::from_utf8(&self.pending).map_err(std::io::Error::other)?;
        let result = Python::attach(|py| self.stream.call_method1(py, "write", (text,)));
        if let Err(error) = result {
            self.error = Some(error);
            return Err(std::io::Error::other("Python stream write failed"));
        }
        self.pending.clear();
        Ok(())
    }
}
impl Write for PythonWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.pending.extend_from_slice(bytes);
        // Rust formatting may split a line across writes. Deliver complete
        // UTF-8 lines, including Unicode catalogue messages, to Python capture.
        if bytes.contains(&b'\n') {
            self.send()?;
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.send()
    }
}
#[pyfunction]
fn cli_main(py: Python<'_>, argv: Vec<String>) -> PyResult<i32> {
    let sys = py.import("sys")?;
    let mut out = PythonWriter::new(sys.getattr("stdout")?.unbind());
    let mut err = PythonWriter::new(sys.getattr("stderr")?.unbind());
    let mut args = vec!["impulcifer".into()];
    args.extend(argv);
    let code = py.detach(|| {
        let code = impulcifer_cli::run(&args, &mut out, &mut err);
        let _ = out.flush();
        let _ = err.flush();
        code
    });
    if let Some(error) = out.error.or(err.error) {
        Err(error)
    } else {
        Ok(code)
    }
}

#[pymodule(gil_used = false)]
fn impulcifer_native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(version, m)?)?;
    m.add_function(wrap_pyfunction!(run, m)?)?;
    m.add_function(wrap_pyfunction!(detect_sweep, m)?)?;
    m.add_function(wrap_pyfunction!(generate_sweep_set, m)?)?;
    m.add_function(wrap_pyfunction!(recover_brir_outputs, m)?)?;
    m.add_function(wrap_pyfunction!(cli_main, m)?)?;
    Ok(())
}
