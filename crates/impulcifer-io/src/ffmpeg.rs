//! FFmpeg discovery and TrueHD/MLP probe/decode. No shells or installers.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::IoError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfmpegPaths {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

fn command(executable: &Path) -> Result<Command, IoError> {
    if !executable.is_absolute() {
        return Err(IoError::InvalidArgument(
            "executable path must be absolute".into(),
        ));
    }
    // Batch files require a shell. Discovery intentionally only accepts native binaries.
    if executable
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("cmd") || s.eq_ignore_ascii_case("bat"))
    {
        return Err(IoError::InvalidArgument(
            "shell scripts requiring cmd.exe are not supported".into(),
        ));
    }
    let mut command = Command::new(executable);
    command.stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    Ok(command)
}

fn run(mut cmd: Command) -> Result<Output, IoError> {
    let executable = PathBuf::from(cmd.get_program());
    // output() drains both pipes concurrently, including stderr on failure.
    let output = cmd.output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let lines: Vec<_> = stderr.lines().collect();
        return Err(IoError::Process {
            executable,
            status: output.status,
            stderr: lines[lines.len().saturating_sub(20)..].join("\n"),
        });
    }
    Ok(output)
}

fn binary_name(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}
fn pair(directory: &Path) -> Option<FfmpegPaths> {
    let ffmpeg = directory.join(binary_name("ffmpeg"));
    let ffprobe = directory.join(binary_name("ffprobe"));
    if !ffmpeg.is_file() || !ffprobe.is_file() {
        return None;
    }
    Some(FfmpegPaths {
        ffmpeg: ffmpeg.canonicalize().ok()?,
        ffprobe: ffprobe.canonicalize().ok()?,
    })
}
fn version_supported(output: &[u8]) -> bool {
    let text = String::from_utf8_lossy(output);
    let Some(version) = text
        .lines()
        .next()
        .and_then(|line| line.split_once("version"))
        .and_then(|(_, rest)| rest.split_whitespace().next())
    else {
        return false;
    };
    if let Some(git) = version.strip_prefix("N-") {
        return git
            .split('-')
            .next()
            .and_then(|n| n.parse::<u32>().ok())
            .is_some_and(|n| n >= 80000);
    }
    let mut parts = version.split('.');
    let number = |s: &str| {
        s.chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse::<u32>()
            .ok()
    };
    let major = parts.next().and_then(number);
    let minor = parts.next().and_then(number);
    matches!((major, minor), (Some(4..), Some(_)))
}
fn usable(paths: &FfmpegPaths) -> bool {
    let Ok(mut cmd) = command(&paths.ffmpeg) else {
        return false;
    };
    cmd.arg("-version");
    run(cmd).is_ok_and(|output| version_supported(&output.stdout))
}
fn discover_in(
    bundled: &Path,
    path_dirs: &[PathBuf],
    common: &[PathBuf],
    validate: impl Fn(&FfmpegPaths) -> bool,
) -> Option<FfmpegPaths> {
    // App-local binaries first; then PATH (the two programs may live in different dirs).
    for dir in [
        bundled.to_path_buf(),
        bundled.join("ffmpeg/bin"),
        bundled.join("bin"),
    ] {
        if let Some(paths) = pair(&dir).filter(&validate) {
            return Some(paths);
        }
    }
    let on_path = |name: &str| {
        path_dirs
            .iter()
            .map(|dir| dir.join(binary_name(name)))
            .find(|file| file.is_file())
            .and_then(|file| file.canonicalize().ok())
    };
    if let (Some(ffmpeg), Some(ffprobe)) = (on_path("ffmpeg"), on_path("ffprobe")) {
        let paths = FfmpegPaths { ffmpeg, ffprobe };
        if validate(&paths) {
            return Some(paths);
        }
    }
    common.iter().filter_map(|dir| pair(dir)).find(validate)
}

fn common_dirs() -> Vec<PathBuf> {
    #[cfg(windows)]
    let mut dirs: Vec<PathBuf> = [
        "C:/ProgramData/chocolatey/bin",
        "C:/Program Files/ffmpeg/bin",
        "C:/Program Files (x86)/ffmpeg/bin",
        "C:/ffmpeg/bin",
        "C:/tools/ffmpeg/bin",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();
    #[cfg(target_os = "macos")]
    let mut dirs: Vec<PathBuf> = ["/usr/local/bin", "/opt/homebrew/bin", "/usr/bin"]
        .into_iter()
        .map(PathBuf::from)
        .collect();
    #[cfg(not(any(windows, target_os = "macos")))]
    let mut dirs: Vec<PathBuf> = ["/usr/bin", "/usr/local/bin", "/snap/bin", "/opt/ffmpeg/bin"]
        .into_iter()
        .map(PathBuf::from)
        .collect();
    #[cfg(windows)]
    if let Some(home) = std::env::var_os("USERPROFILE") {
        let packages = PathBuf::from(home).join("AppData/Local/Microsoft/WinGet/Packages");
        if let Ok(entries) = packages.read_dir() {
            let mut packages: Vec<_> = entries
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().starts_with("Gyan.FFmpeg_"))
                .collect();
            packages.sort_by_key(|e| e.file_name());
            for package in packages {
                if let Ok(entries) = package.path().read_dir() {
                    let mut versions: Vec<_> = entries
                        .flatten()
                        .filter(|e| e.file_name().to_string_lossy().starts_with("ffmpeg-"))
                        .collect();
                    versions.sort_by_key(|e| e.file_name());
                    dirs.extend(versions.into_iter().map(|e| e.path().join("bin")));
                }
            }
        }
    }
    #[cfg(not(windows))]
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/bin"));
    }
    dirs
}

/// App-local/bundled, PATH, then platform common locations; requires FFmpeg
/// version 4.0 or later as in the oracle. `auto_install` prints a hint only, never installs.
/// Unlike the current Python discovery, P07 explicitly adds app-local priority.
pub fn discover(auto_install: bool) -> Result<Option<FfmpegPaths>, IoError> {
    let exe = std::env::current_exe()?;
    let bundled = exe
        .parent()
        .ok_or_else(|| IoError::InvalidArgument("executable has no parent".into()))?;
    let path_dirs: Vec<_> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    let paths = discover_in(bundled, &path_dirs, &common_dirs(), usable);
    if paths.is_none() && auto_install {
        let hint = if cfg!(windows) {
            "winget install Gyan.FFmpeg"
        } else if cfg!(target_os = "macos") {
            "brew install ffmpeg"
        } else {
            "sudo apt install ffmpeg or sudo dnf install ffmpeg"
        };
        eprintln!("FFmpeg >=4.0 was not found. Install manually: {hint}");
    }
    Ok(paths)
}

fn probe_field(paths: &FfmpegPaths, file: &Path, field: &str) -> Result<String, IoError> {
    let mut cmd = command(&paths.ffprobe)?;
    cmd.args(["-v", "error", "-select_streams", "a:0", "-show_entries"])
        .arg(format!("stream={field}"))
        .args(["-of", "json"])
        .arg(std::path::absolute(file)?);
    let output = run(cmd)?;
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|err| IoError::Format(format!("ffprobe returned invalid JSON: {err}")))?;
    let stream = document
        .get("streams")
        .and_then(serde_json::Value::as_array)
        .and_then(|s| s.first())
        .ok_or_else(|| IoError::Format("ffprobe returned no audio stream".into()))?;
    match stream.get(field) {
        Some(serde_json::Value::String(s)) => Ok(s.clone()),
        None | Some(serde_json::Value::Null) if field == "profile" => Ok(String::new()),
        _ => Err(IoError::Format(format!(
            "ffprobe returned no string {field}"
        ))),
    }
}

pub fn probe_codec(paths: &FfmpegPaths, file: &Path) -> Result<String, IoError> {
    Ok(probe_field(paths, file, "codec_name")?
        .trim()
        .to_lowercase())
}
pub fn is_truehd(paths: &FfmpegPaths, file: &Path) -> Result<bool, IoError> {
    Ok(matches!(
        probe_codec(paths, file)?.as_str(),
        "truehd" | "mlp"
    ))
}
pub fn is_truehd_atmos_object_master(paths: &FfmpegPaths, file: &Path) -> Result<bool, IoError> {
    Ok(probe_field(paths, file, "profile")?
        .to_lowercase()
        .contains("atmos"))
}
/// Decodes every input channel, without `-ac` downmixing; matches the oracle's
/// `-acodec pcm_f32le -ar 48000 output -y`. Float PCM here is a decode intermediate.
pub fn decode_to_wav(paths: &FfmpegPaths, file: &Path, out: &Path) -> Result<(), IoError> {
    let mut cmd = command(&paths.ffmpeg)?;
    cmd.arg("-i")
        .arg(std::path::absolute(file)?)
        .args(["-acodec", "pcm_f32le", "-ar", "48000"])
        .arg(std::path::absolute(out)?)
        .arg("-y");
    run(cmd)?;
    Ok(())
}
pub fn truehd_channel_names(channels: usize) -> Option<Vec<&'static str>> {
    use impulcifer_types::constants::{TRUEHD_11CH_ORDER, TRUEHD_13CH_ORDER};
    match channels {
        11 => Some(TRUEHD_11CH_ORDER.to_vec()),
        13 => Some(TRUEHD_13CH_ORDER.to_vec()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    #[test]
    fn ffmpeg_discovery_prefers_bundled_dir() {
        let tmp = TempDir::new();
        let bundled = tmp.0.join("app");
        let path = tmp.0.join("path");
        let common = tmp.0.join("common");
        for dir in [&bundled, &path, &common] {
            std::fs::create_dir_all(dir).unwrap();
            for name in ["ffmpeg", "ffprobe"] {
                std::fs::write(dir.join(binary_name(name)), []).unwrap();
            }
        }
        let found = discover_in(
            &bundled,
            std::slice::from_ref(&path),
            std::slice::from_ref(&common),
            |_| true,
        )
        .unwrap();
        assert_eq!(found, pair(&bundled).unwrap());
        let found = discover_in(
            &bundled,
            std::slice::from_ref(&path),
            std::slice::from_ref(&common),
            |p| !p.ffmpeg.starts_with(bundled.canonicalize().unwrap()),
        )
        .unwrap();
        assert_eq!(found, pair(&path).unwrap());
        assert_eq!(
            discover_in(
                &tmp.0.join("absent"),
                &[],
                std::slice::from_ref(&common),
                |_| true
            ),
            pair(&common)
        );
    }
    #[test]
    fn ffmpeg_version_and_absolute_executable_policy() {
        for version in ["4.0", "6.1.2-static", "N-80000-g123", "N-110000-g123"] {
            assert!(version_supported(
                format!("ffmpeg version {version}\n").as_bytes()
            ));
        }
        for version in ["3.9", "N-79999-g123", "unknown", "6", ""] {
            assert!(!version_supported(
                format!("ffmpeg version {version}\n").as_bytes()
            ));
        }
        assert!(command(Path::new("ffmpeg")).is_err());
        assert!(command(&std::path::absolute("fake.cmd").unwrap()).is_err());
        assert!(truehd_channel_names(8).is_none());
        assert_eq!(truehd_channel_names(13).unwrap()[9], "TSL");
    }
    // A .cmd fixture would invoke cmd.exe, contradicting the no-shell rule.
    // Use the native Rust test executable as a deliberately failing decoder:
    // libtest rejects FFmpeg's -i option and writes its diagnostic to stderr.
    /// Serialises the tests that spawn processes. On Linux a fork on another
    /// thread while a fixture script is still open for writing makes the
    /// script's own exec fail with ETXTBSY ("Text file busy"), which CI hit.
    static PROCESS_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());
    fn process_guard() -> std::sync::MutexGuard<'static, ()> {
        PROCESS_TESTS.lock().unwrap_or_else(|e| e.into_inner())
    }
    #[cfg(windows)]
    #[test]
    fn ffmpeg_decode_error_carries_stderr() {
        let _guard = process_guard();
        let tmp = TempDir::new();
        let executable = std::env::current_exe().unwrap();
        let paths = FfmpegPaths {
            ffmpeg: executable.clone(),
            ffprobe: executable,
        };
        match decode_to_wav(&paths, &tmp.0.join("input ; file"), &tmp.0.join("out.wav"))
            .unwrap_err()
        {
            IoError::Process { status, stderr, .. } => {
                assert!(!status.success());
                assert!(stderr.contains("Unrecognized option"), "{stderr}");
            }
            error => panic!("{error}"),
        }
    }
    #[cfg(unix)]
    fn script(tmp: &TempDir, body: &str) -> FfmpegPaths {
        use std::os::unix::fs::PermissionsExt;
        let file = tmp.0.join("fake.sh");
        std::fs::write(&file, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700)).unwrap();
        FfmpegPaths {
            ffmpeg: file.clone(),
            ffprobe: file,
        }
    }
    #[cfg(unix)]
    #[test]
    fn ffmpeg_decode_error_carries_stderr() {
        let _guard = process_guard();
        let tmp = TempDir::new();
        let paths = script(&tmp, "printf 'decode failed: fixture\\n' >&2\nexit 7");
        let error =
            decode_to_wav(&paths, &tmp.0.join("input ; file"), &tmp.0.join("out.wav")).unwrap_err();
        match error {
            IoError::Process { status, stderr, .. } => {
                assert_eq!(status.code(), Some(7));
                assert_eq!(stderr, "decode failed: fixture");
            }
            _ => panic!("{error}"),
        }
    }
    #[cfg(unix)]
    #[test]
    fn ffmpeg_probe_json_and_decode_arguments() {
        let _guard = process_guard();
        let tmp = TempDir::new();
        let paths = script(
            &tmp,
            "printf '%s' '{\"streams\":[{\"codec_name\":\"truehd\",\"profile\":\"Dolby TrueHD + Dolby Atmos\"}]}'",
        );
        assert!(is_truehd(&paths, Path::new("input")).unwrap());
        assert!(is_truehd_atmos_object_master(&paths, Path::new("input")).unwrap());
        let paths = script(
            &tmp,
            "[ \"$1\" = '-i' ] && [ \"$3\" = '-acodec' ] && [ \"$4\" = 'pcm_f32le' ] && [ \"$5\" = '-ar' ] && [ \"$6\" = '48000' ] && [ \"$8\" = '-y' ] && [ \"$#\" = '8' ]",
        );
        decode_to_wav(
            &paths,
            &tmp.0.join("input ; file"),
            &tmp.0.join("out file.wav"),
        )
        .unwrap();
    }
}
