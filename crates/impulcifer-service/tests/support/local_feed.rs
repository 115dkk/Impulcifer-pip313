use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

pub struct TemporaryRoot(pub PathBuf);

impl TemporaryRoot {
    pub fn new() -> Self {
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

pub struct LocalFeed {
    pub address: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<Result<(), String>>>,
}

impl LocalFeed {
    pub fn new(files: Vec<(String, PathBuf)>, log_path: &Path) -> Self {
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

    pub fn finish(&mut self) -> Result<(), String> {
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
