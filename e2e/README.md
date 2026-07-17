# Local desktop E2E

This suite launches the packaged Tauri binary and drives its real WebKitGTK
window through WebDriver. It does not mock Tauri commands, Rust detection, or
the autosave/naming path.

On Linux, install the WebDriver prerequisites once:

```sh
sudo apt-get install webkit2gtk-driver xvfb
cargo install tauri-driver --locked
```

Then run the base flow from the repository root:

```sh
pnpm install
pnpm run e2e:check
pnpm run e2e:local
```

`e2e:local` builds with the visible-window override in
`src-tauri/tauri.e2e.conf.json`, then runs the test. The harness sets an
isolated `HOME`/XDG data directory and leaves it in `.e2e-tmp/` for inspection.

For a headless Linux runner, use `pnpm run e2e:ci` after the build. If
`tauri-driver` is installed outside Cargo's default location, set
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

Spec execution order is fixed explicitly in `wdio.conf.ts` (`specs` array):
numbered functional specs run first as a shared-session story, then the
`security/` group runs last on the populated sandbox.
