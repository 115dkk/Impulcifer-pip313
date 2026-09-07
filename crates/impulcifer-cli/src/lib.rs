#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

//! Headless 2.x-compatible entry point shared by the executable and Python wheel.
mod console;
pub mod options;
mod settings;

use clap::error::{ContextKind, ErrorKind};
use impulcifer_types::{config::ProcessingConfig, constants::SPEAKER_NAMES};
use options::{CliType, OPTIONS};
use serde_json::{Map, Value, json};
use std::io::Write;

#[derive(Debug, PartialEq)]
pub enum Parsed {
    Help(String),
    Version,
    Info,
    Kwargs(Map<String, Value>),
}

#[derive(Debug, PartialEq, Eq)]
pub struct CliError {
    pub exit_code: i32,
    pub message: String,
}
impl CliError {
    fn argument(message: impl Into<String>) -> Self {
        Self {
            exit_code: 2,
            message: message.into(),
        }
    }
    fn value(message: impl Into<String>) -> Self {
        Self {
            exit_code: 1,
            message: message.into(),
        }
    }
}

/// Parse argv including the program name, without starting jobs or reading settings.
/// Suppressed defaults remain absent, just as in `create_cli()`.
pub fn parse(argv: &[String]) -> Result<Parsed, CliError> {
    let matches = match options::command().try_get_matches_from(argv) {
        Ok(matches) => matches,
        Err(error) => {
            return match error.kind() {
                ErrorKind::DisplayHelp => Ok(Parsed::Help(
                    options::command().render_long_help().to_string(),
                )),
                ErrorKind::DisplayVersion => Ok(Parsed::Version),
                _ => Err(clap_error(&error)),
            };
        }
    };
    let mut kwargs = Map::new();
    for option in OPTIONS {
        if option.dest == "version" {
            continue;
        }
        let value = match option.kind {
            CliType::FlagTrue | CliType::FlagFalse => Some(json!(matches.get_flag(option.dest))),
            _ => match matches.get_one::<String>(option.dest) {
                Some(raw) => {
                    if !option.choices.is_empty() && !option.choices.contains(&raw.as_str()) {
                        let choices = option
                            .choices
                            .iter()
                            .map(|v| format!("'{v}'"))
                            .collect::<Vec<_>>()
                            .join(", ");
                        return Err(CliError::argument(format!(
                            "argument {}: invalid choice: '{}' (choose from {choices})",
                            option.flag, raw
                        )));
                    }
                    let invalid = |kind| {
                        CliError::argument(format!(
                            "argument {}: invalid {kind} value: '{raw}'",
                            option.flag
                        ))
                    };
                    Some(match option.kind {
                        CliType::Int => {
                            json!(raw.trim().parse::<i64>().map_err(|_| invalid("int"))?)
                        }
                        CliType::Float => {
                            let number = raw.trim().parse::<f64>().map_err(|_| invalid("float"))?;
                            finite(number)?
                        }
                        _ => json!(raw),
                    })
                }
                None => option.default.value(),
            },
        };
        if let Some(value) = value {
            kwargs.insert(option.dest.into(), value);
        }
    }
    if kwargs.remove("info") == Some(json!(true)) {
        return Ok(Parsed::Info);
    }
    if kwargs.get("dir_path").is_none_or(Value::is_null) {
        return Err(CliError::argument(
            "the following arguments are required: --dir_path",
        ));
    }
    if let Some(Value::String(raw)) = kwargs.remove("bass_boost") {
        let parts: Vec<_> = raw.split(',').collect();
        if !matches!(parts.len(), 1 | 3) {
            return Err(CliError::value(
                "\"--bass_boost\" must have one value or three values separated by commas!",
            ));
        }
        let defaults = ProcessingConfig::default();
        kwargs.insert("bass_boost_gain".into(), finite(python_float(parts[0])?)?);
        kwargs.insert(
            "bass_boost_fc".into(),
            if parts.len() == 3 {
                finite(python_float(parts[1])?)?
            } else {
                // Python's dataclass stores the default Fc as an integer.
                json!(defaults.bass_boost_fc as i64)
            },
        );
        kwargs.insert(
            "bass_boost_q".into(),
            finite(if parts.len() == 3 {
                python_float(parts[2])?
            } else {
                defaults.bass_boost_q
            })?,
        );
    }
    if let Some(Value::String(raw)) = kwargs.remove("decay") {
        let mut decay = Map::new();
        if let Ok(number) = python_float(&raw) {
            for speaker in SPEAKER_NAMES {
                decay.insert(speaker.into(), finite(number / 1000.0)?);
            }
        } else {
            for pair in raw.split(',') {
                let mut parts = pair.split(':');
                let speaker = parts.next().unwrap_or_default().to_uppercase();
                let value = parts
                    .next()
                    .ok_or_else(|| CliError::value("list index out of range"))?;
                decay.insert(speaker, finite(python_float(value)? / 1000.0)?);
            }
        }
        kwargs.insert("decay".into(), Value::Object(decay));
    }
    Ok(Parsed::Kwargs(kwargs))
}

fn python_float(raw: &str) -> Result<f64, CliError> {
    raw.trim()
        .parse()
        .map_err(|_| CliError::value(format!("could not convert string to float: '{raw}'")))
}
fn finite(number: f64) -> Result<Value, CliError> {
    serde_json::Number::from_f64(number)
        .map(Value::Number)
        .ok_or_else(|| CliError::value("non-finite numbers are not supported by ProcessingConfig"))
}
fn clap_error(error: &clap::Error) -> CliError {
    let context = |kind| error.get(kind).map(ToString::to_string).unwrap_or_default();
    let argument = context(ContextKind::InvalidArg);
    let flag = argument.split_whitespace().next().unwrap_or(&argument);
    CliError::argument(match error.kind() {
        ErrorKind::UnknownArgument => format!("unrecognized arguments: {argument}"),
        ErrorKind::InvalidValue | ErrorKind::TooFewValues => {
            format!("argument {flag}: expected one argument")
        }
        ErrorKind::TooManyValues => format!(
            "argument {flag}: ignored explicit argument '{}'",
            context(ContextKind::InvalidValue)
        ),
        _ => error
            .to_string()
            .trim()
            .strip_prefix("error: ")
            .unwrap_or(error.to_string().trim())
            .to_owned(),
    })
}

/// Run synchronously. Diagnostics and job failures never terminate the embedding process.
/// Ctrl-C cancellation is intentionally not installed (no signal-handling dependency).
pub fn run(argv: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let result = match parse(argv) {
        Ok(Parsed::Help(text)) => out.write_all(text.as_bytes()).map(|_| 0),
        Ok(Parsed::Version) => writeln!(out, "Impulcifer {}", env!("CARGO_PKG_VERSION")).map(|_| 0),
        Ok(Parsed::Info) => console::info(out).map(|_| 0),
        Ok(Parsed::Kwargs(kwargs)) => match ProcessingConfig::from_kwargs(&kwargs) {
            Ok(config) => console::execute(config, settings::catalog(), out, err),
            Err(error) => writeln!(err, "{error}").map(|_| 1),
        },
        Err(error) => {
            let message = if error.exit_code == 2 {
                format!(
                    "{}\nimpulcifer: error: {}\n",
                    options::usage(),
                    error.message
                )
            } else {
                format!("{}\n", error.message)
            };
            err.write_all(message.as_bytes()).map(|_| error.exit_code)
        }
    };
    match result {
        Ok(code) => code,
        Err(error) => {
            let _ = writeln!(err, "{error}");
            1
        }
    }
}
