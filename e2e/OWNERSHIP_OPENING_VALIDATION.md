# Ownership-aware file opening — validation record

Validated on Linux/WebKitGTK on 17 October 2026, on
`feat/ownership-aware-file-opening` (base HEAD `2547f18`, with uncommitted changes).
No commits or release publication were performed.

## Completed work

- Validated exclusive file ownership, batch opening, OS-request routing,
  independent autosave identities, unsaved guards, and primary-window survival.
- Completed the 12-scenario multi-window spec, including deterministic pending
  owner reuse, declined batch encoding, edits above the 5,000,000-character text
  synchronization threshold, and CSV edits above 100,000 rows.
- Put batch success notifications behind warnings/errors so a successful
  library insertion cannot obscure files that failed to open.
- Extended the test-only capability to secondary editor windows, and made
  WebDriver window selection explicit to prevent native-focus auto-following
  from redirecting test actions to the wrong window.
- Replaced separate toast-element reads with one visible-stack snapshot;
  waited for document operations after modal choices and checked the CSV
  toggle's actual click target before interaction. No fixed sleeps or skipped
  assertions were added.
- Corrected the naming test's stale expectation for the refactored frontend
  language-loader registry. Naming implementation behavior was unchanged.

## Automated results

| Check | Result |
| --- | --- |
| `pnpm run check` | Pass; zero errors or warnings |
| `pnpm run e2e:check` | Pass; 179 executable requirements reconciled |
| `cargo test --workspace --manifest-path src-tauri/Cargo.toml --offline` | Pass; 1,141 tests: app 355, detection 338, naming 448 |
| Packaged desktop scenarios | 179/179 unique requirements passed; none missing or skipped |
| External-file + multi-window specs, `--repeat 2` | Pass; 60 executions across four fresh sessions, zero recorded interaction retries |
| Transformation + CSV + encoding helper regressions | Pass; 40 scenarios across three fresh sessions, zero recorded interaction retries |
| Visual and packaged security specs | Pass; typography, hover states, grants, ACL, headers, and navigation |
| `pnpm run check:file-associations` | Pass; 108 extensions |
| Release-script, package-repository, and release-guard unit tests | Pass |
| Normal Linux release packages | Pass; binary, DEB, RPM, and AppImage generated |
| `pnpm run verify:release-build` | Pass; no E2E backend commands or frontend runtime markers |

The first full pass exposed an external-file test's window-focus race. Follow-up
runs exposed notification-reading and premature window-count assertions. These
were fixed, and all 18 external-file scenarios and all 12 ownership scenarios
subsequently passed twice. Results are reconciled by requirement across the full
pass and final targeted reruns, rather than treating earlier failures as passes.

Local evidence is retained under `/tmp/grayslate-validation/`: full-run reports,
final opening/helper JUnit reports, retry reports, and `scenario-results.json`.
Build and runner logs use `/tmp/grayslate-*.log`. These are local temporary
artifacts, not committed fixtures.

The normal build uses
`pnpm run tauri build --config src-tauri/tauri.system-package.conf.json`, which
disables updater-artifact signing for a local build. This is separate from the
E2E-feature build used by the desktop runner.

The Tauri build compiled the release and generated DEB/RPM, then its AppImage
step failed because `appimagetool` could not download the type-2 runtime inside
the network-restricted sandbox. The official x86-64 runtime was downloaded
separately, and the already-prepared AppDir was successfully packaged with
`appimagetool --runtime-file`. Both the release binary and AppDir binary passed
the test-surface exclusion scan. No application configuration was weakened.

Generated local artifacts:

- `target/release/bundle/deb/Grayslate_0.5.1_amd64.deb`
- `target/release/bundle/rpm/Grayslate-0.5.1-1.x86_64.rpm`
- `target/release/bundle/appimage/Grayslate_0.5.1_amd64.AppImage`

## Manual sign-off still required

Automated Linux validation cannot sign off macOS/Windows native file activation,
Dock/taskbar/launcher drops, or focused macOS menu behavior. Follow
[the cross-platform opening checklist](FILE_DROP_MANUAL_CHECKLIST.md) on packaged
artifacts for each desktop target.

The existing coverage catalogue also keeps real Git sync with credentials,
signed updater installation/signature verification, and statistical heap
reclamation as manual release checks. None were marked passed by this run.
