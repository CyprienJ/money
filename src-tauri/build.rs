fn main() {
    // Rebuild when a migration is added, since `sqlx::migrate!` embeds them at compile time.
    println!("cargo:rerun-if-changed=migrations");

    // tauri-build only embeds its manifest in the app binary, so test executables lack the
    // Common Controls v6 dependency and fail with STATUS_ENTRYPOINT_NOT_FOUND. Embed it
    // ourselves through linker arguments, which apply to every target, tests included.
    let windows = if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        embed_windows_manifest();
        tauri_build::WindowsAttributes::new_without_app_manifest()
    } else {
        tauri_build::WindowsAttributes::new()
    };
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run tauri-build");
}

fn embed_windows_manifest() {
    let manifest = std::env::current_dir()
        .expect("no current directory")
        .join("windows-app-manifest.xml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    // Turn linker warnings into errors, so a broken manifest fails the build.
    println!("cargo:rustc-link-arg=/WX");
}
