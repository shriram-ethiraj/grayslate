use dprint_plugin_jsonc::{
    configuration::Configuration as JsoncFormatConfiguration, format_text as format_jsonc_text,
};
use jsonc_parser::{parse_to_value, ParseOptions};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

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
fn minify_jsonc_preserving_comments(text: &str) -> String {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut out = Vec::with_capacity(len);
    let mut i = 0;
    let mut in_string = false;
    let mut escape_next = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while i < len {
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
    unsafe { String::from_utf8_unchecked(out) }
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
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteTransformationRequest {
    pub action_id: TransformationActionId,
    pub text: String,
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

fn trim_trailing_whitespace(text: &str) -> String {
    let mut result = String::with_capacity(text.len());

    for line in text.split_inclusive('\n') {
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

    result
}

fn collapse_blank_lines(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut previous_blank = false;

    for line in text.split_inclusive('\n') {
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
        return String::new();
    }

    result
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

fn execute_transformation_blocking(
    request: ExecuteTransformationRequest,
) -> Result<ExecuteTransformationResponse, String> {
    match request.action_id {
        TransformationActionId::JsonFormat => {
            // format_jsonc_text validates internally; no separate pre-validation needed.
            let formatted = format_jsonc_text(&request.text, &JSONC_FORMAT_CONFIG)
                .map_err(|error| format!("Invalid JSON: {}", error))?;

            Ok(replace_text_response(
                &request.text,
                formatted,
                "Formatted JSON.".to_string(),
                "JSON is already formatted.".to_string(),
            ))
        }
        TransformationActionId::JsonMinify => {
            validate_jsonc(&request.text)?;
            let minified = minify_jsonc_preserving_comments(&request.text);

            Ok(replace_text_response(
                &request.text,
                minified,
                "Minified JSON.".to_string(),
                "JSON is already minified.".to_string(),
            ))
        }
        TransformationActionId::JsonValidate => match validate_jsonc(&request.text) {
            Ok(()) => Ok(ExecuteTransformationResponse::ShowMessage {
                message: "JSON is valid.".to_string(),
                level: TransformationMessageLevel::Success,
            }),
            Err(error) => Ok(ExecuteTransformationResponse::ShowMessage {
                message: error,
                level: TransformationMessageLevel::Error,
            }),
        },
        TransformationActionId::TextTrimTrailingWhitespace => {
            let trimmed = trim_trailing_whitespace(&request.text);

            Ok(replace_text_response(
                &request.text,
                trimmed,
                "Trimmed trailing whitespace.".to_string(),
                "No trailing whitespace found.".to_string(),
            ))
        }
        TransformationActionId::TextCollapseBlankLines => {
            let collapsed = collapse_blank_lines(&request.text);

            Ok(replace_text_response(
                &request.text,
                collapsed,
                "Collapsed blank lines.".to_string(),
                "No repeated blank lines found.".to_string(),
            ))
        }
    }
}

#[tauri::command]
pub async fn execute_transformation(
    request: ExecuteTransformationRequest,
) -> Result<ExecuteTransformationResponse, String> {
    tauri::async_runtime::spawn_blocking(move || execute_transformation_blocking(request))
        .await
        .map_err(|error| format!("Failed to join transformation task: {}", error))?
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn json_validate_returns_scope_agnostic_success_message() {
        let response = execute_transformation_blocking(ExecuteTransformationRequest {
            action_id: TransformationActionId::JsonValidate,
            text: "{\n  \"ok\": true\n}".to_string(),
        })
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
        let response = execute_transformation_blocking(ExecuteTransformationRequest {
            action_id: TransformationActionId::TextTrimTrailingWhitespace,
            text: "hello  \nworld\t".to_string(),
        })
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
        let error = execute_transformation_blocking(ExecuteTransformationRequest {
            action_id: TransformationActionId::JsonFormat,
            text: "not json".to_string(),
        })
        .expect_err("invalid JSON should fail");

        assert!(error.starts_with("Invalid JSON:"));
    }
}
