# Transformation Baseline Roadmap — Grayslate

> **Updated:** 2026-07-13
>
> **Current registry:** 82 built-in transformations after Priority 1 implementation
> **Product boundary:** editor-native text transformations; no secondary input panels or per-action option dialogs

## Purpose

Grayslate is a developer scratchpad first. Its built-ins should cover the common jobs that otherwise send developers to small online utilities: format data, inspect API payloads, convert text encodings, generate identifiers, and perform deterministic text conversions locally.

This roadmap is grounded in the live action registry and informed by the default tools and script collections in [DevToys](https://devtoys.app/), [Boop](https://github.com/IvanMathy/Boop/tree/main/Scripts), [CyberChef](https://github.com/gchq/CyberChef), and transform.tools. Competitor coverage is evidence of recurring use, not a reason to copy tools that do not fit an editor scratchpad.

## Corrected Current State

Before this roadmap was implemented, Grayslate registered **55** actions, not the 46 recorded by the previous version of this document. Priority 1 added 27 actions, bringing the live registry to **82**. The shipped set includes:

- JSON/JSONC format, minify, validate, key-case conversion, and JSON/CSV/YAML conversion
- SQL, JavaScript, TypeScript, CSS, HTML, Svelte, YAML, Markdown, and TOML formatting
- common plain-text, case, URL, Base64, numeric-base, and statistics actions
- Rust-backed cancellation, real progress, chunked result delivery, and one-transaction CodeMirror application

Therefore **Format SQL** and **Format TOML** are complete and are not gaps.

## Baseline Interaction Contract

The first baseline must remain editor-native:

- A replacement transform consumes the non-empty primary selection; otherwise it consumes the whole document.
- A replacement result replaces exactly that source range as one undoable transaction.
- A validation action returns a message and never changes the document.
- No action opens a parameter dialog, reads an inline directive, or asks for a second input.
- Options are represented as clear fixed actions, such as separate SHA-256 and SHA-512 entries.
- UUID generators are the only no-input exception: they replace a non-empty primary selection or insert at the primary cursor.
- Structured full-document results set the known output language immediately.
- Existing Rust cancellation, natural progress, UTF-8-safe chunking, and CodeMirror rope assembly remain mandatory.

## Priority 1 — Everyday Developer Baseline (Implemented)

These actions are now implemented and registered in the transformation palette.

### 1. Hashes and Checksums

| Proposed action ID | Title | Fixed behavior |
|---|---|---|
| `hash.sha-256` | SHA-256 Hash | Hash the exact UTF-8 input bytes and output lowercase hexadecimal. |
| `hash.sha-512` | SHA-512 Hash | Hash the exact UTF-8 input bytes and output lowercase hexadecimal. |
| `checksum.crc32` | CRC32 Checksum | Compute CRC32 over the exact UTF-8 bytes and output eight lowercase hexadecimal digits. |
| `hash.sha-1` | SHA-1 Hash (Legacy) | Compatibility/integrity use only; never describe it as secure. |
| `hash.md5` | MD5 Hash (Legacy) | Compatibility/integrity use only; never recommend it for passwords or security. |

Hash actions replace their source text with the digest. They do not trim whitespace or hash files from disk.

### 2. Web and API Encoding

| Proposed action ID | Title | Fixed behavior |
|---|---|---|
| `encoding.html-encode` | HTML Entity Encode | Encode `&`, `<`, `>`, `"`, and `'`; preserve other Unicode text. |
| `encoding.html-decode` | HTML Entity Decode | Decode standard named entities and decimal/hex numeric entities. |
| `encoding.base64url-encode` | Base64URL Encode | Encode UTF-8 without `=` padding. |
| `encoding.base64url-decode` | Base64URL Decode | Accept padded or unpadded Base64URL and require valid UTF-8 output. |
| `encoding.gzip-to-base64` | GZip Text to Base64 | Gzip-compress UTF-8 input and return standard Base64 text. |
| `encoding.gzip-from-base64` | Base64 GZip to Text | Base64-decode, gzip-decompress, and require valid UTF-8 output. |
| `encoding.jwt-decode` | Decode JWT (Unverified) | Decode a compact three-segment JWT into formatted JSON containing `header`, `payload`, and the encoded `signature`. |

JWT decoding never verifies a signature and must say so in the action title, description, and completion message. Gzip decompression must stream, remain cancellable, and stop before decompressed output exceeds 200 MB.

### 3. Timestamps

| Proposed action ID | Title | Fixed behavior |
|---|---|---|
| `time.unix-seconds-to-rfc3339` | Unix Seconds to RFC 3339 UTC | Parse one signed integer and output an unambiguous UTC timestamp. |
| `time.unix-milliseconds-to-rfc3339` | Unix Milliseconds to RFC 3339 UTC | Parse one signed integer and preserve millisecond precision. |
| `time.rfc3339-to-unix-seconds` | RFC 3339 to Unix Seconds | Require an explicit timezone/offset and output signed epoch seconds. |
| `time.rfc3339-to-unix-milliseconds` | RFC 3339 to Unix Milliseconds | Require an explicit timezone/offset and output signed epoch milliseconds. |

The timestamp actions accept one trimmed value. They do not guess seconds versus milliseconds and do not interpret timezone-free local dates.

### 4. URL and Structured Data

| Proposed action ID | Title | Fixed behavior |
|---|---|---|
| `url.query-to-json` | Query String to JSON | Accept an optional leading `?`; decode form semantics (`+` as space); preserve repeated keys as arrays. |
| `url.json-to-query` | JSON to Query String | Accept a top-level JSON object whose values are scalar values or arrays of scalar values; reject nested objects. |
| `json.lines-to-array` | JSON Lines to JSON Array | Parse every nonblank line as strict JSON and output one formatted array. |
| `json.array-to-lines` | JSON Array to JSON Lines | Require a top-level array and output one compact JSON value per line. |
| `json.sort-keys` | Sort JSON Keys | Recursively sort strict-JSON object keys, preserve array order, and format with the active indentation. |
| `json.to-typescript` | JSON to TypeScript | Infer TypeScript from strict JSON using the fixed top-level name `Root`. |

`json.sort-keys` rejects JSONC rather than silently discarding comments. TypeScript generation uses `export interface Root` for a top-level object and `export type Root = ...` otherwise. It treats observed properties as required, quotes invalid property identifiers, preserves `null`, and unions distinct array element types.

### 5. XML Essentials

| Proposed action ID | Title | Fixed behavior |
|---|---|---|
| `xml.format` | Format XML | Pretty-print well-formed XML without changing text or CDATA content. |
| `xml.minify` | Minify XML | Remove formatting-only whitespace while preserving meaningful text and CDATA. |
| `xml.validate` | Validate XML | Check well-formedness and return a message without changing the document. |

This phase does not perform XSD/DTD validation, load external entities, or convert XML and JSON. XML↔JSON remains deferred until a mapping convention is explicitly approved.

### 6. Identifier Generators

| Proposed action ID | Title | Fixed behavior |
|---|---|---|
| `generate.uuid-v4` | Insert UUID v4 | Generate one lowercase hyphenated random UUID. |
| `generate.uuid-v7` | Insert UUID v7 | Generate one lowercase hyphenated time-ordered UUID. |

Generators replace the primary selection when it is non-empty; otherwise they insert at the primary cursor and leave the cursor after the generated text. They never replace the whole document merely because the selection is empty.

## Completed Implementation Order

1. **Completed — Integrity and common encodings:** hashes/checksums, HTML entities, Base64URL, and JWT inspection.
2. **Completed — API/data workflows:** timestamps, query-string conversion, JSON Lines conversion, recursive key sorting, and UUID insertion behavior.
3. **Completed — Heavier transforms:** bounded gzip, JSON-to-TypeScript inference, and XML format/minify/validation.

Each batch must update the frontend action union/registry and Rust action enum/dispatch together. Use the shared `TransformationContext`; do not introduce action-specific IPC commands or transport protocols.

## Priority 2 — Deterministic Follow-up Actions

These remain good editor-native additions after Priority 1:

- Unicode escape/unescape with correct surrogate-pair handling
- standards-compliant JSON string escape/unescape; the existing Add/Remove Slashes actions are not equivalent
- JSON example to JSON Schema
- JSON↔TOML and optional YAML↔TOML conversion
- Markdown→HTML and HTML→Markdown
- CSV→GitHub-Flavored Markdown table and TSV↔JSON
- URL components to structured JSON
- PascalCase
- numeric/reverse/length line sorting, unique-only lines, and occurrence counts
- add/remove line numbers and indentation using the active editor indentation
- HEX↔RGB color conversion
- UUID-adjacent ULID generation if demand warrants it

## Deferred Because They Need Another Input or Product Decision

The following are useful tools but do not fit the approved no-dialog baseline:

- regex keep/remove filters, regex capture extraction, and parameterized batch replacement
- JSONPath queries
- JSON/text/list diff and set intersection
- JSON Schema or XSD validation against a separate schema
- HMAC generation, JWT signing, password generation, or anything requiring a secret/options input
- XML↔JSON conversion until attribute, namespace, mixed-content, and root-element mapping are specified
- configurable hard wrapping and other actions that require a numeric parameter

These should not use hidden first-line directives. If Grayslate later gains a deliberate parameter surface, they can be reconsidered.

## Explicit Non-Goals

- image/PDF conversion or compression
- QR-code generation
- broad encryption/decryption suites
- certificate, IP/subnet, or forensic binary tooling
- cron builders, color-blindness simulators, and other standalone widgets
- duplicate CSV table tooling already covered by table mode

## Architecture Contract for Implementation

- Keep `execute_transformation` as the single command boundary.
- Keep the command response as the control plane and indexed channel chunks as the result data plane.
- Add a frontend action application mode with `"replace"` as the default and `"insert"` for generators; this does not require a new IPC response type.
- Replacement and insertion results must each apply as one CodeMirror transaction and one undo step.
- Validators return `ShowMessage`; all usable transformation outputs return `ReplaceText`, not transient toast-only data.
- Report progress only from natural work checkpoints; do not add counting passes for loader cosmetics.
- Validate malformed and oversized input in Rust because the frontend is untrusted.
- Set `outputLanguage` for full-document JSON, TypeScript, and other known structured outputs.

## Verification and Acceptance Criteria

Every implemented action requires Rust unit coverage for its standard examples, invalid input, Unicode behavior, empty input, and cancellation checkpoints where work can be large. In addition:

- use standard published digest/checksum vectors
- test padded and unpadded Base64URL
- test malformed and non-UTF-8 decoded payloads
- test JWT segment count and invalid JSON payloads without implying verification
- test timestamp range, negative epochs, offsets, and millisecond precision
- test repeated/empty query keys and rejected nested JSON
- test blank NDJSON lines, non-object JSON values, and array-order preservation
- test TypeScript inference for nested objects, invalid identifiers, nulls, and heterogeneous arrays
- test XML declarations, attributes, comments, CDATA, mixed text, and malformed input
- test gzip corruption and the decompressed-output limit
- manually verify selection replacement, cursor insertion with the cursor ending after generated text, output-language changes, cancellation, and one-step undo in the running app

Required checks after each implementation batch:

```bash
pnpm run check
cargo test --manifest-path src-tauri/Cargo.toml --lib -- commands::transform::tests
```

No automated frontend test framework is currently configured, so editor application behavior requires manual verification.
