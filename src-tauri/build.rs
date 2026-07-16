#[path = "src/command_names.rs"]
mod command_names;

fn main() {
    let manifest = tauri_build::AppManifest::new().commands(command_names::APP_COMMANDS);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("failed to build Tauri application metadata");
}
