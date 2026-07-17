# Local desktop E2E

This suite launches the packaged Tauri binary and drives its real WebKitGTK
window through WebDriver. It does not mock Tauri commands, Rust detection, or
the autosave/naming path.

On Linux, install the WebDriver prerequisites once:

```sh
sudo apt-get install openbox webkit2gtk-driver xvfb
cargo install tauri-driver --locked
```

Then run the base flow from the repository root:

```sh
pnpm install
pnpm run e2e:check
pnpm run e2e:local
```

`e2e:local` builds with the visible-window override in
`src-tauri/tauri.e2e.conf.json`, then runs the test. Before each serial spec
session, the harness resets the isolated `HOME`/XDG runtime under
`.e2e-tmp/runtime/`.

For a headless Linux runner, use `pnpm run e2e:ci` after the build. The command
starts Openbox inside Xvfb so native window-state tests have a real window
manager. If `tauri-driver` is installed outside Cargo's default location, set
`TAURI_DRIVER_PATH` before running the test.

## Test-only build feature

`e2e:build` compiles with `--features e2e`, which adds two debug shims in
`src-tauri/src/commands/e2e.rs` and grants them via a runtime capability
(`src-tauri/e2e-capabilities/e2e.json`, added in `lib.rs` setup). They are
absent from any build that does not set the feature, so a release binary never
carries them:

- `e2e_open_path(path)` — runs the real `pick_document` authorization/grant path
  for a fixture path (no native dialog) and emits the production open event, so
  the app loads the file through its normal authorized-open handler.
- `e2e_save_path(path)` — runs the real `pick_save_document` grant path for a
  Save-As target.

## Fixtures and helpers

- `e2e/fixtures/` holds committed sample files per language/mode.
- `e2e/helpers/app.ts` wraps the common flows: `openExternalFixture`,
  `typeText`, `newSlate`, `pressMod`, `clickTestId`, `waitForFile`,
  `sidebarCard`, `setFilterTab`, `runTransform`, `enterCsvTable`, `csvCell`, and
  `invokeInApp` (webview IPC via `__TAURI_INTERNALS__`).

Spec execution order is fixed explicitly in `wdio.conf.ts`. The flat spec list
starts a fresh packaged-app/WebKit session for every file, while
`maxInstances: 1` keeps those sessions strictly serial. Specs must seed their
own cross-file data; state may only be shared by tests within the same file.
The runtime is safe to reuse because only one worker runs at a time; failure
artifacts use `WDIO_WORKER_ID` directories so each spec keeps its own evidence.

The functional story covers first-run autosave/naming, external files and
Save-As authorization, the detector/naming language matrix, core editing,
formatting, transformations (including chunked large text), sidebar search and
mutations, appearance/settings, Markdown sanitization/scroll sync, CSV table
editing and the >100k-row handoff, keyboard help, and native shell lifecycle.

CI runs the same packaged suite on Linux through
`.github/workflows/e2e.yml`. Failure metadata, screenshots, and page source are
uploaded from each worker's `.e2e-tmp/workers/*/artifacts/` directory.
