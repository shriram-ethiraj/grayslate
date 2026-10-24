# Concurrent file opening — validation record

Linux/WebKitGTK, 20 October 2026, branch `feat/ownership-aware-file-opening`,
base HEAD `2547f18` plus uncommitted work. No commits or release publication.

## Implementation

- Native macOS menu and Windows/Linux menu use **Open Files…** and
  **Open Files in New Windows…**; shortcut help uses **Open Files**. Action
  IDs, shortcuts, production IPC payloads/permissions, and settings are unchanged.
- A typed internal pool owns four slots per request. Each slot includes window
  readiness or the receiving-window load, including an encoding confirmation.
  Slots refill on completion or failure; outcomes retain selection order.
- All four entry points share this pool: picker, New Windows, drop, activation.
  The receiving load starts in the first wave after its unsaved choice resolves.
  Ownership reservations, authorization, and per-window queues remain in place.
- Final focus follows the last successful selection, including a receiving-window
  preference when its final file succeeds. Read/preparation failures are collected
  in selection order while other loads continue.
- `create_editor_window` is asynchronous for Windows-safe native creation.
  Registry locks are released before entering the native window builder.
- Request limits remain 50 unique files and 200 MB per file; no dependencies added.

## Deterministic coverage

`meta/harness-contract.e2e.ts` tests the pool with deferred promises: four active
jobs, immediate refill, isolated rejection/synchronous throws, ordered results,
empty input, and single input. There are no timing sleeps.

`18-opening-batches.e2e.ts` uses the real Rust file-read gate to hold all initial
reads and count four simultaneous slots, including a receiving load. This runs
through all four entry points. A path filter on the E2E-only gate holds one file
while later waves finish, including a failed binary read. Other scenarios cover
independent batches sharing a pending owner, encoding cancellation in a secondary
window, Save/Discard/Cancel before any loads, and native final focus. Existing
opening/security regressions cover duplicates, existing owners, encoding formats,
failed final reads, authorization, and the unchanged request/file limits.

The Linux wrapper now runs suites in a private D-Bus session. HOME/XDG isolation
alone does not isolate the single-instance plugin: the first two attempts timed
out before WebDriver session creation because a desktop copy was already running.
The isolated baseline and subsequent runs launch independently of that copy.

## Automated results

- `pnpm run check`: zero errors and warnings.
- `pnpm run e2e:check`: TypeScript, conventions, and 186 executable requirement claims pass.
- `cargo test --workspace --manifest-path src-tauri/Cargo.toml --offline`:
  1,141 passed (app 355, detection 338, naming 448).
- Initial full packaged run: 185/186 scenarios passed across 24 fresh sessions;
  opening, pool, security, and visual checks passed. The only failure was an
  intercepted click opening Markdown preview immediately after file loading.
  Its helper now waits for completed file work, dismisses transient hover
  surfaces, and checks the actual uncovered click target. Closing a held render
  deliberately does not wait for idle, preserving the cancellation assertion.
- External-file, Markdown, shortcut, ownership, and harness specs passed twice
  against the final packaged code. The expanded batch test initially used an
  idle helper while deliberately holding another read; it now observes visible
  document readiness directly. The complete strengthened batch spec then passed
  twice, including every held secondary read and early receiving-file completion,
  with zero recorded interaction retries.
- Reconciliation across the full run and final reruns: **186/186 unique scenarios
  passed**, none missing. Historical failed attempts remain in the evidence;
  `/tmp/grayslate-opening-validation/scenario-results.json` records each result.

- Normal release build passed:
  `pnpm run tauri build --config src-tauri/tauri.system-package.conf.json`.
  It produced the binary, DEB, RPM, and AppImage without a packaging workaround.
  `pnpm run verify:release-build` and the scan of the AppDir executable both
  pass: no E2E backend commands or frontend runtime markers in release output.

Generated artifacts:

- `target/release/bundle/deb/Grayslate_0.5.1_amd64.deb`
- `target/release/bundle/rpm/Grayslate-0.5.1-1.x86_64.rpm`
- `target/release/bundle/appimage/Grayslate_0.5.1_amd64.AppImage`

Full, repeated-regression, final-batch, benchmark, and reconciliation evidence is
retained in `/tmp/grayslate-opening-validation/`. Build/runner logs are
`/tmp/grayslate-opening-*.log`. These temporary artifacts are not repository fixtures.

## Same-machine opening comparison

Separate fresh packaged sessions, same Xvfb/Openbox setup, 1440×900 source window,
same generated text files (1,000 short lines each), 8 then 16 selections through
**Open Files in New Windows…**. Baseline was built before integrating the pool
and asynchronous command; after uses both. One measurement per size/build,
not a statistical performance guarantee. Time includes menu activation through
completion of the initiating window's tracked batch work.

| Files | Serial | Four concurrent | Sampled aggregate RSS, serial → concurrent | Maximum source probe, serial → concurrent |
| --- | --- | --- | --- | --- |
| 8 | 2.735 s | 1.093 s | 3,224 → 3,273 MiB | 43.9 → 46.2 ms |
| 16 | 5.077 s | 1.930 s | 5,779 → 5,862 MiB | 4.3 → 43.9 ms |

These runs were about 2.5–2.6× faster, with sampled RSS about 1.5% higher.
RSS sums the isolated app and its descendants; shared pages may be counted more
than once. Memory is sampled during source-window polls, not a kernel peak or
private-memory measurement. Probe latency measures a read-only WebDriver DOM/
work-state round trip, not typing latency. No responsiveness threshold was used
to infer concurrency; the held-read assertions prove it independently.

Raw benchmark evidence: `/tmp/grayslate-opening-validation/serial.jsonl` and
`parallel.jsonl`, with the corresponding worker reports in that directory.

## Platform coverage

Linux packaged automation is available here. Native macOS and Windows execution
is unavailable; menu switching, platform file activation, and Windows deadlock
smoke checks remain on [the manual checklist](FILE_DROP_MANUAL_CHECKLIST.md).
