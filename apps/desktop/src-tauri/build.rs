include!("src/command_manifest.rs");

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        // NDK r27 defaults to 4 KiB. Align the final shared library for both
        // 4 KiB and 16 KiB Android kernels without changing desktop linkage.
        println!("cargo:rustc-link-arg-cdylib=-Wl,-z,max-page-size=16384");
        println!("cargo:rustc-link-arg-cdylib=-Wl,-z,common-page-size=16384");
    }
    let manifest = tauri_build::AppManifest::new().commands(APP_COMMANDS);
    let attributes = tauri_build::Attributes::new().app_manifest(manifest);

    tauri_build::try_build(attributes).expect("failed to build NovelGainker Tauri manifest");
}
