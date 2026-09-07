#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

//! Workspace gates. They read the repository tree directly so that no build
//! flag, cfg trick or missing `[lints] workspace = true` can hide a violation.
//!
//! 1. every crate root under crates/ and apps/ carries `#![forbid(unsafe_code)]`
//! 2. no `unsafe` token appears in first-party Rust outside the budget in unsafe-budget.toml
//! 3. features.toml registers every canonical IPC method, config field and pipeline stage
//! 4. every feature marked `implemented` names at least one test that really exists
//! 5. only the exact FFmpeg module may construct first-party subprocess commands

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn first_party_crate_dirs() -> Vec<PathBuf> {
    let root = repo_root();
    let mut dirs = Vec::new();
    for parent in ["crates", "apps"] {
        let Ok(entries) = fs::read_dir(root.join(parent)) else {
            continue;
        };
        for entry in entries.flatten() {
            if entry.path().join("Cargo.toml").is_file() {
                dirs.push(entry.path());
            }
        }
    }
    dirs.sort();
    assert!(!dirs.is_empty(), "no crates found under crates/ or apps/");
    dirs
}

fn crate_name(dir: &Path) -> String {
    dir.file_name().unwrap().to_string_lossy().to_string()
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if p.file_name().map(|n| n == "target").unwrap_or(false) {
                    continue;
                }
                stack.push(p);
            } else if p.extension().map(|e| e == "rs").unwrap_or(false) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

fn crate_roots(dir: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for candidate in ["src/lib.rs", "src/main.rs", "build.rs"] {
        let p = dir.join(candidate);
        if p.is_file() {
            roots.push(p);
        }
    }
    if let Ok(entries) = fs::read_dir(dir.join("tests")) {
        for entry in entries.flatten() {
            if entry.path().extension().map(|e| e == "rs").unwrap_or(false) {
                roots.push(entry.path());
            }
        }
    }
    if let Ok(entries) = fs::read_dir(dir.join("examples")) {
        for entry in entries.flatten() {
            if entry.path().extension().map(|e| e == "rs").unwrap_or(false) {
                roots.push(entry.path());
            }
        }
    }
    roots
}

/// Strip `//` line comments, `/* */` block comments and string literals so
/// that the word `unsafe` in prose does not count.
fn strip_comments_and_strings(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let bytes: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        let next = bytes.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < bytes.len() && bytes[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == '*' && bytes[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else if c == '"' {
            i += 1;
            while i < bytes.len() && bytes[i] != '"' {
                if bytes[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            out.push(' ');
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

fn count_unsafe_tokens(src: &str) -> usize {
    let code = strip_comments_and_strings(src);
    let mut count = 0;
    let chars: Vec<char> = code.chars().collect();
    let needle: Vec<char> = "unsafe".chars().collect();
    let mut i = 0;
    while i + needle.len() <= chars.len() {
        if chars[i..i + needle.len()] == needle[..] {
            let before_ok = i == 0 || !(chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');
            let after = chars.get(i + needle.len()).copied();
            let after_ok = after
                .map(|c| !(c.is_alphanumeric() || c == '_'))
                .unwrap_or(true);
            if before_ok && after_ok {
                count += 1;
            }
        }
        i += 1;
    }
    count
}

#[derive(Deserialize)]
struct Budget {
    budget: BTreeMap<String, usize>,
}

#[derive(Deserialize)]
struct Feature {
    id: String,
    kind: String,
    status: String,
    tests: Vec<String>,
}

#[derive(Deserialize)]
struct Registry {
    feature: Vec<Feature>,
}

fn load_registry() -> Registry {
    let text = fs::read_to_string(repo_root().join("features.toml")).expect("features.toml");
    toml::from_str(&text).expect("features.toml parses")
}

#[test]
fn every_crate_root_forbids_unsafe() {
    let mut missing = Vec::new();
    for dir in first_party_crate_dirs() {
        for root in crate_roots(&dir) {
            let src = fs::read_to_string(&root).unwrap();
            if !src.contains("#![forbid(unsafe_code)]") {
                missing.push(root.display().to_string());
            }
        }
    }
    assert!(
        missing.is_empty(),
        "crate roots without #![forbid(unsafe_code)]:\n{}",
        missing.join("\n")
    );
}

#[test]
fn no_unsafe_outside_budget() {
    let budget: Budget =
        toml::from_str(&fs::read_to_string(repo_root().join("unsafe-budget.toml")).unwrap())
            .unwrap();
    let mut violations = Vec::new();
    for dir in first_party_crate_dirs() {
        let name = crate_name(&dir);
        let allowed = budget.budget.get(&name).copied().unwrap_or(0);
        let mut total = 0;
        let mut local = Vec::new();
        for file in rust_files(&dir) {
            let n = count_unsafe_tokens(&fs::read_to_string(&file).unwrap());
            if n > 0 {
                total += n;
                local.push(format!("{}: {} unsafe token(s)", file.display(), n));
            }
        }
        if total > allowed {
            violations.extend(local);
            violations.push(format!(
                "crate {name}: {total} unsafe token(s) exceeds budget {allowed}"
            ));
        }
    }
    assert!(
        violations.is_empty(),
        "unsafe outside the approved budget:\n{}",
        violations.join("\n")
    );
}

fn contains_command_constructor(src: &str) -> bool {
    src.match_indices("Command").any(|(index, _)| {
        let boundary = src[..index]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        let rest = src[index + "Command".len()..].trim_start();
        boundary
            && rest.strip_prefix("::").is_some_and(|rest| {
                rest.trim_start()
                    .strip_prefix("new")
                    .is_some_and(|rest| rest.trim_start().starts_with('('))
            })
    })
}

/// `Command::new(` counts only when the file reaches `std::process` (clap's
/// `Command::new("impulcifer")` in the CLI is a parser, not a subprocess).
fn uses_process_command(src: &str) -> bool {
    let code = strip_comments_and_strings(src);
    code.contains("std::process") || code.contains("process::Command")
}

fn forbidden_subprocess(relative: &Path, src: &str) -> bool {
    relative != Path::new("crates/impulcifer-io/src/ffmpeg.rs")
        && uses_process_command(src)
        && (contains_command_constructor(&strip_comments_and_strings(src))
            || contains_command_constructor(src))
}

#[test]
fn no_shell_subprocesses() {
    let root = repo_root();
    let mut violations = Vec::new();
    for dir in first_party_crate_dirs() {
        for file in rust_files(&dir.join("src")) {
            let relative = file.strip_prefix(&root).expect("first-party source");
            let src = fs::read_to_string(&file).unwrap();
            if forbidden_subprocess(relative, &src) {
                violations.push(relative.display().to_string());
            }
        }
    }
    assert!(
        violations.is_empty(),
        "subprocess constructors outside crates/impulcifer-io/src/ffmpeg.rs:\n{}",
        violations.join("\n")
    );
}

#[test]
fn subprocess_scanner_checks_code_raw_text_and_exact_allowlist() {
    let service = Path::new("crates/impulcifer-service/src/lib.rs");
    for src in [
        "std::process::Command::new(\"cmd.exe\")",
        "use std::process::Command;\nCommand \n :: new \t (\"date\")",
        "use std::process;\nprocess::Command/* split */::new(\"date\")",
        "use std::process::Command;\n// Command::new(\"cmd.exe\")",
        "use std::process::Command;\nlet example = r#\"Command::new(\"cmd.exe\")\"#;",
    ] {
        assert!(forbidden_subprocess(service, src), "{src}");
        assert!(!forbidden_subprocess(
            Path::new("crates/impulcifer-io/src/ffmpeg.rs"),
            src
        ));
        for impostor in [
            "crates/impulcifer-service/src/ffmpeg.rs",
            "apps/impulcifer-io/src/ffmpeg.rs",
            "crates/impulcifer-io/src/nested/ffmpeg.rs",
        ] {
            assert!(forbidden_subprocess(Path::new(impostor), src));
        }
    }
    for src in [
        "use std::process::Command;\nCommand::newer()",
        "use std::process::Command;\nOtherCommand::new()",
        "use std::process::Command;\ncommand()",
        // clap's parser builder, as in crates/impulcifer-cli/src/options.rs
        "use clap::{Arg, Command};\nlet c = Command::new(\"impulcifer\");",
    ] {
        assert!(!forbidden_subprocess(service, src), "{src}");
    }
}

const CANONICAL_IPC: [&str; 23] = [
    "bootstrap",
    "list_audio_devices",
    "start_recording",
    "start_brir",
    "start_output_recovery",
    "poll_job",
    "cancel_job",
    "get_ui_settings",
    "set_language",
    "set_theme",
    "set_skin",
    "set_frontend",
    "get_system_info",
    "resolve_recording_paths",
    "detect_sweep",
    "generate_sweep_set",
    "open_path",
    "check_for_updates",
    "start_update",
    "apply_pending_update",
    "select_file",
    "select_directory",
    "open_url",
];
const CANONICAL_CONFIG: [&str; 33] = [
    "dir_path",
    "test_signal",
    "room_target",
    "room_mic_calibration",
    "headphone_compensation_file",
    "fs",
    "plot",
    "interactive_plots",
    "channel_balance",
    "decay",
    "target_level",
    "fr_combination_method",
    "specific_limit",
    "generic_limit",
    "bass_boost_gain",
    "bass_boost_fc",
    "bass_boost_q",
    "tilt",
    "do_room_correction",
    "do_headphone_compensation",
    "do_equalization",
    "remove_silent_channels",
    "head_ms",
    "jamesdsp",
    "hangloose",
    "microphone_deviation_correction",
    "mic_deviation_strength",
    "mic_deviation_debug_plots",
    "output_truehd_layouts",
    "vbass",
    "vbass_freq",
    "vbass_hp",
    "vbass_polarity",
];
const CANONICAL_STAGES: [&str; 26] = [
    "estimator",
    "room_correction",
    "headphone_compensation",
    "equalization_files",
    "target",
    "open_measurements",
    "plot_pre",
    "crop_and_align",
    "virtual_bass",
    "mic_deviation_skipped",
    "mic_deviation",
    "write_responses",
    "equalize",
    "decay",
    "channel_balance",
    "normalize",
    "write_readme",
    "plot_post",
    "plot_results",
    "plot_additional",
    "interactive_plots",
    "resample",
    "write_brirs",
    "truehd_layouts",
    "jamesdsp",
    "hangloose",
];

#[test]
fn canonical_features_registered() {
    let registry = load_registry();
    let ids: BTreeSet<&str> = registry.feature.iter().map(|f| f.id.as_str()).collect();
    let mut missing = Vec::new();
    for m in CANONICAL_IPC {
        if !ids.contains(format!("ipc.{m}").as_str()) {
            missing.push(format!("ipc.{m}"));
        }
    }
    for c in CANONICAL_CONFIG {
        if !ids.contains(format!("config.{c}").as_str()) {
            missing.push(format!("config.{c}"));
        }
    }
    for s in CANONICAL_STAGES {
        if !ids.contains(format!("stage.{s}").as_str()) {
            missing.push(format!("stage.{s}"));
        }
    }
    let mut seen = BTreeSet::new();
    let dupes: Vec<&str> = registry
        .feature
        .iter()
        .map(|f| f.id.as_str())
        .filter(|id| !seen.insert(*id))
        .collect();
    assert!(dupes.is_empty(), "duplicate feature ids: {dupes:?}");
    for f in &registry.feature {
        assert!(
            matches!(f.status.as_str(), "planned" | "implemented"),
            "{}: bad status {}",
            f.id,
            f.status
        );
        assert!(!f.kind.is_empty(), "{}: empty kind", f.id);
    }
    assert!(
        missing.is_empty(),
        "canonical features missing from features.toml:\n{}",
        missing.join("\n")
    );
}

fn test_fn_exists(crate_dir: &Path, test_name: &str) -> bool {
    let needle = format!("fn {test_name}(");
    for file in rust_files(crate_dir) {
        let src = fs::read_to_string(&file).unwrap();
        if src.contains(&needle) && src.contains("#[test]") {
            return true;
        }
    }
    false
}

#[test]
fn implemented_features_have_existing_tests() {
    let registry = load_registry();
    let crates: BTreeMap<String, PathBuf> = first_party_crate_dirs()
        .into_iter()
        .map(|d| (crate_name(&d), d))
        .collect();
    let mut failures = Vec::new();
    for f in &registry.feature {
        if f.status != "implemented" {
            continue;
        }
        if f.tests.is_empty() {
            failures.push(format!("{}: implemented but has no tests", f.id));
            continue;
        }
        for t in &f.tests {
            if let Some(reference) = t.strip_prefix("pytest:") {
                // `pytest:<repo-relative file>::<test_fn>` for checks that need
                // an interpreter (the installed-wheel tests of
                // crates/impulcifer-python, run by the `wheel` job of rust.yml).
                let Some((file, fn_name)) = reference.split_once("::") else {
                    failures.push(format!(
                        "{}: test reference '{}' must be pytest:path::test_fn",
                        f.id, t
                    ));
                    continue;
                };
                let found = fs::read_to_string(repo_root().join(file))
                    .map(|src| src.contains(&format!("def {fn_name}(")))
                    .unwrap_or(false);
                if !found {
                    failures.push(format!(
                        "{}: pytest fn '{}' not found in '{}'",
                        f.id, fn_name, file
                    ));
                }
                continue;
            }
            let Some((crate_name, fn_name)) = t.split_once("::") else {
                failures.push(format!(
                    "{}: test reference '{}' must be crate-name::test_fn or pytest:path::test_fn",
                    f.id, t
                ));
                continue;
            };
            let Some(dir) = crates.get(crate_name) else {
                failures.push(format!("{}: unknown crate '{}'", f.id, crate_name));
                continue;
            };
            if !test_fn_exists(dir, fn_name) {
                failures.push(format!(
                    "{}: test fn '{}' not found in crate '{}'",
                    f.id, fn_name, crate_name
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "feature registry violations:\n{}",
        failures.join("\n")
    );
}
