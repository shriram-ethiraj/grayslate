use dprint_plugin_jsonc::{
    configuration::Configuration as JsoncFormatConfiguration, format_text as format_jsonc_text,
};
use jsonc_parser::{parse_to_value, ParseOptions};
use serde::{Deserialize, Serialize};
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
}
