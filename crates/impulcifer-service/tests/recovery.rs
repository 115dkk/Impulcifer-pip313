#![forbid(unsafe_code)]

use impulcifer_io::{read_wav, write_wav};
use impulcifer_jobs::registry::JobRegistry;
use impulcifer_service::{
    ImpulciferService, NoopHost,
    recovery::{RecoveryOptions, plan_brir_outputs, recover_brir_outputs},
};
use impulcifer_types::constants::{HEXADECAGONAL_TRACK_ORDER, SPEAKER_NAMES};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "impulcifer-p17-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn golden() -> Value {
    serde_json::from_str(include_str!(
        "../../../tests/migration/goldens/p17_recovery.json"
    ))
    .unwrap()
}
fn map_strings(value: &Value, f: &impl Fn(&str) -> String) -> Value {
    match value {
        Value::String(s) => json!(f(s)),
        Value::Array(a) => Value::Array(a.iter().map(|v| map_strings(v, f)).collect()),
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, v)| (k.clone(), map_strings(v, f)))
                .collect(),
        ),
        _ => value.clone(),
    }
}
fn normalize(value: &Value, root: &Path) -> Value {
    // Results carry canonical paths: macOS resolves the /var temp directory to
    // /private/var and Windows canonicalization adds the \\?\ prefix the
    // crate strips again. Replace the canonical form first so no
    // "/private$ROOT" remains.
    let mut roots = vec![root.to_str().unwrap().to_owned()];
    if let Ok(canonical) = fs::canonicalize(root) {
        let text = canonical.to_string_lossy();
        let text = text
            .strip_prefix(r"\\?\")
            .map(str::to_owned)
            .unwrap_or_else(|| text.into_owned());
        if !roots.contains(&text) {
            roots.insert(0, text);
        }
    }
    map_strings(value, &|s| {
        let mut s = s.to_owned();
        for r in &roots {
            s = s.replace(r, "$ROOT");
        }
        s.replace('\\', "/")
    })
}
fn signal(name: &str, count: usize) -> Vec<f64> {
    match name {
        "zero" => vec![0.0; count],
        "tiny" => {
            let mut data = vec![0.0; count];
            data[count - 1] = 1e-30;
            data
        }
        "nan" | "inf" => {
            let mut data = vec![0.0; count];
            data[0] = if name == "nan" {
                f64::NAN
            } else {
                f64::INFINITY
            };
            data
        }
        "rounding" => {
            let values = [
                -2.0,
                -1.0,
                -0.5,
                -2.5 / 2147483648.0,
                -1.5 / 2147483648.0,
                -0.5 / 2147483648.0,
                0.5 / 2147483648.0,
                1.5 / 2147483648.0,
                2.5 / 2147483648.0,
                0.5,
                1.0,
                2.0,
            ];
            (0..count).map(|i| values[i % values.len()]).collect()
        }
        _ if name.starts_with("LFE-") => vec![0.0; count],
        _ => {
            let names: Vec<_> = SPEAKER_NAMES
                .into_iter()
                .flat_map(|s| [format!("{s}-left"), format!("{s}-right")])
                .collect();
            let index = names.iter().position(|n| n == name).unwrap() + 1;
            (0..count)
                .map(|i| (((i * 17 + index * 101) % 8192) as i64 - 4096) as f64 / 65536.0)
                .collect()
        }
    }
}
fn write_double(path: &Path, rate: u32, tracks: &[Vec<f64>]) {
    let size = (tracks.len() * tracks[0].len() * 8) as u32;
    let align = (tracks.len() * 8) as u16;
    let mut bytes = Vec::new();
    bytes.extend(b"RIFF");
    bytes.extend((size + 36).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(3u16.to_le_bytes());
    bytes.extend((tracks.len() as u16).to_le_bytes());
    bytes.extend(rate.to_le_bytes());
    bytes.extend((rate * u32::from(align)).to_le_bytes());
    bytes.extend(align.to_le_bytes());
    bytes.extend(64u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(size.to_le_bytes());
    for i in 0..tracks[0].len() {
        for track in tracks {
            bytes.extend(track[i].to_le_bytes());
        }
    }
    fs::write(path, bytes).unwrap();
}
fn setup(root: &Path, case: &Value) -> Vec<(PathBuf, Vec<u8>)> {
    if let Some(dirs) = case["directories"].as_array() {
        for dir in dirs {
            fs::create_dir_all(root.join(dir.as_str().unwrap())).unwrap();
        }
    }
    let mut originals = Vec::new();
    for item in case["inputs"].as_array().unwrap() {
        let path = root.join(item["path"].as_str().unwrap());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        if item["broken"] == true {
            fs::write(&path, b"broken").unwrap();
        } else {
            let count = item["count"].as_u64().unwrap_or(4800) as usize;
            let rate = item["rate"].as_u64().unwrap_or(48000) as u32;
            let tracks: Vec<_> = item["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| signal(n.as_str().unwrap(), count))
                .collect();
            if item["subtype"] == "DOUBLE" {
                write_double(&path, rate, &tracks);
            } else {
                write_wav(&path, rate, &tracks, 32).unwrap();
            }
            if let Some(maps) = item["maps"].as_array() {
                for map in maps {
                    let payload = if let Some(s) = map.as_str() {
                        s.to_owned()
                    } else if map.is_array() {
                        format!("{{\"version\":1,\"tracks\":{map}}}")
                    } else {
                        map.to_string()
                    };
                    let mut file = fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open(&path)
                        .unwrap();
                    file.seek(SeekFrom::End(0)).unwrap();
                    file.write_all(b"ICHL").unwrap();
                    file.write_all(&(payload.len() as u32).to_le_bytes())
                        .unwrap();
                    file.write_all(payload.as_bytes()).unwrap();
                    if payload.len() % 2 != 0 {
                        file.write_all(&[0]).unwrap();
                    }
                    let end = file.stream_position().unwrap();
                    file.seek(SeekFrom::Start(4)).unwrap();
                    file.write_all(&((end - 8) as u32).to_le_bytes()).unwrap();
                }
            }
        }
        originals.push((path.clone(), fs::read(path).unwrap()));
    }
    originals
}
fn preserve(originals: &[(PathBuf, Vec<u8>)]) {
    for (path, bytes) in originals {
        assert_eq!(fs::read(path).unwrap(), *bytes, "{}", path.display());
    }
}
fn no_temps(root: &Path) {
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            no_temps(&path);
        } else {
            assert!(
                !path.file_name().unwrap().to_string_lossy().starts_with('.'),
                "{}",
                path.display()
            );
        }
    }
}
fn verify_written(root: &Path, case: &Value) {
    for file in case["written"].as_array().unwrap() {
        let path = PathBuf::from(
            file["path"]
                .as_str()
                .unwrap()
                .replace("$ROOT", root.to_str().unwrap()),
        );
        let bytes = fs::read(&path).unwrap();
        assert_eq!(
            sha256(&bytes),
            file["sha256"],
            "{}: {}",
            case["name"],
            path.display()
        );
        let wav = read_wav(&path).unwrap();
        assert_eq!(json!(wav.sample_rate), file["rate"]);
        assert_eq!(json!(wav.tracks.len()), file["channels"]);
        assert_eq!(json!(wav.tracks[0].len()), file["frames"]);
        let first: Vec<_> = wav.tracks.iter().map(|t| &t[..t.len().min(64)]).collect();
        let last: Vec<_> = wav
            .tracks
            .iter()
            .map(|t| &t[t.len().saturating_sub(64)..])
            .collect();
        assert_eq!(json!(first), file["first64"], "{} first", case["name"]);
        assert_eq!(json!(last), file["last64"], "{} last", case["name"]);
    }
}
#[test]
fn golden_recovery_scenarios_match_python() {
    let g = golden();
    let mut count = 0;
    for case in g["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c.get("result").is_some())
    {
        let temp = Temp::new();
        let original = setup(&temp.0, case);
        let options: RecoveryOptions = serde_json::from_value(case["options"].clone()).unwrap();
        let result =
            recover_brir_outputs(&temp.0.join(case["selected"].as_str().unwrap()), &options)
                .unwrap_or_else(|e| panic!("{}: {e:?}", case["name"]));
        assert_eq!(
            normalize(&json!(result), &temp.0),
            case["result"],
            "{}",
            case["name"]
        );
        verify_written(&temp.0, case);
        preserve(&original);
        no_temps(&temp.0);
        count += 1;
    }
    assert!(count >= 35, "only {count} success scenarios");
}
fn tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn visit(root: &Path, dir: &Path, files: &mut Vec<(PathBuf, Vec<u8>)>) {
        let mut entries: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                visit(root, &path, files);
            } else {
                files.push((
                    path.strip_prefix(root).unwrap().to_owned(),
                    fs::read(path).unwrap(),
                ));
            }
        }
    }
    let mut files = Vec::new();
    visit(root, root, &mut files);
    files
}

#[test]
fn recovery_plan_matches_the_written_result() {
    let g = golden();
    for case in g["scenarios"].as_array().unwrap() {
        let temp = Temp::new();
        setup(&temp.0, case);
        let selected = temp.0.join(case["selected"].as_str().unwrap());
        let options: RecoveryOptions = serde_json::from_value(case["options"].clone()).unwrap();
        let before = tree(&temp.0);
        let planned = plan_brir_outputs(&selected, &options);
        assert_eq!(
            tree(&temp.0),
            before,
            "{} changed during plan",
            case["name"]
        );
        let recovered = recover_brir_outputs(&selected, &options);
        match (planned, recovered) {
            (Ok(plan), Ok(result)) => {
                assert_eq!(
                    plan.planned_files
                        .iter()
                        .map(|file| &file.path)
                        .collect::<Vec<_>>(),
                    result.created_files.iter().collect::<Vec<_>>(),
                    "{}",
                    case["name"]
                );
                assert_eq!(plan.existing_files, result.existing_files);
                assert_eq!(plan.sample_rate, result.sample_rate);
                assert_eq!(plan.sample_count, result.sample_count);
                assert_eq!(plan.speakers, result.speakers);
                assert_eq!(plan.source_kind, result.source_kind);
            }
            (Err(plan), Err(recovery)) => assert_eq!(plan.code, recovery.code, "{}", case["name"]),
            pair => panic!("{} plan/recovery mismatch: {pair:?}", case["name"]),
        }
    }
}

#[test]
fn golden_recovery_errors_match_python() {
    let g = golden();
    let mut count = 0;
    for case in g["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c.get("error").is_some() && c.get("fault").is_none())
    {
        let temp = Temp::new();
        let original = setup(&temp.0, case);
        let options: RecoveryOptions = serde_json::from_value(case["options"].clone()).unwrap();
        let error =
            recover_brir_outputs(&temp.0.join(case["selected"].as_str().unwrap()), &options)
                .unwrap_err();
        let actual = normalize(&json!(error), &temp.0);
        if case["library_reason"] == true {
            assert_eq!(actual["code"], case["error"]["code"]);
            assert_eq!(actual["message"], case["error"]["message"]);
            assert_eq!(actual["details"]["path"], case["error"]["details"]["path"]);
            assert!(
                actual["details"]["reason"]
                    .as_str()
                    .is_some_and(|s| !s.is_empty())
            );
            // Preserve both diagnostics: Rust's strict RIFF reader is not libsndfile.
            eprintln!(
                "{} library diagnostic difference: Rust={} Python={}",
                case["name"], actual["details"]["reason"], case["error"]["details"]["reason"]
            );
        } else {
            assert_eq!(actual, case["error"], "{}", case["name"]);
        }
        preserve(&original);
        no_temps(&temp.0);
        count += 1;
    }
    assert!(count >= 40, "only {count} error scenarios");
}
fn service(temp: &Temp) -> ImpulciferService {
    ImpulciferService::with_dependencies(
        Box::new(NoopHost),
        temp.0.join("settings.json"),
        impulcifer_audio_io::default_backend(),
        JobRegistry::new(),
        temp.0.clone(),
    )
}
fn poll(service: &ImpulciferService, id: &Value) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let result = service.call("poll_job", vec![id.clone(), json!(0)]);
        assert_eq!(result["ok"], true, "{result}");
        if ["succeeded", "failed", "cancelled"]
            .contains(&result["data"]["job"]["status"].as_str().unwrap())
        {
            return result["data"].clone();
        }
        assert!(Instant::now() < deadline, "recovery did not terminate");
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn recovery_ipc_start_and_poll_round_trip() {
    let g = golden();
    let temp = Temp::new();
    let svc = service(&temp);
    for case in g["validation"].as_array().unwrap() {
        let request = map_strings(&case["request"], &|s| {
            s.replace("$ROOT", temp.0.to_str().unwrap())
        });
        if case["response"]["ok"] == true {
            continue;
        }
        let actual = svc.call("start_output_recovery", vec![request]);
        assert_eq!(normalize(&actual, &temp.0), case["response"]);
    }
    for name in [
        "hrir_only",
        "compact_output",
        "both_mismatch",
        "empty",
        "silent_split",
    ] {
        let case = g["scenarios"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == name)
            .unwrap();
        let temp = Temp::new();
        let original = setup(&temp.0, case);
        let svc = service(&temp);
        let mut request = case["options"].clone();
        request["dir_path"] = json!(temp.0);
        let started = svc.call("start_output_recovery", vec![request]);
        assert_eq!(started["ok"], true, "{started}");
        let job = &started["data"]["job"];
        assert_eq!(job["kind"], "output_recovery");
        assert_eq!(job["cancellable"], false);
        let data = poll(&svc, &job["job_id"]);
        if let Some(result) = case.get("result") {
            assert_eq!(data["job"]["status"], "succeeded");
            assert_eq!(normalize(&data["job"]["result"], &temp.0), *result);
            verify_written(&temp.0, case);
        } else {
            assert_eq!(data["job"]["status"], "failed");
            let mut error = case["error"].clone();
            error["retryable"] = json!(false);
            assert_eq!(normalize(&data["job"]["error"], &temp.0), error);
        }
        assert_eq!(
            data["events"].as_array().unwrap().last().unwrap()["payload"]["status"],
            data["job"]["status"]
        );
        let replay = svc.call(
            "poll_job",
            vec![job["job_id"].clone(), data["next_seq"].clone()],
        );
        assert_eq!(replay["data"]["events"], json!([]));
        preserve(&original);
        no_temps(&temp.0);
    }
}
#[test]
fn ipc_plan_output_recovery_shape() {
    let g = golden();
    let case = g["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "hrir_only")
        .unwrap();
    let temp = Temp::new();
    setup(&temp.0, case);
    let svc = service(&temp);
    let mut request = case["options"].clone();
    request["dir_path"] = json!(temp.0);
    let plan = svc.call("plan_output_recovery", vec![request.clone()]);
    assert_eq!(plan["ok"], true, "{plan}");
    let mut keys: Vec<_> = plan["data"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "existing_files",
            "hangloose_dir",
            "output_dir",
            "planned_files",
            "sample_count",
            "sample_rate",
            "source_kind",
            "source_path",
            "speakers"
        ]
    );
    let mut file_keys: Vec<_> = plan["data"]["planned_files"][0]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    file_keys.sort();
    assert_eq!(file_keys, ["channels", "kind", "path", "speaker"]);

    let empty = Temp::new();
    let empty_service = service(&empty);
    assert_eq!(
        empty_service.call("plan_output_recovery", vec![json!({"dir_path":empty.0})])["error"]["code"],
        "NO_RECOVERY_SOURCE"
    );
    assert_eq!(
        empty_service.call(
            "plan_output_recovery",
            vec![json!({"dir_path":empty.0.join("absent")})]
        )["error"]["code"],
        "FILE_NOT_FOUND"
    );

    let jobs = JobRegistry::new();
    let busy_service = ImpulciferService::with_dependencies(
        Box::new(NoopHost),
        temp.0.join("settings-busy.json"),
        impulcifer_audio_io::default_backend(),
        jobs.clone(),
        temp.0.clone(),
    );
    let (send, receive) = std::sync::mpsc::channel();
    jobs.start(
        impulcifer_types::job::JobKind::Recording,
        false,
        move |_| {
            receive.recv_timeout(Duration::from_secs(10)).unwrap();
            Ok(json!({}))
        },
    )
    .unwrap();
    assert_eq!(
        busy_service.call("plan_output_recovery", vec![request])["ok"],
        true
    );
    send.send(()).unwrap();
}

#[test]
fn recovery_never_overwrites_existing_outputs() {
    let g = golden();
    let case = g["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "both_consistent")
        .unwrap();
    let temp = Temp::new();
    let originals = setup(&temp.0, case);
    for _ in 0..2 {
        assert!(
            recover_brir_outputs(&temp.0, &RecoveryOptions::default())
                .unwrap()
                .created_files
                .is_empty()
        );
    }
    preserve(&originals);
}
#[test]
fn recovery_cleans_up_partial_writes_on_failure() {
    let temp = Temp::new();
    let tracks: Vec<_> = HEXADECAGONAL_TRACK_ORDER
        .iter()
        .map(|n| {
            if n.starts_with("FL-") {
                signal(n, 4800)
            } else {
                vec![0.0; 4800]
            }
        })
        .collect();
    write_wav(&temp.0.join("hrir.wav"), 48000, &tracks, 32).unwrap();
    let before = fs::read(temp.0.join("hrir.wav")).unwrap();
    // Combined output stages successfully; the later split destination fails.
    fs::write(temp.0.join("Hangloose"), b"not a directory").unwrap();
    let error = recover_brir_outputs(
        &temp.0,
        &RecoveryOptions {
            include_hangloose: true,
            remove_silent_channels: false,
        },
    )
    .unwrap_err();
    assert_eq!(json!(error.code), "OUTPUT_WRITE_FAILED");
    assert!(!temp.0.join("hesuvi.wav").exists());
    assert_eq!(fs::read(temp.0.join("hrir.wav")).unwrap(), before);
    assert_eq!(
        fs::read(temp.0.join("Hangloose")).unwrap(),
        b"not a directory"
    );
    no_temps(&temp.0);
}

// Same dependency-free SHA256 helper as the neighbouring recording tests.
fn sha256(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut data = bytes.to_vec();
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&((bytes.len() as u64) * 8).to_be_bytes());
    for chunk in data.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes(word.try_into().unwrap());
        }
        for i in 16..64 {
            let x = w[i - 15];
            let y = w[i - 2];
            let s0 = x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3);
            let s1 = y.rotate_right(17) ^ y.rotate_right(19) ^ (y >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut z] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let t1 = z
                .wrapping_add(s1)
                .wrapping_add((e & f) ^ (!e & g))
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let t2 = s0.wrapping_add((a & b) ^ (a & c) ^ (b & c));
            z = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (value, add) in h.iter_mut().zip([a, b, c, d, e, f, g, z]) {
            *value = value.wrapping_add(add);
        }
    }
    h.iter().map(|n| format!("{n:08x}")).collect()
}
