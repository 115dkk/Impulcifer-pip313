//! PA04 direct-call workloads, shared by the release bench and smoke test.
use impulcifer_jobs::registry::JobRegistry;
use impulcifer_service::{ImpulciferService, NoopHost, brir::Catalog};
use impulcifer_types::job::JobKind;
use serde_json::json;
use std::hint::black_box;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub const OPS: [(&str, usize); 9] = [
    ("bootstrap", 200),
    ("get_ui_settings", 200),
    ("set_language_round_trip", 100),
    ("resolve_recording_paths", 200),
    ("start_brir_to_first_event", 5),
    ("poll_job_drain", 5),
    ("job_event_emit", 5),
    ("detect_sweep", 20),
    ("catalog_translate", 100000),
];

pub struct Fixture {
    pub service: ImpulciferService,
    jobs: JobRegistry,
    root: PathBuf,
    temp: tempfile::TempDir,
    catalog: Catalog,
}
impl Fixture {
    pub fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let temp = tempfile::Builder::new()
            .prefix("impulcifer-pa04-")
            .tempdir()
            .unwrap();
        let settings = temp.path().join("settings.json");
        std::fs::write(&settings, r#"{"language":"en","language_selected":true}"#).unwrap();
        let jobs = JobRegistry::new();
        let service = ImpulciferService::with_dependencies(
            Box::new(NoopHost),
            settings,
            impulcifer_audio_io::default_backend(),
            jobs.clone(),
            root.join("data"),
        );
        assert_eq!(service.call("get_ui_settings", vec![])["ok"], true);
        Self {
            service,
            jobs,
            root,
            temp,
            catalog: Catalog::english(),
        }
    }

    pub fn sample(&self, op: &str, calls: usize, tiny: bool) -> f64 {
        let demo = self.root.join("data/demo");
        let demo = demo.to_str().unwrap();
        let args = json!({"date":"2026-09-08", "fs":48000});
        if [
            "start_brir_to_first_event",
            "poll_job_drain",
            "job_event_emit",
        ]
        .contains(&op)
        {
            return (0..calls)
                .map(|_| match op {
                    "start_brir_to_first_event" => self.start(),
                    "poll_job_drain" => self.drain(if tiny { 8 } else { 2000 }),
                    _ => self.emit(if tiny { 16 } else { 10000 }),
                })
                .sum::<f64>()
                / calls as f64;
        }
        let start = Instant::now();
        for _ in 0..calls {
            let result = match op {
                "bootstrap" | "get_ui_settings" => self.service.call(op, vec![]),
                "set_language_round_trip" => {
                    black_box(self.service.call("set_language", vec![json!("ko")]));
                    self.service.call("set_language", vec![json!("en")])
                }
                "resolve_recording_paths" => {
                    self.service.call(op, vec![json!(demo), json!("FL,FR.wav")])
                }
                "detect_sweep" => self.service.call(op, vec![json!(demo)]),
                "catalog_translate" => {
                    black_box(
                        self.catalog
                            .translate(black_box("cli_readme_processed"), black_box(&args)),
                    );
                    continue;
                }
                _ => panic!("unknown operation"),
            };
            black_box(result);
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0 / calls as f64;
        // Validate the same real operation outside the timed batch.
        match op {
            "catalog_translate" => assert_eq!(
                self.catalog.translate("cli_readme_processed", &args),
                "Processed on 2026-09-08. Output sampling rate is 48000 Hz."
            ),
            "resolve_recording_paths" => assert!(
                self.service.call(op, vec![json!(demo), json!("FL,FR.wav")])["data"]["record_path"]
                    .as_str()
                    .unwrap()
                    .ends_with("FL,FR.wav")
            ),
            "detect_sweep" => {
                let value = self.service.call(op, vec![json!(demo)]);
                assert_eq!(value["ok"], true);
                assert_eq!(value["data"]["fs"], 48000);
                assert_eq!(value["data"]["found"], true);
            }
            _ => {
                let value = self.service.call(
                    if op == "set_language_round_trip" {
                        "get_ui_settings"
                    } else {
                        op
                    },
                    vec![],
                );
                assert_eq!(value["ok"], true);
                let ui = if op == "bootstrap" {
                    &value["data"]["ui"]
                } else {
                    &value["data"]
                };
                assert_eq!(ui["language"], "en");
                assert!(ui["strings"]["cli_readme_processed"].is_string());
            }
        }
        ms
    }

    fn start(&self) -> f64 {
        let dir = tempfile::Builder::new()
            .prefix("demo-")
            .tempdir_in(self.temp.path())
            .unwrap();
        // Identical explicit manifest in the Python bench; inputs only.
        for name in [
            "FL,FR.wav",
            "FC.wav",
            "BL,SL.wav",
            "SR,BR.wav",
            "headphones.wav",
            "room.wav",
            "room-BL,SL-left.wav",
            "room-BL,SL-right.wav",
            "room-FC-left.wav",
            "room-FC-right.wav",
            "room-FL,FR-left.wav",
            "room-FL,FR-right.wav",
            "room-SR,BR-left.wav",
            "room-SR,BR-right.wav",
        ] {
            let source = self.root.join("data/demo").join(name);
            if source.is_file() {
                std::fs::copy(source, dir.path().join(name)).unwrap();
            }
        }
        let request = json!({"dir_path":dir.path(), "test_signal":self.root.join("data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")});
        let start = Instant::now();
        let response = self.service.call("start_brir", vec![request]);
        let id = response["data"]["job"]["job_id"]
            .as_str()
            .expect("start success");
        let first = loop {
            let value = self.service.call("poll_job", vec![json!(id), json!(0)]);
            if !value["data"]["events"]
                .as_array()
                .expect("poll success")
                .is_empty()
            {
                break value;
            }
        };
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(first["data"]["events"][0]["seq"], 1);
        assert_eq!(first["data"]["events"][0]["type"], "status");
        self.jobs.cancel(id).unwrap();
        self.jobs.join(id).unwrap();
        assert!(self.jobs.poll(id, 0).unwrap().job.status.is_terminal());
        ms
    }

    fn drain(&self, count: usize) -> f64 {
        let page = count.min(500);
        let (ready_tx, ready_rx) = mpsc::channel();
        let (ack_tx, ack_rx) = mpsc::channel();
        let job = self
            .jobs
            .start(JobKind::Brir, false, move |ctx| {
                for end in (page..=count).step_by(page) {
                    for i in end - page..end {
                        ctx.log(json!({"level":"INFO", "message":"PA04 event", "index":i}));
                    }
                    ready_tx.send(()).unwrap();
                    ack_rx.recv_timeout(Duration::from_secs(30)).unwrap();
                }
                Ok(json!({}))
            })
            .unwrap();
        let mut seq = 0;
        let mut logs = 0;
        let mut ms = 0.0;
        for _ in 0..count / page {
            ready_rx.recv_timeout(Duration::from_secs(30)).unwrap();
            let start = Instant::now();
            let value = self
                .service
                .call("poll_job", vec![json!(job.job_id), json!(seq)]);
            ms += start.elapsed().as_secs_f64() * 1000.0;
            assert_eq!(value["ok"], true);
            for event in value["data"]["events"].as_array().unwrap() {
                assert_eq!(event["seq"].as_u64().unwrap(), seq + 1);
                seq += 1;
                if event["type"] == "log" {
                    assert_eq!(event["payload"]["index"], logs);
                    assert_eq!(event["payload"]["message"], "PA04 event");
                    logs += 1;
                }
            }
            assert_eq!(value["data"]["next_seq"], seq);
            ack_tx.send(()).unwrap();
        }
        self.jobs.join(&job.job_id).unwrap();
        assert_eq!(logs, count);
        let final_poll = self
            .service
            .call("poll_job", vec![json!(job.job_id), json!(seq)]);
        assert_eq!(final_poll["data"]["events"].as_array().unwrap().len(), 1);
        assert_eq!(final_poll["data"]["job"]["status"], "succeeded");
        ms
    }

    fn emit(&self, count: usize) -> f64 {
        self.emit_payload(count, false).0
    }

    fn emit_payload(&self, count: usize, index_only: bool) -> (f64, f64) {
        let (tx, rx) = mpsc::channel();
        let job = self
            .jobs
            .start(JobKind::Brir, false, move |ctx| {
                let start = Instant::now();
                for i in 0..count {
                    ctx.log(if index_only {
                        json!({"index":i})
                    } else {
                        json!({"level":"INFO", "message":"PA04 event", "index":i})
                    });
                }
                tx.send(start.elapsed().as_secs_f64() * 1000.0).unwrap();
                Ok(json!({}))
            })
            .unwrap();
        let ms = rx.recv_timeout(Duration::from_secs(30)).unwrap();
        let join_start = Instant::now();
        self.jobs.join(&job.job_id).unwrap();
        let join_ms = join_start.elapsed().as_secs_f64() * 1000.0;
        let poll = self.jobs.poll(&job.job_id, 0).unwrap();
        assert_eq!(poll.next_seq, count as u64 + 2);
        assert_eq!(poll.events.len(), (count + 2).min(2000));
        assert_eq!(
            poll.events[poll.events.len() - 2].payload["index"],
            count - 1
        );
        (ms, join_ms)
    }
}

// Optional diagnostic only: the default nine-row workload remains unchanged.
// Keep every batch quiet until both payload shapes and their joins have run.
pub fn emit_diagnostic() {
    let fixture = Fixture::new();
    let mut values = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    for batch in 0..14 {
        let mut totals = [0.0; 4];
        for _ in 0..5 {
            for shape in 0..2 {
                let (emit, join) = fixture.emit_payload(10000, shape == 1);
                totals[shape * 2] += emit / 5.0;
                totals[shape * 2 + 1] += join / 5.0;
            }
        }
        if batch >= 3 {
            for (values, value) in values.iter_mut().zip(totals) {
                values.push(value);
            }
        }
    }
    for (name, mut values) in [
        "full_payload_emit",
        "full_payload_join",
        "index_only_emit",
        "index_only_join",
    ]
    .into_iter()
    .zip(values)
    {
        values.sort_by(f64::total_cmp);
        println!(
            "| {name} | 5/batch; per unit | {:.9} | {:.9} |",
            values[5], values[0]
        );
    }
}

pub fn run() {
    let fixture = Fixture::new();
    println!(
        "PA04 rust={} OS={} arch={} logical_cores={:?} threads={:?}",
        env!("IMPULCIFER_RUSTC_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::thread::available_parallelism(),
        [
            "OMP_NUM_THREADS",
            "MKL_NUM_THREADS",
            "OPENBLAS_NUM_THREADS",
            "RAYON_NUM_THREADS"
        ]
        .map(|k| (k, std::env::var(k).ok()))
    );
    println!("| op | size | rust median ms | rust min ms |");
    for (op, calls) in OPS {
        let mut values = Vec::new();
        for i in 0..14 {
            let ms = fixture.sample(op, calls, false);
            println!("PA04_RAW {op} {i} {ms:.9}");
            if i >= 3 {
                values.push(ms);
            }
        }
        values.sort_by(f64::total_cmp);
        println!(
            "| {op} | {calls}/batch; per unit | {:.9} | {:.9} |",
            values[5], values[0]
        );
    }
}
