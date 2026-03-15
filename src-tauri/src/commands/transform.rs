use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use dprint_plugin_jsonc::{
    configuration::Configuration as JsoncFormatConfiguration, format_text as format_jsonc_text,
};
use heck::{AsKebabCase, AsLowerCamelCase, AsSnakeCase, AsTitleCase};
use jsonc_parser::{parse_to_serde_value, parse_to_value, ParseOptions};
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

static JSONC_PARSE_OPTIONS: LazyLock<ParseOptions> = LazyLock::new(|| ParseOptions {
    allow_comments: true,
    allow_loose_object_property_names: false,
    allow_trailing_commas: true,
    allow_missing_commas: false,
    allow_single_quoted_strings: false,
    allow_hexadecimal_numbers: false,
    allow_unary_plus_numbers: false,
});

static JSONC_FORMAT_CONFIG: LazyLock<JsoncFormatConfiguration> = LazyLock::new(|| {
    serde_json::from_value(serde_json::json!({
        "lineWidth": 120,
        "useTabs": false,
        "indentWidth": 2,
        "newLineKind": "auto",
        "commentLine.forceSpaceAfterSlashes": false,
    }))
    .expect("built-in JSON formatter configuration must be valid")
});

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
    let mut out = Vec::with_capacity(len);
    let mut i = 0;
    let mut in_string = false;
    let mut escape_next = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while i < len {
        ctx.checkpoint(i, BYTE_CANCEL_CHECK_INTERVAL)?;
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum TransformationActionId {
    #[serde(rename = "json.format")]
    JsonFormat,
    #[serde(rename = "json.minify")]
    JsonMinify,
    #[serde(rename = "json.validate")]
    JsonValidate,
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
    #[serde(rename = "security.url-defang")]
    SecurityUrlDefang,
    #[serde(rename = "security.url-refang")]
    SecurityUrlRefang,

    // ── Encoding ────────────────────────────────────────────────────────
    #[serde(rename = "encoding.base64-encode")]
    EncodingBase64Encode,
    #[serde(rename = "encoding.base64-decode")]
    EncodingBase64Decode,

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteTransformationRequest {
    pub action_id: TransformationActionId,
    pub text: String,
    /// Unique ID generated by the frontend for each invocation, used to
    /// look up the cancellation flag via [`TransformationCancellationRegistry`].
    #[serde(default)]
    pub request_id: u64,
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

/// Shared cancellation-aware context for all transformation implementations.
/// New transformations should take `&TransformationContext` and use the
/// provided helpers instead of interacting with the raw `AtomicBool` directly.
struct TransformationContext<'a> {
    original: &'a str,
    cancelled: &'a AtomicBool,
}

impl<'a> TransformationContext<'a> {
    fn new(original: &'a str, cancelled: &'a AtomicBool) -> Self {
        Self {
            original,
            cancelled,
        }
    }

    fn text(&self) -> &'a str {
        self.original
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

    // Stream JSON directly. Estimate output at ~3× input size.
    let mut out = String::with_capacity(trimmed.len().saturating_mul(3));
    out.push('[');

    let mut record_count: usize = 0;
    for result in rdr.records() {
        ctx.check_cancelled()?;

        let record = result.map_err(|e| format!("CSV parse error: {}", e))?;

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
    let estimated_size = (array.len() + 1) * headers.len() * 50;
    let mut wtr = csv::WriterBuilder::new()
        .terminator(csv::Terminator::Any(b'\n')) // LF-only, matching app CSV serialization
        .from_writer(Vec::with_capacity(estimated_size));

    wtr.write_record(&headers)
        .map_err(|e| format!("CSV write error: {}", e))?;

    // Reuse a single row buffer across all records to avoid per-row Vec allocations.
    let mut row: Vec<String> = Vec::with_capacity(headers.len());

    for item in array {
        if let serde_json::Value::Object(obj) = item {
            ctx.check_cancelled()?;

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

    let mut yaml =
        serde_yaml::to_string(&value).map_err(|error| format!("YAML serialization error: {}", error))?;
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
        out.push(if hi < 10 { (b'0' + hi) as char } else { (b'A' + hi - 10) as char });
        out.push(if lo < 10 { (b'0' + lo) as char } else { (b'A' + lo - 10) as char });
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

fn execute_transformation_blocking(
    request: ExecuteTransformationRequest,
    cancelled: &AtomicBool,
) -> Result<ExecuteTransformationResponse, String> {
    let ctx = TransformationContext::new(&request.text, cancelled);

    match request.action_id {
        TransformationActionId::JsonFormat => {
            ctx.run_replace_text("Formatted JSON.", "JSON is already formatted.", |ctx| {
                let formatted = format_jsonc_text(ctx.text(), &JSONC_FORMAT_CONFIG)
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
        TransformationActionId::TextTrim => {
            ctx.run_replace_text("Trimmed whitespace.", "No leading or trailing whitespace found.", |ctx| {
                text_trim(ctx.text(), ctx)
            })
        }
        TransformationActionId::TextUppercase => {
            ctx.run_replace_text("Converted to uppercase.", "Text is already uppercase.", |ctx| {
                text_uppercase(ctx.text(), ctx)
            })
        }
        TransformationActionId::TextLowercase => {
            ctx.run_replace_text("Converted to lowercase.", "Text is already lowercase.", |ctx| {
                text_lowercase(ctx.text(), ctx)
            })
        }
        TransformationActionId::TextReverseLines => ctx.run_replace_text(
            "Reversed line order.",
            "Text is already reversed.",
            |ctx| text_reverse_lines(ctx.text(), ctx),
        ),
        TransformationActionId::TextReverseString => ctx.run_replace_text(
            "Reversed string.",
            "Text is already reversed.",
            |ctx| text_reverse_string(ctx.text(), ctx),
        ),
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
        TransformationActionId::TextAddSlashes => ctx.run_replace_text(
            "Added slashes.",
            "No characters needed escaping.",
            |ctx| text_add_slashes(ctx.text(), ctx),
        ),
        TransformationActionId::TextRemoveSlashes => ctx.run_replace_text(
            "Removed slashes.",
            "No escape sequences found.",
            |ctx| text_remove_slashes(ctx.text(), ctx),
        ),
        TransformationActionId::TextSortLines => ctx.run_replace_text(
            "Sorted lines.",
            "Lines are already sorted.",
            |ctx| text_sort_lines(ctx.text(), ctx),
        ),
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

        // ── URL ───────────────────────────────────────────────────────────
        TransformationActionId::UrlEncode => ctx.run_replace_text(
            "URL-encoded.",
            "Text is already URL-encoded.",
            |ctx| url_encode(ctx.text(), ctx),
        ),
        TransformationActionId::UrlDecode => ctx.run_replace_text(
            "URL-decoded.",
            "Text is already URL-decoded.",
            |ctx| url_decode(ctx.text(), ctx),
        ),
        TransformationActionId::SecurityUrlDefang => ctx.run_replace_text(
            "Defanged URLs.",
            "No URLs to defang.",
            |ctx| url_defang(ctx.text(), ctx),
        ),
        TransformationActionId::SecurityUrlRefang => ctx.run_replace_text(
            "Refanged URLs.",
            "No defanged URLs found.",
            |ctx| url_refang(ctx.text(), ctx),
        ),

        // ── Encoding ──────────────────────────────────────────────────────
        TransformationActionId::EncodingBase64Encode => ctx.run_replace_text(
            "Encoded to Base64.",
            "Text is already valid Base64.",
            |ctx| encoding_base64_encode(ctx.text(), ctx),
        ),
        TransformationActionId::EncodingBase64Decode => ctx.run_replace_text(
            "Decoded from Base64.",
            "Text is already decoded.",
            |ctx| encoding_base64_decode(ctx.text(), ctx),
        ),

        // ── Numeric conversions ───────────────────────────────────────────
        TransformationActionId::ConvertAsciiToHex => ctx.run_replace_text(
            "Converted to hex.",
            "Text is already hex.",
            |ctx| convert_ascii_to_hex(ctx.text(), ctx),
        ),
        TransformationActionId::ConvertHexToAscii => ctx.run_replace_text(
            "Decoded hex to text.",
            "Text is already decoded.",
            |ctx| convert_hex_to_ascii(ctx.text(), ctx),
        ),
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
) -> Result<ExecuteTransformationResponse, String> {
    let request_id = request.request_id;
    let cancelled = registry.register(request_id);
    let joined = tauri::async_runtime::spawn_blocking(move || {
        execute_transformation_blocking(request, &cancelled)
    })
    .await;
    registry.finish(request_id);
    joined.map_err(|error| format!("Failed to join transformation task: {}", error))?
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
    fn json_validate_returns_scope_agnostic_success_message() {
        let nc = not_cancelled();
        let response = execute_transformation_blocking(
            ExecuteTransformationRequest {
                action_id: TransformationActionId::JsonValidate,
                text: "{\n  \"ok\": true\n}".to_string(),
                request_id: 0,
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
            },
            &nc,
        )
        .expect_err("invalid JSON should fail");

        assert!(error.starts_with("Invalid JSON:"));
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
        assert_eq!(text_collapse_lines("hello\nworld\n", &ctx).unwrap(), "hello world ");
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
        assert_eq!(text_snake_case("Hello World\n", &ctx).unwrap(), "hello_world\n");
    }

    #[test]
    fn text_kebab_case_converts_per_line() {
        let nc = not_cancelled();
        let ctx = test_ctx("Hello World\n", &nc);
        assert_eq!(text_kebab_case("Hello World\n", &ctx).unwrap(), "hello-world\n");
    }

    #[test]
    fn text_title_case_converts_per_line() {
        let nc = not_cancelled();
        let ctx = test_ctx("hello world\n", &nc);
        assert_eq!(text_title_case("hello world\n", &ctx).unwrap(), "Hello World\n");
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
        assert_eq!(encoding_base64_decode("  SGVsbG8=  ", &ctx).unwrap(), "Hello");
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
        assert_eq!(convert_hex_to_ascii("48 65 6C 6C 6F", &ctx).unwrap(), "Hello");
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
}
