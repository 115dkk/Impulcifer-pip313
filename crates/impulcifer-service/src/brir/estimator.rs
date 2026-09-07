use super::{
    BrirError, BrirEvents,
    discovery::MeasurementDir,
    sweep_grid::{detect_sweep_parameters, snap_sweep_samples},
};
use impulcifer_dsp::estimator::SweepEstimator;
use impulcifer_io::{ffmpeg, read_wav, sweep_files::sweep_file_name, write_wav};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const DEFAULT_SIGNAL: &str = "sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav";
fn alias(name: &str) -> Option<String> {
    let prefix = match name {
        "default" | "sweep" | "1" | "2" => return Some(DEFAULT_SIGNAL.into()),
        "stereo" | "3" => "sweep-seg-FL,FR-stereo",
        "mono-left" | "4" => "sweep-seg-FL-mono",
        "left" | "5" => "sweep-seg-FL-stereo",
        "right" | "6" => "sweep-seg-FR-stereo",
        _ => return None,
    };
    Some(format!("{prefix}-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"))
}
pub fn open_estimator(
    dir: &MeasurementDir,
    test_signal: Option<&str>,
) -> Result<SweepEstimator, BrirError> {
    open_with_events(dir, test_signal, &crate::default_data_dir(), None)
}
pub(crate) fn open_with_events(
    dir: &MeasurementDir,
    test_signal: Option<&str>,
    data: &Path,
    mut events: Option<&mut dyn BrirEvents>,
) -> Result<SweepEstimator, BrirError> {
    let mut log = |level, key, args| {
        if let Some(e) = events.as_deref_mut() {
            e.log(level, key, args);
        }
    };
    let mut path = test_signal.map(PathBuf::from);
    if let Some(name) = test_signal.and_then(alias) {
        let candidate = data.join(&name);
        if candidate.is_file() {
            path = Some(candidate);
        } else {
            log(
                "warning",
                "cli_warning_test_signal_not_found",
                json!({"signal":test_signal,"name":name}),
            );
        }
    }
    if let Some(spec) = test_signal.and_then(|s| s.strip_prefix("generate:")) {
        let (duration, fs) = spec
            .split_once("s@")
            .ok_or_else(|| BrirError::Unsupported(spec.into()))?;
        let duration: f64 = duration
            .parse()
            .map_err(|_| BrirError::Unsupported(spec.into()))?;
        let fs: u32 = fs
            .parse()
            .map_err(|_| BrirError::Unsupported(spec.into()))?;
        if !duration.is_finite() || duration <= 0.0 || fs <= 10 {
            return Err(BrirError::Unsupported(spec.into()));
        }
        let (_, n, _) = snap_sweep_samples(duration * fs as f64, fs);
        return Ok(SweepEstimator::new((n - 1) as f64 / fs as f64, fs)?);
    }
    if test_signal.is_none() || test_signal == Some("auto") {
        if let Some(sidecar) = &dir.test_signal {
            path = Some(sidecar.clone());
        } else {
            let detection = detect_sweep_parameters(&dir.dir)?;
            if let Some(d) = &detection
                && d.confidence == "high"
                && !d.is_default()
            {
                log(
                    "info",
                    "cli_info_sweep_detected",
                    json!({"fs":d.fs,"duration":format!("{:.2}",d.duration())}),
                );
                return Ok(SweepEstimator::new(
                    (d.sweep_samples - 1) as f64 / d.fs as f64,
                    d.fs,
                )?);
            }
            if detection.is_none_or(|d| d.confidence != "high") {
                log("warning", "cli_warning_sweep_detect_fallback", json!({}));
            }
            path = Some(data.join(DEFAULT_SIGNAL));
        }
    }
    let path = path.ok_or_else(|| BrirError::Missing(DEFAULT_SIGNAL.into()))?;
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let load = |path: &Path| -> Result<SweepEstimator, BrirError> {
        let wav = read_wav(path)?;
        Ok(SweepEstimator::from_samples(
            wav.sample_rate,
            &wav.tracks[0],
        )?)
    };
    match ext.as_str() {
        "wav" => load(&path),
        "mlp" | "thd" | "truehd" => {
            let tools = ffmpeg::discover(false)?.ok_or(BrirError::FfmpegMissing)?;
            if !ffmpeg::is_truehd(&tools, &path)? {
                return Err(BrirError::Unsupported(path.display().to_string()));
            }
            log("info", "cli_info_converting_truehd", json!({"file":path}));
            let temp = DecodeTemp::new()?;
            let output = temp.0.join("decoded.wav");
            ffmpeg::decode_to_wav(&tools, &path, &output)?;
            load(&output)
        }
        _ => Err(BrirError::Unsupported(path.display().to_string())),
    }
}
struct DecodeTemp(PathBuf);
impl DecodeTemp {
    fn new() -> Result<Self, std::io::Error> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        loop {
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("impulcifer-decode-{}-{n}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
    }
}
impl Drop for DecodeTemp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn generate_sweep_set(dir: &Path) -> Result<Value, BrirError> {
    if !dir.is_dir() {
        return Err(BrirError::Missing(dir.display().to_string()));
    }
    let estimator = SweepEstimator::new(5.0, 48000)?;
    let mut files = Vec::new();
    for (names, layout) in [
        (&["FL", "FR"][..], "stereo"),
        (&["FC"][..], "stereo"),
        (&["SL", "SR"][..], "stereo"),
        (&["BL", "BR"][..], "stereo"),
        (&["FL", "FR", "FC", "SL", "SR", "BL", "BR"][..], "7.1"),
    ] {
        let path = dir.join(sweep_file_name(
            names,
            layout,
            estimator.duration,
            estimator.fs,
            32,
            estimator.low,
            estimator.high,
        ));
        write_wav(
            &path,
            estimator.fs,
            &estimator.sweep_sequence(names, layout)?,
            32,
        )?;
        files.push(path);
    }
    // Explicit P11 addition: 2.x sweep_set_generator itself does not write a sidecar.
    write_wav(
        &dir.join("test.wav"),
        estimator.fs,
        std::slice::from_ref(&estimator.test_signal),
        32,
    )?;
    Ok(json!({"files":files,"play_path":files.first()}))
}
