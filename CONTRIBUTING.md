# Contributing to Grayslate

Contributions are welcome. Bug fixes, documentation improvements, tests, and
well-scoped features can all be proposed through pull requests.

## Before you start

- Search the existing issues and pull requests to avoid duplicating work.
- For a substantial feature or architectural change, open an issue first so the
  approach can be discussed before you invest significant time.
- Do not include secrets, personal data, generated build artifacts, or unrelated
  changes in a pull request.
- Report security vulnerabilities privately as described in [SECURITY.md](SECURITY.md).

## Submit a pull request

You do not need write access to the repository. Use GitHub's standard fork
workflow:

1. Fork `shriram-ethiraj/grayslate` on GitHub.
2. Clone your fork and create a branch from the latest `main`:

   ```bash
   git clone https://github.com/YOUR-USERNAME/grayslate.git
   cd grayslate
   git checkout -b fix/short-description
   ```

3. Make one focused change and add or update tests where practical.
4. Run the checks relevant to your change.
5. Push the branch to your fork and open a pull request against Grayslate's
   `main` branch.

Maintainer review and passing required checks are expected before a pull request
is merged. Please do not open pull requests from branches in the upstream
repository unless you are a maintainer.

## Development setup

Grayslate requires Node.js 24 or newer, Rust, pnpm, and the platform-specific
prerequisites for Tauri 2.

```bash
pnpm install
pnpm tauri dev
```

The project uses Svelte 5 and TypeScript in the frontend, CodeMirror 6 for the
editor, and Rust 2021 with Tauri 2 in the backend. Follow the established style
in the surrounding code and keep security-sensitive validation in Rust; the
frontend must be treated as untrusted.

## Checks

Run the checks that cover the area you changed. For changes spanning the whole
application, run all of these:

```bash
pnpm run check
cargo test --manifest-path src-tauri/Cargo.toml
pnpm run tauri build
```

The frontend does not currently have a unit-test suite. Describe any manual
testing in the pull request, including the operating systems tested and before
and after screenshots for visible UI changes.

## Pull request expectations

- Keep the pull request focused and explain what changed and why.
- Link a related issue when one exists.
- Update user-facing documentation when behavior changes.
- Respond to review feedback with follow-up commits; do not rewrite shared
  history after review has started unless a maintainer asks you to.
- By contributing, you agree that your contribution is licensed under the
  repository's [MIT License](LICENSE).

Thank you for helping improve Grayslate.
