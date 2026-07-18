# Linux package repository runbook

Grayslate publishes its own APT and RPM repositories from the same source repo
as the app and website. GitHub Releases hold the versioned DEB/RPM artifacts;
the `Publish Linux package repositories` workflow rebuilds all indexes and
publishes them to Cloudflare R2 at `packages.grayslate.app` whenever a stable
release is published. A manual workflow run performs the same full backfill.

DEB and RPM builds remain `system-managed`. They do not use the Tauri in-app
updater: APT, Linux Mint Update Manager, DNF, and compatible graphical software
managers own installation and updates.

## Recommended installer

The public Linux installation command is:

```bash
curl -fsSL https://packages.grayslate.app/install.sh | sh
```

The POSIX shell installer supports x86_64 Debian-family and Fedora/RHEL-family
systems. It reads `/etc/os-release`, stages and validates the repository files
before changing system configuration, uses `sudo` only for privileged commands,
and installs or upgrades `grayslate` through APT or DNF. Unsupported systems
exit without changing repository configuration and point users to the AppImage.

The source is `packaging/repository/install.sh`. At publication time, the
repository builder replaces its checksum placeholders with the SHA-256 hashes
of both signing keys and both repository configuration files, then writes it to
the R2 root. Repository verification checks those pins, required security
policy, and shell syntax; the publication workflow syntax-checks the public copy.
Keep all installer actions inside functions and keep `main "$@"` as the final
line so a truncated `curl | sh` transfer cannot invoke an incomplete body.

For manual installation or troubleshooting, use the same canonical files that
the installer consumes.

Linux Mint, Debian, and Ubuntu:

```bash
sudo install -d -m 0755 /etc/apt/keyrings
curl -fsSL https://packages.grayslate.app/keys/grayslate-archive-keyring.gpg | sudo tee /etc/apt/keyrings/grayslate-archive-keyring.gpg >/dev/null
curl -fsSL https://packages.grayslate.app/config/grayslate.sources | sudo tee /etc/apt/sources.list.d/grayslate.sources >/dev/null
sudo apt-get update
sudo apt-get install grayslate
```

Fedora, RHEL, and compatible RPM systems:

```bash
sudo rpm --import https://packages.grayslate.app/keys/grayslate-archive-key.asc
sudo curl -fsSL https://packages.grayslate.app/config/grayslate.repo -o /etc/yum.repos.d/grayslate.repo
sudo dnf install grayslate
```

## One-time Cloudflare setup

1. Create an R2 bucket named `grayslate-packages` (or choose another name).
2. Add `packages.grayslate.app` as the bucket's public custom domain. Keep the
   development `r2.dev` URL disabled for production use.
3. Create a Cloudflare API token for Wrangler with `Workers R2 Storage: Edit`,
   scoped to the account that owns the bucket. This is the bearer token stored
   in `CLOUDFLARE_API_TOKEN`, not an R2 S3 Access Key ID or Secret Access Key.
4. Add these GitHub environment secrets to `release-signing`:
   - `CLOUDFLARE_ACCOUNT_ID`
   - `CLOUDFLARE_API_TOKEN`
   - `GRAYSLATE_R2_BUCKET` (for example, `grayslate-packages`)

The website can remain on Cloudflare Pages. The package subdomain points
directly to R2; no second source repository or Pages project is needed.

## One-time signing-key setup

Create a dedicated RSA 4096-bit signing key on a trusted maintainer machine.
Do not reuse the Tauri updater key and do not use this key for personal email.

```bash
gpg --quick-generate-key "Grayslate Linux Repository <packages@grayslate.app>" rsa4096 sign 2y
gpg --list-secret-keys --with-subkey-fingerprint packages@grayslate.app
```

Use the full 40-character primary-key fingerprint shown by GPG below:

```bash
gpg --armor --export FINGERPRINT > packaging/repository/keys/grayslate-archive-key.asc
gpg --export FINGERPRINT > packaging/repository/keys/grayslate-archive-keyring.gpg
gpg --armor --export-secret-keys FINGERPRINT > /secure/grayslate-linux-repository-private.asc
```

Commit only the two public exports. Store the private export in the GitHub
secret `LINUX_REPOSITORY_GPG_PRIVATE_KEY`, its password in
`LINUX_REPOSITORY_GPG_PASSPHRASE`, and the fingerprint in
`LINUX_REPOSITORY_GPG_FINGERPRINT`. Keep an encrypted offline backup. Losing
this key requires users to explicitly trust a replacement key.

## Publishing and recovery

Publishing a non-prerelease GitHub Release triggers the workflow. It downloads
every stable release's versioned `Grayslate-<version>-linux-x86_64.deb` and RPM,
checks every package's embedded name/architecture, requires the newest DEB and
RPM to contain the current desktop/AppStream payloads, signs the RPM copies,
and regenerates:

- signed APT `InRelease`, `Release`, `Packages`, and DEP-11/AppStream metadata;
- signed RPM packages, RPM-MD metadata, and AppStream metadata;
- the one-line installer, public repository configuration, signing keys, and
  application icon.

R2 uploads package payloads and checksum-addressed objects first and the small
live indexes last. DEBs and hash-addressed metadata receive immutable caching;
RPMs are forced to revalidate because historical unsigned release assets are
signed while the repository is rebuilt.
If a run fails, fix the problem and use **Run workflow**; the entire public
repository is reconstructed from GitHub Releases. Do not edit bucket objects by
hand.

Before advertising the repository, test in clean Debian/Ubuntu/Mint and Fedora
VMs. Confirm command-line installation, graphical discovery (name, icon,
summary, screenshots), and an update from the previous version.
