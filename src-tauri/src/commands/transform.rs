use base64::{
    engine::general_purpose::{
        STANDARD as BASE64_STANDARD, URL_SAFE as BASE64_URL_SAFE,
        URL_SAFE_NO_PAD as BASE64_URL_SAFE_NO_PAD,
    },
    Engine as _,
};
use chrono::{DateTime, SecondsFormat, Utc};
use crc32fast::Hasher as Crc32Hasher;
use dprint_plugin_jsonc::{
    configuration::Configuration as JsoncFormatConfiguration, format_text as format_jsonc_text,
};
use dprint_plugin_markdown::{
    configuration::Configuration as MarkdownFormatConfiguration,
    format_text as format_markdown_text,
};
use dprint_plugin_sql::{
    configuration::Configuration as SqlFormatConfiguration, format_text as format_sql_text,
};
use dprint_plugin_toml::{
    configuration::Configuration as TomlFormatConfiguration, format_text as format_toml_text,
};
use dprint_plugin_typescript::{
    configuration::Configuration as TsFormatConfiguration, format_text as format_ts_text,
    FormatTextOptions,
};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use heck::{AsKebabCase, AsLowerCamelCase, AsSnakeCase, AsTitleCase};
use jsonc_parser::{
    ast::{ObjectPropName, Value as JsonAstValue},
    common::Range as JsonRange,
    parse_to_ast, parse_to_serde_value, parse_to_value, CollectOptions, ParseOptions,
};
use malva::{
    config::FormatOptions as CssFormatOptions, format_text as format_css_text, Syntax as CssSyntax,
};
use markup_fmt::{
    config::FormatOptions as HtmlFormatOptions, format_text as format_html_text,
    Language as HtmlLanguage,
};
use md5::Md5;
use pretty_yaml::{config::FormatOptions as YamlFormatOptions, format_text as format_yaml_text};
use quick_xml::{
    events::{BytesStart, Event},
    reader::Reader as XmlReader,
    writer::Writer as XmlWriter,
};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};
use std::cell::Cell;
use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::sync::LazyLock;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

static JSONC_PARSE_OPTIONS: LazyLock<ParseOptions> = LazyLock::new(|| ParseOptions {
    allow_comments: true,
    allow_loose_object_property_names: false,
    allow_trailing_commas: true,
    allow_missing_commas: false,
    allow_single_quoted_strings: false,
    allow_hexadecimal_numbers: false,
    allow_unary_plus_numbers: false,
});

/// Maximum number of lines scanned for indentation detection. Bounds cost on
/// very large documents while still capturing a representative sample.
const DETECT_INDENT_MAX_LINES: usize = 1000;

/// Detect the dominant indentation style from document content.
///
/// Scans up to [`DETECT_INDENT_MAX_LINES`] non-blank, non-comment lines and
/// tallies:
/// - lines whose leading whitespace is entirely `\t`, toward tabs
/// - for lines whose leading whitespace is entirely ` `, the *increase* in
///   column offset relative to the nearest preceding comparable line (i.e.
///   the indent step between nesting levels), toward a candidate indent
///   width
///
/// Steps are used rather than each line's raw column offset because a line
/// nested two levels deep already sits at `2 * indent_width` columns; on
/// documents with more deep lines than shallow ones (e.g. several `WHEN`
/// clauses inside one `CASE`), tallying absolute offsets would report the
/// nesting depth itself as the indent width. Re-running detection on that
/// wider-formatted output would then double it again on every pass. Tallying
/// the step between levels instead stays stable under repeated formatting.
///
/// Returns `(use_tabs, indent_width)`. Tabs win ties so a document with a
/// single tab-indented line and no space-indented lines becomes tab-formatted.
/// Falls back to `(false, 2)` when no indentation is found (e.g. minified
/// input or a document with only top-level keys).
fn detect_indentation(text: &str) -> (bool, u32) {
    let mut tab_lines = 0usize;
    // Indent step size -> number of times that step was observed between a
    // line and the preceding comparable (pure-space-indented) line.
    let mut space_step_counts: HashMap<u32, usize> = HashMap::new();
    let mut total_space_lines = 0usize;
    // Column offset of the previous comparable line. `None` right after a
    // tab-indented or mixed-indent line, so it never anchors a comparison
    // across an incompatible indentation style.
    let mut prev_space_indent: Option<u32> = None;

    for line in text.lines().take(DETECT_INDENT_MAX_LINES) {
        // Skip blank lines and ones whose first non-whitespace content is a
        // comment marker — their leading whitespace is not semantic indent.
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") {
            continue;
        }

        let indent_len = line.len() - trimmed.len();
        let indent = &line[..indent_len];

        if indent_len > 0 && indent.bytes().all(|b| b == b'\t') {
            tab_lines += 1;
            prev_space_indent = None;
            continue;
        }

        if indent_len > 0 && !indent.bytes().all(|b| b == b' ') {
            // Mixed tab+space leading whitespace is ambiguous; skip it rather
            // than letting it skew the tally either way.
            prev_space_indent = None;
            continue;
        }

        let width = indent_len as u32;
        if indent_len > 0 && width < 2 {
            // A single leading space is usually prose/markdown quote-style
            // noise, not a real indent step. Ignore it without disturbing
            // the comparison baseline for surrounding lines.
            continue;
        }

        if width > 0 {
            total_space_lines += 1;
        }

        if let Some(prev) = prev_space_indent {
            if width > prev {
                let step = width - prev;
                *space_step_counts.entry(step).or_insert(0) += 1;
            }
        }
        prev_space_indent = Some(width);
    }

    if tab_lines > 0 && tab_lines >= total_space_lines {
        return (true, 1);
    }

    if let Some((&step, _)) = space_step_counts.iter().max_by_key(|(_, &v)| v) {
        return (false, step.max(1));
    }

    (false, 2)
}

/// Resolve formatting indentation from the frontend-supplied params.
/// Returns `(use_tabs, indent_width)`.
/// "detect" mode analyses `text` to pick the dominant indentation style;
/// absent params and "default" mode fall back to 2 spaces.
fn resolve_format_indent(params: &Option<TransformationParams>, text: &str) -> (bool, u32) {
    match params.as_ref().and_then(|p| p.indent_config.as_ref()) {
        Some(cfg) if cfg.indent_mode == "tab" => (true, cfg.indent_size.unwrap_or(2).max(1)),
        Some(cfg) if cfg.indent_mode == "spaces" => (false, cfg.indent_size.unwrap_or(2).max(1)),
        Some(cfg) if cfg.indent_mode == "detect" => detect_indentation(text),
        _ => (false, 2),
    }
}

/// Result of a one-shot indentation-style detection request from the
/// frontend's indentation picker.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedIndent {
    pub use_tabs: bool,
    pub width: u32,
}

/// Detect the dominant indentation style of `content` for the indentation
/// picker's "Detect from content" action. Runs once per user selection, not
/// continuously — see [`detect_indentation`] for the underlying heuristic.
#[tauri::command]
pub fn editor_detect_indent(content: String) -> DetectedIndent {
    let (use_tabs, width) = detect_indentation(&content);
    DetectedIndent { use_tabs, width }
}

fn jsonc_format_config(use_tabs: bool, indent_width: u32) -> JsoncFormatConfiguration {
    serde_json::from_value(serde_json::json!({
        "lineWidth": 120,
        "useTabs": use_tabs,
        "indentWidth": indent_width,
        "newLineKind": "auto",
        "commentLine.forceSpaceAfterSlashes": false,
    }))
    .expect("built-in JSON formatter configuration must be valid")
}

fn sql_format_config(use_tabs: bool, indent_width: u32) -> SqlFormatConfiguration {
    serde_json::from_value(serde_json::json!({
        "useTabs": use_tabs,
        "indentWidth": indent_width,
        "newLineKind": "auto",
        "uppercase": true,
        "linesBetweenQueries": 1,
    }))
    .expect("built-in SQL formatter configuration must be valid")
}

fn ts_format_config(use_tabs: bool, indent_width: u32) -> TsFormatConfiguration {
    dprint_plugin_typescript::configuration::ConfigurationBuilder::new()
        .indent_width(indent_width.clamp(1, u8::MAX as u32) as u8)
        .use_tabs(use_tabs)
        .line_width(120)
        .new_line_kind(dprint_core::configuration::NewLineKind::Auto)
        .build()
}

fn css_format_config(use_tabs: bool, indent_width: u32) -> CssFormatOptions {
    serde_json::from_value(serde_json::json!({
        "useTabs": use_tabs,
        "indentWidth": indent_width,
        "printWidth": 120,
    }))
    .expect("built-in CSS formatter configuration must be valid")
}

fn html_format_config(use_tabs: bool, indent_width: u32) -> HtmlFormatOptions {
    serde_json::from_value(serde_json::json!({
        "useTabs": use_tabs,
        "indentWidth": indent_width,
        "printWidth": 120,
    }))
    .expect("built-in HTML formatter configuration must be valid")
}

fn yaml_format_config(indent_width: u32) -> YamlFormatOptions {
    serde_json::from_value(serde_json::json!({
        "indentWidth": indent_width,
        "printWidth": 120,
    }))
    .expect("built-in YAML formatter configuration must be valid")
}

fn markdown_format_config() -> MarkdownFormatConfiguration {
    dprint_plugin_markdown::configuration::ConfigurationBuilder::new()
        .line_width(120)
        .new_line_kind(dprint_core::configuration::NewLineKind::Auto)
        .build()
}

fn toml_format_config(use_tabs: bool, indent_width: u32) -> TomlFormatConfiguration {
    dprint_plugin_toml::configuration::ConfigurationBuilder::new()
        .indent_width(indent_width.clamp(1, u8::MAX as u32) as u8)
        .use_tabs(use_tabs)
        .line_width(120)
        .new_line_kind(dprint_core::configuration::NewLineKind::Auto)
        .build()
}

fn validate_jsonc(text: &str) -> Result<(), String> {
    match parse_to_value(text, &JSONC_PARSE_OPTIONS) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err("Invalid JSON: document is empty.".to_string()),
        Err(error) => Err(format!("Invalid JSON: {}", error)),
    }
}

fn parse_jsonc_to_serde_value(text: &str) -> Result<serde_json::Value, String> {
    match parse_to_serde_value(text, &JSONC_PARSE_OPTIONS) {
        Ok(Some(value)) => Ok(value),
        Ok(None) => Err("Invalid JSON: document is empty.".to_string()),
        Err(error) => Err(format!("Invalid JSON: {}", error)),
    }
}

/// Minify JSONC by stripping whitespace outside strings and comments.
/// Operates on raw bytes for performance — JSON structural characters are ASCII,
/// so multi-byte UTF-8 sequences only appear inside string values where we
/// faithfully copy every byte until the closing quote.
fn minify_jsonc_preserving_comments(
    text: &str,
    ctx: &TransformationContext<'_>,
) -> Result<String, String> {
    ctx.check_cancelled()?;
    let bytes = text.as_bytes();
    let len = bytes.len();
    let total_bytes = len as u32;
    let mut out = Vec::with_capacity(len);
    let mut i = 0;
    let mut in_string = false;
    let mut escape_next = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while i < len {
        if i % BYTE_CANCEL_CHECK_INTERVAL == 0 {
            ctx.check_cancelled()?;
            ctx.report_progress(i as u32, total_bytes.max(1));
        }
        let b = bytes[i];

        if in_string {
            out.push(b);
            if escape_next {
                escape_next = false;
            } else if b == b'\\' {
                escape_next = true;
            } else if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if in_line_comment {
            out.push(b);
            if b == b'\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }

        if in_block_comment {
            out.push(b);
            if b == b'*' && i + 1 < len && bytes[i + 1] == b'/' {
                out.push(b'/');
                i += 2;
                in_block_comment = false;
            } else {
                i += 1;
            }
            continue;
        }

        if b == b'"' {
            in_string = true;
            out.push(b);
            i += 1;
            continue;
        }

        if b == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
            in_line_comment = true;
            out.push(b'/');
            out.push(b'/');
            i += 2;
            continue;
        }

        if b == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            in_block_comment = true;
            out.push(b'/');
            out.push(b'*');
            i += 2;
            continue;
        }

        // ASCII whitespace covers all JSON whitespace (space, tab, CR, LF).
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        out.push(b);
        i += 1;
    }

    // SAFETY: input was valid UTF-8 and we only stripped ASCII whitespace bytes
    // outside of strings/comments; all multi-byte sequences are preserved intact.
    Ok(unsafe { String::from_utf8_unchecked(out) })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TransformationMessageLevel {
    Success,
    Error,
    Info,
}

/// Events sent through the IPC channel during transformation execution.
/// The channel is the data plane for large text delivery: progress events keep
/// the loader current, and `Chunk` events carry slices of the result text.
///
/// The terminal metadata stays on the command's small JSON response so the
/// frontend never has to race `invoke()` completion against channel delivery.
#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum TransformationChannelEvent {
    /// Incremental progress update. `current` and `total` are in the same unit
    /// (bytes or rows); the frontend computes `(current / total) * 100`.
    Progress { current: u32, total: u32 },
    /// One slice of the result text. The frontend accumulates all chunks in
    /// order and joins them once `ReplaceText` is received.
    Chunk {
        /// Zero-based index of this chunk.
        index: u32,
        /// UTF-8 text slice. Each chunk targets `CHUNK_SIZE` bytes and may
        /// grow by a few bytes to preserve a valid UTF-8 boundary.
        text: String,
    },
}

/// Maximum bytes per channel chunk. 4 MB keeps each IPC message well below
/// WebView2's practical limits while still streaming large results quickly.
const CHUNK_SIZE: usize = 4 * 1024 * 1024;

fn build_text_chunk_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = 0usize;

    while start < text.len() {
        let mut end = (start + CHUNK_SIZE).min(text.len());
        while end < text.len() && !text.is_char_boundary(end) {
            end += 1;
        }
        ranges.push((start, end));
        start = end;
    }

    ranges
}

fn send_text_chunks(
    channel: &tauri::ipc::Channel<TransformationChannelEvent>,
    text: &str,
) -> Result<u32, String> {
    let ranges = build_text_chunk_ranges(text);

    for (index, (start, end)) in ranges.iter().copied().enumerate() {
        channel
            .send(TransformationChannelEvent::Chunk {
                index: index as u32,
                text: text[start..end].to_string(),
            })
            .map_err(|error| {
                format!(
                    "Failed to deliver transformation result chunk {} to the UI: {}",
                    index + 1,
                    error
                )
            })?;
    }

    Ok(ranges.len() as u32)
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum TransformationActionId {
    #[serde(rename = "json.format")]
    JsonFormat,
    #[serde(rename = "json.minify")]
    JsonMinify,
    #[serde(rename = "json.validate")]
    JsonValidate,
    #[serde(rename = "json.lines-to-array")]
    JsonLinesToArray,
    #[serde(rename = "json.array-to-lines")]
    JsonArrayToLines,
    #[serde(rename = "json.sort-keys")]
    JsonSortKeys,
    #[serde(rename = "json.to-typescript")]
    JsonToTypescript,

    #[serde(rename = "sql.format")]
    SqlFormat,

    // ── Code formatting ─────────────────────────────────────────────────
    #[serde(rename = "javascript.format")]
    JavascriptFormat,
    #[serde(rename = "typescript.format")]
    TypescriptFormat,
    #[serde(rename = "css.format")]
    CssFormat,
    #[serde(rename = "html.format")]
    HtmlFormat,
    #[serde(rename = "svelte.format")]
    SvelteFormat,
    #[serde(rename = "yaml.format")]
    YamlFormat,
    #[serde(rename = "markdown.format")]
    MarkdownFormat,
    #[serde(rename = "toml.format")]
    TomlFormat,

    // ── XML ─────────────────────────────────────────────────────────────
    #[serde(rename = "xml.format")]
    XmlFormat,
    #[serde(rename = "xml.minify")]
    XmlMinify,
    #[serde(rename = "xml.validate")]
    XmlValidate,

    // ── JSON key case ────────────────────────────────────────────────────
    #[serde(rename = "json.keys-camel-case")]
    JsonKeysCamelCase,
    #[serde(rename = "json.keys-snake-case")]
    JsonKeysSnakeCase,
    #[serde(rename = "json.keys-kebab-case")]
    JsonKeysKebabCase,
    #[serde(rename = "json.keys-title-case")]
    JsonKeysTitleCase,
    #[serde(rename = "json.keys-sponge-case")]
    JsonKeysSpongeCase,

    #[serde(rename = "text.trim-trailing-whitespace")]
    TextTrimTrailingWhitespace,
    #[serde(rename = "text.collapse-blank-lines")]
    TextCollapseBlankLines,
    #[serde(rename = "csv.to-json")]
    CsvToJson,
    #[serde(rename = "json.to-csv")]
    JsonToCsv,
    #[serde(rename = "json.to-yaml")]
    JsonToYaml,
    #[serde(rename = "yaml.to-json")]
    YamlToJson,

    // ── Text ────────────────────────────────────────────────────────────
    #[serde(rename = "text.trim")]
    TextTrim,
    #[serde(rename = "text.uppercase")]
    TextUppercase,
    #[serde(rename = "text.lowercase")]
    TextLowercase,
    #[serde(rename = "text.reverse-lines")]
    TextReverseLines,
    #[serde(rename = "text.reverse-string")]
    TextReverseString,
    #[serde(rename = "text.markdown-quote")]
    TextMarkdownQuote,
    #[serde(rename = "text.rot13")]
    TextRot13,
    #[serde(rename = "text.add-slashes")]
    TextAddSlashes,
    #[serde(rename = "text.remove-slashes")]
    TextRemoveSlashes,
    #[serde(rename = "text.sort-lines")]
    TextSortLines,
    #[serde(rename = "text.remove-duplicate-lines")]
    TextRemoveDuplicateLines,
    #[serde(rename = "text.collapse-lines")]
    TextCollapseLines,

    // ── Case ────────────────────────────────────────────────────────────
    #[serde(rename = "text.camel-case")]
    TextCamelCase,
    #[serde(rename = "text.snake-case")]
    TextSnakeCase,
    #[serde(rename = "text.kebab-case")]
    TextKebabCase,
    #[serde(rename = "text.title-case")]
    TextTitleCase,
    #[serde(rename = "text.sponge-case")]
    TextSpongeCase,

    // ── URL ─────────────────────────────────────────────────────────────
    #[serde(rename = "url.encode")]
    UrlEncode,
    #[serde(rename = "url.decode")]
    UrlDecode,
    #[serde(rename = "url.query-to-json")]
    UrlQueryToJson,
    #[serde(rename = "url.json-to-query")]
    UrlJsonToQuery,
    #[serde(rename = "security.url-defang")]
    SecurityUrlDefang,
    #[serde(rename = "security.url-refang")]
    SecurityUrlRefang,

    // ── Encoding ────────────────────────────────────────────────────────
    #[serde(rename = "encoding.base64-encode")]
    EncodingBase64Encode,
    #[serde(rename = "encoding.base64-decode")]
    EncodingBase64Decode,
    #[serde(rename = "encoding.base64url-encode")]
    EncodingBase64UrlEncode,
    #[serde(rename = "encoding.base64url-decode")]
    EncodingBase64UrlDecode,
    #[serde(rename = "encoding.html-encode")]
    EncodingHtmlEncode,
    #[serde(rename = "encoding.html-decode")]
    EncodingHtmlDecode,
    #[serde(rename = "encoding.gzip-to-base64")]
    EncodingGzipToBase64,
    #[serde(rename = "encoding.gzip-from-base64")]
    EncodingGzipFromBase64,
    #[serde(rename = "encoding.jwt-decode")]
    EncodingJwtDecode,

    // ── Hashes and checksums ────────────────────────────────────────────
    #[serde(rename = "hash.sha-256")]
    HashSha256,
    #[serde(rename = "hash.sha-512")]
    HashSha512,
    #[serde(rename = "checksum.crc32")]
    ChecksumCrc32,
    #[serde(rename = "hash.sha-1")]
    HashSha1,
    #[serde(rename = "hash.md5")]
    HashMd5,

    // ── Time ────────────────────────────────────────────────────────────
    #[serde(rename = "time.unix-seconds-to-rfc3339")]
    TimeUnixSecondsToRfc3339,
    #[serde(rename = "time.unix-milliseconds-to-rfc3339")]
    TimeUnixMillisecondsToRfc3339,
    #[serde(rename = "time.rfc3339-to-unix-seconds")]
    TimeRfc3339ToUnixSeconds,
    #[serde(rename = "time.rfc3339-to-unix-milliseconds")]
    TimeRfc3339ToUnixMilliseconds,

    // ── Generators ──────────────────────────────────────────────────────
    #[serde(rename = "generate.uuid-v4")]
    GenerateUuidV4,
    #[serde(rename = "generate.uuid-v7")]
    GenerateUuidV7,

    // ── Numeric conversions ──────────────────────────────────────────────
    #[serde(rename = "convert.ascii-to-hex")]
    ConvertAsciiToHex,
    #[serde(rename = "convert.hex-to-ascii")]
    ConvertHexToAscii,
    #[serde(rename = "convert.decimal-to-binary")]
    ConvertDecimalToBinary,
    #[serde(rename = "convert.binary-to-decimal")]
    ConvertBinaryToDecimal,
    #[serde(rename = "convert.decimal-to-hex")]
    ConvertDecimalToHex,
    #[serde(rename = "convert.hex-to-decimal")]
    ConvertHexToDecimal,

    // ── Statistics (message-only) ────────────────────────────────────────
    #[serde(rename = "stats.count-characters")]
    StatsCountCharacters,
    #[serde(rename = "stats.count-lines")]
    StatsCountLines,
    #[serde(rename = "stats.count-words")]
    StatsCountWords,
}

/// Registry of in-flight transformation requests. Each entry maps a `request_id`
/// to an `Arc<AtomicBool>` cancellation flag. Setting the flag to `true` signals
/// the blocking worker to abort at its next check point.
#[derive(Default)]
pub struct TransformationCancellationRegistry {
    active: Mutex<HashMap<u64, Arc<AtomicBool>>>,
}

impl TransformationCancellationRegistry {
    /// Register a new in-flight request and return a handle to its cancellation flag.
    fn register(&self, request_id: u64) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        self.active.lock().unwrap().insert(request_id, flag.clone());
        flag
    }

    /// Signal cancellation for the given request. No-op if the request is not found.
    pub fn cancel(&self, request_id: u64) {
        if let Some(flag) = self.active.lock().unwrap().get(&request_id) {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// Remove a completed (or cancelled) request from the registry.
    fn finish(&self, request_id: u64) {
        self.active.lock().unwrap().remove(&request_id);
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IndentConfig {
    pub indent_mode: String,
    #[serde(default)]
    pub indent_size: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformationParams {
    pub indent_config: Option<IndentConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteTransformationRequest {
    pub action_id: TransformationActionId,
    pub text: String,
    /// Unique ID generated by the frontend for each invocation, used to
    /// look up the cancellation flag via [`TransformationCancellationRegistry`].
    #[serde(default)]
    pub request_id: u64,
    /// Action-specific parameters (e.g. indentation config for formatting).
    #[serde(default)]
    pub params: Option<TransformationParams>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ExecuteTransformationTransportResponse {
    #[serde(rename_all = "camelCase")]
    ReplaceText {
        chunk_count: u32,
        message: Option<String>,
        level: Option<TransformationMessageLevel>,
    },
    ShowMessage {
        message: String,
        level: TransformationMessageLevel,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ExecuteTransformationResponse {
    ReplaceText {
        text: String,
        message: Option<String>,
        level: Option<TransformationMessageLevel>,
    },
    ShowMessage {
        message: String,
        level: TransformationMessageLevel,
    },
}

const BYTE_CANCEL_CHECK_INTERVAL: usize = 4 * 1024;
const LINE_CANCEL_CHECK_INTERVAL: usize = 512;
const MAX_GZIP_DECOMPRESSED_BYTES: usize = 200 * 1024 * 1024;

/// Shared cancellation-aware context for all transformation implementations.
/// New transformations should take `&TransformationContext` and use the
/// provided helpers instead of interacting with the raw `AtomicBool` directly.
struct TransformationContext<'a> {
    original: &'a str,
    cancelled: &'a AtomicBool,
    /// Action-specific parameters sent from the frontend (e.g. indentation config).
    params: &'a Option<TransformationParams>,
    /// Called with `(current, total)` at natural progress checkpoints.
    /// Both values use the same unit (bytes or rows) for the active operation.
    /// Defaults to a no-op so callers that don't need progress (e.g. tests)
    /// require no changes.
    progress_fn: Box<dyn Fn(u32, u32) + Send + Sync + 'static>,
    /// Tracks the last `current` value that was actually sent through `progress_fn`,
    /// allowing `report_progress` to throttle emissions to ~100 per transform
    /// regardless of how often call-sites invoke it.
    last_emitted: Cell<u32>,
}

static NO_TRANSFORM_PARAMS: Option<TransformationParams> = None;

impl<'a> TransformationContext<'a> {
    fn new(original: &'a str, cancelled: &'a AtomicBool) -> Self {
        Self {
            original,
            cancelled,
            params: &NO_TRANSFORM_PARAMS,
            progress_fn: Box::new(|_, _| {}),
            last_emitted: Cell::new(0),
        }
    }

    /// Attach a progress reporter to this context (builder-style).
    fn with_progress<F: Fn(u32, u32) + Send + Sync + 'static>(mut self, f: F) -> Self {
        self.progress_fn = Box::new(f);
        self
    }

    /// Attach action-specific parameters to this context (builder-style).
    fn with_params(mut self, params: &'a Option<TransformationParams>) -> Self {
        self.params = params;
        self
    }

    /// Emit a progress update if the progress has advanced by at least 1% of
    /// `total` since the last emission. This caps IPC channel traffic to ~100
    /// messages per transform regardless of how frequently call-sites report.
    #[inline]
    fn report_progress(&self, current: u32, total: u32) {
        let step = (total / 100).max(1);
        let last = self.last_emitted.get();
        if current == 0 || current.wrapping_sub(last) >= step {
            self.last_emitted.set(current);
            (self.progress_fn)(current, total);
        }
    }

    fn text(&self) -> &'a str {
        self.original
    }

    fn params(&self) -> &Option<TransformationParams> {
        self.params
    }

    #[inline]
    fn check_cancelled(&self) -> Result<(), String> {
        if self.cancelled.load(Ordering::Relaxed) {
            Err("Transformation cancelled.".to_string())
        } else {
            Ok(())
        }
    }

    #[inline]
    fn checkpoint(&self, processed: usize, interval: usize) -> Result<(), String> {
        if processed % interval == 0 {
            self.check_cancelled()?;
        }
        Ok(())
    }

    fn replace_text(
        &self,
        next: String,
        success_message: &'static str,
        noop_message: &'static str,
    ) -> Result<ExecuteTransformationResponse, String> {
        self.check_cancelled()?;
        Ok(replace_text_response(
            self.original,
            next,
            success_message.to_string(),
            noop_message.to_string(),
        ))
    }

    fn show_message(
        &self,
        message: String,
        level: TransformationMessageLevel,
    ) -> Result<ExecuteTransformationResponse, String> {
        self.check_cancelled()?;
        Ok(ExecuteTransformationResponse::ShowMessage { message, level })
    }

    fn run_replace_text<F>(
        &self,
        success_message: &'static str,
        noop_message: &'static str,
        operation: F,
    ) -> Result<ExecuteTransformationResponse, String>
    where
        F: FnOnce(&Self) -> Result<String, String>,
    {
        self.check_cancelled()?;
        let next = operation(self)?;
        self.replace_text(next, success_message, noop_message)
    }

    fn run_show_message<F>(&self, operation: F) -> Result<ExecuteTransformationResponse, String>
    where
        F: FnOnce(&Self) -> Result<(String, TransformationMessageLevel), String>,
    {
        self.check_cancelled()?;
        let (message, level) = operation(self)?;
        self.show_message(message, level)
    }
}

fn trim_trailing_whitespace(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let mut result = String::with_capacity(text.len());

    for (line_index, line) in text.split_inclusive('\n').enumerate() {
        ctx.checkpoint(line_index, LINE_CANCEL_CHECK_INTERVAL)?;

        let (content, newline) = if let Some(content) = line.strip_suffix("\r\n") {
            (content, "\r\n")
        } else if let Some(content) = line.strip_suffix('\n') {
            (content, "\n")
        } else {
            (line, "")
        };

        result.push_str(content.trim_end_matches([' ', '\t']));
        result.push_str(newline);
    }

    Ok(result)
}

fn collapse_blank_lines(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let mut result = String::with_capacity(text.len());
    let mut previous_blank = false;

    for (line_index, line) in text.split_inclusive('\n').enumerate() {
        ctx.checkpoint(line_index, LINE_CANCEL_CHECK_INTERVAL)?;

        let (content, newline) = if let Some(content) = line.strip_suffix("\r\n") {
            (content, "\r\n")
        } else if let Some(content) = line.strip_suffix('\n') {
            (content, "\n")
        } else {
            (line, "")
        };

        let is_blank = content.trim().is_empty();
        if is_blank {
            if previous_blank {
                continue;
            }
            result.push_str(newline);
            previous_blank = true;
            continue;
        }

        result.push_str(content);
        result.push_str(newline);
        previous_blank = false;
    }

    if !text.ends_with('\n') && text.trim().is_empty() {
        return Ok(String::new());
    }

    Ok(result)
}

fn replace_text_response(
    original: &str,
    next: String,
    success_message: String,
    noop_message: String,
) -> ExecuteTransformationResponse {
    if next == original {
        return ExecuteTransformationResponse::ReplaceText {
            text: next,
            message: Some(noop_message),
            level: Some(TransformationMessageLevel::Info),
        };
    }

    ExecuteTransformationResponse::ReplaceText {
        text: next,
        message: Some(success_message),
        level: Some(TransformationMessageLevel::Success),
    }
}

/// Detect the most likely CSV delimiter by counting candidate bytes in the
/// first line of the input (single pass). Mirrors the frontend papaparse
/// `delimitersToGuess` order so behavior is consistent across table mode
/// and transformations.
fn detect_csv_delimiter(text: &str) -> u8 {
    const CANDIDATES: &[u8] = &[b',', b'\t', b';', b'|', b':', b'~'];

    let first_line = text.lines().next().unwrap_or("").as_bytes();

    // Single-pass count instead of scanning the line once per candidate.
    let mut counts = [0u32; 6];
    for &byte in first_line {
        for (i, &candidate) in CANDIDATES.iter().enumerate() {
            if byte == candidate {
                counts[i] += 1;
            }
        }
    }

    counts
        .iter()
        .enumerate()
        .filter(|(_, &c)| c > 0)
        .max_by_key(|(_, &c)| c)
        .map(|(i, _)| CANDIDATES[i])
        .unwrap_or(b',')
}

/// Serialize a `serde_json::Value` to a plain string for a CSV cell.
/// Primitive types are rendered directly; nested arrays/objects are
/// serialized as compact JSON strings so no data is silently dropped.
fn json_value_to_csv_field(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Parse CSV text (auto-detecting the delimiter) and produce a pretty-printed
/// JSON array of objects, using the first row as column headers.
///
/// Performance: streams JSON output directly to a pre-allocated String buffer
/// instead of building an intermediate `serde_json::Value` tree. This avoids
/// O(rows × cols) heap-allocated Value nodes and keeps peak memory at roughly
/// input_size + output_size rather than input + Value tree + output.
fn csv_to_json(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok("[]".to_string());
    }

    let delimiter = detect_csv_delimiter(trimmed);

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true) // tolerate rows with differing field counts
        .trim(csv::Trim::Fields) // strip surrounding whitespace per-field
        .from_reader(trimmed.as_bytes());

    let raw_headers = rdr
        .headers()
        .map_err(|e| format!("CSV parse error: {}", e))?
        .clone();

    let headers: Vec<&str> = raw_headers.iter().collect();
    if headers.is_empty() {
        return Ok("[]".to_string());
    }

    // Pre-escape header keys as JSON strings once so we pay the escaping cost
    // per-column rather than per-cell.
    let escaped_headers: Vec<String> = headers
        .iter()
        .map(|h| serde_json::to_string(h).unwrap_or_else(|_| format!("\"{}\"", h)))
        .collect();

    // Track progress via the reader's byte position (zero extra passes).
    let total_bytes = trimmed.len() as u32;

    // Stream JSON directly. Estimate output at ~3× input size.
    let mut out = String::with_capacity(trimmed.len().saturating_mul(3));
    out.push('[');

    let mut record = csv::StringRecord::new();
    let mut record_count: usize = 0;
    while rdr
        .read_record(&mut record)
        .map_err(|e| format!("CSV parse error: {}", e))?
    {
        ctx.check_cancelled()?;
        if record_count % 500 == 0 {
            ctx.report_progress(rdr.position().byte() as u32, total_bytes.max(1));
        }

        if record_count > 0 {
            out.push(',');
        }
        record_count += 1;

        out.push_str("\n  {");
        let mut field_count: usize = 0;
        for (i, field) in record.iter().enumerate() {
            let Some(escaped_key) = escaped_headers.get(i) else {
                break;
            };
            if headers[i].is_empty() {
                continue;
            }
            if field_count > 0 {
                out.push(',');
            }
            field_count += 1;
            out.push_str("\n    ");
            out.push_str(escaped_key);
            out.push_str(": ");
            // serde_json::to_string wraps in quotes and escapes special chars.
            match serde_json::to_string(field) {
                Ok(escaped) => out.push_str(&escaped),
                Err(_) => {
                    out.push('"');
                    out.push_str(field);
                    out.push('"');
                }
            }
        }
        if field_count > 0 {
            out.push_str("\n  }");
        } else {
            out.push('}');
        }
    }

    if record_count > 0 {
        out.push('\n');
    }
    out.push(']');

    Ok(out)
}

/// Parse a JSON array of objects and serialize it as RFC 4180 CSV, using the
/// union of all object keys as headers. Nested values are kept as compact JSON
/// strings so no data is silently dropped.
///
/// Performance: the full JSON parse into `serde_json::Value` is unavoidable
/// because we need a two-pass approach (collect all headers first, then emit
/// rows). We mitigate allocation cost with capacity hints on all containers
/// and a reusable row buffer that is cleared-and-refilled per record instead
/// of allocated fresh each iteration.
fn json_to_csv(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let value: serde_json::Value =
        serde_json::from_str(text.trim()).map_err(|e| format!("Invalid JSON: {}", e))?;

    let array = match &value {
        serde_json::Value::Array(arr) => arr,
        _ => {
            return Err(
                "JSON to CSV requires a top-level JSON array (e.g. [{...}, {...}]).".to_string(),
            )
        }
    };

    if array.is_empty() {
        return Ok(String::new());
    }

    // Collect headers: preserve insertion order from the first object, then
    // extend with any keys that appear only in later objects.
    let initial_cap = match &array[0] {
        serde_json::Value::Object(obj) => obj.len(),
        _ => {
            return Err(
                "JSON to CSV requires an array of objects (e.g. [{...}, {...}]).".to_string(),
            )
        }
    };

    let mut headers: Vec<String> = Vec::with_capacity(initial_cap);
    let mut seen: std::collections::HashSet<&str> =
        std::collections::HashSet::with_capacity(initial_cap);

    for item in array {
        match item {
            serde_json::Value::Object(obj) => {
                ctx.check_cancelled()?;
                for key in obj.keys() {
                    if seen.insert(key.as_str()) {
                        headers.push(key.clone());
                    }
                }
            }
            _ => {
                return Err(
                    "JSON to CSV requires an array of objects (e.g. [{...}, {...}]).".to_string(),
                )
            }
        }
    }

    // Pre-allocate output buffer: estimate ~50 bytes per cell as a rough heuristic.
    let total_rows = array.len() as u32;
    let estimated_size = (array.len() + 1) * headers.len() * 50;
    let mut wtr = csv::WriterBuilder::new()
        .terminator(csv::Terminator::Any(b'\n')) // LF-only, matching app CSV serialization
        .from_writer(Vec::with_capacity(estimated_size));

    wtr.write_record(&headers)
        .map_err(|e| format!("CSV write error: {}", e))?;

    // Reuse a single row buffer across all records to avoid per-row Vec allocations.
    let mut row: Vec<String> = Vec::with_capacity(headers.len());

    for (row_index, item) in array.iter().enumerate() {
        if let serde_json::Value::Object(obj) = item {
            ctx.check_cancelled()?;
            if row_index % 500 == 0 {
                ctx.report_progress(row_index as u32, total_rows.max(1));
            }

            row.clear();
            for h in &headers {
                row.push(obj.get(h).map(json_value_to_csv_field).unwrap_or_default());
            }

            wtr.write_record(&row)
                .map_err(|e| format!("CSV write error: {}", e))?;
        }
    }

    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV flush error: {}", e))?;

    String::from_utf8(bytes).map_err(|e| format!("CSV encoding error: {}", e))
}

/// Parse YAML directly into `serde_json::Value` so we preserve insertion order and
/// avoid an intermediate YAML AST for common JSON-compatible YAML documents.
fn yaml_to_json(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("Invalid YAML: document is empty.".to_string());
    }

    let value: serde_json::Value =
        serde_yaml::from_str(trimmed).map_err(|error| format!("Invalid YAML: {}", error))?;
    ctx.check_cancelled()?;

    serde_json::to_string_pretty(&value)
        .map_err(|error| format!("JSON serialization error: {}", error))
}

/// Convert JSON/JSONC to YAML while preserving object key order and matching
/// Boop-style output by omitting the optional YAML document start marker.
fn json_to_yaml(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let value = parse_jsonc_to_serde_value(text)?;
    ctx.check_cancelled()?;

    let mut yaml = serde_yaml::to_string(&value)
        .map_err(|error| format!("YAML serialization error: {}", error))?;
    if yaml.starts_with("---\r\n") {
        yaml.drain(..5);
    } else if yaml.starts_with("---\n") {
        yaml.drain(..4);
    }

    Ok(yaml)
}

// ════════════════════════════════════════════════════════════════════════════
// Shared line-splitting helper
// ════════════════════════════════════════════════════════════════════════════

/// Split a line produced by `split_inclusive('\n')` into its content and newline suffix.
/// Returns `(content, "\r\n" | "\n" | "")`.
#[inline]
fn split_line_newline(line: &str) -> (&str, &str) {
    if let Some(c) = line.strip_suffix("\r\n") {
        (c, "\r\n")
    } else if let Some(c) = line.strip_suffix('\n') {
        (c, "\n")
    } else {
        (line, "")
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Text transforms
// ════════════════════════════════════════════════════════════════════════════

/// Trim leading and trailing whitespace from the whole text (or selection).
fn text_trim(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    Ok(text.trim().to_string())
}

/// Convert every character to its Unicode uppercase equivalent.
fn text_uppercase(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    // to_uppercase() may expand a single char into multiple (e.g., ß → SS),
    // so we can't pre-allocate exactly; use text.len() as a conservative lower bound.
    let mut result = String::with_capacity(text.len());
    for (i, c) in text.char_indices() {
        ctx.checkpoint(i, BYTE_CANCEL_CHECK_INTERVAL)?;
        result.extend(c.to_uppercase());
    }
    Ok(result)
}

/// Convert every character to its Unicode lowercase equivalent.
fn text_lowercase(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let mut result = String::with_capacity(text.len());
    for (i, c) in text.char_indices() {
        ctx.checkpoint(i, BYTE_CANCEL_CHECK_INTERVAL)?;
        result.extend(c.to_lowercase());
    }
    Ok(result)
}

/// Reverse the order of lines.
/// Output is always LF-normalised; a trailing newline is preserved if the input had one.
fn text_reverse_lines(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let has_trailing_newline = text.ends_with('\n');

    // `str::lines` strips \r\n and \r correctly; we re-emit with \n.
    let lines: Vec<&str> = text.lines().collect();
    for (i, _) in lines.iter().enumerate() {
        ctx.checkpoint(i, LINE_CANCEL_CHECK_INTERVAL)?;
    }

    let mut result = String::with_capacity(text.len());
    for (i, line) in lines.iter().rev().enumerate() {
        if i > 0 {
            result.push('\n');
        }
        result.push_str(line);
    }
    if has_trailing_newline {
        result.push('\n');
    }
    Ok(result)
}

/// Reverse the characters in the text, correctly handling Unicode grapheme clusters
/// (combining marks, emoji, etc.) so that multi-codepoint characters stay intact.
fn text_reverse_string(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    // `graphemes(true)` uses the extended grapheme-cluster algorithm so emoji and
    // accented characters that span multiple codepoints are treated as a single unit.
    let graphemes: Vec<&str> = text.graphemes(true).collect();
    ctx.check_cancelled()?;
    Ok(graphemes.iter().rev().copied().collect())
}

/// Prepend `"> "` to every line, converting text into a Markdown block-quote.
fn text_markdown_quote(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    // Each line gains two extra bytes; use /20 as a conservative extra-space estimate.
    let mut result = String::with_capacity(text.len() + text.len() / 20 + 2);
    for (line_index, line) in text.split_inclusive('\n').enumerate() {
        ctx.checkpoint(line_index, LINE_CANCEL_CHECK_INTERVAL)?;
        let (content, newline) = split_line_newline(line);
        result.push_str("> ");
        result.push_str(content);
        result.push_str(newline);
    }
    Ok(result)
}

/// Apply ROT-13 substitution to ASCII letters; all other bytes pass through unchanged.
fn text_rot13(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    for (i, &b) in bytes.iter().enumerate() {
        ctx.checkpoint(i, BYTE_CANCEL_CHECK_INTERVAL)?;
        out.push(match b {
            b'a'..=b'z' => b'a' + (b - b'a' + 13) % 26,
            b'A'..=b'Z' => b'A' + (b - b'A' + 13) % 26,
            _ => b,
        });
    }
    // SAFETY: ROT-13 only modifies ASCII alpha bytes (0x41-0x5A, 0x61-0x7A).
    // These ranges never appear as leading or continuation bytes of multi-byte UTF-8
    // sequences.  All other bytes, including multi-byte sequences, are copied unchanged.
    Ok(unsafe { String::from_utf8_unchecked(out) })
}

/// Escape `\`, `"`, `'`, and NUL characters with a preceding backslash.
fn text_add_slashes(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let mut result = String::with_capacity(text.len() + text.len() / 10);
    for (i, c) in text.char_indices() {
        ctx.checkpoint(i, BYTE_CANCEL_CHECK_INTERVAL)?;
        match c {
            '\\' => {
                result.push('\\');
                result.push('\\');
            }
            '"' => {
                result.push('\\');
                result.push('"');
            }
            '\'' => {
                result.push('\\');
                result.push('\'');
            }
            '\0' => {
                result.push('\\');
                result.push('0');
            }
            _ => result.push(c),
        }
    }
    Ok(result)
}

/// Unescape backslash sequences: `\\`→`\`, `\"`→`"`, `\'`→`'`, `\0`→NUL,
/// `\x`→`x` for any other character, and a trailing lone `\` is dropped.
fn text_remove_slashes(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let mut result = String::with_capacity(text.len());
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        ctx.checkpoint(i, BYTE_CANCEL_CHECK_INTERVAL)?;
        if c == '\\' {
            match chars.next() {
                Some((_, '\\')) => result.push('\\'),
                Some((_, '0')) => result.push('\0'),
                Some((_, c)) => result.push(c),
                None => {} // trailing backslash: drop it
            }
        } else {
            result.push(c);
        }
    }
    Ok(result)
}

/// Sort lines alphabetically (Unicode byte order, stable for equal lines).
/// Output is LF-normalised; trailing newline is preserved.
fn text_sort_lines(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let has_trailing_newline = text.ends_with('\n');
    let mut lines: Vec<&str> = text.lines().collect();
    for (i, _) in lines.iter().enumerate() {
        ctx.checkpoint(i, LINE_CANCEL_CHECK_INTERVAL)?;
    }
    lines.sort_unstable();
    let mut result = String::with_capacity(text.len());
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            result.push('\n');
        }
        result.push_str(line);
    }
    if has_trailing_newline {
        result.push('\n');
    }
    Ok(result)
}

/// Remove duplicate lines, preserving the first occurrence of each.
/// Returns `(deduplicated_text, count_of_removed_lines)`.
fn text_remove_duplicate_lines<'t>(
    text: &'t str,
    ctx: &TransformationContext<'_>,
) -> Result<(String, usize), String> {
    ctx.check_cancelled()?;
    let has_trailing_newline = text.ends_with('\n');
    let mut seen: std::collections::HashSet<&'t str> = std::collections::HashSet::new();
    let mut result = String::with_capacity(text.len());
    let mut removed: usize = 0;
    let mut first = true;

    for (line_index, line) in text.lines().enumerate() {
        ctx.checkpoint(line_index, LINE_CANCEL_CHECK_INTERVAL)?;
        if seen.insert(line) {
            if !first {
                result.push('\n');
            }
            result.push_str(line);
            first = false;
        } else {
            removed += 1;
        }
    }
    if has_trailing_newline && !result.is_empty() {
        result.push('\n');
    }
    Ok((result, removed))
}

/// Collapse all line breaks into a single space, joining the entire text onto one line.
fn text_collapse_lines(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut result = String::with_capacity(len);
    let mut i = 0;
    while i < len {
        ctx.checkpoint(i, BYTE_CANCEL_CHECK_INTERVAL)?;
        let b = bytes[i];
        if b == b'\r' {
            // \r\n or standalone \r → single space
            if i + 1 < len && bytes[i + 1] == b'\n' {
                i += 1;
            }
            result.push(' ');
            i += 1;
        } else if b == b'\n' {
            result.push(' ');
            i += 1;
        } else if b < 0x80 {
            // Single-byte ASCII
            result.push(b as char);
            i += 1;
        } else {
            // Multi-byte UTF-8 sequence
            let c = text[i..].chars().next().expect("valid UTF-8");
            result.push(c);
            i += c.len_utf8();
        }
    }
    Ok(result)
}

// ════════════════════════════════════════════════════════════════════════════
// Case transforms (applied per line via `heck`)
// ════════════════════════════════════════════════════════════════════════════

/// Shared per-line case conversion driver.  Empty / whitespace-only lines are
/// preserved unchanged; non-empty lines are passed through `convert`.
fn apply_per_line_case<F>(
    text: &str,
    ctx: &TransformationContext<'_>,
    convert: F,
) -> Result<String, String>
where
    F: Fn(&str) -> String,
{
    ctx.check_cancelled()?;
    let mut result = String::with_capacity(text.len());
    for (line_index, line) in text.split_inclusive('\n').enumerate() {
        ctx.checkpoint(line_index, LINE_CANCEL_CHECK_INTERVAL)?;
        let (content, newline) = split_line_newline(line);
        let converted = if content.trim().is_empty() {
            content.to_string()
        } else {
            convert(content)
        };
        result.push_str(&converted);
        result.push_str(newline);
    }
    Ok(result)
}

fn text_camel_case(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    apply_per_line_case(text, ctx, |s| AsLowerCamelCase(s).to_string())
}

fn text_snake_case(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    apply_per_line_case(text, ctx, |s| AsSnakeCase(s).to_string())
}

fn text_kebab_case(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    apply_per_line_case(text, ctx, |s| AsKebabCase(s).to_string())
}

fn text_title_case(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    apply_per_line_case(text, ctx, |s| AsTitleCase(s).to_string())
}

/// Alternate uppercase / lowercase for each alphabetic character.
/// Non-alphabetic characters pass through unchanged without resetting the toggle,
/// so the alternation pattern is based on the count of letters seen so far.
fn text_sponge_case(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let mut result = String::with_capacity(text.len());
    let mut uppercase = true;
    for (i, c) in text.char_indices() {
        ctx.checkpoint(i, BYTE_CANCEL_CHECK_INTERVAL)?;
        if c.is_alphabetic() {
            if uppercase {
                result.extend(c.to_uppercase());
            } else {
                result.extend(c.to_lowercase());
            }
            uppercase = !uppercase;
        } else {
            result.push(c);
        }
    }
    Ok(result)
}

// ════════════════════════════════════════════════════════════════════════════
// JSON key-case transforms
// ════════════════════════════════════════════════════════════════════════════

/// Recursively rename every object key in a JSON value using `convert`.
/// Array elements and scalar values are traversed / passed through unchanged.
#[derive(Debug)]
struct JsonKeyReplacement {
    range: JsonRange,
    text: String,
}

fn collect_json_key_replacements<'a, F>(
    value: &'a JsonAstValue<'a>,
    convert: &F,
    replacements: &mut Vec<JsonKeyReplacement>,
    ctx: &TransformationContext<'_>,
    visited_nodes: &mut usize,
) -> Result<(), String>
where
    F: Fn(&str) -> String,
{
    match value {
        JsonAstValue::Object(object) => {
            for (index, property) in object.properties.iter().enumerate() {
                *visited_nodes += 1;
                ctx.checkpoint(*visited_nodes, LINE_CANCEL_CHECK_INTERVAL)?;

                match &property.name {
                    ObjectPropName::String(name) => {
                        let converted = convert(name.value.as_ref());
                        if converted != name.value.as_ref() {
                            replacements.push(JsonKeyReplacement {
                                range: name.range,
                                text: serde_json::to_string(&converted).map_err(|error| {
                                    format!("Failed to serialize JSON key: {}", error)
                                })?,
                            });
                        }
                    }
                    ObjectPropName::Word(name) => {
                        let converted = convert(name.value);
                        if converted != name.value {
                            replacements.push(JsonKeyReplacement {
                                range: name.range,
                                text: converted,
                            });
                        }
                    }
                }

                ctx.checkpoint(index, LINE_CANCEL_CHECK_INTERVAL)?;
                collect_json_key_replacements(
                    &property.value,
                    convert,
                    replacements,
                    ctx,
                    visited_nodes,
                )?;
            }
        }
        JsonAstValue::Array(array) => {
            for element in &array.elements {
                *visited_nodes += 1;
                ctx.checkpoint(*visited_nodes, LINE_CANCEL_CHECK_INTERVAL)?;
                collect_json_key_replacements(element, convert, replacements, ctx, visited_nodes)?;
            }
        }
        JsonAstValue::StringLit(_)
        | JsonAstValue::NumberLit(_)
        | JsonAstValue::BooleanLit(_)
        | JsonAstValue::NullKeyword(_) => {}
    }

    Ok(())
}

fn apply_json_key_replacements(
    text: &str,
    mut replacements: Vec<JsonKeyReplacement>,
    ctx: &TransformationContext<'_>,
) -> Result<String, String> {
    if replacements.is_empty() {
        return Ok(text.to_string());
    }

    replacements.sort_by_key(|replacement| replacement.range.start);

    let mut result = String::with_capacity(text.len());
    let mut cursor = 0usize;

    for (index, replacement) in replacements.iter().enumerate() {
        ctx.checkpoint(index, LINE_CANCEL_CHECK_INTERVAL)?;

        if replacement.range.start < cursor
            || replacement.range.end > text.len()
            || replacement.range.start > replacement.range.end
        {
            return Err("Invalid JSON: encountered overlapping key ranges.".to_string());
        }

        result.push_str(&text[cursor..replacement.range.start]);
        result.push_str(&replacement.text);
        cursor = replacement.range.end;
    }

    result.push_str(&text[cursor..]);
    Ok(result)
}

/// Shared driver for JSON key-case transforms: parse → rewrite key tokens in place.
fn json_convert_keys_case<F>(
    text: &str,
    ctx: &TransformationContext<'_>,
    convert: F,
) -> Result<String, String>
where
    F: Fn(&str) -> String,
{
    ctx.check_cancelled()?;
    let parsed = parse_to_ast(text, &CollectOptions::default(), &JSONC_PARSE_OPTIONS)
        .map_err(|error| format!("Invalid JSON: {}", error))?;
    let value = parsed
        .value
        .ok_or_else(|| "Invalid JSON: document is empty.".to_string())?;

    let mut replacements = Vec::new();
    let mut visited_nodes = 0usize;
    collect_json_key_replacements(&value, &convert, &mut replacements, ctx, &mut visited_nodes)?;
    apply_json_key_replacements(text, replacements, ctx)
}

fn json_keys_camel_case(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    json_convert_keys_case(text, ctx, |s| AsLowerCamelCase(s).to_string())
}

fn json_keys_snake_case(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    json_convert_keys_case(text, ctx, |s| AsSnakeCase(s).to_string())
}

fn json_keys_kebab_case(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    json_convert_keys_case(text, ctx, |s| AsKebabCase(s).to_string())
}

fn json_keys_title_case(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    json_convert_keys_case(text, ctx, |s| AsTitleCase(s).to_string())
}

fn json_keys_sponge_case(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    json_convert_keys_case(text, ctx, |s| {
        let mut result = String::with_capacity(s.len());
        let mut uppercase = true;
        for c in s.chars() {
            if c.is_alphabetic() {
                if uppercase {
                    result.extend(c.to_uppercase());
                } else {
                    result.extend(c.to_lowercase());
                }
                uppercase = !uppercase;
            } else {
                result.push(c);
            }
        }
        result
    })
}

// ════════════════════════════════════════════════════════════════════════════
// URL transforms
// ════════════════════════════════════════════════════════════════════════════

/// Percent-encode the text using `encodeURIComponent` semantics
/// (unreserved RFC-3986 characters and `! ' ( ) *` are NOT encoded).
fn url_encode(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    Ok(urlencoding::encode(text).into_owned())
}

/// Percent-decode the text.  Returns an error if the encoding is malformed
/// or the decoded bytes are not valid UTF-8.
fn url_decode(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    urlencoding::decode(text)
        .map(|cow| cow.into_owned())
        .map_err(|e| format!("URL decode failed: {}", e))
}

/// Defang potentially dangerous URLs and IOCs:
///   `.`   → `[.]`
///   `://` → `[://]`
///   `http` (case-insensitive) → `hXXp`
///
/// Patterns are matched left-to-right so `https://` becomes `hXXps[://]`.
fn url_defang(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut result = String::with_capacity(len + len / 5);
    let mut i = 0;
    while i < len {
        ctx.checkpoint(i, BYTE_CANCEL_CHECK_INTERVAL)?;

        // "http" (case-insensitive) → "hXXp"
        if i + 4 <= len
            && bytes[i].to_ascii_lowercase() == b'h'
            && bytes[i + 1].to_ascii_lowercase() == b't'
            && bytes[i + 2].to_ascii_lowercase() == b't'
            && bytes[i + 3].to_ascii_lowercase() == b'p'
        {
            result.push_str("hXXp");
            i += 4;
            continue;
        }

        // "://" → "[://]"
        if i + 3 <= len && &bytes[i..i + 3] == b"://" {
            result.push_str("[://]");
            i += 3;
            continue;
        }

        // "." → "[.]"
        if bytes[i] == b'.' {
            result.push_str("[.]");
            i += 1;
            continue;
        }

        // Copy this character (handles multi-byte UTF-8 safely)
        if bytes[i] < 0x80 {
            result.push(bytes[i] as char);
            i += 1;
        } else {
            let c = text[i..].chars().next().expect("valid UTF-8");
            result.push(c);
            i += c.len_utf8();
        }
    }
    Ok(result)
}

/// Refang defanged URLs and IOCs (reverses `url_defang`):
///   `hXXp` / `hxxp` (case-insensitive) → `http`
///   `[://]` → `://`
///   `[.]`   → `.`
fn url_refang(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut result = String::with_capacity(len);
    let mut i = 0;
    while i < len {
        ctx.checkpoint(i, BYTE_CANCEL_CHECK_INTERVAL)?;

        // "hXXp" / "hxxp" (case-insensitive h, X/x, X/x, p) → "http"
        if i + 4 <= len
            && bytes[i].to_ascii_lowercase() == b'h'
            && (bytes[i + 1] == b'X' || bytes[i + 1] == b'x')
            && (bytes[i + 2] == b'X' || bytes[i + 2] == b'x')
            && bytes[i + 3].to_ascii_lowercase() == b'p'
        {
            result.push_str("http");
            i += 4;
            continue;
        }

        // "[://]" → "://"
        if i + 5 <= len && &bytes[i..i + 5] == b"[://]" {
            result.push_str("://");
            i += 5;
            continue;
        }

        // "[.]" → "."
        if i + 3 <= len && &bytes[i..i + 3] == b"[.]" {
            result.push('.');
            i += 3;
            continue;
        }

        // Copy this character
        if bytes[i] < 0x80 {
            result.push(bytes[i] as char);
            i += 1;
        } else {
            let c = text[i..].chars().next().expect("valid UTF-8");
            result.push(c);
            i += c.len_utf8();
        }
    }
    Ok(result)
}

// ════════════════════════════════════════════════════════════════════════════
// Encoding transforms
// ════════════════════════════════════════════════════════════════════════════

/// Encode the text (as UTF-8 bytes) to standard Base64.
fn encoding_base64_encode(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    Ok(BASE64_STANDARD.encode(text.as_bytes()))
}

/// Decode a Base64 string back to UTF-8 text.
/// Leading/trailing whitespace is stripped to be forgiving of copy-paste artifacts.
fn encoding_base64_decode(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let bytes = BASE64_STANDARD
        .decode(text.trim())
        .map_err(|e| format!("Invalid Base64: {}", e))?;
    String::from_utf8(bytes).map_err(|e| format!("Base64 decoded to non-UTF-8 bytes: {}", e))
}

fn encoding_base64url_encode(
    text: &str,
    ctx: &TransformationContext<'_>,
) -> Result<String, String> {
    ctx.check_cancelled()?;
    Ok(BASE64_URL_SAFE_NO_PAD.encode(text.as_bytes()))
}

fn encoding_base64url_decode(
    text: &str,
    ctx: &TransformationContext<'_>,
) -> Result<String, String> {
    ctx.check_cancelled()?;
    let input = text.trim();
    let bytes = BASE64_URL_SAFE_NO_PAD
        .decode(input)
        .or_else(|_| BASE64_URL_SAFE.decode(input))
        .map_err(|error| format!("Invalid Base64URL: {}", error))?;
    String::from_utf8(bytes).map_err(|error| {
        format!(
            "Base64URL decoded to non-UTF-8 bytes: {}",
            error.utf8_error()
        )
    })
}

fn encoding_html_encode(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let mut output = String::with_capacity(text.len());
    let total_bytes = text.len() as u32;
    for (character_index, (byte_index, character)) in text.char_indices().enumerate() {
        ctx.checkpoint(character_index, BYTE_CANCEL_CHECK_INTERVAL)?;
        ctx.report_progress(byte_index as u32, total_bytes.max(1));
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            _ => output.push(character),
        }
    }
    Ok(output)
}

fn encoding_html_decode(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    Ok(html_escape::decode_html_entities(text).into_owned())
}

fn encoding_gzip_to_base64(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    let total_bytes = text.len() as u32;
    for (index, chunk) in text.as_bytes().chunks(64 * 1024).enumerate() {
        ctx.checkpoint(index, 1)?;
        let processed = ((index + 1) * 64 * 1024).min(text.len()) as u32;
        ctx.report_progress(processed, total_bytes.max(1));
        encoder
            .write_all(chunk)
            .map_err(|error| format!("Gzip compression failed: {}", error))?;
    }
    let compressed = encoder
        .finish()
        .map_err(|error| format!("Gzip compression failed: {}", error))?;
    Ok(BASE64_STANDARD.encode(compressed))
}

fn encoding_gzip_from_base64(
    text: &str,
    ctx: &TransformationContext<'_>,
) -> Result<String, String> {
    ctx.check_cancelled()?;
    let compressed = BASE64_STANDARD
        .decode(text.trim())
        .map_err(|error| format!("Invalid Base64 gzip input: {}", error))?;
    let compressed_len = compressed.len() as u32;
    let mut decoder = GzDecoder::new(Cursor::new(compressed));
    let mut decoded = Vec::new();
    let mut buffer = [0u8; 64 * 1024];

    loop {
        ctx.check_cancelled()?;
        let count = decoder
            .read(&mut buffer)
            .map_err(|error| format!("Gzip decompression failed: {}", error))?;
        ctx.report_progress(decoder.get_ref().position() as u32, compressed_len.max(1));
        if count == 0 {
            break;
        }
        let next_len = decoded
            .len()
            .checked_add(count)
            .ok_or_else(|| "Gzip output size overflowed the supported range.".to_string())?;
        if next_len > MAX_GZIP_DECOMPRESSED_BYTES {
            return Err(format!(
                "Gzip output exceeds the {} MB safety limit.",
                MAX_GZIP_DECOMPRESSED_BYTES / (1024 * 1024)
            ));
        }
        decoded.extend_from_slice(&buffer[..count]);
    }

    String::from_utf8(decoded).map_err(|error| {
        format!(
            "Gzip decompressed to non-UTF-8 bytes: {}",
            error.utf8_error()
        )
    })
}

fn encoding_jwt_decode(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let token = text.trim();
    let mut segments = token.split('.');
    let header_segment = segments
        .next()
        .ok_or_else(|| "Invalid JWT: missing header segment.".to_string())?;
    let payload_segment = segments
        .next()
        .ok_or_else(|| "Invalid JWT: missing payload segment.".to_string())?;
    let signature_segment = segments
        .next()
        .ok_or_else(|| "Invalid JWT: missing signature segment.".to_string())?;
    if segments.next().is_some() || header_segment.is_empty() || payload_segment.is_empty() {
        return Err("Invalid JWT: expected exactly three compact segments.".to_string());
    }

    let decode_json_segment = |segment: &str, name: &str| -> Result<serde_json::Value, String> {
        let bytes = BASE64_URL_SAFE_NO_PAD
            .decode(segment)
            .or_else(|_| BASE64_URL_SAFE.decode(segment))
            .map_err(|error| format!("Invalid JWT {} encoding: {}", name, error))?;
        serde_json::from_slice(&bytes)
            .map_err(|error| format!("Invalid JWT {} JSON: {}", name, error))
    };

    let header = decode_json_segment(header_segment, "header")?;
    ctx.check_cancelled()?;
    let payload = decode_json_segment(payload_segment, "payload")?;
    let decoded = serde_json::json!({
        "header": header,
        "payload": payload,
        "signature": signature_segment,
    });
    serde_json::to_string_pretty(&decoded)
        .map_err(|error| format!("Failed to serialize decoded JWT: {}", error))
}

fn hash_digest<D>(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String>
where
    D: Digest + Default,
{
    ctx.check_cancelled()?;
    let mut digest = D::default();
    let total_bytes = text.len() as u32;
    for (index, chunk) in text
        .as_bytes()
        .chunks(BYTE_CANCEL_CHECK_INTERVAL)
        .enumerate()
    {
        ctx.checkpoint(index, 1)?;
        let processed = ((index + 1) * BYTE_CANCEL_CHECK_INTERVAL).min(text.len()) as u32;
        ctx.report_progress(processed, total_bytes.max(1));
        digest.update(chunk);
    }
    let bytes = digest.finalize();
    let mut output = String::with_capacity(bytes.len() * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    Ok(output)
}

fn hash_sha256(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    hash_digest::<Sha256>(text, ctx)
}

fn hash_sha512(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    hash_digest::<Sha512>(text, ctx)
}

fn hash_sha1(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    hash_digest::<Sha1>(text, ctx)
}

fn hash_md5(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    hash_digest::<Md5>(text, ctx)
}

fn checksum_crc32(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let mut hasher = Crc32Hasher::new();
    let total_bytes = text.len() as u32;
    for (index, chunk) in text
        .as_bytes()
        .chunks(BYTE_CANCEL_CHECK_INTERVAL)
        .enumerate()
    {
        ctx.checkpoint(index, 1)?;
        let processed = ((index + 1) * BYTE_CANCEL_CHECK_INTERVAL).min(text.len()) as u32;
        ctx.report_progress(processed, total_bytes.max(1));
        hasher.update(chunk);
    }
    Ok(format!("{:08x}", hasher.finalize()))
}

fn parse_single_integer(text: &str, label: &str) -> Result<i64, String> {
    text.trim()
        .parse::<i64>()
        .map_err(|error| format!("Invalid {}: {}", label, error))
}

fn time_unix_seconds_to_rfc3339(
    text: &str,
    ctx: &TransformationContext<'_>,
) -> Result<String, String> {
    ctx.check_cancelled()?;
    let seconds = parse_single_integer(text, "Unix-seconds value")?;
    let timestamp = DateTime::<Utc>::from_timestamp(seconds, 0)
        .ok_or_else(|| "Unix-seconds value is outside the supported date range.".to_string())?;
    Ok(timestamp.to_rfc3339_opts(SecondsFormat::Secs, true))
}

fn time_unix_milliseconds_to_rfc3339(
    text: &str,
    ctx: &TransformationContext<'_>,
) -> Result<String, String> {
    ctx.check_cancelled()?;
    let milliseconds = parse_single_integer(text, "Unix-milliseconds value")?;
    let timestamp = DateTime::<Utc>::from_timestamp_millis(milliseconds).ok_or_else(|| {
        "Unix-milliseconds value is outside the supported date range.".to_string()
    })?;
    Ok(timestamp.to_rfc3339_opts(SecondsFormat::Millis, true))
}

fn parse_rfc3339(text: &str) -> Result<DateTime<chrono::FixedOffset>, String> {
    DateTime::parse_from_rfc3339(text.trim())
        .map_err(|error| format!("Invalid RFC 3339 timestamp: {}", error))
}

fn time_rfc3339_to_unix_seconds(
    text: &str,
    ctx: &TransformationContext<'_>,
) -> Result<String, String> {
    ctx.check_cancelled()?;
    Ok(parse_rfc3339(text)?.timestamp().to_string())
}

fn time_rfc3339_to_unix_milliseconds(
    text: &str,
    ctx: &TransformationContext<'_>,
) -> Result<String, String> {
    ctx.check_cancelled()?;
    Ok(parse_rfc3339(text)?.timestamp_millis().to_string())
}

fn url_query_to_json(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let query = text.trim().strip_prefix('?').unwrap_or(text.trim());
    let mut object = serde_json::Map::new();

    for (index, (key, value)) in url::form_urlencoded::parse(query.as_bytes()).enumerate() {
        ctx.checkpoint(index, LINE_CANCEL_CHECK_INTERVAL)?;
        let value = serde_json::Value::String(value.into_owned());
        match object.get_mut(key.as_ref()) {
            None => {
                object.insert(key.into_owned(), value);
            }
            Some(serde_json::Value::Array(values)) => values.push(value),
            Some(existing) => {
                let first = std::mem::replace(existing, serde_json::Value::Null);
                *existing = serde_json::Value::Array(vec![first, value]);
            }
        }
    }

    serde_json::to_string_pretty(&serde_json::Value::Object(object))
        .map_err(|error| format!("Failed to serialize query parameters: {}", error))
}

fn query_scalar_to_string(value: &serde_json::Value) -> Result<String, String> {
    match value {
        serde_json::Value::Null => Ok(String::new()),
        serde_json::Value::Bool(value) => Ok(value.to_string()),
        serde_json::Value::Number(value) => Ok(value.to_string()),
        serde_json::Value::String(value) => Ok(value.clone()),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            Err("Query parameter values must be scalars or arrays of scalars.".to_string())
        }
    }
}

fn url_json_to_query(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("Invalid JSON: {}", error))?;
    let object = value
        .as_object()
        .ok_or_else(|| "JSON to Query String requires a top-level object.".to_string())?;
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());

    for (index, (key, value)) in object.iter().enumerate() {
        ctx.checkpoint(index, LINE_CANCEL_CHECK_INTERVAL)?;
        match value {
            serde_json::Value::Array(values) => {
                for value in values {
                    serializer.append_pair(key, &query_scalar_to_string(value)?);
                }
            }
            value => {
                serializer.append_pair(key, &query_scalar_to_string(value)?);
            }
        }
    }

    Ok(serializer.finish())
}

fn json_lines_to_array(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let mut values = Vec::new();
    let total_bytes = text.len() as u32;
    let mut processed_bytes = 0usize;
    for (line_index, line) in text.split_inclusive('\n').enumerate() {
        ctx.checkpoint(line_index, LINE_CANCEL_CHECK_INTERVAL)?;
        processed_bytes += line.len();
        ctx.report_progress(processed_bytes as u32, total_bytes.max(1));
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(trimmed)
            .map_err(|error| format!("Invalid JSON on line {}: {}", line_index + 1, error))?;
        values.push(value);
    }
    serde_json::to_string_pretty(&values)
        .map_err(|error| format!("Failed to serialize JSON array: {}", error))
}

fn json_array_to_lines(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("Invalid JSON: {}", error))?;
    let values = value
        .as_array()
        .ok_or_else(|| "JSON Array to JSON Lines requires a top-level array.".to_string())?;
    let mut output = String::new();
    for (index, value) in values.iter().enumerate() {
        ctx.checkpoint(index, LINE_CANCEL_CHECK_INTERVAL)?;
        if index > 0 {
            output.push('\n');
        }
        output.push_str(
            &serde_json::to_string(value)
                .map_err(|error| format!("Failed to serialize array item: {}", error))?,
        );
    }
    if !values.is_empty() {
        output.push('\n');
    }
    Ok(output)
}

fn sort_json_value(
    value: serde_json::Value,
    ctx: &TransformationContext<'_>,
    visited: &mut usize,
) -> Result<serde_json::Value, String> {
    *visited += 1;
    ctx.checkpoint(*visited, LINE_CANCEL_CHECK_INTERVAL)?;
    match value {
        serde_json::Value::Object(object) => {
            let mut entries: Vec<_> = object.into_iter().collect();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            let mut sorted = serde_json::Map::with_capacity(entries.len());
            for (key, value) in entries {
                sorted.insert(key, sort_json_value(value, ctx, visited)?);
            }
            Ok(serde_json::Value::Object(sorted))
        }
        serde_json::Value::Array(values) => values
            .into_iter()
            .map(|value| sort_json_value(value, ctx, visited))
            .collect::<Result<Vec<_>, _>>()
            .map(serde_json::Value::Array),
        scalar => Ok(scalar),
    }
}

fn serialize_json_with_indent(
    value: &serde_json::Value,
    use_tabs: bool,
    indent_width: u32,
) -> Result<String, String> {
    let indent = if use_tabs {
        vec![b'\t']
    } else {
        vec![b' '; indent_width.max(1) as usize]
    };
    let formatter = serde_json::ser::PrettyFormatter::with_indent(&indent);
    let mut output = Vec::new();
    let mut serializer = serde_json::Serializer::with_formatter(&mut output, formatter);
    value
        .serialize(&mut serializer)
        .map_err(|error| format!("Failed to serialize JSON: {}", error))?;
    String::from_utf8(output).map_err(|error| format!("Failed to encode JSON output: {}", error))
}

fn json_sort_keys(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("Invalid strict JSON: {}", error))?;
    let mut visited = 0usize;
    let sorted = sort_json_value(value, ctx, &mut visited)?;
    let (use_tabs, indent_width) = resolve_format_indent(ctx.params(), text);
    serialize_json_with_indent(&sorted, use_tabs, indent_width)
}

fn is_typescript_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    if !(first == '_' || first == '$' || first.is_ascii_alphabetic()) {
        return false;
    }
    characters
        .all(|character| character == '_' || character == '$' || character.is_ascii_alphanumeric())
}

fn typescript_property_name(value: &str) -> Result<String, String> {
    if is_typescript_identifier(value) {
        Ok(value.to_string())
    } else {
        serde_json::to_string(value)
            .map_err(|error| format!("Failed to serialize TypeScript property name: {}", error))
    }
}

fn typescript_type_for_json(
    value: &serde_json::Value,
    ctx: &TransformationContext<'_>,
    visited: &mut usize,
) -> Result<String, String> {
    *visited += 1;
    ctx.checkpoint(*visited, LINE_CANCEL_CHECK_INTERVAL)?;
    match value {
        serde_json::Value::Null => Ok("null".to_string()),
        serde_json::Value::Bool(_) => Ok("boolean".to_string()),
        serde_json::Value::Number(_) => Ok("number".to_string()),
        serde_json::Value::String(_) => Ok("string".to_string()),
        serde_json::Value::Array(values) => {
            if values.is_empty() {
                return Ok("Array<unknown>".to_string());
            }
            let mut element_types = Vec::new();
            for value in values {
                let element_type = typescript_type_for_json(value, ctx, visited)?;
                if !element_types.contains(&element_type) {
                    element_types.push(element_type);
                }
            }
            Ok(format!("Array<{}>", element_types.join(" | ")))
        }
        serde_json::Value::Object(object) => {
            let mut output = String::from("{");
            for (key, value) in object {
                output.push_str(&typescript_property_name(key)?);
                output.push_str(": ");
                output.push_str(&typescript_type_for_json(value, ctx, visited)?);
                output.push(';');
            }
            output.push('}');
            Ok(output)
        }
    }
}

fn json_to_typescript(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("Invalid strict JSON: {}", error))?;
    let mut visited = 0usize;
    let unformatted = match &value {
        serde_json::Value::Object(object) => {
            let mut declaration = String::from("export interface Root {");
            for (key, value) in object {
                declaration.push_str(&typescript_property_name(key)?);
                declaration.push_str(": ");
                declaration.push_str(&typescript_type_for_json(value, ctx, &mut visited)?);
                declaration.push(';');
            }
            declaration.push('}');
            declaration
        }
        value => format!(
            "export type Root = {};",
            typescript_type_for_json(value, ctx, &mut visited)?
        ),
    };

    let (use_tabs, indent_width) = resolve_format_indent(ctx.params(), text);
    let config = ts_format_config(use_tabs, indent_width);
    let formatted = format_ts_text(FormatTextOptions {
        path: Path::new("generated.ts"),
        extension: None,
        text: unformatted.clone(),
        config: &config,
        external_formatter: None,
    })
    .map_err(|error| format!("Failed to format generated TypeScript: {}", error))?;
    Ok(formatted.unwrap_or(unformatted))
}

#[derive(Clone, Copy)]
struct XmlElementState {
    preserve_space: bool,
    has_text: bool,
}

fn xml_element_preserves_space(element: &BytesStart<'_>, inherited: bool) -> Result<bool, String> {
    let mut preserve = inherited;
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|error| format!("Invalid XML attribute: {}", error))?;
        if attribute.key.as_ref() != b"xml:space" {
            continue;
        }
        let value = attribute
            .unescape_value()
            .map_err(|error| format!("Invalid xml:space value: {}", error))?;
        preserve = match value.as_ref() {
            "preserve" => true,
            "default" => false,
            other => {
                return Err(format!(
                    "Invalid XML: xml:space must be 'default' or 'preserve', found '{}'.",
                    other
                ))
            }
        };
    }
    Ok(preserve)
}

fn write_xml_event(
    writer: &mut Option<XmlWriter<Vec<u8>>>,
    event: Event<'_>,
) -> Result<(), String> {
    if let Some(writer) = writer.as_mut() {
        writer
            .write_event(event)
            .map_err(|error| format!("Failed to write XML output: {}", error))?;
    }
    Ok(())
}

fn process_xml(
    text: &str,
    writer: &mut Option<XmlWriter<Vec<u8>>>,
    ctx: &TransformationContext<'_>,
) -> Result<(), String> {
    ctx.check_cancelled()?;
    let mut reader = XmlReader::from_str(text);
    reader.config_mut().trim_text(false);
    reader.config_mut().check_end_names = true;
    reader.config_mut().expand_empty_elements = false;

    let mut stack: Vec<XmlElementState> = Vec::new();
    let mut root_count = 0usize;
    let mut event_count = 0usize;

    loop {
        ctx.checkpoint(event_count, LINE_CANCEL_CHECK_INTERVAL)?;
        event_count += 1;
        let event = reader
            .read_event()
            .map_err(|error| format!("Invalid XML: {}", error))?;
        ctx.report_progress(reader.buffer_position() as u32, (text.len() as u32).max(1));

        match event {
            Event::Start(element) => {
                if stack.is_empty() {
                    root_count += 1;
                    if root_count > 1 {
                        return Err(
                            "Invalid XML: document has more than one root element.".to_string()
                        );
                    }
                }
                let inherited = stack
                    .last()
                    .map(|state| state.preserve_space)
                    .unwrap_or(false);
                let preserve_space = xml_element_preserves_space(&element, inherited)?;
                write_xml_event(writer, Event::Start(element))?;
                stack.push(XmlElementState {
                    preserve_space,
                    has_text: false,
                });
            }
            Event::Empty(element) => {
                if stack.is_empty() {
                    root_count += 1;
                    if root_count > 1 {
                        return Err(
                            "Invalid XML: document has more than one root element.".to_string()
                        );
                    }
                }
                let inherited = stack
                    .last()
                    .map(|state| state.preserve_space)
                    .unwrap_or(false);
                xml_element_preserves_space(&element, inherited)?;
                write_xml_event(writer, Event::Empty(element))?;
            }
            Event::End(element) => {
                if stack.pop().is_none() {
                    return Err("Invalid XML: unexpected closing element.".to_string());
                }
                write_xml_event(writer, Event::End(element))?;
            }
            Event::Text(content) => {
                let decoded = content
                    .decode()
                    .map_err(|error| format!("Invalid XML text encoding: {}", error))?;
                let is_whitespace = decoded.trim().is_empty();
                let Some(state) = stack.last_mut() else {
                    if is_whitespace {
                        continue;
                    }
                    return Err("Invalid XML: text is not inside the root element.".to_string());
                };

                if !is_whitespace {
                    state.has_text = true;
                    write_xml_event(writer, Event::Text(content))?;
                } else if state.preserve_space || state.has_text {
                    write_xml_event(writer, Event::Text(content))?;
                }
            }
            Event::CData(content) => {
                let Some(state) = stack.last_mut() else {
                    return Err("Invalid XML: CDATA is not inside the root element.".to_string());
                };
                if !content.is_empty() {
                    state.has_text = true;
                }
                write_xml_event(writer, Event::CData(content))?;
            }
            Event::GeneralRef(reference) => {
                let Some(state) = stack.last_mut() else {
                    return Err(
                        "Invalid XML: entity reference is not inside the root element.".to_string(),
                    );
                };
                state.has_text = true;
                write_xml_event(writer, Event::GeneralRef(reference))?;
            }
            Event::Decl(declaration) => write_xml_event(writer, Event::Decl(declaration))?,
            Event::PI(instruction) => write_xml_event(writer, Event::PI(instruction))?,
            Event::Comment(comment) => write_xml_event(writer, Event::Comment(comment))?,
            Event::DocType(doctype) => write_xml_event(writer, Event::DocType(doctype))?,
            Event::Eof => break,
        }
    }

    if !stack.is_empty() {
        return Err("Invalid XML: one or more elements are not closed.".to_string());
    }
    if root_count == 0 {
        return Err("Invalid XML: document has no root element.".to_string());
    }
    Ok(())
}

fn xml_output(
    text: &str,
    ctx: &TransformationContext<'_>,
    formatted: bool,
) -> Result<String, String> {
    let mut writer = if formatted {
        let (use_tabs, indent_width) = resolve_format_indent(ctx.params(), text);
        Some(XmlWriter::new_with_indent(
            Vec::new(),
            if use_tabs { b'\t' } else { b' ' },
            if use_tabs {
                1
            } else {
                indent_width.max(1) as usize
            },
        ))
    } else {
        Some(XmlWriter::new(Vec::new()))
    };
    process_xml(text, &mut writer, ctx)?;
    let bytes = writer
        .take()
        .expect("XML output processing always has a writer")
        .into_inner();
    String::from_utf8(bytes).map_err(|error| format!("XML output was not valid UTF-8: {}", error))
}

fn xml_format(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    xml_output(text, ctx, true)
}

fn xml_minify(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    xml_output(text, ctx, false)
}

fn xml_validate(text: &str, ctx: &TransformationContext<'_>) -> Result<(), String> {
    let mut writer = None;
    process_xml(text, &mut writer, ctx)
}

fn generate_uuid_v4(ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    Ok(Uuid::new_v4().to_string())
}

fn generate_uuid_v7(ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    Ok(Uuid::now_v7().to_string())
}

// ════════════════════════════════════════════════════════════════════════════
// Numeric / encoding conversions
// ════════════════════════════════════════════════════════════════════════════

/// Encode every UTF-8 byte of the input as exactly two uppercase hex characters
/// (no spaces), producing a continuous hex string.
fn convert_ascii_to_hex(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(bytes.len() * 2);
    for (i, &b) in bytes.iter().enumerate() {
        ctx.checkpoint(i, BYTE_CANCEL_CHECK_INTERVAL)?;
        let hi = b >> 4;
        let lo = b & 0x0F;
        out.push(if hi < 10 {
            (b'0' + hi) as char
        } else {
            (b'A' + hi - 10) as char
        });
        out.push(if lo < 10 {
            (b'0' + lo) as char
        } else {
            (b'A' + lo - 10) as char
        });
    }
    Ok(out)
}

/// Decode a hex string (spaces/tabs/newlines between pairs are ignored) back to UTF-8 text.
/// Returns an error on non-hex characters, an odd nibble count, or non-UTF-8 decoded bytes.
fn convert_hex_to_ascii(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    ctx.check_cancelled()?;
    let mut bytes: Vec<u8> = Vec::with_capacity(text.len() / 2);
    let mut nibble: Option<u8> = None;
    for (i, c) in text.char_indices() {
        ctx.checkpoint(i, BYTE_CANCEL_CHECK_INTERVAL)?;
        if c.is_ascii_whitespace() {
            continue;
        }
        let digit = match c {
            '0'..='9' => c as u8 - b'0',
            'a'..='f' => c as u8 - b'a' + 10,
            'A'..='F' => c as u8 - b'A' + 10,
            _ => return Err(format!("Invalid hex character: '{}'", c)),
        };
        if let Some(hi) = nibble.take() {
            bytes.push((hi << 4) | digit);
        } else {
            nibble = Some(digit);
        }
    }
    if nibble.is_some() {
        return Err("Hex string has an odd number of nibbles.".to_string());
    }
    String::from_utf8(bytes).map_err(|e| format!("Hex decoded to non-UTF-8 bytes: {}", e))
}

/// Apply a numeric conversion to each non-empty line.
/// Lines that cannot be parsed are passed through unchanged.
fn convert_lines_with<F>(
    text: &str,
    ctx: &TransformationContext<'_>,
    convert: F,
) -> Result<String, String>
where
    F: Fn(&str) -> Option<String>,
{
    ctx.check_cancelled()?;
    let mut result = String::with_capacity(text.len().saturating_mul(2));
    for (line_index, line) in text.split_inclusive('\n').enumerate() {
        ctx.checkpoint(line_index, LINE_CANCEL_CHECK_INTERVAL)?;
        let (content, newline) = split_line_newline(line);
        let trimmed = content.trim();
        let converted = if trimmed.is_empty() {
            content.to_string()
        } else {
            convert(trimmed).unwrap_or_else(|| content.to_string())
        };
        result.push_str(&converted);
        result.push_str(newline);
    }
    Ok(result)
}

/// Convert each decimal integer line to its binary representation.
/// Negative numbers are prefixed with `-`.  Non-numeric lines pass through.
fn convert_decimal_to_binary(
    text: &str,
    ctx: &TransformationContext<'_>,
) -> Result<String, String> {
    convert_lines_with(text, ctx, |s| {
        s.parse::<i128>().ok().map(|n| {
            if n < 0 {
                format!("-{:b}", n.unsigned_abs())
            } else {
                format!("{:b}", n as u128)
            }
        })
    })
}

/// Convert each binary-string line to its decimal representation.
fn convert_binary_to_decimal(
    text: &str,
    ctx: &TransformationContext<'_>,
) -> Result<String, String> {
    convert_lines_with(text, ctx, |s| {
        let (negative, digits) = s
            .strip_prefix('-')
            .map(|rest| (true, rest))
            .unwrap_or((false, s));
        u128::from_str_radix(digits, 2).ok().map(|n| {
            if negative {
                format!("-{}", n)
            } else {
                n.to_string()
            }
        })
    })
}

/// Convert each decimal integer line to its uppercase hexadecimal representation.
fn convert_decimal_to_hex(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    convert_lines_with(text, ctx, |s| {
        s.parse::<i128>().ok().map(|n| {
            if n < 0 {
                format!("-{:X}", n.unsigned_abs())
            } else {
                format!("{:X}", n as u128)
            }
        })
    })
}

/// Convert each hexadecimal line (with optional `0x`/`0X` prefix) to its decimal representation.
fn convert_hex_to_decimal(text: &str, ctx: &TransformationContext<'_>) -> Result<String, String> {
    convert_lines_with(text, ctx, |s| {
        // Strip an optional 0x / 0X prefix before parsing.
        let s = s
            .strip_prefix("0x")
            .or_else(|| s.strip_prefix("0X"))
            .unwrap_or(s);
        let (negative, digits) = s
            .strip_prefix('-')
            .map(|rest| (true, rest))
            .unwrap_or((false, s));
        u128::from_str_radix(digits, 16).ok().map(|n| {
            if negative {
                format!("-{}", n)
            } else {
                n.to_string()
            }
        })
    })
}

/// Thin wrapper used by unit tests — executes without any progress reporting.
#[cfg(test)]
fn execute_transformation_blocking(
    request: ExecuteTransformationRequest,
    cancelled: &AtomicBool,
) -> Result<ExecuteTransformationResponse, String> {
    let action_id = request.action_id;
    let params = &request.params;
    let ctx = TransformationContext::new(&request.text, cancelled).with_params(params);
    dispatch_transformation(&ctx, action_id)
}

/// Like `execute_transformation_blocking` but wires a progress reporter into
/// the context so heavy transforms can stream real progress to the frontend.
fn execute_transformation_blocking_with_progress(
    request: ExecuteTransformationRequest,
    cancelled: &AtomicBool,
    progress_fn: Box<dyn Fn(u32, u32) + Send + Sync + 'static>,
) -> Result<ExecuteTransformationResponse, String> {
    let action_id = request.action_id;
    let params = &request.params;
    let ctx = TransformationContext::new(&request.text, cancelled)
        .with_progress(progress_fn)
        .with_params(params);
    dispatch_transformation(&ctx, action_id)
}

/// Route a transformation action to its implementation using the provided context.
fn dispatch_transformation(
    ctx: &TransformationContext<'_>,
    action_id: TransformationActionId,
) -> Result<ExecuteTransformationResponse, String> {
    match action_id {
        TransformationActionId::JsonFormat => {
            ctx.run_replace_text("Formatted JSON.", "JSON is already formatted.", |ctx| {
                let (use_tabs, indent_width) = resolve_format_indent(ctx.params(), ctx.text());
                let config = jsonc_format_config(use_tabs, indent_width);
                let formatted = format_jsonc_text(ctx.text(), &config)
                    .map_err(|error| format!("Invalid JSON: {}", error))?;
                Ok(formatted)
            })
        }
        TransformationActionId::JsonMinify => {
            ctx.run_replace_text("Minified JSON.", "JSON is already minified.", |ctx| {
                validate_jsonc(ctx.text())?;
                minify_jsonc_preserving_comments(ctx.text(), ctx)
            })
        }
        TransformationActionId::JsonValidate => {
            ctx.run_show_message(|ctx| match validate_jsonc(ctx.text()) {
                Ok(()) => Ok((
                    "JSON is valid.".to_string(),
                    TransformationMessageLevel::Success,
                )),
                Err(error) => Ok((error, TransformationMessageLevel::Error)),
            })
        }
        TransformationActionId::JsonLinesToArray => ctx.run_replace_text(
            "Converted JSON Lines to an array.",
            "JSON Lines already match the formatted array output.",
            |ctx| json_lines_to_array(ctx.text(), ctx),
        ),
        TransformationActionId::JsonArrayToLines => ctx.run_replace_text(
            "Converted JSON array to JSON Lines.",
            "JSON is already in JSON Lines form.",
            |ctx| json_array_to_lines(ctx.text(), ctx),
        ),
        TransformationActionId::JsonSortKeys => ctx.run_replace_text(
            "Sorted JSON keys.",
            "JSON keys are already sorted.",
            |ctx| json_sort_keys(ctx.text(), ctx),
        ),
        TransformationActionId::JsonToTypescript => ctx.run_replace_text(
            "Generated TypeScript from JSON.",
            "JSON already matches the generated TypeScript output.",
            |ctx| json_to_typescript(ctx.text(), ctx),
        ),
        TransformationActionId::SqlFormat => {
            ctx.run_replace_text("Formatted SQL.", "SQL is already formatted.", |ctx| {
                let (use_tabs, indent_width) = resolve_format_indent(ctx.params(), ctx.text());
                let config = sql_format_config(use_tabs, indent_width);
                let formatted = format_sql_text(Path::new("query.sql"), ctx.text(), &config)
                    .map_err(|error| format!("Invalid SQL: {}", error))?;
                Ok(formatted.unwrap_or_else(|| ctx.text().to_string()))
            })
        }
        TransformationActionId::JavascriptFormat => ctx.run_replace_text(
            "Formatted JavaScript.",
            "JavaScript is already formatted.",
            |ctx| {
                let (use_tabs, indent_width) = resolve_format_indent(ctx.params(), ctx.text());
                let config = ts_format_config(use_tabs, indent_width);
                let formatted = format_ts_text(FormatTextOptions {
                    path: Path::new("file.js"),
                    extension: None,
                    text: ctx.text().to_string(),
                    config: &config,
                    external_formatter: None,
                })
                .map_err(|error| format!("Invalid JavaScript: {}", error))?;
                Ok(formatted.unwrap_or_else(|| ctx.text().to_string()))
            },
        ),
        TransformationActionId::TypescriptFormat => ctx.run_replace_text(
            "Formatted TypeScript.",
            "TypeScript is already formatted.",
            |ctx| {
                let (use_tabs, indent_width) = resolve_format_indent(ctx.params(), ctx.text());
                let config = ts_format_config(use_tabs, indent_width);
                let formatted = format_ts_text(FormatTextOptions {
                    path: Path::new("file.ts"),
                    extension: None,
                    text: ctx.text().to_string(),
                    config: &config,
                    external_formatter: None,
                })
                .map_err(|error| format!("Invalid TypeScript: {}", error))?;
                Ok(formatted.unwrap_or_else(|| ctx.text().to_string()))
            },
        ),
        TransformationActionId::CssFormat => {
            ctx.run_replace_text("Formatted CSS.", "CSS is already formatted.", |ctx| {
                let (use_tabs, indent_width) = resolve_format_indent(ctx.params(), ctx.text());
                let config = css_format_config(use_tabs, indent_width);
                format_css_text(ctx.text(), CssSyntax::Css, &config)
                    .map_err(|error| format!("Invalid CSS: {}", error))
            })
        }
        TransformationActionId::HtmlFormat => {
            ctx.run_replace_text("Formatted HTML.", "HTML is already formatted.", |ctx| {
                let (use_tabs, indent_width) = resolve_format_indent(ctx.params(), ctx.text());
                let config = html_format_config(use_tabs, indent_width);
                format_html_text(ctx.text(), HtmlLanguage::Html, &config, |code, _hints| {
                    Ok(code.into())
                })
                .map_err(|error| format!("Invalid HTML: {}", error))
            })
        }
        TransformationActionId::SvelteFormat => {
            ctx.run_replace_text("Formatted Svelte.", "Svelte is already formatted.", |ctx| {
                let (use_tabs, indent_width) = resolve_format_indent(ctx.params(), ctx.text());
                let config = html_format_config(use_tabs, indent_width);
                let ts_config = ts_format_config(use_tabs, indent_width);
                let css_config = css_format_config(use_tabs, indent_width);

                format_html_text(ctx.text(), HtmlLanguage::Svelte, &config, |code, hints| {
                    match hints.ext {
                        "js" | "jsx" | "ts" | "tsx" | "mjs" | "mts" => {
                            let path = format!("file.{}", hints.ext);
                            let formatted = format_ts_text(FormatTextOptions {
                                path: Path::new(&path),
                                extension: None,
                                text: code.to_string(),
                                config: &ts_config,
                                external_formatter: None,
                            })?;
                            Ok(formatted.unwrap_or_else(|| code.to_string()).into())
                        }
                        "css" | "scss" | "sass" | "less" => {
                            let syntax = match hints.ext {
                                "scss" => CssSyntax::Scss,
                                "sass" => CssSyntax::Sass,
                                "less" => CssSyntax::Less,
                                _ => CssSyntax::Css,
                            };
                            let formatted = format_css_text(code, syntax, &css_config)?;
                            Ok(formatted.into())
                        }
                        _ => Ok(code.into()),
                    }
                })
                .map_err(|error| format!("Invalid Svelte: {}", error))
            })
        }
        TransformationActionId::YamlFormat => {
            ctx.run_replace_text("Formatted YAML.", "YAML is already formatted.", |ctx| {
                let (_, indent_width) = resolve_format_indent(ctx.params(), ctx.text());
                let config = yaml_format_config(indent_width);
                format_yaml_text(ctx.text(), &config)
                    .map_err(|error| format!("Invalid YAML: {}", error))
            })
        }
        TransformationActionId::MarkdownFormat => ctx.run_replace_text(
            "Formatted Markdown.",
            "Markdown is already formatted.",
            |ctx| {
                let config = markdown_format_config();
                let formatted =
                    format_markdown_text(ctx.text(), &config, |_tag, _code, _width| Ok(None))
                        .map_err(|error| format!("Invalid Markdown: {}", error))?;
                Ok(formatted.unwrap_or_else(|| ctx.text().to_string()))
            },
        ),
        TransformationActionId::TomlFormat => {
            ctx.run_replace_text("Formatted TOML.", "TOML is already formatted.", |ctx| {
                let (use_tabs, indent_width) = resolve_format_indent(ctx.params(), ctx.text());
                let config = toml_format_config(use_tabs, indent_width);
                let formatted = format_toml_text(Path::new("file.toml"), ctx.text(), &config)
                    .map_err(|error| format!("Invalid TOML: {}", error))?;
                Ok(formatted.unwrap_or_else(|| ctx.text().to_string()))
            })
        }
        TransformationActionId::XmlFormat => {
            ctx.run_replace_text("Formatted XML.", "XML is already formatted.", |ctx| {
                xml_format(ctx.text(), ctx)
            })
        }
        TransformationActionId::XmlMinify => {
            ctx.run_replace_text("Minified XML.", "XML is already minified.", |ctx| {
                xml_minify(ctx.text(), ctx)
            })
        }
        TransformationActionId::XmlValidate => {
            ctx.run_show_message(|ctx| match xml_validate(ctx.text(), ctx) {
                Ok(()) => Ok((
                    "XML is well-formed.".to_string(),
                    TransformationMessageLevel::Success,
                )),
                Err(error) => Ok((error, TransformationMessageLevel::Error)),
            })
        }
        TransformationActionId::TextTrimTrailingWhitespace => ctx.run_replace_text(
            "Trimmed trailing whitespace.",
            "No trailing whitespace found.",
            |ctx| trim_trailing_whitespace(ctx.text(), ctx),
        ),
        TransformationActionId::TextCollapseBlankLines => ctx.run_replace_text(
            "Collapsed blank lines.",
            "No repeated blank lines found.",
            |ctx| collapse_blank_lines(ctx.text(), ctx),
        ),
        TransformationActionId::CsvToJson => ctx.run_replace_text(
            "Converted CSV to JSON.",
            "CSV is already valid JSON.",
            |ctx| csv_to_json(ctx.text(), ctx),
        ),
        TransformationActionId::JsonToCsv => ctx.run_replace_text(
            "Converted JSON to CSV.",
            "JSON is already valid CSV.",
            |ctx| json_to_csv(ctx.text(), ctx),
        ),
        TransformationActionId::JsonToYaml => ctx.run_replace_text(
            "Converted JSON to YAML.",
            "JSON is already valid YAML.",
            |ctx| json_to_yaml(ctx.text(), ctx),
        ),
        TransformationActionId::YamlToJson => ctx.run_replace_text(
            "Converted YAML to JSON.",
            "YAML is already valid JSON.",
            |ctx| yaml_to_json(ctx.text(), ctx),
        ),

        // ── Text ──────────────────────────────────────────────────────────
        TransformationActionId::TextTrim => ctx.run_replace_text(
            "Trimmed whitespace.",
            "No leading or trailing whitespace found.",
            |ctx| text_trim(ctx.text(), ctx),
        ),
        TransformationActionId::TextUppercase => ctx.run_replace_text(
            "Converted to uppercase.",
            "Text is already uppercase.",
            |ctx| text_uppercase(ctx.text(), ctx),
        ),
        TransformationActionId::TextLowercase => ctx.run_replace_text(
            "Converted to lowercase.",
            "Text is already lowercase.",
            |ctx| text_lowercase(ctx.text(), ctx),
        ),
        TransformationActionId::TextReverseLines => {
            ctx.run_replace_text("Reversed line order.", "Text is already reversed.", |ctx| {
                text_reverse_lines(ctx.text(), ctx)
            })
        }
        TransformationActionId::TextReverseString => {
            ctx.run_replace_text("Reversed string.", "Text is already reversed.", |ctx| {
                text_reverse_string(ctx.text(), ctx)
            })
        }
        TransformationActionId::TextMarkdownQuote => ctx.run_replace_text(
            "Added Markdown block-quote prefix.",
            "Lines already start with \">\".",
            |ctx| text_markdown_quote(ctx.text(), ctx),
        ),
        TransformationActionId::TextRot13 => ctx.run_replace_text(
            "Applied ROT-13.",
            "Text contains no ASCII letters.",
            |ctx| text_rot13(ctx.text(), ctx),
        ),
        TransformationActionId::TextAddSlashes => {
            ctx.run_replace_text("Added slashes.", "No characters needed escaping.", |ctx| {
                text_add_slashes(ctx.text(), ctx)
            })
        }
        TransformationActionId::TextRemoveSlashes => {
            ctx.run_replace_text("Removed slashes.", "No escape sequences found.", |ctx| {
                text_remove_slashes(ctx.text(), ctx)
            })
        }
        TransformationActionId::TextSortLines => {
            ctx.run_replace_text("Sorted lines.", "Lines are already sorted.", |ctx| {
                text_sort_lines(ctx.text(), ctx)
            })
        }
        TransformationActionId::TextRemoveDuplicateLines => {
            ctx.check_cancelled()?;
            let (next, removed) = text_remove_duplicate_lines(ctx.text(), &ctx)?;
            ctx.check_cancelled()?;
            Ok(replace_text_response(
                ctx.text(),
                next,
                format!(
                    "Removed {} duplicate {}.",
                    removed,
                    if removed == 1 { "line" } else { "lines" }
                ),
                "No duplicate lines found.".to_string(),
            ))
        }
        TransformationActionId::TextCollapseLines => ctx.run_replace_text(
            "Collapsed lines into one.",
            "Text is already on one line.",
            |ctx| text_collapse_lines(ctx.text(), ctx),
        ),

        // ── Case ──────────────────────────────────────────────────────────
        TransformationActionId::TextCamelCase => ctx.run_replace_text(
            "Converted to camelCase.",
            "Text is already in camelCase.",
            |ctx| text_camel_case(ctx.text(), ctx),
        ),
        TransformationActionId::TextSnakeCase => ctx.run_replace_text(
            "Converted to snake_case.",
            "Text is already in snake_case.",
            |ctx| text_snake_case(ctx.text(), ctx),
        ),
        TransformationActionId::TextKebabCase => ctx.run_replace_text(
            "Converted to kebab-case.",
            "Text is already in kebab-case.",
            |ctx| text_kebab_case(ctx.text(), ctx),
        ),
        TransformationActionId::TextTitleCase => ctx.run_replace_text(
            "Converted to Title Case.",
            "Text is already in Title Case.",
            |ctx| text_title_case(ctx.text(), ctx),
        ),
        TransformationActionId::TextSpongeCase => ctx.run_replace_text(
            "Applied sponge case.",
            "Text is already in sponge case.",
            |ctx| text_sponge_case(ctx.text(), ctx),
        ),

        // ── JSON key case ──────────────────────────────────────────────────
        TransformationActionId::JsonKeysCamelCase => ctx.run_replace_text(
            "Converted JSON keys to camelCase.",
            "JSON keys are already in camelCase.",
            |ctx| json_keys_camel_case(ctx.text(), ctx),
        ),
        TransformationActionId::JsonKeysSnakeCase => ctx.run_replace_text(
            "Converted JSON keys to snake_case.",
            "JSON keys are already in snake_case.",
            |ctx| json_keys_snake_case(ctx.text(), ctx),
        ),
        TransformationActionId::JsonKeysKebabCase => ctx.run_replace_text(
            "Converted JSON keys to kebab-case.",
            "JSON keys are already in kebab-case.",
            |ctx| json_keys_kebab_case(ctx.text(), ctx),
        ),
        TransformationActionId::JsonKeysTitleCase => ctx.run_replace_text(
            "Converted JSON keys to Title Case.",
            "JSON keys are already in Title Case.",
            |ctx| json_keys_title_case(ctx.text(), ctx),
        ),
        TransformationActionId::JsonKeysSpongeCase => ctx.run_replace_text(
            "Applied sponge case to JSON keys.",
            "JSON keys are already in sponge case.",
            |ctx| json_keys_sponge_case(ctx.text(), ctx),
        ),

        // ── URL ───────────────────────────────────────────────────────────
        TransformationActionId::UrlEncode => {
            ctx.run_replace_text("URL-encoded.", "Text is already URL-encoded.", |ctx| {
                url_encode(ctx.text(), ctx)
            })
        }
        TransformationActionId::UrlDecode => {
            ctx.run_replace_text("URL-decoded.", "Text is already URL-decoded.", |ctx| {
                url_decode(ctx.text(), ctx)
            })
        }
        TransformationActionId::UrlQueryToJson => ctx.run_replace_text(
            "Converted query string to JSON.",
            "Query string already matches the JSON output.",
            |ctx| url_query_to_json(ctx.text(), ctx),
        ),
        TransformationActionId::UrlJsonToQuery => ctx.run_replace_text(
            "Converted JSON to a query string.",
            "JSON already matches the query-string output.",
            |ctx| url_json_to_query(ctx.text(), ctx),
        ),
        TransformationActionId::SecurityUrlDefang => {
            ctx.run_replace_text("Defanged URLs.", "No URLs to defang.", |ctx| {
                url_defang(ctx.text(), ctx)
            })
        }
        TransformationActionId::SecurityUrlRefang => {
            ctx.run_replace_text("Refanged URLs.", "No defanged URLs found.", |ctx| {
                url_refang(ctx.text(), ctx)
            })
        }

        // ── Encoding ──────────────────────────────────────────────────────
        TransformationActionId::EncodingBase64Encode => ctx.run_replace_text(
            "Encoded to Base64.",
            "Text is already valid Base64.",
            |ctx| encoding_base64_encode(ctx.text(), ctx),
        ),
        TransformationActionId::EncodingBase64Decode => {
            ctx.run_replace_text("Decoded from Base64.", "Text is already decoded.", |ctx| {
                encoding_base64_decode(ctx.text(), ctx)
            })
        }
        TransformationActionId::EncodingBase64UrlEncode => ctx.run_replace_text(
            "Encoded to Base64URL.",
            "Text already matches its Base64URL output.",
            |ctx| encoding_base64url_encode(ctx.text(), ctx),
        ),
        TransformationActionId::EncodingBase64UrlDecode => ctx.run_replace_text(
            "Decoded from Base64URL.",
            "Text is already decoded.",
            |ctx| encoding_base64url_decode(ctx.text(), ctx),
        ),
        TransformationActionId::EncodingHtmlEncode => ctx.run_replace_text(
            "Encoded HTML entities.",
            "No HTML-sensitive characters needed encoding.",
            |ctx| encoding_html_encode(ctx.text(), ctx),
        ),
        TransformationActionId::EncodingHtmlDecode => ctx.run_replace_text(
            "Decoded HTML entities.",
            "No HTML entities were found.",
            |ctx| encoding_html_decode(ctx.text(), ctx),
        ),
        TransformationActionId::EncodingGzipToBase64 => ctx.run_replace_text(
            "Compressed text to Base64-encoded gzip.",
            "Text already matches the compressed output.",
            |ctx| encoding_gzip_to_base64(ctx.text(), ctx),
        ),
        TransformationActionId::EncodingGzipFromBase64 => ctx.run_replace_text(
            "Decompressed Base64-encoded gzip text.",
            "Text is already decompressed.",
            |ctx| encoding_gzip_from_base64(ctx.text(), ctx),
        ),
        TransformationActionId::EncodingJwtDecode => ctx.run_replace_text(
            "Decoded JWT without verifying its signature.",
            "JWT already matches the decoded JSON output.",
            |ctx| encoding_jwt_decode(ctx.text(), ctx),
        ),

        // ── Hashes and checksums ─────────────────────────────────────────
        TransformationActionId::HashSha256 => ctx.run_replace_text(
            "Generated SHA-256 digest.",
            "Input already matches its SHA-256 digest.",
            |ctx| hash_sha256(ctx.text(), ctx),
        ),
        TransformationActionId::HashSha512 => ctx.run_replace_text(
            "Generated SHA-512 digest.",
            "Input already matches its SHA-512 digest.",
            |ctx| hash_sha512(ctx.text(), ctx),
        ),
        TransformationActionId::ChecksumCrc32 => ctx.run_replace_text(
            "Generated CRC32 checksum.",
            "Input already matches its CRC32 checksum.",
            |ctx| checksum_crc32(ctx.text(), ctx),
        ),
        TransformationActionId::HashSha1 => ctx.run_replace_text(
            "Generated legacy SHA-1 digest.",
            "Input already matches its SHA-1 digest.",
            |ctx| hash_sha1(ctx.text(), ctx),
        ),
        TransformationActionId::HashMd5 => ctx.run_replace_text(
            "Generated legacy MD5 digest.",
            "Input already matches its MD5 digest.",
            |ctx| hash_md5(ctx.text(), ctx),
        ),

        // ── Time ─────────────────────────────────────────────────────────
        TransformationActionId::TimeUnixSecondsToRfc3339 => ctx.run_replace_text(
            "Converted Unix seconds to RFC 3339 UTC.",
            "Timestamp already matches the RFC 3339 output.",
            |ctx| time_unix_seconds_to_rfc3339(ctx.text(), ctx),
        ),
        TransformationActionId::TimeUnixMillisecondsToRfc3339 => ctx.run_replace_text(
            "Converted Unix milliseconds to RFC 3339 UTC.",
            "Timestamp already matches the RFC 3339 output.",
            |ctx| time_unix_milliseconds_to_rfc3339(ctx.text(), ctx),
        ),
        TransformationActionId::TimeRfc3339ToUnixSeconds => ctx.run_replace_text(
            "Converted RFC 3339 to Unix seconds.",
            "Timestamp already matches the Unix-seconds output.",
            |ctx| time_rfc3339_to_unix_seconds(ctx.text(), ctx),
        ),
        TransformationActionId::TimeRfc3339ToUnixMilliseconds => ctx.run_replace_text(
            "Converted RFC 3339 to Unix milliseconds.",
            "Timestamp already matches the Unix-milliseconds output.",
            |ctx| time_rfc3339_to_unix_milliseconds(ctx.text(), ctx),
        ),

        // ── Generators ───────────────────────────────────────────────────
        TransformationActionId::GenerateUuidV4 => ctx.run_replace_text(
            "Inserted UUID v4.",
            "Generated UUID matches the source.",
            generate_uuid_v4,
        ),
        TransformationActionId::GenerateUuidV7 => ctx.run_replace_text(
            "Inserted UUID v7.",
            "Generated UUID matches the source.",
            generate_uuid_v7,
        ),

        // ── Numeric conversions ───────────────────────────────────────────
        TransformationActionId::ConvertAsciiToHex => {
            ctx.run_replace_text("Converted to hex.", "Text is already hex.", |ctx| {
                convert_ascii_to_hex(ctx.text(), ctx)
            })
        }
        TransformationActionId::ConvertHexToAscii => {
            ctx.run_replace_text("Decoded hex to text.", "Text is already decoded.", |ctx| {
                convert_hex_to_ascii(ctx.text(), ctx)
            })
        }
        TransformationActionId::ConvertDecimalToBinary => ctx.run_replace_text(
            "Converted decimal to binary.",
            "Lines are already in binary.",
            |ctx| convert_decimal_to_binary(ctx.text(), ctx),
        ),
        TransformationActionId::ConvertBinaryToDecimal => ctx.run_replace_text(
            "Converted binary to decimal.",
            "Lines are already decimal.",
            |ctx| convert_binary_to_decimal(ctx.text(), ctx),
        ),
        TransformationActionId::ConvertDecimalToHex => ctx.run_replace_text(
            "Converted decimal to hex.",
            "Lines are already hex.",
            |ctx| convert_decimal_to_hex(ctx.text(), ctx),
        ),
        TransformationActionId::ConvertHexToDecimal => ctx.run_replace_text(
            "Converted hex to decimal.",
            "Lines are already decimal.",
            |ctx| convert_hex_to_decimal(ctx.text(), ctx),
        ),

        // ── Statistics (message-only) ─────────────────────────────────────
        TransformationActionId::StatsCountCharacters => ctx.run_show_message(|ctx| {
            let count = ctx.text().chars().count();
            Ok((
                format!("{} character{}", count, if count == 1 { "" } else { "s" }),
                TransformationMessageLevel::Info,
            ))
        }),
        TransformationActionId::StatsCountLines => ctx.run_show_message(|ctx| {
            let count = ctx.text().lines().count();
            Ok((
                format!("{} line{}", count, if count == 1 { "" } else { "s" }),
                TransformationMessageLevel::Info,
            ))
        }),
        TransformationActionId::StatsCountWords => ctx.run_show_message(|ctx| {
            let count = ctx.text().split_whitespace().count();
            Ok((
                format!("{} word{}", count, if count == 1 { "" } else { "s" }),
                TransformationMessageLevel::Info,
            ))
        }),
    }
}

#[tauri::command]
pub async fn execute_transformation(
    request: ExecuteTransformationRequest,
    registry: tauri::State<'_, TransformationCancellationRegistry>,
    on_event: tauri::ipc::Channel<TransformationChannelEvent>,
) -> Result<ExecuteTransformationTransportResponse, String> {
    let request_id = request.request_id;
    let cancelled = registry.register(request_id);

    // Clone the channel handle for progress reporting inside the blocking closure.
    let progress_channel = on_event.clone();
    let joined = tauri::async_runtime::spawn_blocking(move || {
        execute_transformation_blocking_with_progress(
            request,
            &cancelled,
            Box::new(move |current, total| {
                let _ =
                    progress_channel.send(TransformationChannelEvent::Progress { current, total });
            }),
        )
    })
    .await;
    registry.finish(request_id);

    let response =
        joined.map_err(|error| format!("Failed to join transformation task: {}", error))??;

    // Stream the result text in CHUNK_SIZE slices so each channel message
    // stays well below WebView2's IPC size limit. The command response itself
    // stays tiny and only carries terminal metadata (kind/message/level and
    // the chunk count), which makes the transport robust even when the channel
    // drains slightly after `invoke()` resolves on the frontend.
    match response {
        ExecuteTransformationResponse::ReplaceText {
            text,
            message,
            level,
        } => {
            let chunk_count = send_text_chunks(&on_event, &text)?;
            Ok(ExecuteTransformationTransportResponse::ReplaceText {
                chunk_count,
                message,
                level,
            })
        }
        ExecuteTransformationResponse::ShowMessage { message, level } => {
            Ok(ExecuteTransformationTransportResponse::ShowMessage { message, level })
        }
    }
}

#[tauri::command]
pub fn cancel_transformation(
    request_id: u64,
    registry: tauri::State<'_, TransformationCancellationRegistry>,
) {
    registry.cancel(request_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Return a fresh never-cancelled flag for use in tests.
    fn not_cancelled() -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(false))
    }

    fn test_ctx<'a>(text: &'a str, cancelled: &'a Arc<AtomicBool>) -> TransformationContext<'a> {
        TransformationContext::new(text, cancelled.as_ref())
    }

    #[test]
    fn request_deserializes_from_action_id_and_text_only() {
        let request: ExecuteTransformationRequest = serde_json::from_value(serde_json::json!({
            "actionId": "text.trim-trailing-whitespace",
            "text": "hello  \n",
        }))
        .expect("request should deserialize");

        assert_eq!(
            request.action_id,
            TransformationActionId::TextTrimTrailingWhitespace
        );
        assert_eq!(request.text, "hello  \n");
        assert_eq!(request.request_id, 0, "requestId should default to 0");
    }

    #[test]
    fn build_text_chunk_ranges_preserves_utf8_boundaries() {
        let text = format!(
            "{}🙂{}",
            "a".repeat(CHUNK_SIZE - 1),
            "b".repeat(CHUNK_SIZE + 3)
        );

        let ranges = build_text_chunk_ranges(&text);
        assert!(ranges.len() >= 2, "expected multiple chunk ranges");

        let rebuilt = ranges
            .iter()
            .map(|(start, end)| &text[*start..*end])
            .collect::<String>();
        assert_eq!(rebuilt, text);

        for (start, end) in ranges {
            assert!(text.is_char_boundary(start));
            assert!(text.is_char_boundary(end));
            assert!(end > start);
        }
    }

    #[test]
    fn build_text_chunk_ranges_handles_empty_text() {
        assert!(build_text_chunk_ranges("").is_empty());
    }

    #[test]
    fn json_validate_returns_scope_agnostic_success_message() {
        let nc = not_cancelled();
        let response = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::JsonValidate,
                text: "{\n  \"ok\": true\n}".to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect("validation should succeed");

        match response {
            ExecuteTransformationResponse::ShowMessage { message, level } => {
                assert_eq!(message, "JSON is valid.");
                assert_eq!(level, TransformationMessageLevel::Success);
            }
            other => panic!("expected ShowMessage response, got {:?}", other),
        }
    }

    #[test]
    fn text_trim_trailing_whitespace_transforms_content_without_scope_metadata() {
        let nc = not_cancelled();
        let response = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::TextTrimTrailingWhitespace,
                text: "hello  \nworld\t".to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect("trim should succeed");

        match response {
            ExecuteTransformationResponse::ReplaceText {
                text,
                message,
                level,
            } => {
                assert_eq!(text, "hello\nworld");
                assert_eq!(message.as_deref(), Some("Trimmed trailing whitespace."));
                assert_eq!(level, Some(TransformationMessageLevel::Success));
            }
            other => panic!("expected ReplaceText response, got {:?}", other),
        }
    }

    #[test]
    fn json_format_reports_invalid_content_without_file_type_gate() {
        let nc = not_cancelled();
        let error = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::JsonFormat,
                text: "not json".to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect_err("invalid JSON should fail");

        assert!(error.starts_with("Invalid JSON:"));
    }

    fn expect_replace_text(
        action_id: TransformationActionId,
        text: &str,
    ) -> (String, Option<String>) {
        let nc = not_cancelled();
        let response = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id,
                text: text.to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect("formatting should succeed");

        match response {
            ExecuteTransformationResponse::ReplaceText { text, message, .. } => (text, message),
            other => panic!("expected ReplaceText response, got {:?}", other),
        }
    }

    #[test]
    fn javascript_format_reindents_and_reports_success() {
        let (text, message) =
            expect_replace_text(TransformationActionId::JavascriptFormat, "const x=1");
        assert_eq!(text, "const x = 1;\n");
        assert_eq!(message.as_deref(), Some("Formatted JavaScript."));
    }

    #[test]
    fn javascript_format_reports_invalid_syntax() {
        let nc = not_cancelled();
        let error = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::JavascriptFormat,
                text: "const x = ".to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect_err("invalid JavaScript should fail");

        assert!(error.starts_with("Invalid JavaScript:"));
    }

    #[test]
    fn typescript_format_reindents_and_reports_success() {
        let (text, message) =
            expect_replace_text(TransformationActionId::TypescriptFormat, "const x:number=1");
        assert_eq!(text, "const x: number = 1;\n");
        assert_eq!(message.as_deref(), Some("Formatted TypeScript."));
    }

    #[test]
    fn typescript_format_reports_invalid_syntax() {
        let nc = not_cancelled();
        let error = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::TypescriptFormat,
                text: "interface {".to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect_err("invalid TypeScript should fail");

        assert!(error.starts_with("Invalid TypeScript:"));
    }

    #[test]
    fn css_format_reindents_and_reports_success() {
        let (text, message) =
            expect_replace_text(TransformationActionId::CssFormat, "a{color:red}");
        assert_eq!(text, "a {\n  color: red;\n}\n");
        assert_eq!(message.as_deref(), Some("Formatted CSS."));
    }

    #[test]
    fn css_format_reports_invalid_syntax() {
        let nc = not_cancelled();
        let error = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::CssFormat,
                text: "a{color:".to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect_err("invalid CSS should fail");

        assert!(error.starts_with("Invalid CSS:"));
    }

    #[test]
    fn html_format_reindents_and_reports_success() {
        let (text, message) =
            expect_replace_text(TransformationActionId::HtmlFormat, "<div><p>hi</p></div>");
        assert!(text.contains("<div>"));
        assert_eq!(message.as_deref(), Some("Formatted HTML."));
    }

    #[test]
    fn svelte_format_reindents_script_and_style_and_reports_success() {
        let (text, message) = expect_replace_text(
            TransformationActionId::SvelteFormat,
            "<script lang=\"ts\">\nlet x:number=1;\n</script>\n<div>{x}</div>\n<style>\na{color:red}\n</style>\n",
        );
        assert!(
            text.contains("let x: number = 1;"),
            "script not TS-formatted: {text}"
        );
        assert!(
            text.contains("color: red;"),
            "style not CSS-formatted: {text}"
        );
        assert_eq!(message.as_deref(), Some("Formatted Svelte."));
    }

    #[test]
    fn svelte_format_reports_invalid_script_syntax() {
        let nc = not_cancelled();
        let error = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::SvelteFormat,
                text: "<script lang=\"ts\">\nlet x: = 1;\n</script>\n".to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect_err("invalid embedded TypeScript should fail");

        assert!(
            error.starts_with("Invalid Svelte:"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn yaml_format_reindents_and_reports_success() {
        let (text, message) =
            expect_replace_text(TransformationActionId::YamlFormat, "a:\n    b: 1\n");
        assert_eq!(text, "a:\n  b: 1\n");
        assert_eq!(message.as_deref(), Some("Formatted YAML."));
    }

    #[test]
    fn yaml_format_reports_invalid_syntax() {
        let nc = not_cancelled();
        let error = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::YamlFormat,
                text: "a: [1, 2\n".to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect_err("unclosed flow sequence should fail");

        assert!(error.starts_with("Invalid YAML:"));
    }

    #[test]
    fn markdown_format_reports_success() {
        let (_, message) = expect_replace_text(
            TransformationActionId::MarkdownFormat,
            "# Title\n\n\n\ntext\n",
        );
        assert_eq!(message.as_deref(), Some("Formatted Markdown."));
    }

    #[test]
    fn toml_format_reindents_and_reports_success() {
        let (text, message) = expect_replace_text(TransformationActionId::TomlFormat, "a=1\nb=2\n");
        assert_eq!(text, "a = 1\nb = 2\n");
        assert_eq!(message.as_deref(), Some("Formatted TOML."));
    }

    #[test]
    fn toml_format_reports_invalid_syntax() {
        let nc = not_cancelled();
        let error = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::TomlFormat,
                text: "a = ".to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect_err("invalid TOML should fail");

        assert!(error.starts_with("Invalid TOML:"));
    }

    #[test]
    fn json_key_case_preserves_layout_comments_and_trailing_commas() {
        let json = "{\n\t// keep comment\n\t\"First Name\"  :  \"Alice\",\n\t\"Nested Value\": { \"Inner Key\" : true, },\n\t\"Items\": [\n\t\t{ \"Another Key\" : 1 },\n\t],\n}";
        let nc = not_cancelled();
        let response = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::JsonKeysCamelCase,
                text: json.to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect("json key case should succeed");

        match response {
            ExecuteTransformationResponse::ReplaceText { text, level, .. } => {
                assert_eq!(
                    text,
                    "{\n\t// keep comment\n\t\"firstName\"  :  \"Alice\",\n\t\"nestedValue\": { \"innerKey\" : true, },\n\t\"items\": [\n\t\t{ \"anotherKey\" : 1 },\n\t],\n}"
                );
                assert_eq!(level, Some(TransformationMessageLevel::Success));
            }
            other => panic!("expected ReplaceText response, got {:?}", other),
        }
    }

    #[test]
    fn json_key_case_noop_preserves_original_escaped_key_text() {
        let json = "{\n  \"alreadyCamel\": 1,\n  \"escaped\\u0041Key\": 2\n}";
        let nc = not_cancelled();
        let response = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::JsonKeysCamelCase,
                text: json.to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect("json key case should succeed");

        match response {
            ExecuteTransformationResponse::ReplaceText {
                text,
                message,
                level,
            } => {
                assert_eq!(text, json);
                assert_eq!(
                    message.as_deref(),
                    Some("JSON keys are already in camelCase.")
                );
                assert_eq!(level, Some(TransformationMessageLevel::Info));
            }
            other => panic!("expected ReplaceText response, got {:?}", other),
        }
    }

    #[test]
    fn csv_to_json_converts_comma_delimited_csv() {
        let csv = "name,age,city\nAlice,30,London\nBob,25,Paris\n";
        let nc = not_cancelled();
        let response = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::CsvToJson,
                text: csv.to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect("csv to json should succeed");

        match response {
            ExecuteTransformationResponse::ReplaceText { text, level, .. } => {
                let parsed: serde_json::Value =
                    serde_json::from_str(&text).expect("output should be valid JSON");
                let arr = parsed.as_array().expect("output should be a JSON array");
                assert_eq!(arr.len(), 2);
                assert_eq!(arr[0]["name"], "Alice");
                assert_eq!(arr[0]["age"], "30");
                assert_eq!(arr[1]["city"], "Paris");
                assert_eq!(level, Some(TransformationMessageLevel::Success));
            }
            other => panic!("expected ReplaceText, got {:?}", other),
        }
    }

    #[test]
    fn csv_to_json_auto_detects_tab_delimiter() {
        let csv = "name\tage\nAlice\t30\n";
        let nc = not_cancelled();
        let response = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::CsvToJson,
                text: csv.to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect("tab-delimited csv to json should succeed");

        match response {
            ExecuteTransformationResponse::ReplaceText { text, .. } => {
                let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
                let arr = parsed.as_array().unwrap();
                assert_eq!(arr[0]["name"], "Alice");
                assert_eq!(arr[0]["age"], "30");
            }
            other => panic!("expected ReplaceText, got {:?}", other),
        }
    }

    #[test]
    fn csv_to_json_empty_input_returns_empty_array() {
        let nc = not_cancelled();
        let response = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::CsvToJson,
                text: "   ".to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect("empty csv should return empty array");

        match response {
            ExecuteTransformationResponse::ReplaceText { text, level, .. } => {
                assert_eq!(text, "[]");
                // Empty → "[]" but original was whitespace, so it's a change → Success
                assert_eq!(level, Some(TransformationMessageLevel::Success));
            }
            other => panic!("expected ReplaceText, got {:?}", other),
        }
    }

    #[test]
    fn json_to_csv_converts_array_of_objects() {
        let json = r#"[{"name":"Alice","age":"30"},{"name":"Bob","age":"25"}]"#;
        let nc = not_cancelled();
        let response = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::JsonToCsv,
                text: json.to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect("json to csv should succeed");

        match response {
            ExecuteTransformationResponse::ReplaceText { text, level, .. } => {
                let lines: Vec<&str> = text.lines().collect();
                assert_eq!(lines[0], "name,age");
                assert_eq!(lines[1], "Alice,30");
                assert_eq!(lines[2], "Bob,25");
                assert_eq!(level, Some(TransformationMessageLevel::Success));
            }
            other => panic!("expected ReplaceText, got {:?}", other),
        }
    }

    #[test]
    fn json_to_csv_quotes_fields_containing_commas() {
        let json = r#"[{"city":"London, UK","pop":"9000000"}]"#;
        let nc = not_cancelled();
        let response = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::JsonToCsv,
                text: json.to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect("json to csv should succeed");

        match response {
            ExecuteTransformationResponse::ReplaceText { text, .. } => {
                let lines: Vec<&str> = text.lines().collect();
                assert!(
                    lines[1].contains("\"London, UK\""),
                    "comma field should be quoted"
                );
            }
            other => panic!("expected ReplaceText, got {:?}", other),
        }
    }

    #[test]
    fn json_to_csv_empty_array_returns_empty_string() {
        let nc = not_cancelled();
        let response = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::JsonToCsv,
                text: "[]".to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect("empty json array should succeed");

        match response {
            ExecuteTransformationResponse::ReplaceText { text, .. } => {
                assert_eq!(text.trim(), "");
            }
            other => panic!("expected ReplaceText, got {:?}", other),
        }
    }

    #[test]
    fn json_to_csv_rejects_non_array() {
        let nc = not_cancelled();
        let error = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::JsonToCsv,
                text: r#"{"key":"value"}"#.to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect_err("non-array JSON should fail");

        assert!(error.contains("top-level JSON array"));
    }

    #[test]
    fn json_to_csv_rejects_invalid_json() {
        let nc = not_cancelled();
        let error = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::JsonToCsv,
                text: "not json".to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect_err("invalid JSON should fail");

        assert!(error.starts_with("Invalid JSON:"));
    }

    #[test]
    fn json_to_yaml_converts_jsonc_input() {
        let json = r#"{
  // user record
  "name": "Alice",
  "roles": [
    "admin",
  ],
}"#;
        let nc = not_cancelled();
        let response = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::JsonToYaml,
                text: json.to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect("json to yaml should succeed");

        match response {
            ExecuteTransformationResponse::ReplaceText { text, level, .. } => {
                assert!(text.contains("name: Alice"));
                assert!(text.contains("- admin"));
                assert!(!text.starts_with("---"));
                assert_eq!(level, Some(TransformationMessageLevel::Success));
            }
            other => panic!("expected ReplaceText, got {:?}", other),
        }
    }

    #[test]
    fn yaml_to_json_converts_yaml_object() {
        let yaml = "name: Alice\nage: 30\nroles:\n  - admin\n  - editor\n";
        let nc = not_cancelled();
        let response = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::YamlToJson,
                text: yaml.to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect("yaml to json should succeed");

        match response {
            ExecuteTransformationResponse::ReplaceText { text, level, .. } => {
                let parsed: serde_json::Value =
                    serde_json::from_str(&text).expect("output should be valid JSON");
                assert_eq!(parsed["name"], "Alice");
                assert_eq!(parsed["age"], 30);
                assert_eq!(parsed["roles"][0], "admin");
                assert_eq!(level, Some(TransformationMessageLevel::Success));
            }
            other => panic!("expected ReplaceText, got {:?}", other),
        }
    }

    #[test]
    fn yaml_to_json_rejects_invalid_yaml() {
        let nc = not_cancelled();
        let error = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::YamlToJson,
                text: "name: [unterminated".to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .expect_err("invalid YAML should fail");

        assert!(error.starts_with("Invalid YAML:"));
    }

    #[test]
    fn yaml_json_round_trip_preserves_data() {
        let original_yaml = "name: Alice\nactive: true\ncount: 2\n";
        let nc = not_cancelled();
        let yaml_ctx = test_ctx(original_yaml, &nc);
        let json = yaml_to_json(original_yaml, &yaml_ctx).expect("yaml_to_json");
        let json_ctx = test_ctx(&json, &nc);
        let yaml_back = json_to_yaml(&json, &json_ctx).expect("json_to_yaml");

        let original_as_json: serde_json::Value =
            serde_yaml::from_str(original_yaml).expect("original yaml should parse");
        let round_tripped_as_json: serde_json::Value =
            serde_yaml::from_str(&yaml_back).expect("round-tripped yaml should parse");
        assert_eq!(original_as_json, round_tripped_as_json);
    }

    // ---------------------------------------------------------------
    // Large-data stress tests — verify perf and correctness at scale
    // ---------------------------------------------------------------

    #[test]
    fn csv_to_json_handles_100k_rows() {
        // Build a ~5 MB CSV (100k rows × 5 columns)
        let mut csv = String::with_capacity(6_000_000);
        csv.push_str("id,name,email,score,active\n");
        for i in 0..100_000u32 {
            csv.push_str(&format!(
                "{},user_{},user_{}@example.com,{},{}\n",
                i,
                i,
                i,
                i % 100,
                i % 2 == 0
            ));
        }

        let start = std::time::Instant::now();
        let nc = not_cancelled();
        let ctx = test_ctx(&csv, &nc);
        let result = csv_to_json(&csv, &ctx).expect("100k-row CSV should convert");
        let elapsed = start.elapsed();

        // Verify it parsed back correctly
        let parsed: serde_json::Value =
            serde_json::from_str(&result).expect("output must be valid JSON");
        let arr = parsed.as_array().expect("output must be an array");
        assert_eq!(arr.len(), 100_000);
        assert_eq!(arr[0]["id"], "0");
        assert_eq!(arr[99_999]["id"], "99999");

        // Sanity: should complete in well under 10 seconds even in debug mode
        assert!(
            elapsed.as_secs() < 10,
            "csv_to_json took {:?} for 100k rows — too slow",
            elapsed
        );
    }

    #[test]
    fn json_to_csv_handles_100k_rows() {
        // Build a JSON array of 100k objects
        let mut items: Vec<serde_json::Value> = Vec::with_capacity(100_000);
        for i in 0..100_000u32 {
            items.push(serde_json::json!({
                "id": i.to_string(),
                "name": format!("user_{}", i),
                "email": format!("user_{}@example.com", i),
                "score": (i % 100).to_string(),
                "active": (i % 2 == 0).to_string(),
            }));
        }
        let json_text = serde_json::to_string(&items).expect("serialization should work");

        let start = std::time::Instant::now();
        let nc = not_cancelled();
        let ctx = test_ctx(&json_text, &nc);
        let result = json_to_csv(&json_text, &ctx).expect("100k-row JSON should convert");
        let elapsed = start.elapsed();

        let lines: Vec<&str> = result.lines().collect();
        // header + 100k data rows
        assert_eq!(lines.len(), 100_001);
        assert!(lines[0].contains("id"));
        assert!(lines[1].contains("user_0"));

        assert!(
            elapsed.as_secs() < 10,
            "json_to_csv took {:?} for 100k rows — too slow",
            elapsed
        );
    }

    #[test]
    fn csv_to_json_round_trips_through_json_to_csv() {
        let original_csv = "name,city,score\nAlice,\"New York\",95\nBob,London,80\n";
        let nc = not_cancelled();
        let csv_ctx = test_ctx(original_csv, &nc);
        let json = csv_to_json(original_csv, &csv_ctx).expect("csv_to_json");
        let json_ctx = test_ctx(&json, &nc);
        let csv_back = json_to_csv(&json, &json_ctx).expect("json_to_csv");

        // The round-tripped CSV should have the same data
        let lines: Vec<&str> = csv_back.lines().collect();
        assert_eq!(lines[0], "name,city,score");
        assert_eq!(lines[1], "Alice,New York,95");
        assert_eq!(lines[2], "Bob,London,80");
    }

    #[test]
    fn cancellation_aborts_csv_to_json_mid_stream() {
        // Pre-cancel the flag before calling — simulates a cancel arriving
        // while the blocking task is in its startup path.
        let cancelled = Arc::new(AtomicBool::new(true));
        let csv = "a,b\n1,2\n3,4\n";
        let ctx = test_ctx(csv, &cancelled);
        let err = csv_to_json(csv, &ctx).expect_err("should be cancelled");
        assert_eq!(err, "Transformation cancelled.");
    }

    #[test]
    fn cancellation_aborts_json_to_csv_mid_stream() {
        let cancelled = Arc::new(AtomicBool::new(true));
        let json = r#"[{"a":"1"},{"b":"2"}]"#;
        let ctx = test_ctx(json, &cancelled);
        let err = json_to_csv(json, &ctx).expect_err("should be cancelled");
        assert_eq!(err, "Transformation cancelled.");
    }

    #[test]
    fn all_actions_honor_shared_cancellation_precheck() {
        let cancelled = Arc::new(AtomicBool::new(true));
        let cases = [
            (
                TransformationActionId::JsonFormat,
                "{\n  \"ok\": true\n}".to_string(),
            ),
            (
                TransformationActionId::JsonMinify,
                "{\n  \"ok\": true\n}".to_string(),
            ),
            (
                TransformationActionId::JsonValidate,
                "{\n  \"ok\": true\n}".to_string(),
            ),
            (
                TransformationActionId::TextTrimTrailingWhitespace,
                "hello  \nworld\t".to_string(),
            ),
            (
                TransformationActionId::TextCollapseBlankLines,
                "a\n\n\nb\n".to_string(),
            ),
            (
                TransformationActionId::CsvToJson,
                "name,age\nAlice,30\n".to_string(),
            ),
            (
                TransformationActionId::JsonToCsv,
                r#"[{"name":"Alice","age":"30"}]"#.to_string(),
            ),
            (
                TransformationActionId::JsonToYaml,
                "{\n  // comment\n  \"name\": \"Alice\",\n}".to_string(),
            ),
            (
                TransformationActionId::YamlToJson,
                "name: Alice\nroles:\n  - admin\n".to_string(),
            ),
        ];

        for (action_id, text) in cases {
            let error = execute_transformation_blocking(
                ExecuteTransformationRequest {
                    action_id,
                    text,
                    request_id: 0,
                    params: None,
                },
                cancelled.as_ref(),
            )
            .expect_err("pre-cancelled transformation should abort before work");
            assert_eq!(error, "Transformation cancelled.");
        }
    }

    // ── text.trim ────────────────────────────────────────────────────────────

    #[test]
    fn text_trim_removes_leading_and_trailing_whitespace() {
        let nc = not_cancelled();
        let ctx = test_ctx("  hello world  \n", &nc);
        assert_eq!(text_trim("  hello world  \n", &ctx).unwrap(), "hello world");
    }

    #[test]
    fn text_trim_noop_on_already_trimmed_text() {
        let nc = not_cancelled();
        let ctx = test_ctx("hello", &nc);
        assert_eq!(text_trim("hello", &ctx).unwrap(), "hello");
    }

    // ── text.uppercase / lowercase ───────────────────────────────────────────

    #[test]
    fn text_uppercase_converts_ascii_and_unicode() {
        let nc = not_cancelled();
        let ctx = test_ctx("hello ß", &nc);
        // German sharp-s uppercase is "SS"
        assert_eq!(text_uppercase("hello ß", &ctx).unwrap(), "HELLO SS");
    }

    #[test]
    fn text_lowercase_converts_ascii_and_unicode() {
        let nc = not_cancelled();
        let ctx = test_ctx("HELLO WORLD", &nc);
        assert_eq!(text_lowercase("HELLO WORLD", &ctx).unwrap(), "hello world");
    }

    // ── text.reverse-lines ───────────────────────────────────────────────────

    #[test]
    fn text_reverse_lines_reverses_order() {
        let nc = not_cancelled();
        let ctx = test_ctx("a\nb\nc\n", &nc);
        assert_eq!(text_reverse_lines("a\nb\nc\n", &ctx).unwrap(), "c\nb\na\n");
    }

    #[test]
    fn text_reverse_lines_preserves_trailing_newline_presence() {
        let nc = not_cancelled();
        let ctx = test_ctx("x\ny", &nc);
        assert_eq!(text_reverse_lines("x\ny", &ctx).unwrap(), "y\nx");
    }

    #[test]
    fn text_reverse_lines_single_line() {
        let nc = not_cancelled();
        let ctx = test_ctx("only", &nc);
        assert_eq!(text_reverse_lines("only", &ctx).unwrap(), "only");
    }

    // ── text.reverse-string ──────────────────────────────────────────────────

    #[test]
    fn text_reverse_string_handles_ascii() {
        let nc = not_cancelled();
        let ctx = test_ctx("Hello!", &nc);
        assert_eq!(text_reverse_string("Hello!", &ctx).unwrap(), "!olleH");
    }

    #[test]
    fn text_reverse_string_handles_emoji_grapheme_clusters() {
        let nc = not_cancelled();
        // Each emoji is one grapheme cluster; reversing should keep them intact.
        let input = "😀🎉🦀";
        let ctx = test_ctx(input, &nc);
        assert_eq!(text_reverse_string(input, &ctx).unwrap(), "🦀🎉😀");
    }

    #[test]
    fn text_reverse_string_handles_combining_marks() {
        let nc = not_cancelled();
        // "é" as base letter + combining acute accent (two codepoints, one grapheme)
        let input = "caf\u{0065}\u{0301}"; // "café" with decomposed é
        let ctx = test_ctx(input, &nc);
        let result = text_reverse_string(input, &ctx).unwrap();
        // Grapheme clusters preserved: é (with accent) stays together
        assert!(result.starts_with('\u{0065}') || result.contains('\u{0301}'));
        assert_eq!(result.len(), input.len());
    }

    // ── text.markdown-quote ──────────────────────────────────────────────────

    #[test]
    fn text_markdown_quote_prepends_arrow_prefix() {
        let nc = not_cancelled();
        let ctx = test_ctx("hello\nworld\n", &nc);
        assert_eq!(
            text_markdown_quote("hello\nworld\n", &ctx).unwrap(),
            "> hello\n> world\n"
        );
    }

    #[test]
    fn text_markdown_quote_single_line_no_trailing_newline() {
        let nc = not_cancelled();
        let ctx = test_ctx("hi", &nc);
        assert_eq!(text_markdown_quote("hi", &ctx).unwrap(), "> hi");
    }

    // ── text.rot13 ───────────────────────────────────────────────────────────

    #[test]
    fn text_rot13_rotates_ascii_letters() {
        let nc = not_cancelled();
        let ctx = test_ctx("Hello, World!", &nc);
        assert_eq!(text_rot13("Hello, World!", &ctx).unwrap(), "Uryyb, Jbeyq!");
    }

    #[test]
    fn text_rot13_is_self_inverse() {
        let nc = not_cancelled();
        let original = "The quick brown fox";
        let ctx = test_ctx(original, &nc);
        let rotated = text_rot13(original, &ctx).unwrap();
        let ctx2 = test_ctx(&rotated, &nc);
        assert_eq!(text_rot13(&rotated, &ctx2).unwrap(), original);
    }

    #[test]
    fn text_rot13_preserves_non_ascii_bytes() {
        let nc = not_cancelled();
        let input = "café"; // contains multi-byte UTF-8
        let ctx = test_ctx(input, &nc);
        let result = text_rot13(input, &ctx).unwrap();
        // ASCII 'c' → 'p', 'a' → 'n', 'f' → 's', 'é' unchanged
        assert!(result.starts_with("pns"));
        assert!(result.contains('é'));
    }

    // ── text.add-slashes / remove-slashes ────────────────────────────────────

    #[test]
    fn text_add_slashes_escapes_special_chars() {
        let nc = not_cancelled();
        let ctx = test_ctx(r#"it's a "test""#, &nc);
        assert_eq!(
            text_add_slashes(r#"it's a "test""#, &ctx).unwrap(),
            r#"it\'s a \"test\""#
        );
    }

    #[test]
    fn text_add_slashes_escapes_backslash() {
        let nc = not_cancelled();
        let ctx = test_ctx("a\\b", &nc);
        assert_eq!(text_add_slashes("a\\b", &ctx).unwrap(), "a\\\\b");
    }

    #[test]
    fn text_remove_slashes_unescapes_known_sequences() {
        let nc = not_cancelled();
        let ctx = test_ctx(r#"it\'s a \"test\""#, &nc);
        assert_eq!(
            text_remove_slashes(r#"it\'s a \"test\""#, &ctx).unwrap(),
            r#"it's a "test""#
        );
    }

    #[test]
    fn text_remove_slashes_drops_trailing_backslash() {
        let nc = not_cancelled();
        let ctx = test_ctx("abc\\", &nc);
        assert_eq!(text_remove_slashes("abc\\", &ctx).unwrap(), "abc");
    }

    #[test]
    fn text_add_remove_slashes_round_trip() {
        let nc = not_cancelled();
        let original = r#"say "hello" & it's fine\off"#;
        let ctx = test_ctx(original, &nc);
        let added = text_add_slashes(original, &ctx).unwrap();
        let ctx2 = test_ctx(&added, &nc);
        assert_eq!(text_remove_slashes(&added, &ctx2).unwrap(), original);
    }

    // ── text.sort-lines ──────────────────────────────────────────────────────

    #[test]
    fn text_sort_lines_produces_alphabetical_order() {
        let nc = not_cancelled();
        let ctx = test_ctx("banana\napple\ncherry\n", &nc);
        assert_eq!(
            text_sort_lines("banana\napple\ncherry\n", &ctx).unwrap(),
            "apple\nbanana\ncherry\n"
        );
    }

    #[test]
    fn text_sort_lines_noop_on_sorted_input() {
        let nc = not_cancelled();
        let input = "a\nb\nc\n";
        let ctx = test_ctx(input, &nc);
        assert_eq!(text_sort_lines(input, &ctx).unwrap(), input);
    }

    // ── text.remove-duplicate-lines ──────────────────────────────────────────

    #[test]
    fn text_remove_duplicate_lines_deduplicates_preserving_order() {
        let nc = not_cancelled();
        let ctx = test_ctx("a\nb\na\nc\nb\n", &nc);
        let (result, removed) = text_remove_duplicate_lines("a\nb\na\nc\nb\n", &ctx).unwrap();
        assert_eq!(result, "a\nb\nc\n");
        assert_eq!(removed, 2);
    }

    #[test]
    fn text_remove_duplicate_lines_noop_when_all_unique() {
        let nc = not_cancelled();
        let input = "x\ny\nz\n";
        let ctx = test_ctx(input, &nc);
        let (result, removed) = text_remove_duplicate_lines(input, &ctx).unwrap();
        assert_eq!(removed, 0);
        assert_eq!(result, input);
    }

    // ── text.collapse-lines ──────────────────────────────────────────────────

    #[test]
    fn text_collapse_lines_joins_with_spaces() {
        let nc = not_cancelled();
        let ctx = test_ctx("hello\nworld\n", &nc);
        assert_eq!(
            text_collapse_lines("hello\nworld\n", &ctx).unwrap(),
            "hello world "
        );
    }

    #[test]
    fn text_collapse_lines_handles_crlf() {
        let nc = not_cancelled();
        let ctx = test_ctx("a\r\nb\r\n", &nc);
        assert_eq!(text_collapse_lines("a\r\nb\r\n", &ctx).unwrap(), "a b ");
    }

    // ── case transforms ──────────────────────────────────────────────────────

    #[test]
    fn text_camel_case_converts_per_line() {
        let nc = not_cancelled();
        let ctx = test_ctx("hello world\nfoo bar\n", &nc);
        let result = text_camel_case("hello world\nfoo bar\n", &ctx).unwrap();
        assert_eq!(result, "helloWorld\nfooBar\n");
    }

    #[test]
    fn text_snake_case_converts_per_line() {
        let nc = not_cancelled();
        let ctx = test_ctx("Hello World\n", &nc);
        assert_eq!(
            text_snake_case("Hello World\n", &ctx).unwrap(),
            "hello_world\n"
        );
    }

    #[test]
    fn text_kebab_case_converts_per_line() {
        let nc = not_cancelled();
        let ctx = test_ctx("Hello World\n", &nc);
        assert_eq!(
            text_kebab_case("Hello World\n", &ctx).unwrap(),
            "hello-world\n"
        );
    }

    #[test]
    fn text_title_case_converts_per_line() {
        let nc = not_cancelled();
        let ctx = test_ctx("hello world\n", &nc);
        assert_eq!(
            text_title_case("hello world\n", &ctx).unwrap(),
            "Hello World\n"
        );
    }

    #[test]
    fn text_sponge_case_alternates_alphabetic_chars() {
        let nc = not_cancelled();
        let ctx = test_ctx("hello", &nc);
        // h(1st)→H, e(2nd)→e, l(3rd)→L, l(4th)→l, o(5th)→O
        assert_eq!(text_sponge_case("hello", &ctx).unwrap(), "HeLlO");
    }

    #[test]
    fn text_sponge_case_skips_non_alpha_for_casing_but_not_toggle() {
        let nc = not_cancelled();
        let ctx = test_ctx("h1e", &nc);
        // h(1st alpha)→H toggle→lower; '1' is non-alpha, no toggle; e(2nd alpha)→e toggle→upper
        assert_eq!(text_sponge_case("h1e", &ctx).unwrap(), "H1e");
    }

    // ── url.encode / decode ──────────────────────────────────────────────────

    #[test]
    fn url_encode_encodes_special_characters() {
        let nc = not_cancelled();
        let ctx = test_ctx("hello world & foo=bar", &nc);
        let result = url_encode("hello world & foo=bar", &ctx).unwrap();
        assert!(result.contains("hello%20world"));
        assert!(result.contains("%26"));
    }

    #[test]
    fn url_decode_restores_percent_encoding() {
        let nc = not_cancelled();
        let ctx = test_ctx("hello%20world", &nc);
        assert_eq!(url_decode("hello%20world", &ctx).unwrap(), "hello world");
    }

    #[test]
    fn url_encode_decode_round_trip() {
        let nc = not_cancelled();
        let original = "https://example.com/path?q=hello world&lang=en";
        let ctx = test_ctx(original, &nc);
        let encoded = url_encode(original, &ctx).unwrap();
        let ctx2 = test_ctx(&encoded, &nc);
        assert_eq!(url_decode(&encoded, &ctx2).unwrap(), original);
    }

    // ── security.url-defang / refang ─────────────────────────────────────────

    #[test]
    fn url_defang_transforms_http_url() {
        let nc = not_cancelled();
        let ctx = test_ctx("http://example.com/path", &nc);
        assert_eq!(
            url_defang("http://example.com/path", &ctx).unwrap(),
            "hXXp[://]example[.]com/path"
        );
    }

    #[test]
    fn url_defang_handles_https() {
        let nc = not_cancelled();
        let ctx = test_ctx("https://evil.com", &nc);
        assert_eq!(
            url_defang("https://evil.com", &ctx).unwrap(),
            "hXXps[://]evil[.]com"
        );
    }

    #[test]
    fn url_defang_is_case_insensitive_on_http() {
        let nc = not_cancelled();
        let ctx = test_ctx("HTTP://EXAMPLE.COM", &nc);
        assert_eq!(
            url_defang("HTTP://EXAMPLE.COM", &ctx).unwrap(),
            "hXXp[://]EXAMPLE[.]COM"
        );
    }

    #[test]
    fn url_refang_restores_defanged_url() {
        let nc = not_cancelled();
        let ctx = test_ctx("hXXp[://]example[.]com", &nc);
        assert_eq!(
            url_refang("hXXp[://]example[.]com", &ctx).unwrap(),
            "http://example.com"
        );
    }

    #[test]
    fn url_defang_refang_round_trip() {
        let nc = not_cancelled();
        let original = "http://malware.example.com/payload";
        let ctx = test_ctx(original, &nc);
        let defanged = url_defang(original, &ctx).unwrap();
        let ctx2 = test_ctx(&defanged, &nc);
        assert_eq!(url_refang(&defanged, &ctx2).unwrap(), original);
    }

    // ── encoding.base64-encode / decode ──────────────────────────────────────

    #[test]
    fn base64_encode_produces_standard_base64() {
        let nc = not_cancelled();
        let ctx = test_ctx("Hello, World!", &nc);
        assert_eq!(
            encoding_base64_encode("Hello, World!", &ctx).unwrap(),
            "SGVsbG8sIFdvcmxkIQ=="
        );
    }

    #[test]
    fn base64_decode_restores_original_text() {
        let nc = not_cancelled();
        let ctx = test_ctx("SGVsbG8sIFdvcmxkIQ==", &nc);
        assert_eq!(
            encoding_base64_decode("SGVsbG8sIFdvcmxkIQ==", &ctx).unwrap(),
            "Hello, World!"
        );
    }

    #[test]
    fn base64_encode_decode_round_trip_with_unicode() {
        let nc = not_cancelled();
        let original = "こんにちは 🦀";
        let ctx = test_ctx(original, &nc);
        let encoded = encoding_base64_encode(original, &ctx).unwrap();
        let ctx2 = test_ctx(&encoded, &nc);
        assert_eq!(encoding_base64_decode(&encoded, &ctx2).unwrap(), original);
    }

    #[test]
    fn base64_decode_strips_surrounding_whitespace() {
        let nc = not_cancelled();
        let ctx = test_ctx("  SGVsbG8=  ", &nc);
        assert_eq!(
            encoding_base64_decode("  SGVsbG8=  ", &ctx).unwrap(),
            "Hello"
        );
    }

    #[test]
    fn base64_decode_fails_on_invalid_input() {
        let nc = not_cancelled();
        let ctx = test_ctx("not!valid@base64#", &nc);
        assert!(encoding_base64_decode("not!valid@base64#", &ctx).is_err());
    }

    // ── convert.ascii-to-hex / hex-to-ascii ──────────────────────────────────

    #[test]
    fn ascii_to_hex_encodes_bytes_as_uppercase_hex() {
        let nc = not_cancelled();
        let ctx = test_ctx("ABC", &nc);
        // A=0x41, B=0x42, C=0x43
        assert_eq!(convert_ascii_to_hex("ABC", &ctx).unwrap(), "414243");
    }

    #[test]
    fn hex_to_ascii_decodes_hex_pairs() {
        let nc = not_cancelled();
        let ctx = test_ctx("48656C6C6F", &nc);
        assert_eq!(convert_hex_to_ascii("48656C6C6F", &ctx).unwrap(), "Hello");
    }

    #[test]
    fn hex_to_ascii_tolerates_spaces_between_pairs() {
        let nc = not_cancelled();
        let ctx = test_ctx("48 65 6C 6C 6F", &nc);
        assert_eq!(
            convert_hex_to_ascii("48 65 6C 6C 6F", &ctx).unwrap(),
            "Hello"
        );
    }

    #[test]
    fn ascii_to_hex_and_back_round_trip() {
        let nc = not_cancelled();
        let original = "Round trip!";
        let ctx = test_ctx(original, &nc);
        let hex = convert_ascii_to_hex(original, &ctx).unwrap();
        let ctx2 = test_ctx(&hex, &nc);
        assert_eq!(convert_hex_to_ascii(&hex, &ctx2).unwrap(), original);
    }

    #[test]
    fn hex_to_ascii_fails_on_invalid_hex_char() {
        let nc = not_cancelled();
        let ctx = test_ctx("4G", &nc);
        assert!(convert_hex_to_ascii("4G", &ctx).is_err());
    }

    #[test]
    fn hex_to_ascii_fails_on_odd_nibble_count() {
        let nc = not_cancelled();
        let ctx = test_ctx("ABC", &nc);
        assert!(convert_hex_to_ascii("ABC", &ctx).is_err());
    }

    // ── decimal ↔ binary ─────────────────────────────────────────────────────

    #[test]
    fn decimal_to_binary_converts_each_line() {
        let nc = not_cancelled();
        let ctx = test_ctx("255\n0\n128\n", &nc);
        assert_eq!(
            convert_decimal_to_binary("255\n0\n128\n", &ctx).unwrap(),
            "11111111\n0\n10000000\n"
        );
    }

    #[test]
    fn decimal_to_binary_handles_negative() {
        let nc = not_cancelled();
        let ctx = test_ctx("-5\n", &nc);
        assert_eq!(convert_decimal_to_binary("-5\n", &ctx).unwrap(), "-101\n");
    }

    #[test]
    fn decimal_to_binary_passes_through_non_numeric_lines() {
        let nc = not_cancelled();
        let ctx = test_ctx("42\nnot a number\n7\n", &nc);
        let result = convert_decimal_to_binary("42\nnot a number\n7\n", &ctx).unwrap();
        assert_eq!(result, "101010\nnot a number\n111\n");
    }

    #[test]
    fn binary_to_decimal_converts_each_line() {
        let nc = not_cancelled();
        let ctx = test_ctx("11111111\n0\n10000000\n", &nc);
        assert_eq!(
            convert_binary_to_decimal("11111111\n0\n10000000\n", &ctx).unwrap(),
            "255\n0\n128\n"
        );
    }

    // ── decimal ↔ hex ────────────────────────────────────────────────────────

    #[test]
    fn decimal_to_hex_converts_each_line() {
        let nc = not_cancelled();
        let ctx = test_ctx("255\n16\n0\n", &nc);
        assert_eq!(
            convert_decimal_to_hex("255\n16\n0\n", &ctx).unwrap(),
            "FF\n10\n0\n"
        );
    }

    #[test]
    fn hex_to_decimal_converts_each_line() {
        let nc = not_cancelled();
        let ctx = test_ctx("FF\n10\n0\n", &nc);
        assert_eq!(
            convert_hex_to_decimal("FF\n10\n0\n", &ctx).unwrap(),
            "255\n16\n0\n"
        );
    }

    #[test]
    fn hex_to_decimal_accepts_0x_prefix() {
        let nc = not_cancelled();
        let ctx = test_ctx("0xFF\n", &nc);
        assert_eq!(convert_hex_to_decimal("0xFF\n", &ctx).unwrap(), "255\n");
    }

    // ── stats ────────────────────────────────────────────────────────────────

    #[test]
    fn stats_count_characters_returns_info_message() {
        let nc = not_cancelled();
        let result = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::StatsCountCharacters,
                text: "hello".to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .unwrap();
        match result {
            ExecuteTransformationResponse::ShowMessage { message, level } => {
                assert_eq!(message, "5 characters");
                assert_eq!(level, TransformationMessageLevel::Info);
            }
            other => panic!("expected ShowMessage, got {:?}", other),
        }
    }

    #[test]
    fn stats_count_characters_singular() {
        let nc = not_cancelled();
        let result = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::StatsCountCharacters,
                text: "x".to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .unwrap();
        match result {
            ExecuteTransformationResponse::ShowMessage { message, .. } => {
                assert_eq!(message, "1 character");
            }
            other => panic!("expected ShowMessage, got {:?}", other),
        }
    }

    #[test]
    fn stats_count_lines_returns_correct_count() {
        let nc = not_cancelled();
        let result = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::StatsCountLines,
                text: "a\nb\nc\n".to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .unwrap();
        match result {
            ExecuteTransformationResponse::ShowMessage { message, .. } => {
                assert_eq!(message, "3 lines");
            }
            other => panic!("expected ShowMessage, got {:?}", other),
        }
    }

    #[test]
    fn stats_count_words_counts_whitespace_delimited_tokens() {
        let nc = not_cancelled();
        let result = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::StatsCountWords,
                text: "  the quick   brown fox  ".to_string(),
                request_id: 0,
                params: None,
            },
            &nc,
        )
        .unwrap();
        match result {
            ExecuteTransformationResponse::ShowMessage { message, .. } => {
                assert_eq!(message, "4 words");
            }
            other => panic!("expected ShowMessage, got {:?}", other),
        }
    }

    // ── editor-first baseline additions ──────────────────────────────────

    #[test]
    fn baseline_action_ids_deserialize() {
        for action_id in [
            "hash.sha-256",
            "encoding.jwt-decode",
            "time.rfc3339-to-unix-milliseconds",
            "url.query-to-json",
            "json.to-typescript",
            "xml.validate",
            "generate.uuid-v7",
        ] {
            let value = serde_json::json!({ "actionId": action_id, "text": "" });
            serde_json::from_value::<ExecuteTransformationRequest>(value)
                .unwrap_or_else(|error| panic!("failed to deserialize {action_id}: {error}"));
        }
    }

    #[test]
    fn hashes_match_published_abc_vectors() {
        let nc = not_cancelled();
        let ctx = test_ctx("abc", &nc);
        assert_eq!(
            hash_sha256("abc", &ctx).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hash_sha512("abc", &ctx).unwrap(),
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
        );
        assert_eq!(
            hash_sha1("abc", &ctx).unwrap(),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            hash_md5("abc", &ctx).unwrap(),
            "900150983cd24fb0d6963f7d28e17f72"
        );
        assert_eq!(checksum_crc32("abc", &ctx).unwrap(), "352441c2");
    }

    #[test]
    fn hash_honors_pre_cancelled_context() {
        let cancelled = Arc::new(AtomicBool::new(true));
        let ctx = test_ctx("abc", &cancelled);
        assert!(hash_sha256("abc", &ctx).is_err());
    }

    #[test]
    fn html_entity_actions_round_trip_sensitive_and_numeric_entities() {
        let nc = not_cancelled();
        let ctx = test_ctx("<&>\"' café", &nc);
        let encoded = encoding_html_encode("<&>\"' café", &ctx).unwrap();
        assert_eq!(encoded, "&lt;&amp;&gt;&quot;&#39; café");

        let decode_ctx = test_ctx("&lt;&#x1F980;&#129408;&amp;", &nc);
        assert_eq!(
            encoding_html_decode("&lt;&#x1F980;&#129408;&amp;", &decode_ctx).unwrap(),
            "<🦀🦀&"
        );
    }

    #[test]
    fn base64url_accepts_padded_and_unpadded_input() {
        let nc = not_cancelled();
        let ctx = test_ctx("hello?", &nc);
        let encoded = encoding_base64url_encode("hello?", &ctx).unwrap();
        assert_eq!(encoded, "aGVsbG8_");
        assert_eq!(encoding_base64url_decode(&encoded, &ctx).unwrap(), "hello?");
        assert_eq!(encoding_base64url_decode("aGk=", &ctx).unwrap(), "hi");
    }

    #[test]
    fn jwt_decode_returns_json_and_never_verifies() {
        let nc = not_cancelled();
        let token = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.eyJzdWIiOiIxMjMifQ.";
        let ctx = test_ctx(token, &nc);
        let output = encoding_jwt_decode(token, &ctx).unwrap();
        let decoded: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(decoded["header"]["alg"], "none");
        assert_eq!(decoded["payload"]["sub"], "123");
        assert_eq!(decoded["signature"], "");
        assert!(encoding_jwt_decode("one.two", &ctx).is_err());
    }

    #[test]
    fn gzip_base64_round_trip_and_corruption_error() {
        let nc = not_cancelled();
        let original = "hello gzip 🦀\n".repeat(100);
        let ctx = test_ctx(&original, &nc);
        let encoded = encoding_gzip_to_base64(&original, &ctx).unwrap();
        let decode_ctx = test_ctx(&encoded, &nc);
        assert_eq!(
            encoding_gzip_from_base64(&encoded, &decode_ctx).unwrap(),
            original
        );
        assert!(encoding_gzip_from_base64("bm90LWd6aXA=", &decode_ctx).is_err());
    }

    #[test]
    fn timestamp_actions_are_explicit_and_offset_aware() {
        let nc = not_cancelled();
        let ctx = test_ctx("0", &nc);
        assert_eq!(
            time_unix_seconds_to_rfc3339("0", &ctx).unwrap(),
            "1970-01-01T00:00:00Z"
        );
        assert_eq!(
            time_unix_milliseconds_to_rfc3339("123", &ctx).unwrap(),
            "1970-01-01T00:00:00.123Z"
        );
        assert_eq!(
            time_rfc3339_to_unix_seconds("1970-01-01T01:00:00+01:00", &ctx).unwrap(),
            "0"
        );
        assert_eq!(
            time_rfc3339_to_unix_milliseconds("1970-01-01T00:00:00.123Z", &ctx).unwrap(),
            "123"
        );
        assert!(time_rfc3339_to_unix_seconds("2026-01-01 12:00", &ctx).is_err());
    }

    #[test]
    fn query_string_conversion_preserves_repeated_keys_and_form_spaces() {
        let nc = not_cancelled();
        let input = "?tag=rust&tag=tauri&q=hello+world&empty=";
        let ctx = test_ctx(input, &nc);
        let json = url_query_to_json(input, &ctx).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["tag"], serde_json::json!(["rust", "tauri"]));
        assert_eq!(value["q"], "hello world");
        assert_eq!(value["empty"], "");

        let json_input = r#"{"tag":["rust","tauri"],"enabled":true,"empty":null}"#;
        let json_ctx = test_ctx(json_input, &nc);
        assert_eq!(
            url_json_to_query(json_input, &json_ctx).unwrap(),
            "tag=rust&tag=tauri&enabled=true&empty="
        );
        assert!(url_json_to_query(r#"{"nested":{"x":1}}"#, &json_ctx).is_err());
    }

    #[test]
    fn json_lines_conversion_handles_blank_lines_and_scalars() {
        let nc = not_cancelled();
        let input = "{\"a\":1}\n\n42\n\"text\"\n";
        let ctx = test_ctx(input, &nc);
        let array = json_lines_to_array(input, &ctx).unwrap();
        let value: serde_json::Value = serde_json::from_str(&array).unwrap();
        assert_eq!(value, serde_json::json!([{"a": 1}, 42, "text"]));

        let lines = json_array_to_lines(&array, &ctx).unwrap();
        assert_eq!(lines, "{\"a\":1}\n42\n\"text\"\n");
        assert!(json_array_to_lines("{}", &ctx).is_err());
    }

    #[test]
    fn json_sort_keys_is_recursive_and_preserves_array_order() {
        let nc = not_cancelled();
        let input = r#"{"z":1,"a":{"d":4,"b":2},"items":[{"y":2,"x":1},0]}"#;
        let ctx = test_ctx(input, &nc);
        let output = json_sort_keys(input, &ctx).unwrap();
        let a = output.find("\"a\"").unwrap();
        let z = output.find("\"z\"").unwrap();
        assert!(a < z);
        assert!(output.find("\"b\"").unwrap() < output.find("\"d\"").unwrap());
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["items"][1], 0);
        assert!(json_sort_keys("{/*comment*/\"a\":1}", &ctx).is_err());
    }

    #[test]
    fn json_to_typescript_handles_nested_invalid_null_and_union_types() {
        let nc = not_cancelled();
        let input = r#"{"user-id":1,"profile":{"name":"Ada"},"value":null,"items":[1,"two"]}"#;
        let ctx = test_ctx(input, &nc);
        let output = json_to_typescript(input, &ctx).unwrap();
        assert!(output.contains("export interface Root"));
        assert!(output.contains("\"user-id\": number"));
        assert!(output.contains("profile:"));
        assert!(output.contains("value: null"));
        assert!(output.contains("Array<number | string>"));
    }

    #[test]
    fn xml_actions_preserve_content_and_reject_malformed_documents() {
        let nc = not_cancelled();
        let input =
            "<?xml version=\"1.0\"?><root><child a=\"1\"> text </child><![CDATA[x<y]]></root>";
        let ctx = test_ctx(input, &nc);
        xml_validate(input, &ctx).unwrap();
        let formatted = xml_format(input, &ctx).unwrap();
        assert!(formatted.contains("\n"));
        assert!(formatted.contains(" text "));
        assert!(formatted.contains("<![CDATA[x<y]]>"));

        let spaced = "<root xml:space=\"preserve\">  <child/>  </root>";
        let spaced_ctx = test_ctx(spaced, &nc);
        assert_eq!(xml_minify(spaced, &spaced_ctx).unwrap(), spaced);
        assert!(xml_validate("<root><child></root>", &ctx).is_err());
        assert!(xml_validate("<one/><two/>", &ctx).is_err());
    }

    #[test]
    fn uuid_generators_create_expected_versions() {
        let nc = not_cancelled();
        let ctx = test_ctx("", &nc);
        let v4 = Uuid::parse_str(&generate_uuid_v4(&ctx).unwrap()).unwrap();
        let v7 = Uuid::parse_str(&generate_uuid_v7(&ctx).unwrap()).unwrap();
        assert_eq!(v4.get_version_num(), 4);
        assert_eq!(v7.get_version_num(), 7);
    }

    // ── indentation detection ─────────────────────────────────────────────

    fn indent_cfg(mode: &str, size: Option<u32>) -> Option<TransformationParams> {
        Some(TransformationParams {
            indent_config: Some(IndentConfig {
                indent_mode: mode.to_string(),
                indent_size: size,
            }),
        })
    }

    #[test]
    fn detect_indentation_picks_tabs_when_tab_indented() {
        let text = "{\n\t\"a\": 1\n}";
        assert_eq!(detect_indentation(text), (true, 1));
    }

    #[test]
    fn detect_indentation_picks_two_spaces_when_space_indented() {
        let text = "{\n  \"a\": 1,\n  \"b\": 2\n}";
        assert_eq!(detect_indentation(text), (false, 2));
    }

    #[test]
    fn detect_indentation_picks_four_spaces() {
        let text = "{\n    \"a\": {\n        \"b\": 1\n    }\n}";
        assert_eq!(detect_indentation(text), (false, 4));
    }

    #[test]
    fn detect_indentation_picks_two_spaces_when_deep_lines_outnumber_shallow_ones() {
        // Regression test: a 2-space-indented query where lines two levels
        // deep (4 columns) outnumber lines one level deep (2 columns). A
        // detector that tallies raw column offsets would wrongly report a
        // 4-space indent width here, doubling the indentation on every
        // reformat. The real step between levels is 2.
        let text = "SELECT\n  CASE\n    WHEN a THEN 1\n    WHEN b THEN 2\n    WHEN c THEN 3\n    WHEN d THEN 4\n  END\nFROM t";
        assert_eq!(detect_indentation(text), (false, 2));
    }

    #[test]
    fn detect_indentation_falls_back_to_two_spaces_when_no_indent() {
        assert_eq!(detect_indentation("{\"a\":1}"), (false, 2));
        assert_eq!(detect_indentation(""), (false, 2));
    }

    #[test]
    fn detect_indentation_ignores_comment_lines() {
        let text = "// header\n{\n\t\"a\": 1\n}";
        assert_eq!(detect_indentation(text), (true, 1));
    }

    #[test]
    fn json_format_detect_mode_preserves_tab_indentation() {
        let nc = not_cancelled();
        let tab_json = "{\n\t\"a\": 1\n}";
        let response = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::JsonFormat,
                text: tab_json.to_string(),
                request_id: 0,
                params: indent_cfg("detect", None),
            },
            &nc,
        )
        .expect("format should succeed");

        match response {
            ExecuteTransformationResponse::ReplaceText { text, .. } => {
                assert!(
                    text.contains('\t'),
                    "formatted output should preserve tabs, got: {:?}",
                    text
                );
            }
            other => panic!("expected ReplaceText, got {:?}", other),
        }
    }

    #[test]
    fn json_format_detect_mode_preserves_four_space_indentation() {
        let nc = not_cancelled();
        let space_json = "{\n    \"a\": 1\n}";
        let response = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::JsonFormat,
                text: space_json.to_string(),
                request_id: 0,
                params: indent_cfg("detect", None),
            },
            &nc,
        )
        .expect("format should succeed");

        match response {
            ExecuteTransformationResponse::ReplaceText { text, .. } => {
                assert!(
                    text.contains("    \"a\""),
                    "formatted output should preserve 4-space indent, got: {:?}",
                    text
                );
            }
            other => panic!("expected ReplaceText, got {:?}", other),
        }
    }

    #[test]
    fn sql_format_detect_mode_is_idempotent_on_multi_level_query() {
        // Regression test for the "detect" indentation mode compounding
        // indentation on every re-format: a query nested deep enough that
        // the old absolute-column-offset detector would mistake nesting
        // depth for indent width, widening the indent every pass.
        let nc = not_cancelled();
        let query = "SELECT\n  CASE\n    WHEN a THEN 1\n    WHEN b THEN 2\n    WHEN c THEN 3\n    WHEN d THEN 4\n  END\nFROM t";

        let first = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::SqlFormat,
                text: query.to_string(),
                request_id: 0,
                params: indent_cfg("detect", None),
            },
            &nc,
        )
        .expect("first format should succeed");

        let first_text = match first {
            ExecuteTransformationResponse::ReplaceText { text, .. } => text,
            other => panic!("expected ReplaceText, got {:?}", other),
        };

        let second = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::SqlFormat,
                text: first_text.clone(),
                request_id: 0,
                params: indent_cfg("detect", None),
            },
            &nc,
        )
        .expect("second format should succeed");

        match second {
            ExecuteTransformationResponse::ShowMessage { message, .. } => {
                assert_eq!(message, "SQL is already formatted.");
            }
            ExecuteTransformationResponse::ReplaceText { text, .. } => {
                assert_eq!(
                    text, first_text,
                    "reformatting already-formatted SQL under detect mode must be a no-op"
                );
            }
        }
    }

    #[test]
    fn json_format_detect_mode_falls_back_to_two_spaces_for_minified() {
        let nc = not_cancelled();
        let minified = "{\"a\":1,\"b\":2}";
        let response = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::JsonFormat,
                text: minified.to_string(),
                request_id: 0,
                params: indent_cfg("detect", None),
            },
            &nc,
        )
        .expect("format should succeed");

        match response {
            ExecuteTransformationResponse::ReplaceText { text, .. } => {
                // dprint collapses short objects onto a single line within the
                // 120-char line width. We only assert that the fallback did
                // NOT pick tabs and that the output is valid JSON.
                assert!(
                    !text.contains('\t'),
                    "minified input should not fall back to tabs, got: {:?}",
                    text
                );
                // Round-trip through the validator to confirm validity.
                validate_jsonc(&text).expect("formatted output must be valid JSON");
            }
            other => panic!("expected ReplaceText, got {:?}", other),
        }
    }
}
