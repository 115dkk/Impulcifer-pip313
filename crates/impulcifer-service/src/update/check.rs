#![forbid(unsafe_code)]

use super::UpdateOptions;
use serde_json::{Value, json};

pub fn normalize_version(version: &str) -> String {
    let version = version.trim_start_matches('v');
    let bytes = version.as_bytes();
    let mut end = 0;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end == 0 {
        return version.to_owned();
    }
    while end + 1 < bytes.len() && bytes[end] == b'.' && bytes[end + 1].is_ascii_digit() {
        end += 1;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
    }
    version[..end].to_owned()
}

pub fn is_newer_version(current: &str, latest: &str) -> bool {
    let current = normalize_version(current);
    let latest = normalize_version(latest);
    // Python integers are unbounded. Compare numeric releases without reducing
    // components to machine integers (PEP 440 pads trailing zeros).
    let numeric = |value: &str| {
        !value.is_empty()
            && value
                .split('.')
                .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
    };
    if numeric(&current) && numeric(&latest) {
        let current: Vec<_> = current.split('.').collect();
        let latest: Vec<_> = latest.split('.').collect();
        for index in 0..current.len().max(latest.len()) {
            let a = current.get(index).unwrap_or(&"0").trim_start_matches('0');
            let b = latest.get(index).unwrap_or(&"0").trim_start_matches('0');
            let ordering = b.len().cmp(&a.len()).then_with(|| b.cmp(a));
            if !ordering.is_eq() {
                return ordering.is_gt();
            }
        }
        return false;
    }
    let current = current.parse::<pep440_rs::Version>();
    let latest = latest.parse::<pep440_rs::Version>();
    matches!((current, latest), (Ok(current), Ok(latest)) if latest > current)
}

pub fn download_url(release: &Value, platform: &str) -> Value {
    let Some(assets) = release["assets"].as_array() else {
        return Value::Null;
    };
    if platform == "linux" {
        for extension in [".appimage", ".deb", ".rpm"] {
            for asset in assets {
                if asset["name"]
                    .as_str()
                    .unwrap_or("")
                    .to_lowercase()
                    .ends_with(extension)
                {
                    return asset["browser_download_url"].clone();
                }
            }
        }
    } else {
        for asset in assets {
            let name = asset["name"].as_str().unwrap_or("").to_lowercase();
            if (platform == "windows" && name.ends_with(".exe") && name.contains("setup"))
                || (platform == "darwin" && (name.ends_with(".dmg") || name.ends_with(".pkg")))
            {
                return asset["browser_download_url"].clone();
            }
        }
    }
    Value::Null
}

pub fn release_payload(release: &Value, current: &str, platform: &str) -> Value {
    let raw = release["tag_name"].as_str().unwrap_or("");
    let latest = normalize_version(raw);
    let url = if is_newer_version(current, raw) {
        download_url(release, platform)
    } else {
        Value::Null
    };
    let available = url.as_str().is_some_and(|url| !url.is_empty());
    json!({
        "update_available": available,
        "current_version": current,
        "latest_version": if latest.is_empty() { Value::Null } else { json!(latest) },
        "download_url": url,
        "release_notes": if available { release["body"].clone() } else { Value::Null },
        "release_url": release["html_url"],
    })
}

pub fn check(options: &UpdateOptions) -> Result<Value, String> {
    let mut response = options
        .agent()
        .get(&options.latest_endpoint)
        .header(
            "User-Agent",
            &format!("Impulcifer/{}", options.current_version),
        )
        .call()
        .map_err(|error| error.to_string())?;
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|error| error.to_string())?;
    let release: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    if !release.is_object() {
        return Err("Release response must be an object.".into());
    }
    Ok(release_payload(
        &release,
        &options.current_version,
        &options.platform,
    ))
}
