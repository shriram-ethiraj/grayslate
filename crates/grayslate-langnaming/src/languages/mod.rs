/// Per-language naming definition registry.
///
/// Each language lives in its own file and exports a `definition()` function
/// returning a [`NamingDefinition`] that describes:
///
///   - the canonical save extension
///   - the extraction function that derives a filename stem from content
///
/// The registry compiles all definitions into a single `LazyLock` lookup map
/// so that `suggest_stem` can dispatch by language ID in O(1).
///
/// **Adding naming support for a new language = create one file here +
/// register it in `all_definitions()`.**
use std::collections::HashMap;
use std::sync::LazyLock;

// ── Per-language modules ─────────────────────────────────────────────────
mod angular;
mod c;
mod clojure;
mod cpp;
mod csharp;
mod css;
mod csv_lang;
mod dart;
mod dockerfile;
mod email;
mod go;
mod html;
mod java;
mod javascript;
mod jinja;
mod json_lang;
mod kotlin;
mod markdown;
mod nginx;
mod objectivec;
mod objectivecpp;
mod cmd;
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
mod toml_lang;
mod typescript;
mod vue;
mod xml;
mod yaml;

// ── Public types ─────────────────────────────────────────────────────────

/// How a language extracts a filename stem from content.
pub enum Extractor {
    /// Declarative: a list of regex patterns with priorities + optional
    /// language-specific noise filter. The shared pipeline in `code.rs`
    /// handles matching, symbol collection, noise filtering, priority
    /// ranking, deduplication, and stem generation automatically.
    ///
    /// Best for languages where extraction is a flat list of regex patterns
    /// (most code languages).
    Patterns {
        symbols: &'static [crate::code::SymbolPattern],
        noise: &'static [&'static str],
    },
    /// Custom extraction function for languages that need multi-stage logic,
    /// known-pattern detection, config cascades, or other complex pipelines.
    ///
    /// Takes bounded content, returns a raw stem or `None`.
    Custom(fn(&str) -> Option<String>),
}

/// Complete definition of a language's naming behaviour.
///
/// Every field that describes how a language generates a filename stem lives
/// here, so that adding naming support for a new language is a single-file
/// operation.
///
/// Two styles of extraction are supported:
///   - **Declarative** (`Extractor::Patterns`): just list regex patterns with
///     priorities. The shared pipeline does the rest.
///   - **Custom** (`Extractor::Custom`): provide a function with full control
///     over extraction logic.
pub struct NamingDefinition {
    /// Canonical language name (must match detection output, e.g. `"python"`).
    pub name: &'static str,

    /// Canonical save extension (e.g. `"py"`, `"rs"`).
    pub extension: &'static str,

    /// Extraction strategy: declarative patterns or custom function.
    pub extract: Extractor,
}

// ── Registry ─────────────────────────────────────────────────────────────

fn all_definitions() -> Vec<NamingDefinition> {
    vec![
        // Structured formats
        csv_lang::definition(),
        json_lang::definition(),
        yaml::definition(),
        toml_lang::definition(),
        // Markup / document formats
        xml::definition(),
        html::definition(),
        svelte::definition(),
        vue::definition(),
        angular::definition(),
        markdown::definition(),
        // Code — regex backed
        javascript::definition(),
        typescript::definition(),
        python::definition(),
        rust_lang::definition(),
        java::definition(),
        kotlin::definition(),
        scala::definition(),
        go::definition(),
        c::definition(),
        cpp::definition(),
        // Code — regex backed
        csharp::definition(),
        swift::definition(),
        objectivec::definition(),
        objectivecpp::definition(),
        ruby::definition(),
        php::definition(),
        dart::definition(),
        shell::definition(),
        cmd::definition(),
        dockerfile::definition(),
        email::definition(),
        // SQL
        sql::definition(),
        // Style languages
        css::definition(),
        sass::definition(),
        scss::definition(),
        // Other / currently prose-backed
        perl::definition(),
        clojure::definition(),
        jinja::definition(),
        powershell::definition(),
        prompt::definition(),
        nginx::definition(),
        // Catch-all
        text::definition(),
    ]
}

/// Compiled lookup: language ID → `NamingDefinition`.
pub static NAMING_MAP: LazyLock<HashMap<&'static str, NamingDefinition>> = LazyLock::new(|| {
    let defs = all_definitions();
    let mut map = HashMap::with_capacity(defs.len());
    for def in defs {
        map.insert(def.name, def);
    }
    map
});

/// Default definition used when a language ID isn't in the registry.
pub static DEFAULT: LazyLock<NamingDefinition> = LazyLock::new(text::definition);

/// Look up a naming definition for the given language ID.
/// Falls back to the `text` (prose) definition for unknown languages.
pub fn lookup(language_id: &str) -> &'static NamingDefinition {
    NAMING_MAP
        .get(language_id)
        .unwrap_or(&*DEFAULT)
}
