#![forbid(unsafe_code)]

use super::{Request, UpdateOptions};
use crate::HostAdapter;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::Path;

pub(super) fn execute(
    request: &Request,
    options: &UpdateOptions,
    host: &dyn HostAdapter,
    progress: &(dyn Fn(f64, &str) + Sync),
) -> Result<(), String> {
    if request.download_url.is_empty() {
        return Err("No installer available. Please download manually from GitHub.".into());
    }
    progress(0.1, "update_downloading");
    let url = url::Url::parse(&request.download_url).map_err(|e| e.to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Installer URL must use HTTP or HTTPS.".into());
    }
    let decoded = percent_encoding::percent_decode_str(url.path()).decode_utf8_lossy();
    let filename = decoded
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("impulcifer_update");
    if filename == "." || filename == ".." || filename.contains(':') {
        return Err("Invalid installer filename.".into());
    }
    std::fs::create_dir_all(&options.download_root).map_err(|e| e.to_string())?;
    // A private, unique directory prevents concurrent downloads or symlinks from
    // substituting an unverified installer. It is retained only after opening.
    let directory = tempfile::Builder::new()
        .prefix("update-")
        .tempdir_in(&options.download_root)
        .map_err(|e| e.to_string())?;
    let path = directory.path().join(filename);
    let mut response = options
        .download_agent()
        .get(url.as_str())
        .header("User-Agent", "Impulcifer-Updater")
        .call()
        .map_err(|e| e.to_string())?;
    let total = response
        .headers()
        .get("Content-Length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0_u64;
    let mut reader = response.body_mut().as_reader();
    let mut buffer = [0_u8; 8192];
    loop {
        let count = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        file.write_all(&buffer[..count])
            .map_err(|e| e.to_string())?;
        hasher.update(&buffer[..count]);
        downloaded += count as u64;
        if total > 0 {
            let fraction = downloaded as f64 / total as f64;
            progress(
                fraction,
                &format!("Downloading: {}%", (fraction * 100.0) as u64),
            );
        }
    }
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    let sums_url = format!(
        "{}/SHA256SUMS.txt",
        request
            .download_url
            .rsplit_once('/')
            .map(|v| v.0)
            .unwrap_or("")
    );
    match options
        .agent()
        .get(&sums_url)
        .header("User-Agent", "Impulcifer-Updater")
        .call()
    {
        Err(ureq::Error::StatusCode(404)) => {}
        Err(error) => return Err(format!("Checksum file fetch failed: {error}")),
        Ok(mut response) => {
            let text = response
                .body_mut()
                .read_to_string()
                .map_err(|e| e.to_string())?;
            let expected = text
                .lines()
                .find_map(|line| {
                    let bytes = line.as_bytes();
                    if bytes.len() > 66
                        && bytes[..64].iter().all(u8::is_ascii_hexdigit)
                        && bytes[64] == b' '
                        && matches!(bytes[65], b' ' | b'*')
                        && &line[66..] == filename
                    {
                        Some(line[..64].to_ascii_lowercase())
                    } else {
                        None
                    }
                })
                .ok_or_else(|| format!("No checksum entry for {filename}"))?;
            if format!("{:x}", hasher.finalize()) != expected {
                return Err(format!("Checksum verification failed for {filename}"));
            }
        }
    }
    progress(0.9, "update_opening_installer");
    let mut opened = false;
    if options.platform == "linux" && filename.to_lowercase().ends_with(".appimage") {
        executable(&path)?;
        if let Some(current) = options.appimage.as_deref() {
            // Same-filesystem, exclusive temporary file plus atomic replacement.
            // Like 2.x, fall back to opening the download if replacement fails.
            opened = replace_appimage(&path, current)
                .and_then(|()| host.open_path(&current.to_string_lossy()))
                .is_ok();
        }
    }
    if !opened {
        host.open_path(&path.to_string_lossy())?;
    }
    let _ = directory.keep();
    Ok(())
}

fn executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn replace_appimage(download: &Path, current: &Path) -> Result<(), String> {
    let parent = current.parent().ok_or("AppImage has no parent directory")?;
    let mut replacement = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    let mut source = std::fs::File::open(download).map_err(|e| e.to_string())?;
    std::io::copy(&mut source, &mut replacement).map_err(|e| e.to_string())?;
    executable(replacement.path())?;
    replacement
        .as_file()
        .sync_all()
        .map_err(|e| e.to_string())?;
    replacement.persist(current).map_err(|e| e.to_string())?;
    Ok(())
}
