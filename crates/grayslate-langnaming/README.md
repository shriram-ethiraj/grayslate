# grayslate-langnaming

Smart filename stem suggestion from source code content. Given a document and its language, extracts a meaningful filename using pattern-based heuristics — with **zero dependencies** beyond `regex` and `yake-rust` (keyword extraction for prose).

## Features

- **Pattern-based extraction** for 43+ programming languages
- **Two extraction modes:**
  - `Extractor::Patterns` — declarative symbol patterns (class, function, struct, etc.) with priority ordering
  - `Extractor::Custom` — language-specific extraction functions for formats that need specialized parsing
- **Slug-safe output** — produces clean, filesystem-safe filenames (`my-http-handler`, `user-auth-controller`)
- **Smart noise filtering** — ignores generic names like `main`, `index`, `app`, `test`, `utils`
- **Prose support** — extracts keywords from email, prompts, and plain text using YAKE
- **Structured data support** — recognizes package.json, tsconfig, OpenAPI, Kubernetes manifests, etc.

## Quick Start

```toml
[dependencies]
grayslate-langnaming = "0.1"
```

```rust
use grayslate_langnaming::{suggest_stem, language_to_extension, fallback_stem, slugify};

// Suggest a filename from Python source
let stem = suggest_stem("class UserAuthController:\n    pass", "python");
assert_eq!(stem, Some("user-auth-controller".to_string()));

// Get the canonical extension for a language
let ext = language_to_extension("python");
assert_eq!(ext, "py");

// Fallback when no name can be derived
let fallback = fallback_stem(); // "slate-28-mar-2026-1130"

// Slugify any raw text into a safe filename
let slug = slugify("Hello World: A Test!");
assert_eq!(slug, Some("hello-world-a-test".to_string()));
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

### `suggest_stem(content: &str, language_hint: &str) -> Option<String>`

Main entry point. Given document content and a language ID (e.g. `"python"`, `"json"`), returns a sanitized filename stem or `None` when no meaningful name can be derived.

### `language_to_extension(language_hint: &str) -> &'static str`

Maps a language ID to its canonical file extension (e.g. `"python"` → `"py"`, `"typescript"` → `"ts"`).

### `fallback_stem() -> String`

Returns a timestamp-based fallback stem: `"slate-DD-mon-YYYY-HHMM"`.

### `slugify(raw: &str) -> Option<String>`

Converts raw text into a filesystem-safe slug (lowercase, hyphens, no special chars).

### Modules

| Module | Purpose |
|--------|---------|
| `code` | Symbol extraction (class, function, struct patterns) |
| `markup` | HTML/XML/Markdown/Svelte/Vue extraction |
| `structured` | JSON/YAML/TOML/CSV extraction |
| `sql` | SQL extraction (tables, views, procedures) |
| `prose` | Email/prompt/text keyword extraction (YAKE) |
| `shared` | Slugify, fallback stem, finalization |
| `model` | Core types (`StemKind`, `ExtractedName`, `MAX_TOKENS`) |
| `languages` | Language definitions registry |

## Adding a New Language

### 1. Create a language file

Create `src/languages/your_lang.rs`:

```rust
use super::{Extractor, NamingDefinition};

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "yourlang",
        extension: "yl",
        extract: Extractor::Patterns {
            symbols: &[
                // Patterns are tried in order; first match at highest priority wins
                crate::code::SymbolPattern {
                    pattern: r"(?m)^\s*class\s+([A-Z]\w+)",
                    priority: 3,
                },
                crate::code::SymbolPattern {
                    pattern: r"(?m)^\s*fn\s+(\w+)",
                    priority: 2,
                },
            ],
            noise: &["main", "index", "test"],
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::suggest_stem;
    use crate::shared::slugify;

    #[test]
    fn class_extraction() {
        let src = "class MyWidget:\n    pass";
        let stem = suggest_stem(src, "yourlang").unwrap();
        assert!(stem.contains("my-widget"));
    }
}
```

### 2. For custom extraction

Use `Extractor::Custom` when declarative patterns aren't enough:

```rust
extract: Extractor::Custom(|content| {
    // Your extraction logic here
    // Return Option<String> — the raw extracted name (will be slugified)
    Some("my-extracted-name".to_string())
}),
```

### 3. Register it

In `src/languages/mod.rs`:
- Add `pub(super) mod your_lang;`
- Add `your_lang::definition()` to the `all_definitions()` function

### 4. Run tests

```bash
cargo test
```

## Architecture

```
suggest_stem(content, language_hint)
  │
  ├─ languages::lookup(hint) → NamingDefinition
  │
  ├─ Extractor::Patterns
  │   └─ code::extract_from_patterns(content, symbols, noise)
  │       ├─ Try each regex pattern
  │       ├─ Filter noise names
  │       ├─ Sort by priority
  │       └─ Take top MAX_TOKENS (currently 1)
  │
  ├─ Extractor::Custom(fn)
  │   └─ Language-specific extraction
  │       ├─ markup::extract_xml_html()   (HTML, Svelte, Vue, Angular, Jinja)
  │       ├─ markup::extract_markdown()   (Markdown)
  │       ├─ structured::extract_json()   (JSON)
  │       ├─ structured::extract_toml()   (TOML)
  │       ├─ structured::extract_csv()    (CSV)
  │       ├─ sql::extract_sql()           (SQL)
  │       └─ prose::extract_prose()       (Email, Prompt, Text)
  │
  └─ shared::finalize(raw, kind)
      ├─ slugify() — lowercase, hyphens, truncate
      └─ Append kind suffix ("-email", "-prompt") if applicable
```

## Design Decisions

- **MAX_TOKENS = 1**: Naming produces exactly one token per stem for consistency
- **Priority ordering**: Higher-priority symbols (class > function > variable) win ties
- **Noise filtering**: Generic names (`main`, `index`, `app`, `test`, etc.) are skipped
- **No external parsers**: All extraction is regex-based (no AST parsing)
- **Slug length cap**: Stems are truncated to 60 characters

## License

MIT
