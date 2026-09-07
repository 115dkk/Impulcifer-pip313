#![forbid(unsafe_code)]

use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallKind {
    Velopack,
    Tauri,
    Dev,
}

impl InstallKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Velopack => "velopack",
            Self::Tauri => "tauri",
            Self::Dev => "dev",
        }
    }
}

/// Pure probe: tests supply layouts and APPIMAGE without changing process state.
pub fn detect(platform: &str, executable: &Path, appimage: Option<&Path>) -> InstallKind {
    if platform == "windows"
        && executable
            .parent()
            .and_then(Path::parent)
            .is_some_and(|root| root.join("Update.exe").exists())
    {
        return InstallKind::Velopack;
    }
    if platform == "linux" && appimage.is_some() {
        return InstallKind::Tauri;
    }
    if platform == "darwin"
        && let Some(macos) = executable.parent()
        && macos.file_name().is_some_and(|name| name == "MacOS")
        && let Some(contents) = macos.parent()
        && contents.file_name().is_some_and(|name| name == "Contents")
        && contents
            .parent()
            .is_some_and(|bundle| bundle.extension().is_some_and(|ext| ext == "app"))
    {
        return InstallKind::Tauri;
    }
    InstallKind::Dev
}
