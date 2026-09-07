#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(windows)]
mod app {
    use std::env;
    use std::f32::consts::TAU;
    use std::sync::mpsc;
    use std::thread;

    use impulcifer_sys_win::{CaptureStats, WasapiBackend};
    use impulcifer_types::audio::{
        AudioBackend, AudioError, CancelToken, Direction, Endpoint, InputSession, OutputSession,
        PlaybackReport, ShareMode, StreamSpec,
    };

    #[derive(Debug)]
    struct Args {
        output: Option<String>,
        input: Option<String>,
        play: bool,
        channel: usize,
        rate: u32,
        seconds: f64,
    }

    impl Default for Args {
        fn default() -> Self {
            Self {
                output: None,
                input: None,
                play: false,
                channel: 0,
                rate: 48_000,
                seconds: 1.0,
            }
        }
    }

    fn usage() -> &'static str {
        "hardware_probe [--output <substring>] [--input <substring>] [--play] [--channel N] [--rate 48000] [--seconds 1.0]"
    }

    fn parse_args() -> Result<Args, String> {
        let mut parsed = Args::default();
        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--output" => {
                    parsed.output = Some(args.next().ok_or("--output requires a value")?);
                }
                "--input" => {
                    parsed.input = Some(args.next().ok_or("--input requires a value")?);
                }
                "--play" => parsed.play = true,
                "--channel" => {
                    parsed.channel = args
                        .next()
                        .ok_or("--channel requires a value")?
                        .parse()
                        .map_err(|_| "--channel must be a non-negative integer")?;
                }
                "--rate" => {
                    parsed.rate = args
                        .next()
                        .ok_or("--rate requires a value")?
                        .parse()
                        .map_err(|_| "--rate must be an integer")?;
                }
                "--seconds" => {
                    parsed.seconds = args
                        .next()
                        .ok_or("--seconds requires a value")?
                        .parse()
                        .map_err(|_| "--seconds must be a number")?;
                }
                "--help" | "-h" => return Err(usage().into()),
                other => return Err(format!("unknown argument {other}\n{}", usage())),
            }
        }
        if parsed.rate == 0 {
            return Err("--rate must be greater than zero".into());
        }
        if !parsed.seconds.is_finite() || parsed.seconds <= 0.0 {
            return Err("--seconds must be finite and greater than zero".into());
        }
        Ok(parsed)
    }

    fn direction(endpoint: &Endpoint) -> Direction {
        if endpoint.max_output_channels > 0 {
            Direction::Output
        } else {
            Direction::Input
        }
    }

    fn channels(endpoint: &Endpoint) -> u16 {
        match direction(endpoint) {
            Direction::Output => endpoint.max_output_channels,
            Direction::Input => endpoint.max_input_channels,
        }
    }

    fn print_endpoints(endpoints: &[Endpoint]) {
        println!(
            "{:<7} | {:<48} | {:>8} | {:>8} | {:<11} | id",
            "dir", "name", "channels", "mix Hz", "default"
        );
        println!("{}", "-".repeat(132));
        for endpoint in endpoints {
            let dir = direction(endpoint);
            let default = match dir {
                Direction::Output => endpoint.is_default_output,
                Direction::Input => endpoint.is_default_input,
            };
            println!(
                "{:<7} | {:<48} | {:>8} | {:>8.0} | {:<11} | {}",
                format!("{dir:?}"),
                endpoint.name,
                channels(endpoint),
                endpoint.default_samplerate,
                default,
                endpoint.id
            );
        }
    }

    fn matrix_channels(maximum: u16) -> Vec<u16> {
        let mut result = Vec::new();
        for requested in [2, 8, 16] {
            let clipped = requested.min(maximum);
            if clipped > 0 && !result.contains(&clipped) {
                result.push(clipped);
            }
        }
        result
    }

    fn print_matrix(backend: &WasapiBackend, endpoints: &[Endpoint]) {
        println!("\nProbe matrix");
        println!(
            "{:<7} | {:<38} | {:>6} | {:>2} | {:<18} | {:<11} | detail",
            "dir", "endpoint", "rate", "ch", "mode", "result"
        );
        println!("{}", "-".repeat(142));
        for endpoint in endpoints {
            let dir = direction(endpoint);
            for rate in [44_100, 48_000, 96_000] {
                for channel_count in matrix_channels(channels(endpoint)) {
                    for mode in [ShareMode::Exclusive, ShareMode::SharedAutoConvert] {
                        let spec = StreamSpec {
                            sample_rate: rate,
                            channels: channel_count,
                        };
                        match backend.probe(endpoint, dir, spec, mode) {
                            Ok(result) => println!(
                                "{:<7} | {:<38} | {:>6} | {:>2} | {:<18} | {:<11} | {}",
                                format!("{dir:?}"),
                                endpoint.name,
                                rate,
                                channel_count,
                                format!("{mode:?}"),
                                if result.supported {
                                    "supported"
                                } else {
                                    "unsupported"
                                },
                                result.detail
                            ),
                            Err(err) => println!(
                                "{:<7} | {:<38} | {:>6} | {:>2} | {:<18} | {:<11} | {}",
                                format!("{dir:?}"),
                                endpoint.name,
                                rate,
                                channel_count,
                                format!("{mode:?}"),
                                "error",
                                err
                            ),
                        }
                    }
                }
            }
        }
    }

    fn select_endpoint(
        endpoints: &[Endpoint],
        dir: Direction,
        filter: Option<&str>,
    ) -> Result<Endpoint, String> {
        let mut candidates = endpoints
            .iter()
            .filter(|endpoint| direction(endpoint) == dir);
        if let Some(filter) = filter {
            let filter = filter.to_lowercase();
            return candidates
                .filter(|endpoint| endpoint.name.to_lowercase().contains(&filter))
                .min_by_key(|endpoint| endpoint.name.len())
                .cloned()
                .ok_or_else(|| format!("no {dir:?} endpoint name contains {filter:?}"));
        }
        candidates
            .find(|endpoint| match dir {
                Direction::Output => endpoint.is_default_output,
                Direction::Input => endpoint.is_default_input,
            })
            .cloned()
            .ok_or_else(|| format!("no default {dir:?} endpoint"))
    }

    fn open_output_with_fallback(
        backend: &WasapiBackend,
        endpoint: &Endpoint,
        spec: StreamSpec,
    ) -> Result<(Box<dyn OutputSession>, ShareMode), AudioError> {
        match backend.open_output(endpoint, spec, ShareMode::Exclusive) {
            Ok(session) => Ok((session, ShareMode::Exclusive)),
            Err(exclusive) => {
                eprintln!("exclusive output unavailable: {exclusive}; trying shared auto-convert");
                backend
                    .open_output(endpoint, spec, ShareMode::SharedAutoConvert)
                    .map(|session| (session, ShareMode::SharedAutoConvert))
            }
        }
    }

    fn open_input_with_fallback(
        backend: &WasapiBackend,
        endpoint: &Endpoint,
        spec: StreamSpec,
    ) -> Result<(impulcifer_sys_win::WasapiInputSession, ShareMode), AudioError> {
        match backend.open_input_session(endpoint, spec, ShareMode::Exclusive) {
            Ok(session) => Ok((session, ShareMode::Exclusive)),
            Err(exclusive) => {
                eprintln!("exclusive input unavailable: {exclusive}; trying shared auto-convert");
                backend
                    .open_input_session(endpoint, spec, ShareMode::SharedAutoConvert)
                    .map(|session| (session, ShareMode::SharedAutoConvert))
            }
        }
    }

    struct CaptureOutcome {
        mode: ShareMode,
        frames: usize,
        stats: CaptureStats,
        rms_dbfs: Vec<f64>,
    }

    fn rms_dbfs(samples: &[f32], channels: usize) -> Vec<f64> {
        (0..channels)
            .map(|channel| {
                let mut sum = 0.0f64;
                let mut count = 0usize;
                for sample in samples.iter().skip(channel).step_by(channels) {
                    sum += f64::from(*sample) * f64::from(*sample);
                    count += 1;
                }
                let rms = (sum / count.max(1) as f64).sqrt();
                if rms == 0.0 {
                    f64::NEG_INFINITY
                } else {
                    20.0 * rms.log10()
                }
            })
            .collect()
    }

    fn run_play(
        endpoints: &[Endpoint],
        output_filter: Option<&str>,
        input_filter: Option<&str>,
        rate: u32,
        seconds: f64,
        channel: usize,
    ) -> Result<(), String> {
        let output = select_endpoint(endpoints, Direction::Output, output_filter)?;
        let input = select_endpoint(endpoints, Direction::Input, input_filter)?;
        let output_channels = output.max_output_channels;
        if channel >= usize::from(output_channels) {
            return Err(format!(
                "channel {channel} is outside output range 0..{}",
                output_channels.saturating_sub(1)
            ));
        }
        let output_spec = StreamSpec {
            sample_rate: rate,
            channels: output_channels,
        };
        let input_spec = StreamSpec {
            sample_rate: rate,
            channels: 2.min(input.max_input_channels),
        };
        if input_spec.channels == 0 {
            return Err("selected input has no capture channels".into());
        }

        println!("output: {}", output.name);
        println!("input:  {}", input.name);

        let capture_frames = ((seconds + 1.0) * f64::from(rate)).round() as usize;
        let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<ShareMode, String>>(1);
        let input_for_thread = input.clone();
        let capture_thread = thread::Builder::new()
            .name("hardware-probe-capture".into())
            .spawn(move || -> Result<CaptureOutcome, String> {
                let backend = WasapiBackend::new();
                let (mut session, mode) =
                    match open_input_with_fallback(&backend, &input_for_thread, input_spec) {
                        Ok(opened) => opened,
                        Err(err) => {
                            let message = err.to_string();
                            let _ = ready_tx.send(Err(message.clone()));
                            return Err(message);
                        }
                    };
                if let Err(err) = session.start() {
                    let message = err.to_string();
                    let _ = ready_tx.send(Err(message.clone()));
                    return Err(message);
                }
                ready_tx
                    .send(Ok(mode))
                    .map_err(|_| "playback side closed before capture became ready".to_string())?;
                let mut samples = vec![0.0; capture_frames * usize::from(input_spec.channels)];
                let read = session
                    .read_into(&mut samples, &CancelToken::new())
                    .map_err(|err| err.to_string())?;
                session.stop().map_err(|err| err.to_string())?;
                samples.truncate(read.frames * usize::from(input_spec.channels));
                Ok(CaptureOutcome {
                    mode,
                    frames: read.frames,
                    stats: session.capture_stats(),
                    rms_dbfs: rms_dbfs(&samples, usize::from(input_spec.channels)),
                })
            })
            .map_err(|err| format!("failed to start capture thread: {err}"))?;

        let input_mode = ready_rx
            .recv()
            .map_err(|_| "capture thread exited before reporting ready".to_string())??;
        let backend = WasapiBackend::new();
        let (mut output_session, output_mode) =
            open_output_with_fallback(&backend, &output, output_spec)
                .map_err(|err| err.to_string())?;
        let playback_frames = (seconds * f64::from(rate)).round() as usize;
        let mut sine = vec![0.0f32; playback_frames * usize::from(output_channels)];
        for frame in 0..playback_frames {
            sine[frame * usize::from(output_channels) + channel] =
                (TAU * 1_000.0 * frame as f32 / rate as f32).sin() * 0.1;
        }
        let playback: PlaybackReport = output_session
            .play_to_completion(&sine, &CancelToken::new())
            .map_err(|err| err.to_string())?;
        let capture = capture_thread
            .join()
            .map_err(|_| "capture thread panicked".to_string())??;

        println!("output mode: {output_mode:?}");
        println!("input mode:  {input_mode:?} (reported {:?})", capture.mode);
        println!("frames submitted: {}", playback.frames_submitted);
        println!("frames drained:   {}", playback.frames_drained);
        println!("underruns:        {}", playback.underruns);
        println!("captured frames:  {}", capture.frames);
        println!("discontinuities:  {}", capture.stats.discontinuity_packets);
        println!("silent packets:   {}", capture.stats.silent_packets);
        for (index, rms) in capture.rms_dbfs.iter().enumerate() {
            println!("capture ch {index} RMS: {rms:.2} dBFS");
        }
        Ok(())
    }

    pub fn run() -> Result<(), String> {
        let args = parse_args()?;
        let backend = WasapiBackend::new();
        let endpoints = backend.enumerate().map_err(|err| err.to_string())?;
        print_endpoints(&endpoints);
        if args.play {
            run_play(
                &endpoints,
                args.output.as_deref(),
                args.input.as_deref(),
                args.rate,
                args.seconds,
                args.channel,
            )
        } else {
            print_matrix(&backend, &endpoints);
            Ok(())
        }
    }
}

#[cfg(windows)]
fn main() {
    if let Err(err) = app::run() {
        eprintln!("hardware_probe: {err}");
        std::process::exit(2);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("hardware_probe: WASAPI is available only on Windows");
    std::process::exit(2);
}
