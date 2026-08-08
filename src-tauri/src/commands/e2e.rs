//! End-to-end test-only IPC commands.
//!
//! Compiled ONLY under `--features e2e` (see the `e2e` feature in `Cargo.toml`)
//! and never registered in a distributed release build. Each command runs the
//! exact production authorization + grant path used by the real open / save-as
//! handlers, substituting a caller-provided fixture path for the native file
//! dialog that WebDriver cannot drive. No new file authority is introduced:
//! grants still flow through `classify_*` + `DocumentRegistry`, so an e2e test
//! exercises the same code a real user's dialog pick would.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::{Emitter, Manager};

use crate::document::{
    classify_existing_document, DocumentDescriptor, DocumentRegistry, DocumentRights,
};
use crate::storage::AppStorage;

const OPERATION_GATES: &[&str] = &[
    "file-read",
    "editor-find",
    "transformation",
    "markdown-render",
    "sidebar-search",
    "csv-initialize",
    "csv-dispose",
];

#[derive(Default)]
struct OperationGate {
    reached: bool,
    released: bool,
}

#[derive(Default)]
struct OperationGateRegistry {
    gates: Mutex<HashMap<String, OperationGate>>,
    released: Condvar,
}

static OPERATION_GATE_REGISTRY: OnceLock<OperationGateRegistry> = OnceLock::new();

fn operation_gates() -> &'static OperationGateRegistry {
    OPERATION_GATE_REGISTRY.get_or_init(OperationGateRegistry::default)
}

fn validate_operation_gate(name: &str) -> Result<(), String> {
    if OPERATION_GATES.contains(&name) {
        Ok(())
    } else {
        Err(format!("Unknown E2E operation gate '{name}'."))
    }
}

fn operation_signal_path(name: &str, signal: &str) -> Option<PathBuf> {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .map(|directory| directory.join(format!("grayslate-e2e-{signal}-{name}")))
}

/// Pause a production worker at a deterministic, test-only checkpoint.
///
/// Call sites are themselves behind `#[cfg(feature = "e2e")]`, so normal
/// builds contain neither this registry nor the branch. The blocked worker is
/// always a dedicated `spawn_blocking` task; cancellation IPC remains free to
/// run on the async runtime while the test holds the gate.
pub fn operation_checkpoint(name: &str) {
    let registry = operation_gates();
    let mut gates = registry
        .gates
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(gate) = gates.get_mut(name) else {
        return;
    };
    gate.reached = true;
    if let Some(path) = operation_signal_path(name, "observed") {
        let _ = std::fs::write(path, b"observed\n");
    }

    while gates.get(name).is_some_and(|gate| !gate.released)
        && !operation_signal_path(name, "release").is_some_and(|path| path.exists())
    {
        let (next_gates, _) = registry
            .released
            .wait_timeout(gates, Duration::from_millis(10))
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        gates = next_gates;
    }
    gates.remove(name);
    if let Some(path) = operation_signal_path(name, "release") {
        let _ = std::fs::remove_file(path);
    }
}

/// Mark that a synchronous production command reached an E2E observation
/// point without blocking Tauri's IPC loop.
pub fn operation_mark_reached(name: &str) {
    let registry = operation_gates();
    let mut gates = registry
        .gates
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(gate) = gates.get_mut(name) {
        gate.reached = true;
        if let Some(path) = operation_signal_path(name, "observed") {
            let _ = std::fs::write(path, b"observed\n");
        }
    }
}

/// Arm one deterministic operation checkpoint.
#[tauri::command]
pub fn e2e_arm_operation_gate(name: String) -> Result<(), String> {
    validate_operation_gate(&name)?;
    for signal in ["observed", "release"] {
        if let Some(path) = operation_signal_path(&name, signal) {
            let _ = std::fs::remove_file(path);
        }
    }
    operation_gates()
        .gates
        .lock()
        .map_err(|_| "E2E operation gates are poisoned.".to_string())?
        .insert(name, OperationGate::default());
    Ok(())
}

/// Whether the production worker has reached an armed checkpoint.
#[tauri::command]
pub fn e2e_operation_gate_reached(name: String) -> Result<bool, String> {
    validate_operation_gate(&name)?;
    Ok(operation_gates()
        .gates
        .lock()
        .map_err(|_| "E2E operation gates are poisoned.".to_string())?
        .get(&name)
        .is_some_and(|gate| gate.reached))
}

/// Release one blocked production worker.
#[tauri::command]
pub fn e2e_release_operation_gate(name: String) -> Result<(), String> {
    validate_operation_gate(&name)?;
    let registry = operation_gates();
    let mut gates = registry
        .gates
        .lock()
        .map_err(|_| "E2E operation gates are poisoned.".to_string())?;
    let gate = gates
        .get_mut(&name)
        .ok_or_else(|| format!("E2E operation gate '{name}' was not armed."))?;
    gate.released = true;
    registry.released.notify_all();
    Ok(())
}

static MINIMIZE_PROBE_ARMED: AtomicBool = AtomicBool::new(false);
static MINIMIZE_OBSERVED: AtomicBool = AtomicBool::new(false);
static MINIMIZE_RESTORED: AtomicBool = AtomicBool::new(false);
const MINIMIZE_OBSERVATION_FILE: &str = "grayslate-e2e-minimize-observed";

fn minimize_observation_path() -> Option<PathBuf> {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .map(|directory| directory.join(MINIMIZE_OBSERVATION_FILE))
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct E2eMinimizeObservation {
    armed: bool,
    observed: bool,
    restored: bool,
}

/// Observe one real minimize transition and restore the native window.
///
/// A minimized webview is suspended on Linux, so WebDriver cannot query or
/// restore it after clicking the production title-bar control. This probe runs
/// outside the webview, records the compositor-owned state, and unminimizes the
/// same native window so the test can resume. It never replaces the minimize
/// action itself.
#[tauri::command]
pub fn e2e_arm_minimize_probe(window: tauri::Window) -> Result<(), String> {
    if MINIMIZE_PROBE_ARMED.swap(true, Ordering::SeqCst) {
        return Err("The E2E minimize probe is already armed.".to_string());
    }
    MINIMIZE_OBSERVED.store(false, Ordering::SeqCst);
    MINIMIZE_RESTORED.store(false, Ordering::SeqCst);
    if let Some(path) = minimize_observation_path() {
        let _ = std::fs::remove_file(path);
    }

    tauri::async_runtime::spawn(async move {
        for _ in 0..500 {
            if window.is_minimized().unwrap_or(false) {
                MINIMIZE_OBSERVED.store(true, Ordering::SeqCst);
                if let Some(path) = minimize_observation_path() {
                    let _ = std::fs::write(path, b"observed\n");
                }

                // WebKitGTK suspends the webview while its native window is
                // minimized. Requesting `unminimize` is not enough: on a busy
                // compositor the method can return before the window is mapped
                // and able to receive WebDriver input. Re-assert visibility
                // and focus, then give the native event loop a bounded settle
                // window before reporting the probe complete.
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
                for _ in 0..100 {
                    if !window.is_minimized().unwrap_or(true) {
                        tokio::time::sleep(Duration::from_millis(250)).await;
                        let _ = window.show();
                        let _ = window.set_focus();
                        MINIMIZE_RESTORED.store(true, Ordering::SeqCst);
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        MINIMIZE_PROBE_ARMED.store(false, Ordering::SeqCst);
    });

    Ok(())
}

#[tauri::command]
pub fn e2e_minimize_observation() -> E2eMinimizeObservation {
    E2eMinimizeObservation {
        armed: MINIMIZE_PROBE_ARMED.load(Ordering::SeqCst),
        observed: MINIMIZE_OBSERVED.load(Ordering::SeqCst),
        restored: MINIMIZE_RESTORED.load(Ordering::SeqCst),
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct E2EAutosaveCycle {
    source: Option<String>,
    backend_dirty: bool,
    scheduled_actions: usize,
}

/// Run the real autosave scheduler immediately and wait for its normal
/// frontend-content roundtrip and disk write to settle.
#[tauri::command]
pub async fn e2e_force_autosave_cycle(
    app: tauri::AppHandle,
    window: tauri::Window,
) -> Result<E2EAutosaveCycle, String> {
    const SETTLE_TIMEOUT: Duration = Duration::from_secs(20);
    const POLL_INTERVAL: Duration = Duration::from_millis(10);

    let window_label = window.label().to_string();
    let deadline = Instant::now() + SETTLE_TIMEOUT;
    let mut scheduled_actions = 0;

    loop {
        let registry = app.state::<crate::autosave::AutosaveRegistry>();
        let Some((source, dirty, save_in_flight, failure)) = registry.e2e_state(&window_label)
        else {
            return Ok(E2EAutosaveCycle {
                source: None,
                backend_dirty: false,
                scheduled_actions,
            });
        };

        if let Some(failure) = failure {
            return Err(format!("Forced autosave cycle failed: {failure}"));
        }

        if source != "slates" {
            // Exercise the real scheduler even for local documents. It must
            // return no work: local dirty state is frontend-owned and local
            // files are excluded from backend autosave by source.
            let actions = registry.check_and_force_saves();
            scheduled_actions += actions.len();
            drop(registry);
            crate::autosave::dispatch_save_actions(&app, actions);
            return Ok(E2EAutosaveCycle {
                source: Some(source.to_string()),
                backend_dirty: dirty,
                scheduled_actions,
            });
        }

        if !dirty {
            return Ok(E2EAutosaveCycle {
                source: Some(source.to_string()),
                backend_dirty: false,
                scheduled_actions,
            });
        }

        if !save_in_flight {
            let actions = registry.check_and_force_saves();
            scheduled_actions += actions.len();
            drop(registry);
            crate::autosave::dispatch_save_actions(&app, actions);
        }

        if Instant::now() >= deadline {
            return Err(format!(
                "Forced autosave cycle did not settle within {} seconds.",
                SETTLE_TIMEOUT.as_secs()
            ));
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct E2ENavigationObservation {
    kind: String,
    url: String,
    allowed: bool,
}

#[derive(Default)]
struct NavigationProbe {
    armed: bool,
    observation: Option<E2ENavigationObservation>,
}

static NAVIGATION_PROBE: OnceLock<Mutex<NavigationProbe>> = OnceLock::new();

fn navigation_probe() -> &'static Mutex<NavigationProbe> {
    NAVIGATION_PROBE.get_or_init(|| Mutex::new(NavigationProbe::default()))
}

/// Called from the production navigation hooks only while an E2E probe is armed.
pub fn record_navigation_decision(kind: &str, url: &str, allowed: bool) {
    let mut probe = navigation_probe()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !probe.armed || probe.observation.is_some() {
        return;
    }
    probe.observation = Some(E2ENavigationObservation {
        kind: kind.to_string(),
        url: url.to_string(),
        allowed,
    });
}

#[tauri::command]
pub fn e2e_arm_navigation_probe() -> Result<(), String> {
    let mut probe = navigation_probe()
        .lock()
        .map_err(|_| "E2E navigation probe is poisoned.".to_string())?;
    probe.armed = true;
    probe.observation = None;
    Ok(())
}

#[tauri::command]
pub fn e2e_navigation_observation() -> Result<Option<E2ENavigationObservation>, String> {
    Ok(navigation_probe()
        .lock()
        .map_err(|_| "E2E navigation probe is poisoned.".to_string())?
        .observation
        .clone())
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct E2eExternalAction {
    kind: String,
    target: String,
}

/// Deterministic seam at the OS boundary.
///
/// Tests still click the real UI and run all production validation. Only the
/// final native confirmation/opener call is replaced, because WebDriver cannot
/// observe a file manager or browser process. The small manual release
/// checklist covers that final integration on a real desktop.
#[derive(Default)]
pub struct ExternalActionProbe {
    confirmations: Mutex<VecDeque<bool>>,
    actions: Mutex<VecDeque<E2eExternalAction>>,
}

impl ExternalActionProbe {
    pub fn take_confirmation(&self) -> Result<Option<bool>, String> {
        Ok(self
            .confirmations
            .lock()
            .map_err(|_| "E2E external confirmations are poisoned.".to_string())?
            .pop_front())
    }

    pub fn record(&self, kind: &str, target: String) -> Result<(), String> {
        self.actions
            .lock()
            .map_err(|_| "E2E external actions are poisoned.".to_string())?
            .push_back(E2eExternalAction {
                kind: kind.to_string(),
                target,
            });
        Ok(())
    }
}

#[tauri::command]
pub fn e2e_queue_external_confirmation(
    probe: tauri::State<'_, ExternalActionProbe>,
    confirmed: bool,
) -> Result<(), String> {
    probe
        .confirmations
        .lock()
        .map_err(|_| "E2E external confirmations are poisoned.".to_string())?
        .push_back(confirmed);
    Ok(())
}

#[tauri::command]
pub fn e2e_take_external_action(
    probe: tauri::State<'_, ExternalActionProbe>,
) -> Result<Option<E2eExternalAction>, String> {
    Ok(probe
        .actions
        .lock()
        .map_err(|_| "E2E external actions are poisoned.".to_string())?
        .pop_front())
}

/// What a test says the native file dialog should return.
///
/// `Cancel` has to be a value rather than the absence of one. The earlier
/// version queued bare paths and treated an empty queue as a cancellation, but
/// an empty queue is what a *non*-dialog test also looks like — so "cancel"
/// fell through to the real native dialog, which under WebDriver blocks until
/// the suite times out. Distinguishing "nothing queued" from "queued a
/// cancellation" is what makes the cancel path testable at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueuedDialogResponse {
    Path(PathBuf),
    Cancel,
}

/// Pre-selected native-dialog results, consumed by the real pick commands.
///
/// The open/save shims below grant a path directly, which is enough to test
/// everything *downstream* of the dialog but leaves the dialog-invoking code in
/// `pick_document` / `pick_save_document` — menu wiring, suggested filename,
/// starting directory, cancellation — completely untested.
///
/// This queue closes that gap without weakening anything: a test enqueues the
/// answer the user would have given, then clicks the real menu item. The
/// production command runs unchanged except that it takes the queued answer
/// instead of blocking on a dialog WebDriver cannot drive. Classification,
/// validation, and granting all still happen exactly as they do for a real
/// pick.
#[derive(Default)]
pub struct QueuedDialogPaths {
    open: Mutex<VecDeque<QueuedDialogResponse>>,
    save: Mutex<VecDeque<QueuedDialogResponse>>,
}

/// Take one queued response, distinguishing "nothing queued" from a lock error.
///
/// A poisoned lock previously collapsed into `None`, which reads as "no test
/// queued anything" and opens the real dialog — the same unsafe fallback the
/// `Cancel` variant exists to remove. It is surfaced as an error instead.
fn take_next(
    queue: &Mutex<VecDeque<QueuedDialogResponse>>,
    which: &str,
) -> Result<Option<QueuedDialogResponse>, String> {
    Ok(queue
        .lock()
        .map_err(|_| format!("Queued {which} dialog responses are poisoned."))?
        .pop_front())
}

impl QueuedDialogPaths {
    /// Take the next queued open result, if a test enqueued one.
    pub fn take_open(&self) -> Result<Option<QueuedDialogResponse>, String> {
        take_next(&self.open, "open")
    }

    /// Take the next queued Save-As result, if a test enqueued one.
    pub fn take_save(&self) -> Result<Option<QueuedDialogResponse>, String> {
        take_next(&self.save, "save")
    }
}

/// A queued path, or `None` to make the next dialog report a cancellation.
fn response_from(path: Option<String>) -> QueuedDialogResponse {
    match path {
        Some(path) => QueuedDialogResponse::Path(PathBuf::from(path)),
        None => QueuedDialogResponse::Cancel,
    }
}

/// Enqueue what the native open dialog should return next.
#[tauri::command]
pub async fn e2e_queue_open_path(
    queue: tauri::State<'_, QueuedDialogPaths>,
    path: Option<String>,
) -> Result<(), String> {
    queue
        .open
        .lock()
        .map_err(|_| "Queued open dialog responses are poisoned.".to_string())?
        .push_back(response_from(path));
    Ok(())
}

/// Enqueue what the native Save-As dialog should return next.
#[tauri::command]
pub async fn e2e_queue_save_path(
    queue: tauri::State<'_, QueuedDialogPaths>,
    path: Option<String>,
) -> Result<(), String> {
    queue
        .save
        .lock()
        .map_err(|_| "Queued save dialog responses are poisoned.".to_string())?
        .push_back(response_from(path));
    Ok(())
}

/// Frontend open event (mirror of `OPEN_FILE_PATH_EVENT` in `recentFiles.ts`).
/// The sidebar emits this in production after a grant; the open shim emits it
/// too so a single test call drives the real `openAuthorizedDocument` flow.
const OPEN_FILE_PATH_EVENT: &str = "files://open-path";

/// Matches the frontend `OpenFilePathPayload` shape (camelCase keys).
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct OpenFilePathPayload {
    document_id: String,
    document_generation: u64,
    path: String,
    source: String,
}

/// Open a fixture file exactly as `pick_document` does after the user chooses a
/// file in the native open dialog: classify it, grant a tracked authorization
/// for the current window, and return the descriptor. The frontend then drives
/// its normal open flow with the returned grant.
#[tauri::command]
pub async fn e2e_open_path(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    documents: tauri::State<'_, DocumentRegistry>,
    window: tauri::Window,
    path: String,
) -> Result<Option<DocumentDescriptor>, String> {
    let path = PathBuf::from(path);
    let (canonical, source) = classify_existing_document(&app, storage.inner(), &path)?;
    let granted = documents.grant_existing(
        window.label(),
        &canonical,
        source,
        DocumentRights::tracked(source),
    )?;
    let descriptor = granted.descriptor();

    // Emit the same open event the sidebar emits so the frontend loads the file
    // through its real authorized-open handler.
    window
        .emit(
            OPEN_FILE_PATH_EVENT,
            OpenFilePathPayload {
                document_id: descriptor.document_id.clone(),
                document_generation: descriptor.generation,
                path: descriptor.display_path.clone(),
                source: descriptor.source.clone(),
            },
        )
        .map_err(|error| format!("Failed to emit open event: {error}"))?;

    Ok(Some(descriptor))
}

/// Grant a Save-As target exactly as `pick_save_document` does after the user
/// chooses a path in the native save dialog.
///
/// Delegates to the production `grant_save_target` rather than repeating it.
/// The duplicated copy that used to live here was already identical, which is
/// exactly the problem: a change to the real authorization rules would have
/// left this shim granting on the old ones, and the tests would have kept
/// passing against behavior the app no longer had.
#[tauri::command]
pub async fn e2e_save_path(
    app: tauri::AppHandle,
    storage: tauri::State<'_, AppStorage>,
    documents: tauri::State<'_, DocumentRegistry>,
    window: tauri::Window,
    path: String,
) -> Result<Option<DocumentDescriptor>, String> {
    crate::commands::file::grant_save_target(
        &app,
        storage.inner(),
        documents.inner(),
        &window,
        &PathBuf::from(path),
    )
    .map(Some)
}
