fn main() {
    let mut windows = tauri_build::WindowsAttributes::new();
    if embed_manifest_everywhere() {
        // We embed the manifest ourselves (below), so Tauri mustn't add a second one.
        windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    }
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run tauri build script");
}

/// Tauri embeds its Windows manifest only into the app binary, so `cargo test`
/// binaries that create a (mock) webview crash with STATUS_ENTRYPOINT_NOT_FOUND.
/// Passing the manifest as a linker argument applies it to every target instead.
fn embed_manifest_everywhere() -> bool {
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if !(windows && msvc) {
        return false;
    }
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    true
}
