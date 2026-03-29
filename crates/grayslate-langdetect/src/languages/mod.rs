/// Language definition registry.
///
/// Each language lives in its own file and exports a `definition()` function.
/// The registry compiles all definitions into optimised lookups for every
/// phase of the detection pipeline (extension, shebang, structural, family-gated scoring).
///
/// **Adding a new language = creating one file here. No other changes needed.**
use regex::Regex;
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
mod email;
mod go;
mod html;
mod java;
mod javascript;
mod jinja;
mod json;
mod kotlin;
pub(crate) mod markdown;
mod cmd;
mod nginx;
mod objectivec;
mod objectivecpp;
mod perl;
mod php;
mod powershell;
mod prompt;
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
        $crate::languages::WeightedPattern {
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
    ///
    /// The priority also determines *when* the detector runs:
    /// - **≤ `STRONG_STRUCTURAL_CUTOFF` (70)** — strong/deterministic, runs
    ///   in Phase 0 *before* the family classifier.
    /// - **> 70** — soft/family-gated, runs in Phase 2 *after* the family
    ///   classifier, gated by `content_families`.
    pub structural_priority: Option<u8>,

    /// Structural detection function: `(trimmed_content, was_sliced) -> bool`.
    /// Only called when `structural_priority` is `Some`.
    pub structural_detect: Option<fn(&str, bool) -> bool>,

    // ── Phase 4: Family-gated scoring ────────────────────────

    /// Reserved keywords unique or nearly unique to this language.
    /// Matched as whole words via `\b<keyword>\b` against tokenized content.
    /// Each unique hit adds +1 to the keyword score.
    pub keywords: &'static [&'static str],

    /// Built-in functions, types, and modules distinctive to this language.
    /// Matched the same way as keywords. Each unique hit adds +1.
    pub builtins: &'static [&'static str],

    /// Content families this language belongs to (e.g. Code, StructuredData).
    /// Used by the family classifier to gate which languages are considered.
    pub content_families: &'static [ContentFamily],

    /// High-confidence positive signals (weight ≥ 4).
    /// Nearly exclusive to this language — a match is strong evidence.
    pub anchors: &'static [WeightedPattern],

    /// Lower-confidence secondary signals (weight 1–3).
    /// Supporting evidence that needs corroboration.
    pub hints: &'static [WeightedPattern],

    /// Patterns that definitively rule OUT this language.
    /// Used sparingly — only for truly impossible combinations.
    pub disqualifiers: &'static [WeightedPattern],
}

/// Re-export from family module.
pub use crate::family::ContentFamily;

impl Default for LanguageDefinition {
    fn default() -> Self {
        Self {
            name: "unknown",
            extensions: &[],
            filenames: &[],
            filename_patterns: &[],
            shebangs: &[],
            structural_priority: None,
            structural_detect: None,
            keywords: &[],
            builtins: &[],
            content_families: &[],
            anchors: &[],
            hints: &[],
            disqualifiers: &[],
        }
    }
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
        email::definition(),
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
        prompt::definition(),
        python::definition(),
        ruby::definition(),
        rust_lang::definition(),
        sass::definition(),
        scala::definition(),
        scss::definition(),
        shell::definition(),
        cmd::definition(),
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

/// Soft structural entry — includes content families for family gating.
pub(crate) struct SoftStructuralEntry {
    pub name: &'static str,
    pub detect: fn(&str, bool) -> bool,
    pub content_families: &'static [ContentFamily],
}

/// Priority cutoff between strong and soft structural detectors.
///
/// - **Strong (priority ≤ cutoff):** near-deterministic detectors that match
///   unique syntax (e.g. `<?php`, `<?xml`, valid JSON parse, `<!DOCTYPE html>`).
///   Run in Phase 0 before the family classifier.
///
/// - **Soft (priority > cutoff):** family-gated detectors whose patterns CAN appear
///   in other content types (e.g. `# Heading` vs `# comment`, `key: value`).
///   Run in Phase 2 after the family classifier, gated by `content_families`.
///
/// Current assignments:
///   Strong: json(5) php(10) svelte(20) vue(30) html(40) xml(50) dockerfile(60) csv(70)
///   Soft:   markdown(80) scss(90) sass(91) toml(100) sql(110) prompt(115) yaml(120)
const STRONG_STRUCTURAL_CUTOFF: u8 = 70;

/// Strong structural detectors — near-deterministic, run before the family
/// classifier (Phase 0c). Priority ≤ `STRONG_STRUCTURAL_CUTOFF`.
pub(crate) static STRONG_STRUCTURAL: LazyLock<Vec<StructuralEntry>> = LazyLock::new(|| {
    let mut entries: Vec<(u8, StructuralEntry)> = Vec::new();
    for def in all_definitions() {
        if let (Some(prio), Some(detect)) = (def.structural_priority, def.structural_detect) {
            if prio <= STRONG_STRUCTURAL_CUTOFF {
                entries.push((
                    prio,
                    StructuralEntry {
                        name: def.name,
                        detect,
                    },
                ));
            }
        }
    }
    entries.sort_by_key(|(prio, _)| *prio);
    entries.into_iter().map(|(_, entry)| entry).collect()
});

/// Soft structural detectors — run AFTER the family classifier (Phase 2).
/// Priority > `STRONG_STRUCTURAL_CUTOFF`. Only fire when the classified
/// content family matches the detector's `content_families`.
pub(crate) static SOFT_STRUCTURAL: LazyLock<Vec<SoftStructuralEntry>> = LazyLock::new(|| {
    let mut entries: Vec<(u8, SoftStructuralEntry)> = Vec::new();
    for def in all_definitions() {
        if let (Some(prio), Some(detect)) = (def.structural_priority, def.structural_detect) {
            if prio > STRONG_STRUCTURAL_CUTOFF {
                entries.push((
                    prio,
                    SoftStructuralEntry {
                        name: def.name,
                        detect,
                        content_families: def.content_families,
                    },
                ));
            }
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

/// Helper: compile a single WeightedPattern with a nice panic message.
fn compile_wp(lang: &str, wp: &WeightedPattern) -> CompiledPattern {
    CompiledPattern {
        regex: Regex::new(wp.pattern)
            .unwrap_or_else(|e| panic!("[{}] bad pattern `{}`: {}", lang, wp.pattern, e)),
        weight: wp.weight,
    }
}

// ── New: Family-gated compiled registry ──────────────────────────────────

/// Compiled language entry for the family-gated scoring pipeline.
pub(crate) struct CompiledFamilyLanguage {
    pub name: &'static str,
    pub content_families: &'static [ContentFamily],
    pub anchors: Vec<CompiledPattern>,
    pub hints: Vec<CompiledPattern>,
    pub keywords: &'static [&'static str],
    pub builtins: &'static [&'static str],
    pub disqualifiers: Vec<CompiledPattern>,
}

/// All languages with family-gated scoring fields populated, compiled.
/// Only includes languages that have at least one anchor or hint.
pub(crate) static COMPILED_FAMILY: LazyLock<Vec<CompiledFamilyLanguage>> = LazyLock::new(|| {
    let definitions = all_definitions();

    definitions
        .iter()
        .filter(|def| !def.anchors.is_empty() || !def.hints.is_empty())
        .map(|def| CompiledFamilyLanguage {
            name: def.name,
            content_families: def.content_families,
            anchors: def.anchors.iter().map(|wp| compile_wp(def.name, wp)).collect(),
            hints: def.hints.iter().map(|wp| compile_wp(def.name, wp)).collect(),
            keywords: def.keywords,
            builtins: def.builtins,
            disqualifiers: def.disqualifiers.iter().map(|wp| compile_wp(def.name, wp)).collect(),
        })
        .collect()
});
