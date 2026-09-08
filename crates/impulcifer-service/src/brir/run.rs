use super::{
    BrirError, BrirEvents, Catalog, discovery::discover, estimator::open_with_events,
    inputs::load_inputs, outputs::write_outputs_checked,
};
use impulcifer_dsp::{
    DspError,
    pipeline::{StageObserver, StageProgress, run_pipeline, total_steps},
};
use impulcifer_jobs::registry::{JobContext, JobFailure};
use impulcifer_types::{
    config::ProcessingConfig,
    ipc::{self, ErrorCode},
    stages::StageKey,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub struct BrirRun {
    pub output_path: PathBuf,
}
struct Events<'a> {
    ctx: &'a JobContext,
    catalog: &'a Catalog,
    step: usize,
    total: usize,
    same_fs: bool,
}
impl BrirEvents for Events<'_> {
    fn step(&mut self, key: &str, args: Value) -> Result<(), BrirError> {
        self.check_cancelled()?;
        self.step += 1;
        let message = self.catalog.translate(key, &args);
        // Python emits a PROGRESS log before its progress callback. Preserve both.
        self.ctx
            .log(json!({"level":"PROGRESS","message":message,"key":key}));
        // logger.step truncates to integer percent before service division by 100.
        self.ctx.progress(
            ((self.step as f64 / self.total as f64) * 100.0).trunc() / 100.0,
            json!({"message":message,"key":key}),
        );
        Ok(())
    }
    fn log(&mut self, level: &str, key: &str, args: Value) {
        self.ctx.log(json!({"level":level.to_ascii_uppercase(),"message":self.catalog.translate(key,&args),"key":key}));
    }
    fn check_cancelled(&self) -> Result<(), BrirError> {
        self.ctx.check_cancelled().map_err(|_| BrirError::Cancelled)
    }
    fn translate(&self, key: &str) -> String {
        self.catalog.translate(key, &json!({}))
    }
    fn cancel_token(&self) -> Option<impulcifer_types::audio::CancelToken> {
        Some(self.ctx.cancel.clone())
    }
}
fn stage_key(key: StageKey) -> Option<&'static str> {
    use StageKey::*;
    Some(match key {
        PlotPre => "cli_plotting_pre",
        CropAndAlign => "cli_cropping_responses",
        VirtualBass => "vbass_status_processing",
        MicDeviation => "cli_correcting_deviation",
        Equalize => "cli_equalizing",
        Decay => "cli_adjusting_decay",
        ChannelBalance => "cli_correcting_balance",
        Normalize => "cli_normalizing_gain",
        PlotPost => "cli_plotting_post",
        PlotResults => "cli_plotting_results",
        PlotAdditional => "cli_plotting_additional",
        InteractivePlots => "cli_generating_interactive",
        Resample => "cli_resampling",
        WriteBrirs => "cli_writing_brirs",
        TruehdLayouts => "cli_generating_truehd",
        Jamesdsp => "cli_generating_jamesdsp",
        Hangloose => "cli_generating_hangloose",
        _ => return None,
    })
}
struct Observer<'a, 'b> {
    events: &'a mut Events<'b>,
    config: &'a ProcessingConfig,
    channels: usize,
    decay_channels: usize,
    sample_rate: u32,
    previous: Option<StageKey>,
    directory: &'a Path,
    estimator: &'a impulcifer_dsp::estimator::SweepEstimator,
}
impl StageObserver for Observer<'_, '_> {
    fn on_plot(
        &mut self,
        key: StageKey,
        hrir: &impulcifer_dsp::hrir::Hrir,
    ) -> Result<(), DspError> {
        self.check_cancelled()?;
        let token = self.events.cancel_token();
        let cancelled = move || token.as_ref().is_some_and(|t| t.is_cancelled());
        super::plots::render_stage(self.directory, key, hrir, self.estimator, &cancelled)
    }
    fn on_stage(&mut self, progress: StageProgress) {
        let key = progress.key;
        if self.previous == Some(StageKey::VirtualBass) {
            self.events
                .log("success", "vbass_status_complete", json!({}));
        }
        self.previous = Some(key);
        if key == StageKey::Resample && self.events.same_fs {
            return;
        }
        if let Some(cli) = stage_key(key) {
            let _ = self.events.step(
                cli,
                if key == StageKey::Resample {
                    json!({"fs":self.config.fs})
                } else {
                    json!({})
                },
            );
        }
        if key == StageKey::InteractivePlots
            || (key == StageKey::MicDeviation && self.config.mic_deviation_debug_plots)
        {
            self.events
                .log("warning", "cli_plots_not_available_yet", json!({}));
        }
        if key == StageKey::MicDeviationSkipped {
            self.events
                .log("warning", "cli_mic_deviation_skipped_hpcomp", json!({}));
        }
        if key == StageKey::VirtualBass {
            self.events
                .log("info", "vbass_status_processing", json!({}));
            if self.config.vbass_freq as f64 >= self.sample_rate as f64 / 2.0 {
                self.events.log("error", "vbass_error_sr_limit", json!({}));
            } else if self.config.vbass_freq > 300 {
                self.events.log(
                    "warning",
                    "vbass_warning_high_crossover",
                    json!({"freq":self.config.vbass_freq}),
                );
            }
        }
        if key == StageKey::Decay && self.decay_channels > 0 {
            self.events.log(
                "info",
                "cli_info_parallel_decay",
                json!({"count":self.decay_channels}),
            );
        }
        if key == StageKey::Equalize {
            // Same catalogue contract, honest runtime identity rather than invented Python metadata.
            self.events.log("info","cli_info_parallel_executor",json!({"executor":"Rust","version":env!("IMPULCIFER_RUSTC_VERSION"),"status":"not applicable"}));
            self.events.log(
                "info",
                "cli_info_parallel_eq",
                json!({"count":self.channels}),
            );
        }
        if key == StageKey::WriteBrirs && self.config.remove_silent_channels {
            self.events
                .log("warning", "cli_warning_compact_channels", json!({}));
        }
    }
    fn check_cancelled(&self) -> Result<(), DspError> {
        self.events
            .check_cancelled()
            .map_err(|_| DspError::InvalidArgument("cancelled".into()))
    }
}
pub fn run_brir(
    config: &ProcessingConfig,
    i18n: &Catalog,
    ctx: &JobContext,
) -> Result<BrirRun, JobFailure> {
    run_with_data(config, i18n, ctx, &crate::default_data_dir())
}
pub(crate) fn run_with_data(
    config: &ProcessingConfig,
    i18n: &Catalog,
    ctx: &JobContext,
    data_dir: &Path,
) -> Result<BrirRun, JobFailure> {
    let result = (|| -> Result<BrirRun, BrirError> {
        let mut events = Events {
            ctx,
            catalog: i18n,
            step: 0,
            total: total_steps(config),
            same_fs: false,
        };
        events.log(
            "info",
            "cli_starting_brir_generation",
            json!({"total_steps":events.total}),
        );
        events.check_cancelled()?;
        let dir = discover(Path::new(config.dir_path.as_deref().unwrap_or("")), config)?;
        events.step("cli_creating_estimator", json!({}))?;
        let estimator = open_with_events(
            &dir,
            config.test_signal.as_deref(),
            data_dir,
            Some(&mut events),
        )?;
        events.same_fs = config.fs == Some(estimator.fs);
        events.check_cancelled()?;
        let inputs = load_inputs(&dir, &estimator, config, &mut events)?;
        let channels = inputs
            .hrir
            .speakers
            .iter()
            .map(|s| usize::from(s.left.is_some()) + usize::from(s.right.is_some()))
            .sum();
        let decay_channels = inputs
            .hrir
            .speakers
            .iter()
            .filter(|s| match &config.decay {
                Some(impulcifer_types::config::DecaySpec::Uniform(_)) => true,
                Some(impulcifer_types::config::DecaySpec::PerChannel(m)) => {
                    m.contains_key(&s.speaker)
                }
                None => false,
            })
            .map(|s| usize::from(s.left.is_some()) + usize::from(s.right.is_some()))
            .sum();
        let outputs = run_pipeline(
            config,
            inputs,
            &mut Observer {
                events: &mut events,
                config,
                channels,
                decay_channels,
                sample_rate: estimator.fs,
                previous: None,
                directory: &dir.dir,
                estimator: &estimator,
            },
        )?;
        events.check_cancelled()?;
        let written =
            write_outputs_checked(&dir.dir, &outputs, config, i18n, &estimator, &mut events)?;
        events.check_cancelled()?;
        Ok(BrirRun {
            output_path: written.hesuvi,
        })
    })();
    // An observer cancellation is carried through a DSP error; retain the job contract.
    ctx.check_cancelled()?;
    let run = result.map_err(BrirError::failure)?;
    ensure_output(&run.output_path)?;
    Ok(run)
}
pub fn ensure_output(path: &Path) -> Result<(), JobFailure> {
    if path.is_file() {
        Ok(())
    } else {
        Err(JobFailure {
            cancelled: false,
            error: ipc::error(
                ErrorCode::OutputMissing,
                "BRIR processing finished without producing hesuvi.wav.",
                json!({"output_path":path}),
                false,
            )["error"]
                .clone(),
        })
    }
}
