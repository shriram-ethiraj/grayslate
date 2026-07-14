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
