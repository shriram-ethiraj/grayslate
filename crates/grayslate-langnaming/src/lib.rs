/**
 * grayslate-langnaming
 *
 * Smart filename stem suggestion for untitled documents.
 *
 * Pipeline:
 *   1. Resolve a language naming profile from `language_hint`.
 *   2. Dispatch to a format-specific extractor group.
 *   3. Sanitize + slugify the raw stem.
 *   4. If nothing useful was found, return None (caller falls back to
 *      slate-<DD-mon-YYYY-HHMM>).
 *
 * All extractors operate on at most the first MAX_CONTENT_BYTES bytes of the
 * document to keep naming fast even for very large files.
 */
pub mod code;
pub mod languages;
pub mod markup;
pub mod model;
pub mod prose;
pub mod shared;
pub mod sql;
pub mod structured;

pub use self::shared::{fallback_stem, slugify};

/// Returns a sanitized filename stem (no extension, no path separators) or
/// `None` when no useful name can be derived.
pub fn suggest_stem(content: &str, language_hint: &str) -> Option<String> {
    let bounded = shared::bound(content);
    let def = languages::lookup(language_hint);

    // Prose-backed languages carry kind metadata (email / prompt) so the
    // pipeline can append a descriptive suffix.
    if def.name == "text" {
        let tagged = prose::extract_prose_tagged(bounded);
        return shared::finalize_extracted(tagged);
    }

    // When the language is explicitly "email" or "prompt", the detection
    // pipeline already confirmed the kind — force the correct StemKind so the
    // suffix ("-email" / "-prompt") always appears in the filename.
    let kind = match def.name {
        "email" => model::StemKind::Email,
        "prompt" => model::StemKind::Prompt,
        _ => model::StemKind::Generic,
    };

    let raw = match &def.extract {
        languages::Extractor::Custom(f) => f(bounded),
        languages::Extractor::Patterns { symbols, noise } => {
            code::extract_from_patterns(bounded, symbols, noise)
        }
    };
    shared::finalize(raw, kind)
}

/// Maps a language ID to its canonical file extension.
pub fn language_to_extension(language_hint: &str) -> &'static str {
    languages::lookup(language_hint).extension
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
        // Enhanced YAML now extracts `name:` value → "my-service"
        assert!(
            stem.contains("my-service"),
            "got: {stem}"
        );
    }

    // ── TOML ─────────────────────────────────────────────────────────────────

    #[test]
    fn toml_keys_and_sections() {
        let toml = "[package]\nname = \"grayslate\"\nversion = \"0.1.0\"\n";
        let stem = suggest_stem(toml, "toml").unwrap();
        assert!(
            stem.contains("package") || stem.contains("name") || stem.contains("grayslate"),
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
        // Enhanced HTML now extracts <title> content → "my-page"
        assert!(stem.contains("my-page"), "got: {stem}");
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
    fn ts_exported_function_found() {
        // Regression: file with many imports followed by a large switch body —
        // the export function must still be found within the 5 KB bound.
        let imports = (0..25)
            .map(|i| format!("import {{ mod{i} }} from \"@pkg/mod{i}\";\n"))
            .collect::<String>();
        let switch_cases = (0..60)
            .map(|i| format!("        case \"lang{i}\":\n            return [];\n"))
            .collect::<String>();
        let body = format!(
            "{imports}\nexport function getLanguageExtension(langId: string): string[] {{\n    switch (langId) {{\n{switch_cases}        default: return [];\n    }}\n}}\n"
        );
        let stem = suggest_stem(&body, "typescript");
        assert!(
            stem.as_deref() == Some("get-language-extension"),
            "got: {stem:?}"
        );
    }

    #[test]
    fn ts_lang_extensions_real_file() {
        // Regression: languageExtensions.ts previously produced FALLBACK.
        // The JsTs regex extractor now covers this case.
        let content = include_str!(
            "../../../src/lib/editor/config/languageExtensions.ts"
        );
        let stem = suggest_stem(content, "typescript");
        assert_eq!(stem.as_deref(), Some("get-language-extension"), "got: {stem:?}");
    }

    #[test]
    fn ts_barrel_reexport() {
        // export { Root as Badge, badgeVariants } → "badge" from the alias.
        let src = "import Root, { badgeVariants } from \"./badge.svelte\";\nexport { Root as Badge, badgeVariants };\n";
        let stem = suggest_stem(src, "typescript").unwrap();
        assert!(stem.contains("badge"), "got: {stem}");
    }

    #[test]
    fn ts_exported_camel_const() {
        // export const markdownAutocompleteConfig = {...} → camelCase should be captured.
        let src = "export const markdownAutocompleteConfig = { items: [] };\n";
        let stem = suggest_stem(src, "typescript").unwrap();
        assert!(stem.contains("markdown-autocomplete-config"), "got: {stem}");
    }

    #[test]
    fn js_export_default_call() {
        // export default defineConfig({...}) → callee name extracted.
        let src = "import { defineConfig } from \"vite\";\nexport default defineConfig({ plugins: [] });\n";
        let stem = suggest_stem(src, "javascript").unwrap();
        assert!(stem.contains("define-config"), "got: {stem}");
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

    #[test]
    fn rust_module_doc_fallback() {
        let rs = "//! Connection pooling and retry logic\n\nuse std::net::TcpStream;\n";
        let stem = suggest_stem(rs, "rust").unwrap();
        assert!(stem.contains("connection-pooling"), "got: {stem}");
    }

    #[test]
    fn python_docstring_fallback() {
        let py = "\"\"\"Rate limiting middleware for Flask applications\"\"\"\n\nimport time\nimport functools\n";
        let stem = suggest_stem(py, "python").unwrap();
        assert!(stem.contains("rate-limiting"), "got: {stem}");
    }

    #[test]
    fn go_package_doc_fallback() {
        let go = "// Package ratelimit provides a token bucket rate limiter.\npackage ratelimit\n\nimport \"sync\"\n";
        let stem = suggest_stem(go, "go").unwrap();
        assert!(stem.contains("ratelimit"), "got: {stem}");
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
        assert_eq!(language_to_extension("perl"), "pl");
        assert_eq!(language_to_extension("unknown_lang"), "txt");
    }

    // ── Timestamp fallback ───────────────────────────────────────────────────

    #[test]
    fn fallback_stem_format() {
        let fb = fallback_stem();
        assert!(fb.starts_with("slate-"), "got: {fb}");
        // Format: slate-DD-mon-YYYY-HHMM  e.g. slate-19-mar-2026-0530 → 22 chars
        assert_eq!(fb.len(), 22, "got: {fb}");
    }

    // ── Suffix pipeline ──────────────────────────────────────────────────────

    #[test]
    fn email_gets_suffix() {
        let email = "\
From: test@example.com
To: dev@example.com
Subject: Database migration plan

Content about migration.";
        let stem = suggest_stem(email, "text").unwrap();
        assert!(stem.ends_with("-email"), "expected -email suffix, got: {stem}");
        assert!(stem.contains("database-migration"), "got: {stem}");
    }

    #[test]
    fn email_language_hint_gets_suffix() {
        // When detection returns "email" as the language, suggest_stem must
        // append "-email" even though it is not going through "text" dispatch.
        let email = "\
Hi team,

Quick update on the search improvements:

* Basic indexing is done
* Filters are partially working (need to fix edge cases)
* Performance is still inconsistent for large datasets

I'll continue working on optimization today and share another update tomorrow.

Let me know if anything urgent needs to be prioritize";
        let stem = suggest_stem(email, "email").unwrap();
        assert!(
            stem.ends_with("-email"),
            "explicit 'email' language should have -email suffix, got: {stem}"
        );
    }

    #[test]
    fn prompt_language_hint_gets_suffix() {
        let prompt = "\
You are a code reviewer. Analyze the following function for correctness.

Guidelines:
- Check for edge cases
- Verify error handling
- Review naming conventions";
        let stem = suggest_stem(prompt, "prompt").unwrap();
        assert!(
            stem.ends_with("-prompt"),
            "explicit 'prompt' language should have -prompt suffix, got: {stem}"
        );
    }

    #[test]
    fn prompt_gets_suffix() {
        let prompt = "\
You are a security auditor. Review the authentication module for vulnerabilities.

1. Check for SQL injection
2. Check for XSS
3. Check for CSRF";
        let stem = suggest_stem(prompt, "text").unwrap();
        assert!(stem.ends_with("-prompt"), "expected -prompt suffix, got: {stem}");
    }

    #[test]
    fn generic_text_no_suffix() {
        let text = "Distributed systems require careful consideration of network partitions, \
                     consistency models, and failure modes. The CAP theorem states that a \
                     distributed data store can only guarantee two of three properties.";
        let stem = suggest_stem(text, "text").unwrap();
        assert!(!stem.ends_with("-email"), "no email suffix: {stem}");
        assert!(!stem.ends_with("-prompt"), "no prompt suffix: {stem}");
    }

    #[test]
    fn suffix_survives_length_cap() {
        // Long email subject that exceeds MAX_STEM_LEN when slugified.
        let email = "\
From: a@b.com
To: c@d.com
Subject: Very important quarterly financial report with detailed breakdowns and comprehensive analysis of market trends

Body.";
        let stem = suggest_stem(email, "text").unwrap();
        assert!(stem.ends_with("-email"), "suffix must survive cap, got: {stem}");
        assert!(stem.len() <= MAX_STEM_LEN, "len={}, got: {stem}", stem.len());
    }
}
