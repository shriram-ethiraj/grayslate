use std::collections::BTreeSet;

#[test]
fn main_capability_is_explicit_and_least_privilege() {
    let capability: serde_json::Value =
        serde_json::from_str(include_str!("../capabilities/default.json"))
            .expect("default capability must be valid JSON");
    let permissions = capability["permissions"]
        .as_array()
        .expect("default capability must contain permissions")
        .iter()
        .map(|permission| {
            permission
                .as_str()
                .expect("capability permissions must be strings")
                .to_string()
        })
        .collect::<BTreeSet<_>>();

    let app_permissions = crate::command_names::APP_COMMANDS
        .iter()
        .map(|command| format!("allow-{}", command.replace('_', "-")))
        .collect::<BTreeSet<_>>();
    assert!(app_permissions.is_subset(&permissions));

    let plugin_permissions = permissions
        .difference(&app_permissions)
        .cloned()
        .collect::<BTreeSet<_>>();
    let expected_plugin_permissions = [
        "clipboard-manager:allow-write-text",
        "core:event:allow-emit",
        "core:event:allow-listen",
        "core:event:allow-unlisten",
        "core:window:allow-close",
        "core:window:allow-is-maximized",
        "core:window:allow-minimize",
        "core:window:allow-start-dragging",
        "core:window:allow-toggle-maximize",
        "os:allow-os-type",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<BTreeSet<_>>();
    assert_eq!(plugin_permissions, expected_plugin_permissions);

    let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))
        .expect("Tauri config must be valid JSON");
    assert_eq!(
        config["app"]["security"]["capabilities"],
        serde_json::json!(["default"])
    );
    assert!(!std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("capabilities/desktop.json")
        .exists());
}
