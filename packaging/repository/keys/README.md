# Repository public keys

This directory intentionally contains no key yet. Before enabling the Linux
repository workflow, generate the dedicated repository signing key described in
`docs/linux-package-repository.md`, then commit both public exports here:

- `grayslate-archive-key.asc` (ASCII-armored; used by RPM/DNF)
- `grayslate-archive-keyring.gpg` (binary; used by APT)

Never place the private key or its passphrase in this repository.

