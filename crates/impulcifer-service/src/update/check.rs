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
    let current_base = normalize_version(current);
    let latest_base = normalize_version(latest);
    let compare_bases = || {
        // Python integers are unbounded. Compare numeric releases without reducing
        // components to machine integers (PEP 440 pads trailing zeros).
        let numeric = |value: &str| {
            !value.is_empty()
                && value
                    .split('.')
                    .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
        };
        if numeric(&current_base) && numeric(&latest_base) {
            let current: Vec<_> = current_base.split('.').collect();
            let latest: Vec<_> = latest_base.split('.').collect();
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
        let current = current_base.parse::<pep440_rs::Version>();
        let latest = latest_base.parse::<pep440_rs::Version>();
        matches!((current, latest), (Ok(current), Ok(latest)) if latest > current)
    };

    let base_result = compare_bases();
    if current_base != latest_base {
        return base_result;
    }
    let Ok(current) = current.parse::<pep440_rs::Version>() else {
        return base_result;
    };
    let Ok(latest) = latest.parse::<pep440_rs::Version>() else {
        return base_result;
    };
    match (current.pre(), latest.pre()) {
        (Some(current), Some(latest)) => latest > current,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

/// True for a PEP 440 pre-release such as `3.0.0-alpha.0` or `v3.0.0-rc1`: a
/// prerelease install must be able to see the next prerelease, which GitHub's
/// `/releases/latest` never resolves. Stable, post, dev and unparsable strings
/// are not prereleases.
pub fn is_prerelease(version: &str) -> bool {
    version
        .trim_start_matches(['v', 'V'])
        .parse::<pep440_rs::Version>()
        .map(|version| version.pre().is_some())
        .unwrap_or(false)
}

/// The version a release is shown and requested as: a prerelease keeps its
/// PEP 440 pre segment (`3.0.0-alpha.1`), anything else is the numeric base
/// exactly as 2.x reported it.
pub fn display_version(tag: &str) -> String {
    let trimmed = tag.trim_start_matches(['v', 'V']);
    if is_prerelease(trimmed) {
        trimmed.to_owned()
    } else {
        normalize_version(tag)
    }
}

/// The newest non-draft release whose tag is a PEP 440 version. GitHub orders
/// the list by creation date and rolling feeds such as `updater-3x-pre` carry
/// no version, so the choice rests on version comparison alone.
pub fn select_release(releases: &[Value]) -> Option<&Value> {
    let mut best: Option<(&Value, &str)> = None;
    for release in releases {
        if release["draft"].as_bool() == Some(true) {
            continue;
        }
        let Some(tag) = release["tag_name"].as_str() else {
            continue;
        };
        if tag
            .trim_start_matches(['v', 'V'])
            .parse::<pep440_rs::Version>()
            .is_err()
        {
            continue;
        }
        if best.is_none_or(|(_, current)| is_newer_version(current, tag)) {
            best = Some((release, tag));
        }
    }
    best.map(|(release, _)| release)
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
    let latest = display_version(raw);
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

fn fetch(options: &UpdateOptions, endpoint: &str) -> Result<Value, String> {
    let mut response = options
        .agent()
        .get(endpoint)
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
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

/// A stable install asks `/releases/latest`, which GitHub resolves to the newest
/// stable release and never to a prerelease. A prerelease install reads the
/// release list instead and takes the newest versioned release, so an alpha is
/// offered the next alpha and the following stable alike.
pub fn check(options: &UpdateOptions) -> Result<Value, String> {
    let release = if is_prerelease(&options.current_version) {
        let body = fetch(options, &options.releases_endpoint)?;
        let releases = body
            .as_array()
            .ok_or("Release list response must be an array.")?;
        select_release(releases)
            .ok_or("Release list holds no versioned release.")?
            .clone()
    } else {
        let release = fetch(options, &options.latest_endpoint)?;
        if !release.is_object() {
            return Err("Release response must be an object.".into());
        }
        release
    };
    Ok(release_payload(
        &release,
        &options.current_version,
        &options.platform,
    ))
}
