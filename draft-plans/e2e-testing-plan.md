# Grayslate desktop E2E plan

## Goal

Exercise the installed Tauri desktop app through its real window, while the
real Svelte UI, Tauri IPC, Rust commands, language detector, autosave engine,
and naming pipeline are running. The first milestone is intentionally small so
it can be run locally before adding a CI matrix or more complicated native OS
flows.

## Decisions

- Use WebdriverIO + `tauri-driver` as the native WebDriver client.
- Keep the first runner Linux-first. Linux GitHub runners can provide
  `WebKitWebDriver` and `xvfb`; Windows can be added with its WebView2 driver
  once the Linux flow is stable.
- Do not mock Rust IPC in this suite. Mocked-command tests can be added as a
  separate fast layer later, but they must not replace native E2E coverage.
- Keep test data isolated by setting `HOME` and the XDG directories for the
  app process. A test must never write to a developer's real Grayslate library.
- Use stable `data-testid` attributes only at boundaries that are otherwise
  difficult to select (the editor and language status button). Prefer existing
  accessible labels and paths for future UI flows.

## Phase 1 — implemented base

The repository now contains:

- `e2e/wdio.conf.ts`: one-worker WDIO configuration using the external
  `tauri-driver` provider, the release binary at `target/release/Grayslate`,
  and failure screenshots/page source under `.e2e-tmp/artifacts/`.
- `e2e/helpers/sandbox.ts`: deterministic Rust fixture plus an isolated
  `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME`, and
  `XDG_STATE_HOME`. The freedesktop user-dirs file points Documents into the
  same sandbox, so the default `Documents/Grayslate` notes root is isolated.
- `e2e/specs/autosave-language.e2e.ts`: types a Rust document through
  CodeMirror with WebDriver keys, waits for the real Rust detector to settle,
  waits for backend autosave, and asserts the exact `config.rs` bytes and
  visible Rust status.
- `e2e/tsconfig.json`: strict type-checking for the E2E TypeScript files.
- `src-tauri/tauri.e2e.conf.json`: JSON merge config that makes the otherwise
  hidden main window visible for WebDriver. It does not change production
  configuration.
- `data-testid="editor"` on the CodeMirror host and
  `data-testid="language-mode"` plus language metadata on the status button.
- `e2e/README.md` with local Linux prerequisites and commands.

The service is pinned to `@wdio/tauri-service` 1.1.0. The current 1.2.0
package imports a symbol missing from its published `@wdio/native-utils`
2.4.0 dependency; 1.1.0 is compatible with the current external-driver flow
and is sufficient because this phase deliberately uses no service mocking.

Run from the repository root:

```sh
sudo apt-get install webkit2gtk-driver xvfb
cargo install tauri-driver --locked
pnpm install
pnpm run e2e:check
pnpm run e2e:local
```

For a headless Linux runner, build once and run `pnpm run e2e:ci`.

## Phase 2 — high-value native flows

Add independent specs, each with a fresh sandbox or explicit cleanup:

1. Open a new slate, type text, and verify the editor state and autosaved file.
2. Reopen the saved file from the sidebar and verify the Rust read path and
   preserved language mode.
3. Save a local fixture and verify the real Rust `read_file_content` path,
   size validation, and language detection.
4. Exercise sidebar search/sort and assert backend results plus visible row
   ordering.
5. Exercise find/replace and assert the editor transaction and Rust-backed
   match statistics.
6. Add CSV and transformation flows after the small-document path is stable;
   include separate large-document safety cases.

Native file pickers cannot be controlled by WebDriver. When open/save dialog
coverage is needed, add a debug-only, Rust-validated test command that emits
the existing application event with a fixture path. Keep that shim out of
release builds and continue to assert the same production open/save handlers.

## CI follow-up

Create a Linux GitHub Actions job after Phase 1 passes locally:

1. Install `webkit2gtk-driver`, `xvfb`, Rust, Node, and pnpm.
2. Run `pnpm install --frozen-lockfile`.
3. Run `pnpm run e2e:build`.
4. Run `pnpm run e2e:ci` and upload `.e2e-tmp/artifacts/` on failure.

Keep `pnpm run check` and `cargo test --manifest-path
src-tauri/Cargo.toml` as separate, faster gates. The native E2E job should be
allowed to fail independently while the first few flows are being stabilized.

## Verification status

- `pnpm run check`: passes.
- `pnpm run e2e:check`: passes.
- `pnpm run e2e:build`: passes and produces `target/release/Grayslate`.
- Full native execution is ready once the host has `WebKitWebDriver`; the
  current development container has `tauri-driver` but cannot install the
  system `webkit2gtk-driver` package without root access.
