/// Maximum bytes fed to any extractor.
pub const MAX_CONTENT_BYTES: usize = 5_000;

/// Maximum characters in the final slug.
pub const MAX_STEM_LEN: usize = 60;

/// Maximum symbol tokens combined into the stem.
pub const MAX_TOKENS: usize = 4;

#[derive(Clone, Copy)]
pub struct LanguageNamingProfile {
    pub extension: &'static str,
    pub extractor: ExtractorGroup,
}

#[derive(Clone, Copy)]
pub enum ExtractorGroup {
    Structured(StructuredNamingKind),
    Markup(MarkupNamingKind),
    Code(CodeStyle),
    Sql,
    Prose,
}

#[derive(Clone, Copy)]
pub enum StructuredNamingKind {
    Csv,
    Json,
    Yaml,
    Toml,
}

#[derive(Clone, Copy)]
pub enum MarkupNamingKind {
    XmlHtml,
    Markdown,
}

#[derive(Clone, Copy)]
pub enum CodeStyle {
    JsTs,
    Python,
    Rust,
    JavaLike,
    Go,
    CFamily,
    CSharp,
    Swift,
    Ruby,
    Php,
    Dart,
    Shell,
}
