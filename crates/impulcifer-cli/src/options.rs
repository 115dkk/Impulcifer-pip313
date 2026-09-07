//! Frozen 2.x argparse surface, in dataclass order. Goldens check every field.
use clap::{Arg, ArgAction, Command};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliType {
    Str,
    Int,
    Float,
    FlagTrue,
    FlagFalse,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CliDefault {
    Suppressed,
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(&'static str),
}
impl CliDefault {
    pub fn value(self) -> Option<Value> {
        match self {
            Self::Suppressed => None,
            Self::Null => Some(Value::Null),
            Self::Bool(v) => Some(json!(v)),
            Self::Int(v) => Some(json!(v)),
            Self::Float(v) => Some(json!(v)),
            Self::Str(v) => Some(json!(v)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CliOption {
    pub flag: &'static str,
    pub short: Option<char>,
    pub dest: &'static str,
    pub help: &'static str,
    pub kind: CliType,
    pub choices: &'static [&'static str],
    pub default: CliDefault,
}

pub fn command() -> Command {
    let mut command = Command::new("impulcifer")
        .disable_help_flag(true)
        .disable_version_flag(true)
        .version(env!("CARGO_PKG_VERSION"))
        .args_override_self(true)
        .infer_long_args(true)
        .arg(
            Arg::new("help")
                .short('h')
                .long("help")
                .help("show this help message and exit")
                .action(ArgAction::Help)
                .display_order(0),
        );
    for (index, option) in OPTIONS.iter().enumerate() {
        let mut arg = Arg::new(option.dest)
            .long(&option.flag[2..])
            .help(option.help)
            .long_help(option.help)
            .display_order(index + 1);
        if let Some(short) = option.short {
            arg = arg.short(short);
        }
        arg = match option.kind {
            CliType::FlagTrue => arg.action(ArgAction::SetTrue),
            CliType::FlagFalse => arg.action(ArgAction::SetFalse),
            _ => arg.action(ArgAction::Set).allow_negative_numbers(true),
        };
        if option.dest == "version" {
            arg = arg.action(ArgAction::Version);
        }
        command = command.arg(arg);
    }
    command
}

/// argparse-shaped usage for diagnostics; generated from the same option table.
pub fn usage() -> String {
    let mut text = String::from("usage: impulcifer [-h]");
    for option in OPTIONS {
        text.push_str(" [");
        text.push_str(if option.dest == "version" {
            "-V"
        } else {
            option.flag
        });
        if !matches!(option.kind, CliType::FlagTrue | CliType::FlagFalse) {
            text.push(' ');
            if option.choices.is_empty() {
                text.push_str(&option.dest.to_ascii_uppercase());
            } else {
                text.push('{');
                text.push_str(&option.choices.join(","));
                text.push('}');
            }
        }
        text.push(']');
    }
    text
}

pub const OPTIONS: &[CliOption] = &[
    CliOption {
        flag: "--version",
        short: Some('V'),
        dest: "version",
        help: "show program's version number and exit",
        kind: CliType::FlagTrue,
        choices: &[],
        default: CliDefault::Suppressed,
    },
    CliOption {
        flag: "--info",
        short: None,
        dest: "info",
        help: "Print diagnostic information (version, Python, OS, etc.) and exit.",
        kind: CliType::FlagTrue,
        choices: &[],
        default: CliDefault::Bool(false),
    },
    CliOption {
        flag: "--dir_path",
        short: None,
        dest: "dir_path",
        help: "Path to directory for recordings and outputs.",
        kind: CliType::Str,
        choices: &[],
        default: CliDefault::Null,
    },
    CliOption {
        flag: "--test_signal",
        short: None,
        dest: "test_signal",
        help: "Test signal source. Defaults to automatic detection: <dir>/test.wav if present, otherwise the sweep parameters are recovered from the recordings themselves (falling back to the bundled default sweep). Accepts a path to a sine sweep WAV file, the literal \"auto\", \"generate:<duration>s@<fs>\" (e.g. \"generate:6.15s@48000\") to construct the sweep from parameters, or a predefined name/number: \"default\"/\"1\", \"sweep\"/\"2\", \"stereo\"/\"3\" (FL,FR), \"mono-left\"/\"4\" (FL mono), \"left\"/\"5\" (FL stereo), \"right\"/\"6\" (FR stereo).",
        kind: CliType::Str,
        choices: &[],
        default: CliDefault::Suppressed,
    },
    CliOption {
        flag: "--room_target",
        short: None,
        dest: "room_target",
        help: "Path to room target response AutoEQ style CSV file.",
        kind: CliType::Str,
        choices: &[],
        default: CliDefault::Suppressed,
    },
    CliOption {
        flag: "--room_mic_calibration",
        short: None,
        dest: "room_mic_calibration",
        help: "Path to room measurement microphone calibration file.",
        kind: CliType::Str,
        choices: &[],
        default: CliDefault::Suppressed,
    },
    CliOption {
        flag: "--headphone_compensation_file",
        short: None,
        dest: "headphone_compensation_file",
        help: "Path to the headphone compensation WAV file. Defaults to \"headphones.wav\" in dir_path.",
        kind: CliType::Str,
        choices: &[],
        default: CliDefault::Null,
    },
    CliOption {
        flag: "--fs",
        short: None,
        dest: "fs",
        help: "Output sampling rate in Hertz.",
        kind: CliType::Int,
        choices: &[],
        default: CliDefault::Suppressed,
    },
    CliOption {
        flag: "--plot",
        short: None,
        dest: "plot",
        help: "Plot graphs for debugging.",
        kind: CliType::FlagTrue,
        choices: &[],
        default: CliDefault::Bool(false),
    },
    CliOption {
        flag: "--interactive_plots",
        short: None,
        dest: "interactive_plots",
        help: "Generate interactive Bokeh plots in HTML files.",
        kind: CliType::FlagTrue,
        choices: &[],
        default: CliDefault::Bool(false),
    },
    CliOption {
        flag: "--channel_balance",
        short: None,
        dest: "channel_balance",
        help: "Channel balance correction by equalizing left and right ear results to the same level or frequency response. \"trend\" equalizes right side by the difference trend of right and left side. \"left\" equalizes right side to left side fr, \"right\" equalizes left side to right side fr, \"avg\" equalizes both to the average fr, \"min\" equalizes both to the minimum of left and right side frs. Number values will boost or attenuate right side relative to left side by the number of dBs. \"mids\" is the same as the numerical values but guesses the value automatically from mid frequency levels.",
        kind: CliType::Str,
        choices: &[],
        default: CliDefault::Suppressed,
    },
    CliOption {
        flag: "--decay",
        short: None,
        dest: "decay",
        help: "Target decay time in milliseconds to reach -60 dB. When the natural decay time is longer than the target decay time, a downward slope will be applied to decay tail. Decay cannot be increased with this. By default no decay time adjustment is done. A comma separated list of channel name and  reverberation time pairs, separated by a colon. If only a single numeric value is given, it is used for all channels. When some channel names are give but not all, the missing channels are not affected. For example \"--decay=300\" or \"--decay=FL:500,FC:100,FR:500,SR:700,BR:700,BL:700,SL:700\" or \"--decay=FC:100\".",
        kind: CliType::Str,
        choices: &[],
        default: CliDefault::Suppressed,
    },
    CliOption {
        flag: "--target_level",
        short: None,
        dest: "target_level",
        help: "Target average gain level for left and right channels. This will sum together all left side impulse responses and right side impulse responses respectively and take the average gain from mid frequencies. The averaged level is then normalized to the given target level. This makes it possible to compare HRIRs with somewhat similar loudness levels. This should be negative in most cases to avoid clipping.",
        kind: CliType::Float,
        choices: &[],
        default: CliDefault::Suppressed,
    },
    CliOption {
        flag: "--fr_combination_method",
        short: None,
        dest: "fr_combination_method",
        help: "Method for combining frequency responses of generic room measurements if there are more than one tracks in the file. \"average\" will simply average the frequencyresponses. \"conservative\" will take the minimum absolute value for each frequency but only if the values in all the measurements are positive or negative at the same time.",
        kind: CliType::Str,
        choices: &[],
        default: CliDefault::Str("average"),
    },
    CliOption {
        flag: "--specific_limit",
        short: None,
        dest: "specific_limit",
        help: "Upper limit for room equalization with speaker-ear specific room measurements. Equalization will drop down to 0 dB at this frequency in the leading octave. 0 disables limit.",
        kind: CliType::Float,
        choices: &[],
        default: CliDefault::Int(400),
    },
    CliOption {
        flag: "--generic_limit",
        short: None,
        dest: "generic_limit",
        help: "Upper limit for room equalization with generic room measurements. Equalization will drop down to 0 dB at this frequency in the leading octave. 0 disables limit.",
        kind: CliType::Float,
        choices: &[],
        default: CliDefault::Int(300),
    },
    CliOption {
        flag: "--tilt",
        short: None,
        dest: "tilt",
        help: "Target tilt in dB/octave. Positive value (upwards slope) will result in brighter frequency response and negative value (downwards slope) will result in darker frequency response. 1 dB/octave will produce nearly 10 dB difference in desired value between 20 Hz and 20 kHz. Tilt is applied with bass boost and both will affect the bass gain.",
        kind: CliType::Float,
        choices: &[],
        default: CliDefault::Suppressed,
    },
    CliOption {
        flag: "--no_room_correction",
        short: None,
        dest: "do_room_correction",
        help: "Skip room correction.",
        kind: CliType::FlagFalse,
        choices: &[],
        default: CliDefault::Bool(true),
    },
    CliOption {
        flag: "--no_headphone_compensation",
        short: None,
        dest: "do_headphone_compensation",
        help: "Skip headphone compensation.",
        kind: CliType::FlagFalse,
        choices: &[],
        default: CliDefault::Bool(true),
    },
    CliOption {
        flag: "--no_equalization",
        short: None,
        dest: "do_equalization",
        help: "Skip equalization.",
        kind: CliType::FlagFalse,
        choices: &[],
        default: CliDefault::Bool(true),
    },
    CliOption {
        flag: "--remove_silent_channels",
        short: None,
        dest: "remove_silent_channels",
        help: "Remove all zero channels from combined BRIRs, including gaps. WARNING: changes channel positions and can break HeSuVi compatibility. Default: disabled.",
        kind: CliType::FlagTrue,
        choices: &[],
        default: CliDefault::Bool(false),
    },
    CliOption {
        flag: "--c",
        short: None,
        dest: "head_ms",
        help: "Head room in milliseconds for cropping impulse response heads. Default is 1.0 (ms).",
        kind: CliType::Float,
        choices: &[],
        default: CliDefault::Float(1.0),
    },
    CliOption {
        flag: "--jamesdsp",
        short: None,
        dest: "jamesdsp",
        help: "Generate true stereo IR file (jamesdsp.wav) for JamesDSP from FL/FR channels.",
        kind: CliType::FlagTrue,
        choices: &[],
        default: CliDefault::Bool(false),
    },
    CliOption {
        flag: "--hangloose",
        short: None,
        dest: "hangloose",
        help: "Generate separate stereo IR for each channel for Hangloose Convolver.",
        kind: CliType::FlagTrue,
        choices: &[],
        default: CliDefault::Bool(false),
    },
    CliOption {
        flag: "--microphone_deviation_correction",
        short: None,
        dest: "microphone_deviation_correction",
        help: "Enable v4.0 interaural microphone mismatch correction (direction-independent left/right level mismatch from mic placement/sensitivity). Skipped automatically when headphone compensation is enabled, since the mic response already cancels there.",
        kind: CliType::FlagTrue,
        choices: &[],
        default: CliDefault::Bool(false),
    },
    CliOption {
        flag: "--mic_deviation_strength",
        short: None,
        dest: "mic_deviation_strength",
        help: "Microphone deviation correction strength (0.0-1.0). 0.0 = no correction, 1.0 = full correction. Default is 0.7.",
        kind: CliType::Float,
        choices: &[],
        default: CliDefault::Float(0.7),
    },
    CliOption {
        flag: "--mic_deviation_debug_plots",
        short: None,
        dest: "mic_deviation_debug_plots",
        help: "Save debug plots for microphone deviation correction. (Default: disabled)",
        kind: CliType::FlagTrue,
        choices: &[],
        default: CliDefault::Bool(false),
    },
    CliOption {
        flag: "--output_truehd_layouts",
        short: None,
        dest: "output_truehd_layouts",
        help: "Generate TrueHD layouts.",
        kind: CliType::FlagTrue,
        choices: &[],
        default: CliDefault::Bool(false),
    },
    CliOption {
        flag: "--vbass",
        short: None,
        dest: "vbass",
        help: "Enable virtual bass synthesis.",
        kind: CliType::FlagTrue,
        choices: &[],
        default: CliDefault::Bool(false),
    },
    CliOption {
        flag: "--vbass_freq",
        short: None,
        dest: "vbass_freq",
        help: "Virtual bass crossover frequency in Hz (default: 250).",
        kind: CliType::Int,
        choices: &[],
        default: CliDefault::Int(250),
    },
    CliOption {
        flag: "--vbass_hp",
        short: None,
        dest: "vbass_hp",
        help: "Virtual bass sub-bass high-pass frequency in Hz (default: 15.0).",
        kind: CliType::Float,
        choices: &[],
        default: CliDefault::Float(15.0),
    },
    CliOption {
        flag: "--vbass_polarity",
        short: None,
        dest: "vbass_polarity",
        help: "Virtual bass polarity handling (default: auto).",
        kind: CliType::Str,
        choices: &["auto", "normal", "invert"],
        default: CliDefault::Str("auto"),
    },
    CliOption {
        flag: "--bass_boost",
        short: None,
        dest: "bass_boost",
        help: "Bass boost shelf. Sub-bass frequencies will be boosted by this amount. Can be either a single value for a gain in dB or a comma separated list of three values for parameters of a low shelf filter, where the first is gain in dB, second is center frequency (Fc) in Hz and the last is quality (Q). When only a single value (gain) is given, default values for Fc and Q are used which are 105 Hz and 0.76, respectively. For example \"--bass_boost=6\" or \"--bass_boost=6,150,0.69\".",
        kind: CliType::Str,
        choices: &[],
        default: CliDefault::Suppressed,
    },
];
