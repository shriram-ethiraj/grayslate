#[path = "src/command_names.rs"]
mod command_names;

fn main() {
    // `AppManifest::commands` stores the slice for `'static`, so each branch
    // must yield a `'static` slice. The base list already is; the e2e list is
    // concatenated and leaked (harmless in a short-lived build script).
    //
    // The `e2e` feature exposes test-only IPC shims. Generate their ACL
    // permissions only for that build so a release binary never carries them.
    // Cargo exposes enabled features to build scripts as `CARGO_FEATURE_<NAME>`.
    let commands: &'static [&'static str] = if std::env::var_os("CARGO_FEATURE_E2E").is_some() {
        let mut all: Vec<&'static str> = command_names::APP_COMMANDS.to_vec();
        all.extend_from_slice(command_names::E2E_COMMANDS);
        Box::leak(all.into_boxed_slice())
    } else {
        command_names::APP_COMMANDS
    };

    let manifest = tauri_build::AppManifest::new().commands(commands);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("failed to build Tauri application metadata");
}
