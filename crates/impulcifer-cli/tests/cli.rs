#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

use impulcifer_cli::{
    Parsed,
    options::{CliType, OPTIONS},
    parse, run,
};
use impulcifer_types::config::{FIELD_NAMES, ProcessingConfig};
use serde_json::{Value, json};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn golden(name: &str) -> Value {
    serde_json::from_slice(
        &std::fs::read(root().join("tests/migration/goldens").join(name)).unwrap(),
    )
    .unwrap()
}
fn args(argv: &[&str]) -> Vec<String> {
    argv.iter().map(|s| (*s).into()).collect()
}
fn invoke(argv: &[&str]) -> (i32, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(&args(argv), &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[test]
fn golden_cli_options_match_python() {
    let oracle = golden("p13_options.json");
    let oracle = oracle.as_array().unwrap();
    assert_eq!(OPTIONS.len() + 1, oracle.len());
    for (option, expected) in OPTIONS.iter().zip(&oracle[1..]) {
        let mut flags = Vec::new();
        if let Some(short) = option.short {
            flags.push(format!("-{short}"));
        }
        flags.push(option.flag.into());
        let type_name = match option.kind {
            CliType::Str => Some("str"),
            CliType::Int => Some("int"),
            CliType::Float => Some("float"),
            _ => None,
        };
        let action = match option.kind {
            _ if option.dest == "version" => "_VersionAction",
            CliType::FlagTrue => "_StoreTrueAction",
            CliType::FlagFalse => "_StoreFalseAction",
            _ => "_StoreAction",
        };
        let actual = json!({"option_strings":flags,"dest":option.dest,"help":option.help,
            "type":type_name,"action":action,"choices":if option.choices.is_empty() { Value::Null } else { json!(option.choices) },
            "default":option.default.value().unwrap_or(json!("SUPPRESS")),"required":false});
        assert_eq!(&actual, expected, "{}", option.flag);
    }
    let fields: Vec<_> = OPTIONS
        .iter()
        .filter(|o| FIELD_NAMES.contains(&o.dest))
        .map(|o| o.dest)
        .collect();
    let expected: Vec<_> = FIELD_NAMES
        .into_iter()
        .filter(|name| !name.starts_with("bass_boost_"))
        .collect();
    assert_eq!(fields, expected);
    let Parsed::Kwargs(defaults) =
        parse(&args(&["impulcifer", "--dir_path", "measurements"])).unwrap()
    else {
        panic!()
    };
    let config = ProcessingConfig::from_kwargs(&defaults).unwrap();
    assert_eq!(
        config,
        ProcessingConfig {
            dir_path: Some("measurements".into()),
            ..ProcessingConfig::default()
        }
    );
}

#[test]
fn golden_cli_parsing_matches_python() {
    let mut fixtures: Vec<_> = std::fs::read_dir(root().join("tests/migration/goldens"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("p13_parse_")
        })
        .collect();
    fixtures.sort();
    assert_eq!(fixtures.len(), 12);
    for file in fixtures {
        let oracle: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        let argv: Vec<_> = oracle["argv"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect();
        match parse(&argv) {
            Ok(Parsed::Kwargs(kwargs)) => {
                assert_eq!(json!(kwargs), oracle["kwargs"], "{}", file.display());
                assert_eq!(oracle["exit_code"], 0);
                ProcessingConfig::from_kwargs(&kwargs).unwrap();
            }
            Err(error) => {
                assert_eq!(error.message, oracle["error"], "{}", file.display());
                assert_eq!(error.exit_code, oracle["exit_code"]);
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }
}

#[test]
fn cli_help_mentions_every_option() {
    for flag in ["--help", "-h"] {
        let (code, out, err) = invoke(&["impulcifer", flag]);
        assert_eq!(code, 0);
        assert!(err.is_empty());
        for option in OPTIONS {
            assert!(out.contains(option.flag), "{}", option.flag);
            assert!(out.contains(option.help), "{}", option.flag);
        }
    }
}
#[test]
fn cli_missing_dir_path_exits_2_like_argparse() {
    let (code, out, err) = invoke(&["impulcifer"]);
    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert!(err.starts_with("usage: impulcifer [-h]"));
    assert!(err.ends_with("impulcifer: error: the following arguments are required: --dir_path\n"));
}
#[test]
fn cli_version_prints_crate_version() {
    for flag in ["--version", "-V"] {
        let (code, out, err) = invoke(&["impulcifer", flag]);
        assert_eq!(code, 0);
        assert_eq!(out, format!("Impulcifer {}\n", env!("CARGO_PKG_VERSION")));
        assert!(err.is_empty());
    }
}
#[test]
fn cli_reports_argument_errors_with_argparse_prefixes() {
    for (argv, expected) in [
        (
            vec!["impulcifer", "--unknown"],
            "unrecognized arguments: --unknown",
        ),
        (
            vec!["impulcifer", "--fs"],
            "argument --fs: expected one argument",
        ),
        (
            vec!["impulcifer", "--fs", "bad"],
            "argument --fs: invalid int value: 'bad'",
        ),
        (
            vec!["impulcifer", "--vbass_polarity", "bad"],
            "argument --vbass_polarity: invalid choice: 'bad' (choose from 'auto', 'normal', 'invert')",
        ),
    ] {
        let (code, out, err) = invoke(&argv);
        assert_eq!(code, 2, "{err}");
        assert!(out.is_empty());
        assert!(err.starts_with("usage: impulcifer [-h]"));
        assert!(
            err.ends_with(&format!("impulcifer: error: {expected}\n")),
            "{err}"
        );
    }
}
#[test]
fn cli_info_does_not_require_dir_path() {
    let (code, out, err) = invoke(&["impulcifer", "--info"]);
    assert_eq!(code, 0);
    assert!(err.is_empty());
    for label in [
        "Impulcifer ",
        "OS: ",
        "CPU cores: ",
        "Rust toolchain: rustc ",
        "Audio backend: ",
        "Update channel: ",
        "Data dir: ",
    ] {
        assert!(out.contains(label), "{out}");
    }
}
#[test]
fn cli_repeated_options_negative_values_and_suppression() {
    let Parsed::Kwargs(kwargs) = parse(&args(&[
        "impulcifer",
        "--dir_path=measurements",
        "--target_level",
        "-12",
        "--c=1",
        "--c",
        "2.5",
        "--plot",
        "--plot",
        "--decay=fl:300,FR:250",
    ]))
    .unwrap() else {
        panic!()
    };
    assert_eq!(kwargs["target_level"], -12.0);
    assert_eq!(kwargs["head_ms"], 2.5);
    assert_eq!(kwargs["plot"], true);
    assert_eq!(kwargs["decay"], json!({"FL":0.3,"FR":0.25}));
    for suppressed in [
        "test_signal",
        "room_target",
        "room_mic_calibration",
        "fs",
        "channel_balance",
        "tilt",
        "bass_boost_gain",
    ] {
        assert!(!kwargs.contains_key(suppressed), "{suppressed}");
    }
}

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "impulcifer-cli-test-{}-{}",
                std::process::id(),
                N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(e) => panic!("{e}"),
            }
        }
    }
    fn demo() -> Self {
        let temp = Self::new();
        for entry in std::fs::read_dir(root().join("data/demo")).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy();
            if [
                "hrir.wav",
                "hesuvi.wav",
                "responses.wav",
                "headphone-responses.wav",
                "room-responses.wav",
                "jamesdsp.wav",
            ]
            .contains(&name.as_ref())
                || name.starts_with("truehd_")
            {
                continue;
            }
            if path.is_file()
                && path.extension().is_some_and(|e| {
                    ["wav", "csv", "txt"]
                        .iter()
                        .any(|x| e.eq_ignore_ascii_case(x))
                })
            {
                std::fs::copy(&path, temp.0.join(path.file_name().unwrap())).unwrap();
            }
        }
        temp
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cli_runs_demo_and_writes_hesuvi() {
    let temp = Temp::demo();
    let sweep = root().join("data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav");
    let (code, out, err) = invoke(&[
        "impulcifer",
        "--dir_path",
        temp.0.to_str().unwrap(),
        "--test_signal",
        sweep.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{out}\n{err}");
    assert!(err.is_empty());
    assert!(temp.0.join("hesuvi.wav").is_file());
    let readme = std::fs::read_to_string(temp.0.join("README.md"))
        .unwrap()
        .replace("\r\n", "\n");
    assert!(out.contains(&readme));
    assert!(
        ["en", "ko", "de", "es", "fr", "ja", "ru", "zh_CN", "zh_TW"]
            .iter()
            .any(|language| {
                let catalog: Value = serde_json::from_slice(
                    &std::fs::read(root().join(format!("i18n/locales/{language}.json"))).unwrap(),
                )
                .unwrap();
                out.contains(catalog["cli_writing_brirs"].as_str().unwrap())
            })
    );
    assert!(out.contains("[100%]"));
}
#[test]
fn cli_reports_service_failure_with_exit_1() {
    let temp = Temp::new();
    let path = temp.0.join("missing");
    let (code, _, err) = invoke(&["impulcifer", "--dir_path", path.to_str().unwrap()]);
    assert_eq!(code, 1);
    // run_brir's discovery error carries the missing path verbatim; the
    // IPC validation wrapper uses a different message and is not called here.
    assert_eq!(err, format!("{}\n", path.display()));
    assert!(!path.exists());
}
