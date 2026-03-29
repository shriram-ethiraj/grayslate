# grayslate-langdetect

Content-based programming language detection for source code, config files, data formats, and prose. Identifies **43 languages** from content analysis, filenames, shebangs, and structural patterns using a multi-phase pipeline — with **zero dependencies** beyond `regex`.

## Features

- **Multi-phase detection pipeline** with increasing confidence:
  1. **Extension/filename** — instant, deterministic (`.py` → Python, `Dockerfile` → Dockerfile)
  2. **Shebang** — `#!/usr/bin/env python3` → Python
  3. **Structural signals** — high-confidence content patterns (e.g. `<!DOCTYPE html>` → HTML)
  4. **Heuristic scoring** — family-gated anchor/hint scoring with disambiguation
- **Content family classification** — categorizes content as Code, Prose, Markup, Data, Shell, or Config before scoring, reducing false positives
- **Abstains when uncertain** — returns `None` rather than guessing
- **Fast** — analyses at most 50 KB of content, typically completes in < 10ms
- **Zero external dependencies** except `regex`

## Quick Start

```toml
[dependencies]
grayslate-langdetect = "0.1"
```

```rust
use grayslate_langdetect::detect_language;

// From content only
let lang = detect_language("def hello():\n    print('hi')", None);
assert_eq!(lang, Some("python"));

// From filename only
let lang = detect_language("", Some("app.tsx"));
assert_eq!(lang, Some("typescript"));

// From both (filename takes priority for extension-based detection)
let lang = detect_language("{}", Some("config.json"));
assert_eq!(lang, Some("json"));

// Uncertain content returns None
let lang = detect_language("hello world", None);
assert_eq!(lang, None);
```

## Supported Languages

| Category | Languages |
|----------|-----------|
| **Systems** | C, C++, Rust, Go, Swift, Dart |
| **JVM** | Java, Kotlin, Scala, Clojure |
| **Scripting** | Python, Ruby, Perl, PHP, Shell, PowerShell, CMD |
| **Web** | JavaScript, TypeScript, Angular, HTML, CSS, SCSS, Sass |
| **Frameworks** | Svelte, Vue, Jinja |
| **Data** | JSON, YAML, TOML, CSV, XML, SQL |
| **.NET** | C# |
| **ObjC** | Objective-C, Objective-C++ |
| **Config** | Dockerfile, Nginx |
| **Markup** | Markdown |
| **Prose** | Email, Prompt, Text |

## API

### `detect_language(content: &str, filename: Option<&str>) -> Option<&'static str>`

Main entry point. Returns a language ID string (e.g. `"python"`, `"json"`, `"rust"`) or `None` when detection is uncertain.

### Submodules

| Module | Purpose |
|--------|---------|
| `extension` | Filename/extension-based detection |
| `shebang` | Shebang line detection |
| `structural` | Structural pattern detection |
| `family` | Content family classification |
| `features` | Feature extraction from content |
| `scoring` | Candidate scoring pipeline |
| `disambiguation` | Neighbor language disambiguation |
| `languages` | Language definitions registry |

## Adding a New Language

1. **Create a language definition file** in `src/languages/your_lang.rs`:

```rust
use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "yourlang",
        // Phase 1: file extensions (include the dot)
        extensions: &[".yl", ".yourlang"],
        // Phase 1: exact filename matches
        filenames: &[],
        // Phase 1: glob-style filename patterns
        filename_patterns: &[],
        // Phase 2: shebang interpreter names
        shebangs: &[],
        // Phase 3: structural detection priority (lower = checked first)
        structural_priority: None,  // or Some(5) with a detect function
        structural_detect: None,    // or Some(is_likely_yourlang)
        // Phase 4: keywords for scoring
        keywords: &["keyword1", "keyword2"],
        builtins: &["builtin1"],
        // Family-gated scoring fields
        content_families: &[ContentFamily::Code],
        anchors: &[
            // High-confidence patterns (weight 3-5)
            wp!(r"(?m)^\s*yourlang_keyword\s", 4),
        ],
        hints: &[
            // Supporting patterns (weight 1-2)
            wp!(r"(?m)^\s*common_pattern\b", 2),
        ],
        disqualifiers: &[
            // Patterns that suggest this is NOT your language (negative weight)
            wp!(r"(?m)^\s*other_lang_keyword\b", -3),
        ],
    }
}
```

2. **Register it** in `src/languages/mod.rs`:
   - Add `pub(super) mod your_lang;` to the module declarations
   - Add `your_lang::definition()` to the `all_definitions()` function

3. **Add tests** in your language file or in `src/lib.rs` test module

4. **Run tests**: `cargo test`

## Architecture

```
Detection Pipeline
┌────────────────────────────────────────────────────────┐
│ Phase 0: Deterministic                                 │
│   Extension → Shebang → Strong Structural              │
├────────────────────────────────────────────────────────┤
│ Phase 1: Content Family Classification                 │
│   Prose / Code / Data / Markup / Shell / Config        │
├────────────────────────────────────────────────────────┤
│ Phase 2: Family-Gated Candidate Scoring                │
│   Only languages matching the family are scored        │
│   Anchors (strong) + Hints (weak) → score per language │
├────────────────────────────────────────────────────────┤
│ Phase 3: Disambiguation                                │
│   Superset pairs (JS/TS, C/C++, ObjC/ObjC++) resolved │
│   Score gap threshold eliminates weak runners-up       │
├────────────────────────────────────────────────────────┤
│ Phase 4: Confidence Gate                               │
│   Minimum score threshold → None if unsure             │
└────────────────────────────────────────────────────────┘
```

## License

MIT
