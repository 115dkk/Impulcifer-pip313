//! Render the job journal once, with the Python console logger's prefixes.
use impulcifer_jobs::registry::JobRegistry;
use impulcifer_service::{
    ImpulciferService, NoopHost,
    brir::{Catalog, run::run_brir},
};
use impulcifer_types::{
    config::ProcessingConfig,
    job::{JobEvent, JobEventKind, JobKind, JobStatus},
};
use serde_json::json;
use std::{
    io::{self, Write},
    path::PathBuf,
    time::Duration,
};

pub(crate) fn execute(
    config: ProcessingConfig,
    catalog: Catalog,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> io::Result<i32> {
    let jobs = JobRegistry::new();
    let directory = PathBuf::from(config.dir_path.as_deref().unwrap_or(""));
    let job = match jobs.start(JobKind::Brir, true, move |ctx| {
        let result = run_brir(&config, &catalog, ctx)?;
        Ok(json!({"output_path": result.output_path}))
    }) {
        Ok(job) => job,
        Err(code) => {
            writeln!(err, "Unable to start BRIR job: {code:?}")?;
            return Ok(1);
        }
    };
    let mut after_seq = 0;
    let mut write_error = None;
    loop {
        let poll = match jobs.poll(&job.job_id, after_seq) {
            Ok(poll) => poll,
            Err(code) => {
                writeln!(err, "Unable to poll BRIR job: {code:?}")?;
                return Ok(1);
            }
        };
        for event in &poll.events {
            if write_error.is_none()
                && let Err(error) = render(event, out)
            {
                write_error = Some(error);
                // Wait for the worker's terminal status even on a broken pipe.
                let _ = jobs.cancel(&job.job_id);
            }
        }
        after_seq = poll.next_seq;
        if poll.job.status.is_terminal() {
            if let Some(error) = write_error {
                return Err(error);
            }
            if poll.job.status == JobStatus::Succeeded {
                let readme = std::fs::read_to_string(directory.join("README.md"))?;
                writeln!(out, "{}", readme.replace("\r\n", "\n"))?;
                return Ok(0);
            }
            let message = poll
                .job
                .error
                .as_ref()
                .and_then(|e| e["message"].as_str())
                .unwrap_or("BRIR processing cancelled.");
            writeln!(err, "{message}")?;
            return Ok(1);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn render(event: &JobEvent, out: &mut dyn Write) -> io::Result<()> {
    let message = event.payload["message"].as_str().unwrap_or("");
    match event.kind {
        JobEventKind::Progress => {
            // The service already truncated to integer percent before dividing
            // by 100. Round here to avoid 0.29 * 100 becoming 28.999999999999996.
            let percent =
                (event.payload["progress"].as_f64().unwrap_or(0.0) * 100.0).round() as u32;
            writeln!(out, "[{percent}%] {message}")
        }
        JobEventKind::Log => {
            let prefix = match event.payload["level"].as_str().unwrap_or("INFO") {
                "PROGRESS" => return Ok(()), // paired progress event carries the percentage
                "SUCCESS" => "\u{2713} ",
                "ERROR" => "\u{2717} ",
                "WARNING" => "\u{26a0} ",
                _ => "",
            };
            writeln!(out, "{prefix}{message}")
        }
        JobEventKind::Status => Ok(()),
    }
}

pub(crate) fn info(out: &mut dyn Write) -> io::Result<()> {
    // This read-only method never loads/persists settings or opens audio devices.
    let service = ImpulciferService::new(Box::new(NoopHost));
    let response = service.call("get_system_info", vec![]);
    let info = &response["data"];
    writeln!(out, "Impulcifer {}", env!("CARGO_PKG_VERSION"))?;
    writeln!(
        out,
        "OS: {} ({})",
        info["os"].as_str().unwrap_or(std::env::consts::OS),
        std::env::consts::ARCH
    )?;
    writeln!(out, "CPU cores: {}", info["cpu_count"])?;
    writeln!(
        out,
        "Rust toolchain: {}",
        info["python_version"].as_str().unwrap_or("unknown")
    )?;
    writeln!(
        out,
        "Audio backend: {}",
        if cfg!(windows) { "WASAPI" } else { "cpal" }
    )?;
    writeln!(
        out,
        "Data dir: {}",
        impulcifer_service::default_data_dir().display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_formats_python_levels_and_progress_once() {
        let mut out = Vec::new();
        for (kind, payload) in [
            (JobEventKind::Log, json!({"level":"INFO","message":"info"})),
            (
                JobEventKind::Log,
                json!({"level":"WARNING","message":"warning"}),
            ),
            (
                JobEventKind::Log,
                json!({"level":"ERROR","message":"error"}),
            ),
            (
                JobEventKind::Log,
                json!({"level":"SUCCESS","message":"success"}),
            ),
            (
                JobEventKind::Log,
                json!({"level":"PROGRESS","message":"step"}),
            ),
            (
                JobEventKind::Progress,
                json!({"progress":0.29,"message":"step"}),
            ),
        ] {
            render(
                &JobEvent {
                    seq: 1,
                    kind,
                    payload,
                },
                &mut out,
            )
            .unwrap();
        }
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "info\n\u{26a0} warning\n\u{2717} error\n\u{2713} success\n[29%] step\n"
        );
    }
}
