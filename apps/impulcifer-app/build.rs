#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

use std::path::Path;

/// The Windows application manifest. It asks for version 6 of the common
/// controls: the dialogs the shell links against exist in no other version.
const WINDOWS_MANIFEST: &str = "windows-app-manifest.xml";

fn variable(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

fn main() {
    let mut attributes = tauri_build::Attributes::new();
    if variable("CARGO_CFG_TARGET_OS") == "windows" && variable("CARGO_CFG_TARGET_ENV") == "msvc" {
        // tauri-build puts its manifest into the package's executables only.
        // The shell is a library (ADR 0020), so its unit tests are a program
        // of their own, and Windows ends a program that links those dialogs
        // without the manifest before `main` (STATUS_ENTRYPOINT_NOT_FOUND,
        // rust.yml run 43683166685). The linker therefore embeds the manifest
        // into everything this package links, and tauri-build leaves its copy
        // out: a program with two manifests does not link.
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        let manifest = Path::new(&variable("CARGO_MANIFEST_DIR")).join(WINDOWS_MANIFEST);
        println!("cargo:rerun-if-changed={WINDOWS_MANIFEST}");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        // The file is the whole manifest, as it was when tauri-build embedded
        // it: the linker adds no execution level of its own.
        println!("cargo:rustc-link-arg=/MANIFESTUAC:NO");
    }
    if let Err(error) = tauri_build::try_build(attributes) {
        panic!("{error:#}");
    }
}
