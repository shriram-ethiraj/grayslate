/**
 * naming/mod.rs
 *
 * Smart filename stem suggestion for untitled documents.
 *
 * Pipeline:
 *   1. Resolve a language naming profile from `language_hint`.
 *   2. Dispatch to a format-specific extractor group.
 *   3. Sanitize + slugify the raw stem.
 *   4. If nothing useful was found, return None (caller falls back to
 *      slate-<YYYYMMDDHHmmss>).
 *
 * All extractors operate on at most the first MAX_CONTENT_BYTES bytes of the
 * document to keep naming fast even for very large files.
 */
mod code;
mod markup;
mod model;
mod prose;
mod shared;
mod sql;
mod structured;

use self::model::{
    CodeStyle, ExtractorGroup, LanguageNamingProfile, MarkupNamingKind, StructuredNamingKind,
};

pub use self::shared::{fallback_stem, slugify};

/// Returns a sanitized filename stem (no extension, no path separators) or
/// `None` when no useful name can be derived.
pub fn suggest_stem(content: &str, language_hint: &str) -> Option<String> {
    let bounded = shared::bound(content);
    let profile = language_profile(language_hint);
    let raw = match profile.extractor {
        ExtractorGroup::Structured(kind) => match kind {
            StructuredNamingKind::Csv => structured::extract_csv(bounded),
            StructuredNamingKind::Json => structured::extract_json(bounded),
            StructuredNamingKind::Yaml => structured::extract_yaml(bounded),
            StructuredNamingKind::Toml => structured::extract_toml(bounded),
        },
        ExtractorGroup::Markup(kind) => match kind {
            MarkupNamingKind::XmlHtml => markup::extract_xml_html(bounded),
            MarkupNamingKind::Markdown => markup::extract_markdown(bounded),
        },
        ExtractorGroup::Code(style) => code::extract_code(bounded, style),
        ExtractorGroup::Sql => sql::extract_sql(bounded),
        ExtractorGroup::Prose => prose::extract_yake(bounded),
    };

    raw.and_then(|stem| slugify(&stem))
}

/// Maps a language ID to its canonical file extension.
pub fn language_to_extension(language_hint: &str) -> &'static str {
    language_profile(language_hint).extension
}

fn language_profile(language_hint: &str) -> LanguageNamingProfile {
    match language_hint {
        "csv" => LanguageNamingProfile {
            extension: "csv",
            extractor: ExtractorGroup::Structured(StructuredNamingKind::Csv),
        },
        "json" => LanguageNamingProfile {
            extension: "json",
            extractor: ExtractorGroup::Structured(StructuredNamingKind::Json),
        },
        "yaml" => LanguageNamingProfile {
            extension: "yaml",
            extractor: ExtractorGroup::Structured(StructuredNamingKind::Yaml),
        },
        "toml" => LanguageNamingProfile {
            extension: "toml",
            extractor: ExtractorGroup::Structured(StructuredNamingKind::Toml),
        },
        "xml" => LanguageNamingProfile {
            extension: "xml",
            extractor: ExtractorGroup::Markup(MarkupNamingKind::XmlHtml),
        },
        "html" => LanguageNamingProfile {
            extension: "html",
            extractor: ExtractorGroup::Markup(MarkupNamingKind::XmlHtml),
        },
        "svelte" => LanguageNamingProfile {
            extension: "svelte",
            extractor: ExtractorGroup::Markup(MarkupNamingKind::XmlHtml),
        },
        "vue" => LanguageNamingProfile {
            extension: "vue",
            extractor: ExtractorGroup::Markup(MarkupNamingKind::XmlHtml),
        },
        "angular" => LanguageNamingProfile {
            extension: "angular",
            extractor: ExtractorGroup::Markup(MarkupNamingKind::XmlHtml),
        },
        "markdown" => LanguageNamingProfile {
            extension: "md",
            extractor: ExtractorGroup::Markup(MarkupNamingKind::Markdown),
        },
        "sql" => LanguageNamingProfile {
            extension: "sql",
            extractor: ExtractorGroup::Sql,
        },
        "javascript" => LanguageNamingProfile {
            extension: "js",
            extractor: ExtractorGroup::Code(CodeStyle::JsTs),
        },
        "typescript" => LanguageNamingProfile {
            extension: "ts",
            extractor: ExtractorGroup::Code(CodeStyle::JsTs),
        },
        "python" => LanguageNamingProfile {
            extension: "py",
            extractor: ExtractorGroup::Code(CodeStyle::Python),
        },
        "rust" => LanguageNamingProfile {
            extension: "rs",
            extractor: ExtractorGroup::Code(CodeStyle::Rust),
        },
        "java" => LanguageNamingProfile {
            extension: "java",
            extractor: ExtractorGroup::Code(CodeStyle::JavaLike),
        },
        "kotlin" => LanguageNamingProfile {
            extension: "kt",
            extractor: ExtractorGroup::Code(CodeStyle::JavaLike),
        },
        "scala" => LanguageNamingProfile {
            extension: "scala",
            extractor: ExtractorGroup::Code(CodeStyle::JavaLike),
        },
        "go" => LanguageNamingProfile {
            extension: "go",
            extractor: ExtractorGroup::Code(CodeStyle::Go),
        },
        "cpp" => LanguageNamingProfile {
            extension: "cpp",
            extractor: ExtractorGroup::Code(CodeStyle::CFamily),
        },
        "c" => LanguageNamingProfile {
            extension: "c",
            extractor: ExtractorGroup::Code(CodeStyle::CFamily),
        },
        "csharp" => LanguageNamingProfile {
            extension: "cs",
            extractor: ExtractorGroup::Code(CodeStyle::CSharp),
        },
        "swift" => LanguageNamingProfile {
            extension: "swift",
            extractor: ExtractorGroup::Code(CodeStyle::Swift),
        },
        "objectivec" => LanguageNamingProfile {
            extension: "m",
            extractor: ExtractorGroup::Code(CodeStyle::Swift),
        },
        "objectivecpp" => LanguageNamingProfile {
            extension: "mm",
            extractor: ExtractorGroup::Code(CodeStyle::Swift),
        },
        "ruby" => LanguageNamingProfile {
            extension: "rb",
            extractor: ExtractorGroup::Code(CodeStyle::Ruby),
        },
        "php" => LanguageNamingProfile {
            extension: "php",
            extractor: ExtractorGroup::Code(CodeStyle::Php),
        },
        "dart" => LanguageNamingProfile {
            extension: "dart",
            extractor: ExtractorGroup::Code(CodeStyle::Dart),
        },
        "shell" => LanguageNamingProfile {
            extension: "sh",
            extractor: ExtractorGroup::Code(CodeStyle::Shell),
        },
        "dockerfile" => LanguageNamingProfile {
            extension: "dockerfile",
            extractor: ExtractorGroup::Code(CodeStyle::Shell),
        },
        "css" => LanguageNamingProfile {
            extension: "css",
            extractor: ExtractorGroup::Prose,
        },
        "clojure" => LanguageNamingProfile {
            extension: "clj",
            extractor: ExtractorGroup::Prose,
        },
        "sass" => LanguageNamingProfile {
            extension: "sass",
            extractor: ExtractorGroup::Prose,
        },
        "scss" => LanguageNamingProfile {
            extension: "scss",
            extractor: ExtractorGroup::Prose,
        },
        "jinja" => LanguageNamingProfile {
            extension: "j2",
            extractor: ExtractorGroup::Prose,
        },
        "powershell" => LanguageNamingProfile {
            extension: "ps1",
            extractor: ExtractorGroup::Prose,
        },
        "nginx" => LanguageNamingProfile {
            extension: "conf",
            extractor: ExtractorGroup::Prose,
        },
        _ => LanguageNamingProfile {
            extension: "txt",
            extractor: ExtractorGroup::Prose,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::model::MAX_STEM_LEN;
    use super::*;

    // ── CSV ─────────────────────────────────────────────────────────────────

    #[test]
    fn csv_takes_first_headers() {
        let csv = "id,first_name,last_name,email\n1,Alice,Smith,alice@example.com";
        let stem = suggest_stem(csv, "csv").unwrap();
        assert!(
            stem.contains("first-name") || stem.contains("id"),
            "got: {stem}"
        );
    }

    #[test]
    fn csv_tab_separated() {
        let csv = "product\tprice\tquantity\n";
        let stem = suggest_stem(csv, "csv").unwrap();
        assert!(stem.contains("product"), "got: {stem}");
    }

    #[test]
    fn csv_empty_returns_none() {
        assert!(suggest_stem("", "csv").is_none());
    }

    // ── JSON ─────────────────────────────────────────────────────────────────

    #[test]
    fn json_object_top_keys() {
        let json = r#"{"userId": 1, "name": "Alice", "email": "a@b.com"}"#;
        let stem = suggest_stem(json, "json").unwrap();
        assert!(
            stem.contains("user-id") || stem.contains("name"),
            "got: {stem}"
        );
    }

    #[test]
    fn json_array_of_objects() {
        let json = r#"[{"orderId": 1, "product": "Widget"}]"#;
        let stem = suggest_stem(json, "json").unwrap();
        assert!(
            stem.contains("order-id") || stem.contains("product"),
            "got: {stem}"
        );
    }

    // ── YAML ─────────────────────────────────────────────────────────────────

    #[test]
    fn yaml_top_level_keys() {
        let yaml = "name: my-service\nversion: 1.0\nport: 8080\n";
        let stem = suggest_stem(yaml, "yaml").unwrap();
        assert!(
            stem.contains("name") || stem.contains("version"),
            "got: {stem}"
        );
    }

    // ── TOML ─────────────────────────────────────────────────────────────────

    #[test]
    fn toml_keys_and_sections() {
        let toml = "[package]\nname = \"grayslate\"\nversion = \"0.1.0\"\n";
        let stem = suggest_stem(toml, "toml").unwrap();
        assert!(
            stem.contains("package") || stem.contains("name"),
            "got: {stem}"
        );
    }

    // ── XML / HTML ───────────────────────────────────────────────────────────

    #[test]
    fn xml_root_element() {
        let xml = r#"<?xml version="1.0"?><catalog id="main"><item>foo</item></catalog>"#;
        let stem = suggest_stem(xml, "xml").unwrap();
        assert!(stem.contains("catalog"), "got: {stem}");
    }

    #[test]
    fn html_root_element() {
        let html = r#"<!DOCTYPE html><html lang="en"><head><title>My Page</title></head></html>"#;
        let stem = suggest_stem(html, "html").unwrap();
        assert!(stem.contains("html"), "got: {stem}");
    }

    // ── Markdown ─────────────────────────────────────────────────────────────

    #[test]
    fn markdown_h1_heading() {
        let md = "# Getting Started Guide\n\nSome content.";
        let stem = suggest_stem(md, "markdown").unwrap();
        assert!(stem.contains("getting-started-guide"), "got: {stem}");
    }

    #[test]
    fn markdown_frontmatter_title() {
        let md = "---\ntitle: Release Notes 2026\n---\n\nContent here.";
        let stem = suggest_stem(md, "markdown").unwrap();
        assert!(stem.contains("release-notes"), "got: {stem}");
    }

    // ── Code ─────────────────────────────────────────────────────────────────

    #[test]
    fn js_class_and_function() {
        let js = "export class UserService {\n  async getUser(id) {}\n}";
        let stem = suggest_stem(js, "javascript").unwrap();
        assert!(stem.contains("user-service"), "got: {stem}");
    }

    #[test]
    fn python_class_def() {
        let py = "class DataProcessor:\n    def process(self, data): pass\n";
        let stem = suggest_stem(py, "python").unwrap();
        assert!(stem.contains("data-processor"), "got: {stem}");
    }

    #[test]
    fn rust_struct_and_fn() {
        let rs = "pub struct TokenParser { ... }\npub fn parse(input: &str) -> Token { ... }";
        let stem = suggest_stem(rs, "rust").unwrap();
        assert!(
            stem.contains("token-parser") || stem.contains("parse"),
            "got: {stem}"
        );
    }

    // ── Sanitizer ────────────────────────────────────────────────────────────

    #[test]
    fn slugify_camel_case() {
        let s = slugify("UserAuthService").unwrap();
        assert_eq!(s, "user-auth-service");
    }

    #[test]
    fn slugify_strips_invalid_chars() {
        let s = slugify("my:file/name*here").unwrap();
        assert!(
            !s.contains(':') && !s.contains('/') && !s.contains('*'),
            "got: {s}"
        );
    }

    #[test]
    fn slugify_caps_at_max_len() {
        let long = "a".repeat(100) + "-word";
        let s = slugify(&long).unwrap();
        assert!(s.len() <= MAX_STEM_LEN, "len={}", s.len());
    }

    #[test]
    fn slugify_empty_returns_none() {
        assert!(slugify("   ").is_none());
        assert!(slugify("---").is_none());
    }

    // ── Extension map ────────────────────────────────────────────────────────

    #[test]
    fn extension_map_covers_common_langs() {
        assert_eq!(language_to_extension("json"), "json");
        assert_eq!(language_to_extension("typescript"), "ts");
        assert_eq!(language_to_extension("python"), "py");
        assert_eq!(language_to_extension("rust"), "rs");
        assert_eq!(language_to_extension("sql"), "sql");
        assert_eq!(language_to_extension("unknown_lang"), "txt");
    }

    // ── Timestamp fallback ───────────────────────────────────────────────────

    #[test]
    fn fallback_stem_format() {
        let fb = fallback_stem();
        assert!(fb.starts_with("slate-"), "got: {fb}");
        // Should be slate-YYYYMMDDHHmmss → 6 + 14 = 20 chars
        assert_eq!(fb.len(), 20, "got: {fb}");
    }
}
