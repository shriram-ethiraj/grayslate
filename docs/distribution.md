# Distribution and updater runbook

Grayslate has two update modes. GitHub direct downloads (`.dmg`, AppImage, and
NSIS `.exe`) use the signed Tauri updater. DEB, RPM, Homebrew, Flatpak, Snap, and
AUR builds are compiled with `GRAYSLATE_UPDATE_POLICY=system-managed` and must
be updated by their package manager. Development builds default to `disabled`.
The Rust command boundary enforces this policy even if the webview is bypassed.

## One-time updater setup

Generate the updater key once on a trusted maintainer machine:

```bash
pnpm tauri signer generate --write-keys /secure/path/grayslate-updater.key
```

Then:

1. Replace `REPLACE_WITH_TAURI_UPDATER_PUBLIC_KEY` in
   `src-tauri/tauri.conf.json` with the generated public key. The public key is
   safe and must be committed.
2. Store the private-key contents in the GitHub Actions secret
   `TAURI_SIGNING_PRIVATE_KEY`.
3. Store its password in `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. An empty
   password is supported by Tauri, but a password-protected key is preferred.
4. Back up the private key and password offline. Losing the key means existing
   installations cannot trust updates signed by a replacement key.

Never commit the private key. The tag workflow rejects the public-key sentinel
and a missing private-key secret before any build starts.

## Direct release procedure

Update the same version in:

- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`
- the newest `<release>` entry in the AppStream metadata

Run the normal checks, commit the version bump, and push a tag such as
`v0.2.0`. `.github/workflows/release.yml` then builds:

- one macOS 11+ universal direct-download DMG and updater archive (Intel + ARM),
- a separate universal Homebrew DMG with self-update disabled,
- Linux x64 AppImage with self-update,
- Linux x64 DEB and RPM without self-update,
- Windows x64 and native ARM64 NSIS installers with self-update.

The workflow also attaches `latest.json`, `SHA256SUMS`, a reproducible source
archive, an SPDX JSON SBOM, and rendered Homebrew/AUR/Flatpak submission files.
It creates a **draft** GitHub release. Download and test every artifact from the
draft, including an update from the previous public version, before manually
clicking **Publish**. A draft is not reachable through the configured
`releases/latest/download/latest.json` updater endpoint.

The first milestone intentionally has OS trust warnings:

- macOS is ad-hoc signed, not Developer ID signed or notarized. Test the exact
  Gatekeeper recovery instructions before publishing.
- Windows installers are not Authenticode signed and may trigger SmartScreen.
- Tauri updater signatures are still mandatory on both platforms and are a
  separate trust layer.

## Store and package-manager channels

Publishing the GitHub release triggers `.github/workflows/publish-packages.yml`
and `.github/workflows/publish-linux-repositories.yml`. The latter rebuilds the
official signed APT and RPM repositories on Cloudflare R2 from every stable,
versioned GitHub Release package. See
[`docs/linux-package-repository.md`](linux-package-repository.md) for Cloudflare,
signing-key, recovery, and verification instructions.

Linux Mint/Ubuntu/Debian users who enroll the APT repository receive updates
through APT and their graphical Update/Software Manager. Fedora/RHEL-compatible
users receive them through DNF and compatible software applications. AppStream
and DEP-11 indexes provide the name, description, icon, screenshots, and release
history used by graphical stores. The standalone DEB/RPM release assets remain
available, but installing one directly does not enroll the repository.
The recommended public enrollment path is
`curl -fsSL https://packages.grayslate.app/install.sh | sh`; AppImage remains
the secondary standalone Linux option.

Publishing also triggers `.github/workflows/publish-packages.yml`.
If `SNAPCRAFT_STORE_CREDENTIALS` is configured, it builds the strict x64 snap
from the published tag and releases it to `stable`. If
`HOMEBREW_TAP_DISPATCH_TOKEN` is configured, it dispatches the tag to the
`shriram-ethiraj/homebrew-grayslate` custom tap repository.

The Homebrew template targets the universal DMG and documents the current
Gatekeeper limitation. Do not submit it to homebrew-cask until Grayslate has
Developer ID signing/notarization and meets Homebrew's acceptance requirements.

For AUR, copy the rendered `PKGBUILD` release asset into the AUR package repo,
run `makepkg --printsrcinfo > .SRCINFO`, test with a clean chroot, and push both
files. The package builds from the release source archive and is x64-only.

For Flathub, use the rendered `app.grayslate.Grayslate.yml` as the submission
manifest, then generate and commit its two offline dependency source files:

- `generated-node-sources.json` from `pnpm-lock.yaml`
- `generated-cargo-sources.json` from `src-tauri/Cargo.lock`

Generate them with the current `flatpak-builder-tools` release, validate with
`flatpak-builder --force-clean`, `flatpak run org.freedesktop.appstream.cli
validate`, and Flathub's linter, then submit them to the app's Flathub repo.
The manifest requests only display/GPU access, the app's Documents subfolder,
and the file/open-URI portals. It does not receive broad home-directory access.

## External accounts and secrets still required

Repository code cannot create or accept legal terms for the Apple, Microsoft,
Snapcraft, Flathub, Homebrew, or AUR accounts. Before enabling each channel,
reserve the app/package name, enable required account security, add only the
documented narrowly scoped secret, and test the package in that store's review
or edge channel first.
