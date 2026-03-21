/// Language definition registry.
///
/// Each language lives in its own file and exports a `definition()` function.
/// The registry compiles all definitions into optimised lookups for every
/// phase of the detection pipeline (extension, shebang, structural, heuristic).
///
/// **Adding a new language = creating one file here. No other changes needed.**
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

// ── Per-language modules ─────────────────────────────────────────────────
mod angular;
mod c;
mod clojure;
mod cpp;
mod csharp;
mod css;
mod csv;
mod dart;
mod dockerfile;
mod go;
mod html;
mod java;
mod javascript;
mod jinja;
mod json;
mod kotlin;
pub(crate) mod markdown;
mod nginx;
mod objectivec;
mod objectivecpp;
mod perl;
mod php;
mod powershell;
mod python;
mod ruby;
mod rust_lang;
mod sass;
mod scala;
mod scss;
mod shell;
mod sql;
mod svelte;
mod swift;
mod text;
mod toml;
mod typescript;
mod vue;
mod xml;
mod yaml;

// ── Public types ─────────────────────────────────────────────────────────

/// A single regex pattern with a signed weight.
/// Positive weight = signal *for* this language.
/// Negative weight = anti-signal (rules it out).
pub struct WeightedPattern {
    pub pattern: &'static str,
    pub weight: i32,
}

/// Convenience macro — used inside per-language files.
macro_rules! wp {
    ($pat:expr, $w:expr) => {
        $crate::detection::languages::WeightedPattern {
            pattern: $pat,
            weight: $w,
        }
    };
}
pub(crate) use wp;

/// Complete definition of a language's detection fingerprint.
///
/// Every field that describes a language lives here so that adding a new
/// language is a single-file operation.
pub struct LanguageDefinition {
    /// Canonical language name (e.g. "python", "typescript").
    pub name: &'static str,

    // ── Phase 1: Extension & filename detection ──────────────

    /// File extensions that map to this language (with leading dot).
    /// E.g. `&[".py", ".pyi", ".pyw"]`.
    pub extensions: &'static [&'static str],

    /// Exact filenames (lowercased) that map to this language.
    /// E.g. `&["makefile", ".bashrc"]`.
    pub filenames: &'static [&'static str],

    /// Regex patterns matched against the base filename.
    /// E.g. `&[r"^nginx.*\.conf$"]` for nginx config files.
    pub filename_patterns: &'static [&'static str],

    // ── Phase 2: Shebang detection ───────────────────────────

    /// Regex patterns matched against the `#!` line.
    /// E.g. `&[r"\bpython[23w]?\b"]`.
    pub shebangs: &'static [&'static str],

    // ── Phase 3: Structural detection ────────────────────────

    /// Priority for structural detection (lower = checked first).
    /// `None` means this language has no structural detector.
    pub structural_priority: Option<u8>,

    /// Structural detection function: `(trimmed_content, was_sliced) -> bool`.
    /// Only called when `structural_priority` is `Some`.
    pub structural_detect: Option<fn(&str, bool) -> bool>,

    // ── Phase 4: Heuristic scoring ───────────────────────────

    /// Weighted regex patterns — positive signals for this language.
    pub patterns: &'static [WeightedPattern],

    /// Language-specific anti-signals (e.g. Rust `class` → not Rust).
    pub anti_patterns: &'static [WeightedPattern],

    /// If `true`, the language uses `#` as a line-comment character
    /// (Python, Ruby, Shell). Heading anti-signals (`^#{1,6}\s`) are
    /// NOT auto-applied to avoid penalising legitimate comments.
    pub uses_hash_comments: bool,

    /// Reserved keywords unique or nearly unique to this language.
    /// Matched as whole words via `\b<keyword>\b` against tokenized content.
    /// Each unique hit adds +1 to the keyword score.
    pub keywords: &'static [&'static str],

    /// Built-in functions, types, and modules distinctive to this language.
    /// Matched the same way as keywords. Each unique hit adds +1.
    pub builtins: &'static [&'static str],

    /// Optional regex that, if matched, instantly disqualifies this language.
    /// For example, Java cannot contain `<\/` (HTML close tags).
    pub illegal: Option<&'static str>,

    /// Optional base language whose patterns and keywords are inherited.
    /// E.g. TypeScript extends JavaScript, C++ extends C.
    pub extends: Option<&'static str>,
}

// ── Master definition list ───────────────────────────────────────────────

/// All language definitions. Adding a new language file + entry here is all
/// that's needed.
fn all_definitions() -> Vec<LanguageDefinition> {
    vec![
        angular::definition(),
        c::definition(),
        clojure::definition(),
        cpp::definition(),
        csharp::definition(),
        css::definition(),
        csv::definition(),
        dart::definition(),
        dockerfile::definition(),
        go::definition(),
        html::definition(),
        java::definition(),
        javascript::definition(),
        jinja::definition(),
        json::definition(),
        kotlin::definition(),
        markdown::definition(),
        nginx::definition(),
        objectivec::definition(),
        objectivecpp::definition(),
        perl::definition(),
        php::definition(),
        powershell::definition(),
        python::definition(),
        ruby::definition(),
        rust_lang::definition(),
        sass::definition(),
        scala::definition(),
        scss::definition(),
        shell::definition(),
        sql::definition(),
        svelte::definition(),
        swift::definition(),
        text::definition(),
        toml::definition(),
        typescript::definition(),
        vue::definition(),
        xml::definition(),
        yaml::definition(),
    ]
}

// ── Compiled registries ──────────────────────────────────────────────────

/// All supported language names, auto-derived from definitions.
pub(crate) static SUPPORTED_LANGUAGES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    all_definitions().into_iter().map(|d| d.name).collect()
});

/// Extension → language name (lowercased extension without the dot).
pub(crate) static EXTENSION_MAP: LazyLock<Vec<(&'static str, &'static str)>> =
    LazyLock::new(|| {
        let mut map = Vec::new();
        for def in all_definitions() {
            for ext in def.extensions {
                map.push((*ext, def.name));
            }
        }
        map
    });

/// Exact filename → language name.
pub(crate) static FILENAME_MAP: LazyLock<Vec<(&'static str, &'static str)>> =
    LazyLock::new(|| {
        let mut map = Vec::new();
        for def in all_definitions() {
            for fname in def.filenames {
                map.push((*fname, def.name));
            }
        }
        map
    });

/// Compiled filename regex patterns → language name.
pub(crate) static FILENAME_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> =
    LazyLock::new(|| {
        let mut patterns = Vec::new();
        for def in all_definitions() {
            for pat in def.filename_patterns {
                patterns.push((
                    Regex::new(pat).unwrap_or_else(|e| {
                        panic!("[{}] bad filename pattern `{}`: {}", def.name, pat, e)
                    }),
                    def.name,
                ));
            }
        }
        patterns
    });

/// Compiled shebang regex patterns → language name.
pub(crate) static SHEBANG_MAP: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    let mut map = Vec::new();
    for def in all_definitions() {
        for pat in def.shebangs {
            map.push((
                Regex::new(pat).unwrap_or_else(|e| {
                    panic!("[{}] bad shebang pattern `{}`: {}", def.name, pat, e)
                }),
                def.name,
            ));
        }
    }
    map
});

/// Structural detector entry: (language_name, priority, detect_fn).
pub(crate) struct StructuralEntry {
    pub name: &'static str,
    pub detect: fn(&str, bool) -> bool,
}

/// Structural detectors sorted by priority (lower = checked first).
pub(crate) static STRUCTURAL_DETECTORS: LazyLock<Vec<StructuralEntry>> = LazyLock::new(|| {
    let mut entries: Vec<(u8, StructuralEntry)> = Vec::new();
    for def in all_definitions() {
        if let (Some(prio), Some(detect)) = (def.structural_priority, def.structural_detect) {
            entries.push((
                prio,
                StructuralEntry {
                    name: def.name,
                    detect,
                },
            ));
        }
    }
    entries.sort_by_key(|(prio, _)| *prio);
    entries.into_iter().map(|(_, entry)| entry).collect()
});

// ── Heuristic compiled registry ──────────────────────────────────────────

/// A compiled weighted regex ready for scoring.
pub(crate) struct CompiledPattern {
    pub regex: Regex,
    pub weight: i32,
}

/// All patterns (positive + anti + common) for one language, compiled.
pub(crate) struct CompiledLanguage {
    pub name: &'static str,
    pub patterns: Vec<CompiledPattern>,
    /// Set of keywords (lowercase) for fingerprint matching.
    pub keywords: HashSet<&'static str>,
    /// Set of builtins (lowercase) for fingerprint matching.
    pub builtins: HashSet<&'static str>,
    /// Compiled illegal pattern — if matched, score = -∞.
    pub illegal: Option<Regex>,
}

/// Common anti-signal: markdown headings (`# Heading`).
/// Applied to every language that does NOT use `#` comments.
const HEADING_ANTI: WeightedPattern = WeightedPattern {
    pattern: r"(?m)^#{1,6}\s+\S",
    weight: -3,
};

/// Common anti-signal: fenced code blocks (` ``` `).
/// Applied to ALL languages.
const FENCE_ANTI: WeightedPattern = WeightedPattern {
    pattern: r"(?m)^```\w*",
    weight: -3,
};

/// Collect every language definition that has heuristic patterns, compile
/// all patterns once. Handles inheritance: if a definition sets `extends`,
/// the base language's patterns, keywords, and builtins are merged in.
pub(crate) static COMPILED: LazyLock<Vec<CompiledLanguage>> = LazyLock::new(|| {
    let definitions = all_definitions();

    // Index by name for inheritance lookups
    let by_name: HashMap<&str, &LanguageDefinition> =
        definitions.iter().map(|d| (d.name, d)).collect();

    definitions
        .iter()
        .filter(|def| !def.patterns.is_empty() || !def.anti_patterns.is_empty())
        .map(|def| {
            let mut compiled_patterns: Vec<CompiledPattern> = Vec::new();
            let mut keywords: HashSet<&'static str> = HashSet::new();
            let mut builtins: HashSet<&'static str> = HashSet::new();

            // 0. Inheritance — merge base language's patterns/keywords first
            if let Some(base_name) = def.extends {
                if let Some(base) = by_name.get(base_name) {
                    for wp in base.patterns {
                        compiled_patterns.push(compile_wp(base.name, wp));
                    }
                    keywords.extend(base.keywords.iter());
                    builtins.extend(base.builtins.iter());
                }
            }

            // 1. Own positive patterns
            for wp in def.patterns {
                compiled_patterns.push(compile_wp(def.name, wp));
            }

            // 2. Language-specific anti-patterns
            for wp in def.anti_patterns {
                compiled_patterns.push(compile_wp(def.name, wp));
            }

            // 3. Common anti-signals
            if !def.uses_hash_comments {
                compiled_patterns.push(CompiledPattern {
                    regex: Regex::new(HEADING_ANTI.pattern).unwrap(),
                    weight: HEADING_ANTI.weight,
                });
            }
            compiled_patterns.push(CompiledPattern {
                regex: Regex::new(FENCE_ANTI.pattern).unwrap(),
                weight: FENCE_ANTI.weight,
            });

            // 4. Own keywords/builtins (added after base)
            keywords.extend(def.keywords.iter());
            builtins.extend(def.builtins.iter());

            // 5. Illegal pattern
            let illegal = def.illegal.map(|pat| {
                Regex::new(pat)
                    .unwrap_or_else(|e| panic!("[{}] bad illegal pattern `{}`: {}", def.name, pat, e))
            });

            CompiledLanguage {
                name: def.name,
                patterns: compiled_patterns,
                keywords,
                builtins,
                illegal,
            }
        })
        .collect()
});

/// Helper: compile a single WeightedPattern with a nice panic message.
fn compile_wp(lang: &str, wp: &WeightedPattern) -> CompiledPattern {
    CompiledPattern {
        regex: Regex::new(wp.pattern)
            .unwrap_or_else(|e| panic!("[{}] bad pattern `{}`: {}", lang, wp.pattern, e)),
        weight: wp.weight,
    }
}
