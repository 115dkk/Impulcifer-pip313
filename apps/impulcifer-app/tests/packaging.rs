#![forbid(unsafe_code)]

use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    // Preserve the ordinary absolute path: PowerShell providers reject the
    // extended-length prefix introduced by Windows canonicalize().
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn source_data_files() -> BTreeSet<String> {
    std::fs::read_dir(repo().join("data"))
        .unwrap()
        .map(Result::unwrap)
        .filter(|entry| entry.file_type().unwrap().is_file())
        .map(|entry| entry.file_name().into_string().unwrap())
        .filter(|name| {
            (name.starts_with("sweep") && name.ends_with(".wav"))
                || (name.starts_with("harman") && name.ends_with(".csv"))
        })
        .collect()
}

#[test]
fn bundle_config_contract() {
    let config: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let bundle = &config["bundle"];
    assert_eq!(bundle["active"], true);
    assert_eq!(bundle["targets"], json!([]));
    assert_eq!(bundle["createUpdaterArtifacts"], true);
    assert_eq!(
        bundle["resources"],
        json!({"../../data/sweep*.wav": "data/", "../../data/harman*.csv": "data/"})
    );
    assert_eq!(config["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(
        config["plugins"]["updater"],
        json!({
            "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IERDMDJDN0QxN0RBNjFFMkYKUldRdkhxWjkwY2NDM0d3RXN4d1VJRlE4UFNQZHRESDY0TE56L210TlliVm1ROEE4bGtNVEpGRTUK",
            "endpoints": ["https://github.com/115dkk/Impulcifer-pip313/releases/latest/download/latest.json"]
        })
    );
    for icon in bundle["icon"].as_array().unwrap() {
        assert!(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(icon.as_str().unwrap())
                .is_file()
        );
    }
    assert_eq!(
        std::fs::read(repo().join("logo/pulse.ico")).unwrap(),
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("icons/icon.ico")).unwrap()
    );
    let expected = source_data_files();
    assert!(!expected.is_empty());
    assert!(
        expected
            .iter()
            .all(|name| !name.contains('/') && !name.contains("master"))
    );
    #[cfg(windows)]
    {
        let output = std::process::Command::new("pwsh")
            .args(["-NoProfile", "-File"])
            .arg(repo().join("build_scripts/pack_velopack.ps1"))
            .arg("-ListDataFiles")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let staged: BTreeSet<String> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(staged, expected);
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::ffi::OsString;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    };
    use std::time::Duration;
    use velopack::locator::{VelopackLocator, VelopackLocatorConfig};
    use velopack::{UpdateCheck, UpdateManager, sources::HttpSource};

    struct TemporaryRoot(PathBuf);

    impl TemporaryRoot {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "impulcifer-p21-upgrade-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for TemporaryRoot {
        fn drop(&mut self) {
            if let Err(error) = std::fs::remove_dir_all(&self.0) {
                eprintln!("TEMP cleanup failed at {}: {error}", self.0.display());
            }
        }
    }

    struct LocalFeed {
        address: SocketAddr,
        stop: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<Result<(), String>>>,
    }

    impl LocalFeed {
        fn new(files: Vec<(String, PathBuf)>, log_path: &Path) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let address = listener.local_addr().unwrap();
            let stop = Arc::new(AtomicBool::new(false));
            let stopping = Arc::clone(&stop);
            let mut log = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path)
                .unwrap();
            let thread = std::thread::spawn(move || {
                while !stopping.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            if stopping.load(Ordering::Acquire) {
                                break;
                            }
                            let started = std::time::Instant::now();
                            let result = Self::serve(stream, &files);
                            let message =
                                format!("local feed: {result:?}; elapsed {:?}", started.elapsed());
                            eprintln!("{message}");
                            writeln!(log, "{message}")
                                .map_err(|error| format!("local feed log: {error:?}"))?;
                            result?;
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        Err(error) => return Err(format!("local feed accept: {error:?}")),
                    }
                }
                Ok(())
            });
            Self {
                address,
                stop,
                thread: Some(thread),
            }
        }

        fn serve(mut stream: TcpStream, files: &[(String, PathBuf)]) -> Result<String, String> {
            let mut request = String::new();
            let mut phase = "configure socket";
            let mut copied = 0;
            let mut expected = 0;
            let result = (|| -> std::io::Result<()> {
                // Windows accepted sockets inherit the listener's mode.
                stream.set_nonblocking(false)?;
                stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                stream.set_write_timeout(Some(Duration::from_secs(30)))?;
                phase = "read request";
                let mut reader = BufReader::new(&mut stream);
                reader.read_line(&mut request)?;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line)? == 0 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "incomplete HTTP request headers",
                        ));
                    }
                    if line == "\r\n" {
                        break;
                    }
                }
                let name = request
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("")
                    .split('?')
                    .next()
                    .unwrap_or("")
                    .trim_start_matches('/');
                if let Some((_, path)) = files.iter().find(|(file, _)| file == name) {
                    phase = "open response file";
                    let mut file = std::fs::File::open(path)?;
                    expected = file.metadata()?.len();
                    phase = "write response headers";
                    stream.write_all(
                        format!("HTTP/1.1 200 OK\r\nContent-Length: {expected}\r\nConnection: keep-alive\r\n\r\n").as_bytes(),
                    )?;
                    phase = "write response body";
                    let mut buffer = [0; 64 * 1024];
                    loop {
                        let size = file.read(&mut buffer)?;
                        if size == 0 {
                            break;
                        }
                        stream.write_all(&buffer[..size])?;
                        copied += size as u64;
                    }
                    if copied != expected {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "response file size changed during transfer",
                        ));
                    }
                } else {
                    phase = "write 404 response";
                    stream.write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: keep-alive\r\n\r\n",
                    )?;
                }
                // With Connection: close, the SDK intermittently received EOF
                // before Content-Length even after every server write succeeded;
                // shutdown(Write) did not fix it. Use length-delimited keep-alive
                // and let the SDK finish consuming the response before closing.
                // Velopack 1.2.0 creates an agent per download (download.rs), so
                // dropping that agent closes the connection; it never reuses it
                // for a second request. It reads/writes synchronously without
                // throttling. Allow the same 30s to drain as for a blocked write.
                phase = "wait for client EOF";
                stream.set_read_timeout(Some(Duration::from_secs(30)))?;
                match stream.read(&mut [0; 1]) {
                    Ok(0) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {
                        // Windows can reset when the SDK drops its agent. Only
                        // accept this AFTER all bytes were written; the caller
                        // still checks the SDK result and exact downloaded body.
                        phase = "client reset after complete response";
                    }
                    Err(error) => return Err(error),
                    Ok(_) => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "unexpected data after GET request",
                        ));
                    }
                }
                Ok(())
            })();
            let detail = format!(
                "{}: {phase}, {copied}/{expected} body bytes",
                request.trim()
            );
            result
                .map(|()| detail.clone())
                .map_err(|error| format!("{detail}: {error:?}"))
        }

        fn finish(&mut self) -> Result<(), String> {
            self.stop.store(true, Ordering::Release);
            if let Some(thread) = self.thread.take() {
                thread
                    .join()
                    .map_err(|error| format!("local feed thread panicked: {error:?}"))?
            } else {
                Ok(())
            }
        }
    }

    impl Drop for LocalFeed {
        fn drop(&mut self) {
            if let Err(error) = self.finish() {
                if std::thread::panicking() {
                    eprintln!("{error}");
                } else {
                    panic!("{error}");
                }
            }
        }
    }

    #[test]
    fn local_feed_sdk_download_drains_body_before_close() {
        let temp = TemporaryRoot::new();
        let source = temp.0.join("source.bin");
        // Larger than the SDK's 2 MiB buffer and the socket send queue.
        let body = vec![0x5a; 16 * 1024 * 1024];
        std::fs::write(&source, &body).unwrap();
        let log_path = repo().join(format!("target/p21/feed-drain-{}.log", std::process::id()));
        std::fs::create_dir_all(log_path.parent().unwrap()).unwrap();
        let mut server = LocalFeed::new(vec![("package.bin".into(), source)], &log_path);
        for index in 0..16 {
            let destination = temp.0.join(format!("download-{index}.bin"));
            let mut progress = Vec::new();
            let downloaded = velopack::download::download_url_to_file(
                &format!("http://{}/package.bin", server.address),
                &destination,
                |percent| {
                    progress.push(percent);
                    // Exercise a consumer that occasionally stops reading.
                    std::thread::sleep(Duration::from_millis(1));
                },
            );
            if let Err(error) = downloaded {
                panic!(
                    "SDK download: {error:?}; received {:?} bytes; progress {progress:?}; local feed: {:?}",
                    std::fs::metadata(&destination).map(|metadata| metadata.len()),
                    server.finish()
                );
            }
            assert_eq!(progress.last(), Some(&100));
            assert_eq!(std::fs::read(destination).unwrap(), body);
        }
        server.finish().unwrap();
    }

    #[test]
    fn local_feed_propagates_response_file_errors() {
        let temp = TemporaryRoot::new();
        let log_path = repo().join(format!("target/p21/feed-error-{}.log", std::process::id()));
        std::fs::create_dir_all(log_path.parent().unwrap()).unwrap();
        let mut server = LocalFeed::new(
            vec![("missing.bin".into(), temp.0.join("missing.bin"))],
            &log_path,
        );
        let downloaded = velopack::download::download_url_as_string(&format!(
            "http://{}/missing.bin",
            server.address
        ));
        assert!(downloaded.is_err());
        let error = server.finish().unwrap_err();
        assert!(error.contains("GET /missing.bin"), "{error}");
        assert!(error.contains("open response file"), "{error}");
        assert!(error.contains("NotFound"), "{error}");
        assert!(
            std::fs::read_to_string(log_path)
                .unwrap()
                .contains(&format!("{error:?}"))
        );
    }

    #[test]
    #[ignore = "builds a Windows package; SDK-only TEMP simulation, never installs or applies"]
    fn velopack_upgrade_from_2x_windows() {
        let output = std::process::Command::new("pwsh")
            .args(["-NoProfile", "-File"])
            .arg(repo().join("build_scripts/pack_velopack.ps1"))
            .output()
            .expect("PowerShell 7 is required");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let log = format!("{stdout}\n{stderr}");
        let log_path = repo().join(format!("target/p21/upgrade-{}.log", std::process::id()));
        std::fs::create_dir_all(log_path.parent().unwrap()).unwrap();
        std::fs::write(&log_path, &log).unwrap();
        assert!(output.status.success(), "packaging failed:\n{log}");
        let result: Value = serde_json::from_str(
            stdout
                .lines()
                .find_map(|line| line.strip_prefix("P21_RESULT="))
                .expect("pack result"),
        )
        .unwrap();
        assert_eq!(result["version"], env!("CARGO_PKG_VERSION"));
        let stage = PathBuf::from(result["stage"].as_str().unwrap());
        let output_dir = PathBuf::from(result["output"].as_str().unwrap());
        let expected = source_data_files();
        let actual: BTreeSet<_> = std::fs::read_dir(stage.join("data"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(actual, expected);
        let stage_roots: BTreeSet<_> = std::fs::read_dir(&stage)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(
            stage_roots,
            BTreeSet::from(["data".into(), "impulcifer-app.exe".into()])
        );
        let feed_path = output_dir.join("releases.win.json");
        let feed: Value = serde_json::from_slice(&std::fs::read(&feed_path).unwrap()).unwrap();
        let assets = feed["Assets"].as_array().unwrap();
        assert_eq!(
            assets.len(),
            1,
            "unique pack output must contain only the full release"
        );
        let asset = &assets[0];
        assert_eq!(asset["Type"], "Full");
        assert_eq!(asset["PackageId"], "Impulcifer");
        assert_eq!(asset["Version"], env!("CARGO_PKG_VERSION"));
        let filename = asset["FileName"].as_str().unwrap();
        assert_eq!(Path::new(filename).file_name().unwrap(), filename);
        let mut server = LocalFeed::new(
            vec![
                ("releases.win.json".into(), feed_path),
                (filename.into(), output_dir.join(filename)),
            ],
            &log_path,
        );

        let temp = TemporaryRoot::new();
        let root = temp.0.join("Impulcifer");
        let current = root.join("current");
        let packages = root.join("packages");
        std::fs::create_dir_all(&current).unwrap();
        std::fs::create_dir(&packages).unwrap();
        // An inert marker meets the locator's existence check. Never execute it.
        let update_exe = root.join("Update.exe");
        std::fs::write(&update_exe, b"P21 inert locator marker; never execute").unwrap();
        let manifest_path = current.join("sq.version");
        std::fs::write(
            &manifest_path,
            r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://schemas.microsoft.com/packaging/2010/07/nuspec.xsd"><metadata>
<id>Impulcifer</id><version>2.13.3</version><title>Modern Impulcifer</title>
<mainExe>ImpulciferGUI.exe</mainExe><os>win</os><channel>win</channel>
</metadata></package>"#,
        )
        .unwrap();
        let config = VelopackLocatorConfig {
            RootAppDir: root.clone(),
            UpdateExePath: update_exe.clone(),
            PackagesDir: packages.clone(),
            ManifestPath: manifest_path,
            CurrentBinaryDir: current.clone(),
            IsPortable: false,
        };
        let locator = VelopackLocator::new(&config).unwrap();
        assert_eq!(locator.get_root_dir(), root);
        assert_eq!(locator.get_packages_dir(), packages);
        assert_eq!(locator.get_update_path(), update_exe);
        let kind = impulcifer_service::update::install_kind::detect(
            "windows",
            &locator.get_main_exe_path(),
            None,
        );
        assert_eq!(kind.as_str(), "velopack");
        let manager = UpdateManager::new(
            HttpSource::new(format!("http://{}", server.address)),
            Some(velopack::UpdateOptions {
                ExplicitChannel: Some("win".into()),
                ..Default::default()
            }),
            Some(config),
        )
        .unwrap();
        assert_eq!(manager.get_current_version_as_string(), "2.13.3");
        assert_eq!(manager.get_app_id(), "Impulcifer");
        assert!(!manager.get_is_portable());
        let UpdateCheck::UpdateAvailable(update) = manager.check_for_updates().unwrap() else {
            panic!("3.x update not available");
        };
        assert_eq!(update.TargetFullRelease.Version, env!("CARGO_PKG_VERSION"));
        assert!(
            update.BaseRelease.is_none() && update.DeltasToTarget.is_empty(),
            "never invoke the delta patch helper"
        );
        let (send, receive) = mpsc::channel();
        let progress = std::thread::scope(|scope| {
            let reader = scope.spawn(move || receive.into_iter().collect::<Vec<i16>>());
            let downloaded = manager.download_updates(&update, Some(send));
            let progress = reader.join().unwrap();
            let served = server.finish();
            assert!(
                downloaded.is_ok() && served.is_ok(),
                "SDK download: {downloaded:?}; local feed: {served:?}; log: {}",
                log_path.display()
            );
            progress
        });
        assert!(!progress.is_empty());
        assert!(progress.iter().all(|percent| (0..=100).contains(percent)));
        let package = packages.join(filename);
        assert_eq!(package.parent().unwrap(), packages);
        assert!(package.starts_with(&temp.0));
        assert_eq!(
            std::fs::metadata(&package).unwrap().len(),
            asset["Size"].as_u64().unwrap()
        );
        let mut bundle = velopack::bundle::load_bundle_from_file(&package).unwrap();
        let names = bundle.get_file_names().unwrap();
        assert!(
            names
                .iter()
                .any(|name| name == "lib/app/impulcifer-app.exe")
        );
        let data_names: BTreeSet<_> = names
            .iter()
            .filter_map(|name| name.strip_prefix("lib/app/data/"))
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect();
        assert_eq!(data_names, expected);
        assert!(
            !names
                .iter()
                .any(|name| name.contains("/demo/") || name.contains("/masters/"))
        );
        let packed_manifest = bundle.read_manifest().unwrap();
        assert_eq!(packed_manifest.id, "Impulcifer");
        assert_eq!(
            packed_manifest.version.to_string(),
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(packed_manifest.main_exe, "impulcifer-app.exe");
        let extracted = temp.0.join("inspected-sq.version");
        bundle
            .extract_zip_predicate_to_path(|name| name == "lib/app/sq.version", &extracted)
            .unwrap();
        let packed_sq = velopack::bundle::read_manifest_from_string(
            &std::fs::read_to_string(&extracted).unwrap(),
        )
        .unwrap();
        assert_eq!(packed_sq.id, "Impulcifer");
        assert_eq!(packed_sq.version.to_string(), env!("CARGO_PKG_VERSION"));
        for name in &expected {
            let inspected = temp.0.join("inspected-data").join(name);
            bundle
                .extract_zip_predicate_to_path(
                    |entry| entry == format!("lib/app/data/{name}"),
                    &inspected,
                )
                .unwrap();
            assert_eq!(
                std::fs::read(&inspected).unwrap(),
                std::fs::read(repo().join("data").join(name)).unwrap()
            );
        }
        // SDK 1.2.0 manager.rs constructs these arguments, then immediately spawns.
        // There is no public prepare/dry-run API. This is a source-verified plan only.
        let planned: Vec<OsString> = vec![
            locator.get_update_path().into_os_string(),
            "apply".into(),
            "--package".into(),
            package.clone().into_os_string(),
            "--waitPid".into(),
            std::process::id().to_string().into(),
            "--root".into(),
            locator.get_root_dir().into_os_string(),
            "--packageDir".into(),
            locator.get_packages_dir().into_os_string(),
        ];
        assert_eq!(Path::new(&planned[3]), package);
        assert_eq!(Path::new(&planned[7]), root);
        assert_eq!(Path::new(&planned[9]), packages);
        assert_eq!(manager.get_current_version_as_string(), "2.13.3");
        let report = format!(
            "simulated 2.x locator version: 2.13.3\navailable 3.x: {}\ninstall_kind: {} (simulated)\ndownload: {}\nprogress callbacks: {:?}\nplanned command (source-verified, not SDK interception): {:?}\napply NOT RUN\ninstalled after apply NOT MEASURED\nuninstall NOT NEEDED (nothing installed)\ntouched paths: {} (retained package stage/output), {} (TEMP fixture and SDK files, including extracted Update.exe), {} (transcript)\n",
            env!("CARGO_PKG_VERSION"),
            kind.as_str(),
            package.display(),
            progress,
            planned,
            stage.parent().unwrap().display(),
            temp.0.display(),
            log_path.display()
        );
        drop(bundle);
        drop(manager);
        drop(locator);
        drop(server);
        let temp_path = temp.0.clone();
        drop(temp);
        assert!(!temp_path.exists(), "TEMP fixture cleanup failed");
        let report =
            format!("{report}cleanup: TEMP fixture removed; target/p21 artifacts retained\n");
        let mut transcript = std::fs::OpenOptions::new()
            .append(true)
            .open(&log_path)
            .unwrap();
        transcript.write_all(report.as_bytes()).unwrap();
        println!("{report}");
    }
}
