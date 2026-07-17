# Security Policy

## Supported versions

Grayslate is pre-1.0. Security fixes are applied to the latest released version
only. Please always update to the most recent release before reporting an issue.

| Version | Supported |
| ------- | --------- |
| latest  | ✅        |
| older   | ❌        |

## Reporting a vulnerability

**Please do not open a public issue for security problems.**

Report privately through GitHub's built-in advisory flow:

1. Go to the repository's **Security → Report a vulnerability** page
   (**Private vulnerability reporting**), or
2. use the link: <https://github.com/shriram-ethiraj/grayslate/security/advisories/new>.

Include, where possible:

- affected version and operating system,
- a description of the issue and its impact,
- reproduction steps or a proof of concept,
- any relevant logs (with secrets and personal data redacted).

You can expect an initial acknowledgement within **7 days**. We will keep you
updated on remediation progress and coordinate disclosure timing with you. We do
not currently run a paid bug-bounty program, but we credit reporters in the
release notes unless you prefer to remain anonymous.

## Threat model

Grayslate is a **local-first desktop application** built on Tauri.

- **Local by default.** Opening, editing, transforming, detecting, naming, and
  saving files all run entirely on your machine. No document content is sent
  over the network.
- **Network access is limited and explicit.** The only outbound network
  activity is (a) checking for and downloading application updates, and
  (b) opening a link you explicitly click in the system browser. Both are
  user-initiated or user-approved.
- **The webview is treated as untrusted.** A strict Content Security Policy is
  enforced, the app ships no shell/process-execution or broad filesystem
  plugin, Markdown is rendered and sanitized in Rust (`pulldown-cmark` +
  `ammonia`), and file access goes through validated Rust commands rather than
  the browser filesystem APIs.
- **Inline script remains forbidden.** The CSP permits inline styles only
  because CodeMirror and the CSV virtualizer calculate element positions at
  runtime and apply them through `style` attributes. This exception is not
  extended to scripts; `script-src` remains limited to the application origin.
- **Files are your own.** Like other editors (e.g. VS Code, Sublime Text),
  Grayslate can open any file your operating-system user account can already
  read; it does not escalate privileges. It refuses to open or overwrite
  non-regular targets (directories, device nodes, FIFOs, sockets), and restricts
  the managed "slate" area (which powers rename/delete) to the configured notes
  directory.

## Update trust model

Public release updates will use Tauri updater artifacts verified against a
bundled public signing key and will be installed only after an explicit user
action. macOS Developer ID signing/notarization and Windows Authenticode are
also release requirements; updater signatures do not replace operating-system
code signing.

The source tree currently contains a placeholder updater key and is not ready
to distribute updates. Until the release signing workflow and public key are
configured and tested, updater/signing work remains a release blocker rather
than a protection claimed by this policy.
