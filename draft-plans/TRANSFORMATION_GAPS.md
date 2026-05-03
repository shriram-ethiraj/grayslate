# Transformation Gap Analysis — Grayslate

> **Date:** 2026-05-01
> **Sources researched:** Boop (4.1k stars), DevToys (31.3k stars), CyberChef (34.7k stars), transform.tools, qToolkit.dev
>
> **Current state:** Grayslate has **46 built-in transformations** across JSON, text, encoding, numeric conversion, and format conversion. No user-definable transformations exist yet.

---

## Summary

Grayslate already covers ~60% of what a developer scratchpad needs. The biggest gaps are in **hashing**, **HTML entity encoding**, **JWT inspection**, **SQL/XML formatting**, **timestamp conversion**, **code generation from JSON**, and **regex-filtered line operations**. These are nearly universal across competitor tools and would fill clear use cases.

---

## Proposed New Transformations

### Priority Legend

- **HIGH** — Present in 3+ competitor tools; addresses a daily developer workflow; fill a visible gap
- **MEDIUM** — Present in 1–2 competitor tools; useful but narrower audience
- **LOW** — Niche use case; nice-to-have but not essential for a scratchpad

---

## 1. Hash & Checksum Generators

| # | Transformation | Use Case | Competitors That Have It | Priority |
|---|---------------|----------|--------------------------|----------|
| 1 | **MD5 Hash** | Quickly generate MD5 checksums for file integrity checks, cache keys, or comparing password hashes. | Boop, DevToys, CyberChef, qToolkit | **HIGH** |
| 2 | **SHA-1 Hash** | Generate SHA-1 digests for legacy system compatibility or commit hashes. | DevToys, CyberChef, qToolkit | **HIGH** |
| 3 | **SHA-256 Hash** | Industry-standard cryptographic hash for integrity verification, content fingerprinting. | DevToys, CyberChef, qToolkit | **HIGH** |
| 4 | **SHA-512 Hash** | Stronger hash for high-security contexts. | DevToys, CyberChef | **MEDIUM** |
| 5 | **CRC32 Checksum** | Lightweight checksum for quick data integrity checks in streams/transfers. | CyberChef | **MEDIUM** |

**Value:** Hashing is the most-requested missing feature. Every competitor tool includes it. It's a fundamental developer need: "What's the MD5 of this?" without visiting an untrusted website.

**Implementation note:** Could be a single "Hash" transformation with an algorithm parameter, or discrete entries. The Rust `md5`, `sha1`, `sha2` crates are mature and well-audited.

---

## 2. Encoding & Decoding

| # | Transformation | Use Case | Competitors That Have It | Priority |
|---|---------------|----------|--------------------------|----------|
| 6 | **HTML Entity Encode** | Encode `<`, `>`, `&`, `"` for safe HTML embedding. Essential when pasting code snippets into HTML templates or documentation. | Boop, DevToys, CyberChef, qToolkit | **HIGH** |
| 7 | **HTML Entity Decode** | Decode `&amp;`, `&lt;`, `&gt;` back to characters. Debugging encoded HTML/XML payloads. | Boop, DevToys, CyberChef, qToolkit | **HIGH** |
| 8 | **JWT Decoder** | Decode JWT header and payload (Base64URL) to inspect claims without verifying signature. Daily need for API/debugging. | DevToys, qToolkit, CyberChef | **HIGH** |
| 9 | **Base64URL Encode** | URL-safe Base64 encoding (uses `-` and `_` instead of `+` and `/`, no padding). Used in JWTs, OAuth state params. | CyberChef | **MEDIUM** |
| 10 | **Base64URL Decode** | Decode URL-safe Base64 back to text. | CyberChef | **MEDIUM** |
| 11 | **Hex Dump** | View binary data as a hex+ASCII dump (like `xxd`). Useful for inspecting binary payloads, file headers, network packets. | CyberChef | **MEDIUM** |
| 12 | **GZip Compress** | Compress text to gzip (useful for HTTP debugging, inspecting compressed API payloads). | DevToys, CyberChef | **MEDIUM** |
| 13 | **GZip Decompress** | Decompress gzip data to readable text. | DevToys, CyberChef | **MEDIUM** |
| 14 | **Unicode Escape** (`\uXXXX`) | Escape non-ASCII characters to `\u` sequences. Useful for JSON/JS string literal debugging. | Boop, CyberChef | **MEDIUM** |
| 15 | **Unicode Unescape** | Convert `\uXXXX` back to readable characters. | Boop, CyberChef | **MEDIUM** |

---

## 3. Formatters

| # | Transformation | Use Case | Competitors That Have It | Priority |
|---|---------------|----------|--------------------------|----------|
| 16 | **Format SQL** | Pretty-print SQL queries with consistent indentation. ORM debugging, inline SQL cleanup. | DevToys, qToolkit | **HIGH** |
| 17 | **Format XML** | Pretty-print XML documents. SOAP responses, config files, SVG cleanup. | Boop, DevToys, qToolkit | **HIGH** |
| 18 | **Minify XML** | Remove whitespace from XML. Reducing payload size. | CyberChef | **MEDIUM** |
| 19 | **Validate XML** | Check XML well-formedness and optionally against XSD/DTD. | DevToys | **MEDIUM** |
| 20 | **Format TOML** | Pretty-print TOML documents. Cargo.toml, pyproject.toml cleanup. | (growing format; transform.tools supports TOML conversion) | **MEDIUM** |

---

## 4. Format Conversion (Cross-Language)

| # | Transformation | Use Case | Competitors That Have It | Priority |
|---|---------------|----------|--------------------------|----------|
| 21 | **XML → JSON** | Convert XML documents to JSON. API integration, data pipeline conversion. | transform.tools, CyberChef | **HIGH** |
| 22 | **JSON → XML** | Convert JSON to XML. SOAP/microservice interop. | transform.tools | **MEDIUM** |
| 23 | **JSON → TOML** | Convert JSON config to TOML format. | transform.tools | **MEDIUM** |
| 24 | **TOML → JSON** | Convert TOML config to JSON. | transform.tools | **MEDIUM** |
| 25 | **YAML → TOML** | Convert YAML to TOML. | transform.tools | **LOW** |
| 26 | **TOML → YAML** | Convert TOML to YAML. | transform.tools | **LOW** |
| 27 | **Markdown → HTML** | Convert Markdown to HTML for embedding/export. (Grayslate has preview; actual conversion to HTML in-editor is different.) | DevToys, transform.tools, qToolkit | **MEDIUM** |
| 28 | **CSV → Markdown Table** | Convert CSV data to a GitHub-Flavored Markdown table for README/docs. | Boop (community script) | **MEDIUM** |
| 29 | **TSV → JSON** | Tab-separated values to JSON. Common in data export pipelines. | Boop (community script) | **LOW** |

---

## 5. Code Generation from JSON

| # | Transformation | Use Case | Competitors That Have It | Priority |
|---|---------------|----------|--------------------------|----------|
| 30 | **JSON → TypeScript Interface** | Generate typed TypeScript interfaces from JSON. Extremely popular: save 5–10 minutes per API integration. | transform.tools, qToolkit, DevToys extension | **HIGH** |
| 31 | **JSON → Rust Struct** (serde) | Generate `#[derive(Serialize, Deserialize)]` Rust structs. Natural fit for a Tauri-based app. | transform.tools, qToolkit | **MEDIUM** |
| 32 | **JSON → Go Struct** | Generate Go structs with JSON tags. | transform.tools, qToolkit | **MEDIUM** |
| 33 | **JSON → JSON Schema** | Generate a JSON Schema definition from example JSON. API documentation, validation. | transform.tools, DevToys extension | **MEDIUM** |
| 34 | **JSON → Python Dataclass** | Generate Python dataclasses or Pydantic models. | transform.tools | **LOW** |

---

## 6. Timestamp Utilities

| # | Transformation | Use Case | Competitors That Have It | Priority |
|---|---------------|----------|--------------------------|----------|
| 35 | **Unix Timestamp → Human Date** | Convert epoch seconds/millis to readable UTC/local dates. Debugging logs, API responses. | DevToys, qToolkit, Boop, CyberChef | **HIGH** |
| 36 | **Human Date → Unix Timestamp** | Convert a date string to epoch seconds. Generating timestamps for queries/configs. | DevToys, qToolkit, CyberChef | **HIGH** |
| 37 | **Date Format Converter** | Convert between date string formats (ISO 8601, RFC 2822, US, EU, etc.). | CyberChef | **MEDIUM** |

---

## 7. Generators

| # | Transformation | Use Case | Competitors That Have It | Priority |
|---|---------------|----------|--------------------------|----------|
| 38 | **UUID v4 Generator** | Generate random UUIDs. Test data, unique identifiers, database seeding. | DevToys, qToolkit | **HIGH** |
| 39 | **Lorem Ipsum Generator** | Generate placeholder text for UI mockups, testing. | DevToys, qToolkit | **MEDIUM** |
| 40 | **Password Generator** | Generate cryptographically random passwords with configurable length/character sets. | DevToys | **LOW** |

---

## 8. Color Utilities

| # | Transformation | Use Case | Competitors That Have It | Priority |
|---|---------------|----------|--------------------------|----------|
| 41 | **HEX ↔ RGB Color Converter** | Convert between `#RRGGBB`, `rgb(r, g, b)`, `rgba()`. Daily CSS/tailwind workflow. | qToolkit, Boop (community) | **MEDIUM** |
| 42 | **HEX ↔ HSL Color Converter** | Convert between HEX and HSL for design adjustments. | qToolkit | **LOW** |

---

## 9. Text Manipulation (Extensions to Existing)

| # | Transformation | Use Case | Competitors That Have It | Priority |
|---|---------------|----------|--------------------------|----------|
| 43 | **Filter Lines (Keep Matching Regex)** | Keep only lines that match a regex. Log filtering, data extraction. | CyberChef, DevToys | **HIGH** |
| 44 | **Filter Lines (Remove Matching Regex)** | Remove lines matching a regex. Clean up logs, strip noise. | CyberChef | **HIGH** |
| 45 | **Add Line Numbers** | Prefix each line with its line number (e.g., `1: `). Code review, documentation. | CyberChef, DevToys | **MEDIUM** |
| 46 | **Remove Line Numbers / Strip Prefix** | Strip a fixed-length prefix or line numbers from each line. Cleaning copied code from websites/PDFs. | No direct equivalent, but fills a real need | **MEDIUM** |
| 47 | **Wrap Lines at Column Width** | Hard-wrap text at a specified character column. Prose formatting, commit messages. | DevToys | **MEDIUM** |
| 48 | **Indent Lines** | Add N spaces/tabs to the start of every line. Code formatting. | CyberChef | **MEDIUM** |
| 49 | **Unindent Lines** | Remove N characters/spaces from the start of every line. | CyberChef | **MEDIUM** |
| 50 | **Sort Lines — Numeric** | Sort lines numerically (not alphabetically). Log analysis. | CyberChef | **MEDIUM** |
| 51 | **Sort Lines — Reverse** | Sort in descending order. | CyberChef | **LOW** |
| 52 | **Sort Lines — By Length** | Sort by line length (shortest/longest first). | CyberChef | **LOW** |
| 53 | **Shuffle Lines** | Randomize line order. Test data generation. | Boop (community), CyberChef | **LOW** |
| 54 | **Find & Replace (across entire document, with regex)** | Regex-based find & replace. (Grayslate has CodeMirror search; this is a batch text transform.) | DevToys | **MEDIUM** |
| 55 | **Extract Regex Captures** | Extract all capture groups from a regex into structured output. Data scraping from logs. | CyberChef | **MEDIUM** |

---

## 10. Unique-Line Operations

| # | Transformation | Use Case | Competitors That Have It | Priority |
|---|---------------|----------|--------------------------|----------|
| 56 | **Unique Lines Only** | Output only lines that appear exactly once (strip all duplicates entirely). Data dedup analysis. | CyberChef | **MEDIUM** |
| 57 | **Count Line Occurrences** | Count how many times each unique line appears, output `count\tline`. Frequency analysis. | CyberChef | **MEDIUM** |
| 58 | **Intersection of Lines** | Given two sets of lines, output lines present in both. Set operation. | Boop (LineComparer), CyberChef | **LOW** | *(Requires two-input UI, not a scratchpad fit.)* |

---

## 11. Casing (Extensions)

| # | Transformation | Use Case | Competitors That Have It | Priority |
|---|---------------|----------|--------------------------|----------|
| 59 | **PascalCase** | Convert text to PascalCase (upper camel). Language convention for class names in C#, Java, TypeScript. | (Grayslate has camelCase already; PascalCase is a natural pair.) | **MEDIUM** |
| 60 | **Sentence case** | First letter of each sentence capitalized. Prose editing. | DevToys | **LOW** |

---

## 12. JSON-Specific Tools

| # | Transformation | Use Case | Competitors That Have It | Priority |
|---|---------------|----------|--------------------------|----------|
| 61 | **JSON Diff** | Compare two JSON documents side-by-side and highlight differences. API response comparison, config drift detection. | qToolkit | **MEDIUM** | *(Requires multi-input UI.)* |
| 62 | **JSONPath Query** | Query/transform JSON using JSONPath expressions. API response filtering. | DevToys | **MEDIUM** |
| 63 | **Flatten JSON** | Convert nested JSON to flat dot-notation keys. CSV export prep, database mapping. | (unique to data engineering tools) | **LOW** |
| 64 | **Unflatten JSON** | Convert flat dot-notation keys back to nested JSON. | (unique) | **LOW** |

---

## 13. String / Text Utilities

| # | Transformation | Use Case | Competitors That Have It | Priority |
|---|---------------|----------|--------------------------|----------|
| 65 | **Count Duplicate Lines** | Count and optionally group duplicate lines. Like `sort | uniq -c`. | (unique) | **MEDIUM** |
| 66 | **Escape String (C-style)** | Escape `\n`, `\t`, `\\`, `\"` for C/Java/JS string literals. Embedding text in source code. | CyberChef, Boop | **MEDIUM** |
| 67 | **Unescape String (C-style)** | Convert `\n` → newline, `\t` → tab, etc. Debugging escaped strings from logs/APIs. | CyberChef, Boop | **MEDIUM** |
| 68 | **ROT47** | Full ASCII rotation cipher (extends ROT13 to all printable ASCII). Light obfuscation. | CyberChef | **LOW** |

---

## 14. Character Encoding

| # | Transformation | Use Case | Competitors That Have It | Priority |
|---|---------------|----------|--------------------------|----------|
| 69 | **Character Encoding Converter** | Convert between UTF-8, UTF-16LE/BE, Latin-1, Windows-1252. Fixing mojibake, byte-stream debugging. | CyberChef | **LOW** |

---

## Quick-Reference: Top 15 by Priority

| Rank | Transformation | Priority | Impact |
|------|---------------|----------|--------|
| 1 | **SHA-256 / MD5 / SHA-1 Hash** | HIGH | Fundamental missing feature across all competitors |
| 2 | **HTML Entity Encode/Decode** | HIGH | 4/4 competitor tools have this |
| 3 | **Unix Timestamp ↔ Human Date** | HIGH | 5/5 tools have this; daily debugging need |
| 4 | **JWT Decoder** | HIGH | Universal API debugging need |
| 5 | **Filter Lines (regex keep/remove)** | HIGH | Powerful text utility; fills real gap |
| 6 | **Format SQL** | HIGH | ORM/inline SQL debugging |
| 7 | **Format XML** | HIGH | Major format with no formatting support yet |
| 8 | **XML → JSON** | HIGH | Common integration/conversion need |
| 9 | **JSON → TypeScript Interface** | HIGH | Huge time-saver; popular in transform.tools |
| 10 | **UUID v4 Generator** | HIGH | Test data generation need |
| 11 | **Unicode Escape/Unescape** | MEDIUM | Debugging multilingual content |
| 12 | **GZip Compress/Decompress** | MEDIUM | HTTP/debugging use case |
| 13 | **C-style String Escape/Unescape** | MEDIUM | Embedding text in code |
| 14 | **JSON → Rust Struct** | MEDIUM | Natural fit for Tauri-based app |
| 15 | **Add/Remove Line Numbers** | MEDIUM | Code review, cleaning copied text |

---

## What Competitor Tools Have That Grayslate Should NOT Add

These exist in competitor tools but don't fit a lightweight scratchpad model:

- **Image compression/conversion** (qToolkit, DevToys) — Requires binary file handling; Grayslate is a text scratchpad.
- **PDF manipulation** (qToolkit) — Different product category entirely.
- **AES/DES/RC4 encryption** (CyberChef) — Too domain-specific (forensics/intelligence); security liability.
- **X.509 certificate parsing** (CyberChef) — Too niche.
- **IPv6/IPv4 parsing** (CyberChef) — Infrequent need for a scratchpad.
- **Color blindness simulator** (DevToys) — Different domain (accessibility testing).
- **QR Code generator** (DevToys, qToolkit) — Requires graphical output.
- **Password strength tester** (DevToys) — Outside scope.
- **CSV table editor** — Already exists in Grayslate as a separate mode.
- **Background remover / portrait blur / sticker maker** (qToolkit) — Image editing, not text transformation.
- **Cron expression builder** (qToolkit, DevToys) — Better as a separate tool.

---

## Implementation Approach Suggestions

1. **Batch 1 (High Priority — ~15 transformations):** Hashes, HTML entities, timestamps, JWT decoder, SQL/XML formatting, filter lines, UUID generator, JSON → TypeScript. These fill the most glaring gaps and match the competitive baseline.

2. **Batch 2 (Medium Priority — ~20 transformations):** GZip, Unicode escaping, JSON → Rust/Go structs, line number operations, indent/unindent, wrap lines, Markdown → HTML, CSV → Markdown table, regex extract, PascalCase, TOML conversions.

3. **Batch 3 (Low Priority — ~10 transformations):** Shuffle lines, ROT47, sentence case, flatten/unflatten JSON, character encoding converter.

4. **Architecture:** All proposed transforms fit the existing `TransformationContext` + `dispatch_transformation` pattern. No new IPC protocol needed. Rust ecosystem has mature crates for all proposed features (`md5`, `sha2`, `jsonwebtoken`, `chrono`, `uuid`, `sqlformat`, `quick-xml`, `html-escape`, `flate2`, `heck` for PascalCase, `unescape`).
