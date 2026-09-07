#![forbid(unsafe_code)]

fn main() {
    println!("cargo:rerun-if-env-changed=RUSTC");
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = std::process::Command::new(rustc)
        .arg("--version")
        .output()
        .expect("build-time rustc version probe failed");
    assert!(
        output.status.success(),
        "build-time rustc version probe failed"
    );
    let version = String::from_utf8(output.stdout).expect("rustc version is UTF-8");
    println!(
        "cargo:rustc-env=IMPULCIFER_RUSTC_VERSION={}",
        version.trim()
    );
}
