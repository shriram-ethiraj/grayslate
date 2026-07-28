# Local desktop E2E

This suite launches the packaged Tauri binary and drives its real WebKitGTK
window through WebDriver. It does not mock Tauri commands, Rust detection, or
the autosave/naming path.

On Linux, install the WebDriver prerequisites once:

```sh
sudo apt-get install openbox webkit2gtk-driver xclip xvfb
cargo install tauri-driver --locked
```

`openbox` is not optional: the native window-state tests ask the compositor to
maximize, minimize, and restore the window; without a window manager those
transitions are ignored or cannot be remapped.

`xclip` lets clipboard assertions read the real X11 clipboard without injecting
a focus-stealing paste target into the app.

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

Do not replace `pnpm run e2e:build` with a direct
`cargo build --release --features e2e`. Cargo alone does not apply
`tauri.e2e.conf.json`; the resulting binary can launch with the normal hidden
window/global-bridge settings and appear as an unpainted dark shell to WDIO.

For a headless Linux runner, use `pnpm run e2e:ci` after the build. The command
starts Openbox inside Xvfb so native window-state tests have a real window
manager. If `tauri-driver` is installed outside Cargo's default location, set
`TAURI_DRIVER_PATH` before running the test.

## Commands

| Command | What it does |
| --- | --- |
| `pnpm run e2e:build` | Compile the packaged app with `--features e2e`. |
| `pnpm run e2e:check` | Type-check the suite, then run the convention lint. |
| `pnpm run e2e:lint` | Convention lint only (see "Rules" below). |
| `pnpm run e2e:coverage` | Strictly require every automatable behavior to be claimed. |
| `pnpm run e2e:coverage:report` | Print the reconciliation report without changing strict CI policy. |
| `pnpm run e2e:coverage:strict` | Explicit alias for the strict coverage audit. |
| `pnpm run verify:release-build` | Assert a built binary carries no test-only commands. |
| `pnpm run e2e:test` | Every suite, in tier order. |
| `pnpm run e2e:functional` | Functional and lifecycle behavior only. |
| `pnpm run e2e:security` | Packaged capability, CSP, and authorization checks. |
| `pnpm run e2e:visual` | Typography, theming, and hover measurement. |
| `pnpm run e2e:ci` | Every tier inside Xvfb with Openbox. |
| `pnpm run e2e:ci:functional` | Functional tier inside Xvfb with Openbox. |
| `pnpm run e2e:ci:security` | Security tier inside Xvfb with Openbox. |
| `pnpm run e2e:ci:visual` | Advisory visual tier inside Xvfb with Openbox. |

Run a single spec with
`npx wdio run e2e/wdio.conf.ts --spec e2e/specs/<file>`, or one tier with
`--suite functional`.

## Layout

```
config/      wdio configs, suite definitions, and the shared timeout vocabulary
driver/      WebDriver primitives: IPC, keys, waits, read-only page probes
pages/       page objects — one per surface; they act and read, never assert
fixtures/    committed sample files plus the provisioning factories
assertions/  shared assertions (exact bytes, directory inventories, settled negatives)
specs/       the tests themselves
scripts/     the convention linter
```

Specs are **discovered from disk**, not listed by hand, so a new spec file runs
as soon as it exists. `specs/meta/harness-contract.e2e.ts` asserts that
discovery stays complete and that the isolation contract below still holds.

## What counts as coverage

`coverage/requirements.ts` lists every user-visible behavior with an id, and
each one is assigned to exactly one place:

- **`e2e`** — a packaged scenario claims it with `scenario("<id>", …)`.
- **`rust`** — a pure algorithm or an exhaustive registry, proven by unit tests.
  All 82 transformations are covered this way (126 tests in
  `src-tauri/src/commands/transform.rs`); the E2E suite exercises one
  representative per *UI behavior family* — replace, insert, message, error,
  selection-scoped, chunked, cancelled — because driving 82 actions through the
  desktop UI would be slower and prove less.
- **`static`** — a source-level audit, for drift that no runtime observation can
  detect (the shortcut catalog against the real registrations).
- **`manual`** — automation is impractical; needs a written reason and an owner.

`pnpm run e2e:coverage` parses the specs with the TypeScript compiler and
reconciles the claims. It is deliberately **not** a text search: an earlier
design counted a behavior as covered when a spec merely mentioned its
`data-testid` or IPC command name, which measures vocabulary rather than
verification. A claim counts only when the id resolves to a registry entry and
carries a real test body, and each id may be claimed exactly once.

The repository command runs this audit in strict mode. Calling the script
without `--strict` is reporting-only and is useful while drafting a new
registry entry, but CI never accepts an unclaimed automatable behavior.

## Isolation contract

The flat spec list starts a fresh packaged-app/WebKit session for every file,
and `maxInstances: 1` keeps those sessions strictly serial. Each worker wipes
`.e2e-tmp/runtime/` before its session starts, which is what makes assertions
like `expect(notesRoot).toEqual([])` meaningful.

That holds only while one worker runs at a time and each worker loads one spec
file. **Raising `maxInstances`, adding `specFileRetries`, or grouping spec paths
into nested arrays breaks it.** Specs must seed their own data; state may only
be shared between tests inside the same file — and even that is discouraged,
because an early failure then cascades.

## Rules

`pnpm run e2e:lint` enforces these. Each exists because breaking it produced a
test that passed for the wrong reason.

1. **No fixed sleeps.** `browser.pause` is banned in specs. For a negative
   assertion use `expectSettledAbsent` from `assertions/matchers.ts`: it
   advances the system to a known point, then requires the invariant to hold
   across a sampled quiet window. A sleep does the opposite — it gets *weaker*
   the slower the machine.
2. **Every wait carries a timeout and a message.** Prefer the wrappers in
   `driver/wait.ts`; a bare `waitUntil` inherits the global ceiling and fails
   with a message that names nothing.
3. **UI flows are driven through the UI.** IPC is allowed for the `e2e_*` shims,
   the security specs, and read-only oracles annotated `// ipc-oracle: <why>`.
4. **`browser.execute` lives in `driver/`.** Reads go in `probe.ts`, IPC in
   `invoke.ts`. Scripts must never act — synthesizing a `dblclick` or assigning
   `input.value` skips the code path the test claims to cover.
5. **One `it`, one behavior**, and every `it` runnable on its own.

`scripts/lint-conventions.mjs` carries a `MIGRATION_BACKLOG` of specs that
predate these rules. **That list may only shrink.**

## Test-only build feature

`e2e:build` compiles with `--features e2e`, which adds a small test surface in
`src-tauri/src/commands/e2e.rs` and grants them via a runtime capability
(`src-tauri/e2e-capabilities/e2e.json`, added in `lib.rs` setup). They are
absent from any build that does not set the feature, so a release binary never
carries them:

- `e2e_open_path(path)` — runs the real `pick_document` authorization/grant path
  for a fixture path (no native dialog) and emits the production open event, so
  the app loads the file through its normal authorized-open handler.
- `e2e_save_path(path)` — runs the real `pick_save_document` grant path for a
  Save-As target.

Both run the production authorization code.

The other two commands queue an Open/Save dialog result and then exercise the
normal dialog-owning production command. They cover both accepted and cancelled
dialog boundaries without granting the webview a general filesystem picker.

Three operation-gate commands arm, observe, and release deterministic
cancellation checkpoints. Two external-action commands queue native
confirmation and read the validated file-manager/browser handoff. These five
commands only coordinate or observe the E2E build; all path, URL, document
grant, and source-authority validation remains in the production commands.

## CI

`.github/workflows/e2e.yml` runs Linux-only packaged validation in independent
functional and security jobs. The visual job always runs but is advisory. A
separate normal release build derives and scans the complete `E2E_COMMANDS`
inventory to prove every test-only command
are absent. JUnit XML, failure metadata, screenshots, and page source are
uploaded from each worker's `.e2e-tmp/workers/*/artifacts/` directory.

The few unobservable native integrations are covered by the
[Linux release smoke checklist](MANUAL_RELEASE_CHECKLIST.md). Automated tests
record and assert the validated external boundary; the checklist verifies the
actual file manager, browser, native dialogs, credentials, and signed updater.
