include!("src/command_manifest.rs");

fn main() {
    let manifest = tauri_build::AppManifest::new().commands(APP_COMMANDS);
    let attributes = tauri_build::Attributes::new().app_manifest(manifest);

    tauri_build::try_build(attributes).expect("failed to build NovelGainker Tauri manifest");
}
