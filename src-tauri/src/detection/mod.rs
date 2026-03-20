/// detection/mod.rs
///
/// Content-based language detection for Grayslate.
///
/// Fully synchronous, deterministic pipeline ported from the frontend
/// `languageDetector.ts`, enhanced with tree-sitter validation for
/// ambiguous programming language detection.
///
/// Detection cascade (ordered by priority & reliability):
/// ┌────────┬──────────────────────────────────────────────────┐
/// │ Phase 1│ File extension      (instant, deterministic)     │
/// │ Phase 2│ Shebang line        (instant, deterministic)     │
/// │ Phase 3│ Structural signals  (fast, high confidence)      │
/// │ Phase 4│ Heuristic scoring   (fast, medium confidence)    │
/// │  4a    │ Tree-sitter tiebreak (ambiguous cases only)      │
/// └────────┴──────────────────────────────────────────────────┘
///
/// All phases operate on at most MAX_DETECTION_BYTES of the document
/// to keep detection fast (<10ms) even for very large files.
pub mod extension;
pub mod heuristic;
pub mod shebang;
pub mod structural;
pub mod treesitter;

/// Max bytes analysed — keeps detection < 10 ms even for huge pastes.
const MAX_DETECTION_BYTES: usize = 50_000;

/// Languages the editor can handle. IDs outside this set fall back to "text".
pub const SUPPORTED_LANGUAGES: &[&str] = &[
    "json",
    "javascript",
    "typescript",
    "python",
    "html",
    "css",
    "yaml",
    "c",
    "cpp",
    "java",
    "go",
    "xml",
    "csv",
    "markdown",
    "shell",
    "dockerfile",
    "text",
    "svelte",
    "vue",
    "rust",
    "clojure",
    "sql",
    "php",
    "sass",
    "scss",
    "jinja",
    "angular",
    "nginx",
    "powershell",
    "ruby",
    "swift",
    "toml",
    "kotlin",
    "objectivec",
    "objectivecpp",
    "csharp",
    "scala",
    "dart",
];

/// Detect the language of a document from its content and/or filename.
///
/// Returns a language ID string (e.g. "python", "json", "rust") or `None`
/// when detection is uncertain.
///
/// # Arguments
/// * `content` — The document text to analyse (can be empty for extension-only)
/// * `filename` — Optional filename or full path (e.g. "Dockerfile", "config.yml")
pub fn detect_language(content: &str, filename: Option<&str>) -> Option<&'static str> {
    // Phase 1 — file extension / filename
    if let Some(fname) = filename {
        if let Some(result) = extension::detect_by_filename(fname) {
            return Some(result);
        }
    }

    let trimmed_check = content.trim();
    if trimmed_check.is_empty() {
        return None;
    }

    let (bounded, was_sliced) = bound_content(content);
    // Strip BOM if present
    let trimmed = bounded
        .strip_prefix('\u{FEFF}')
        .unwrap_or(&bounded)
        .trim();
    if trimmed.is_empty() {
        return None;
    }

    // Phase 2 — shebang line
    if let Some(first_line) = trimmed.lines().next() {
        if first_line.starts_with("#!") {
            if let Some(result) = shebang::detect_by_shebang(first_line) {
                return Some(result);
            }
        }
    }

    // Phase 3 — structural signals (data formats & markup)
    if let Some(result) = structural::detect_structural(trimmed, was_sliced) {
        return Some(result);
    }

    // Phase 4 — heuristic scoring (programming languages)
    if trimmed.len() >= 5 {
        if let Some(result) = heuristic::detect_by_scoring(trimmed) {
            return Some(ensure_supported(result));
        }
    }

    None
}

/// Slice content to MAX_DETECTION_BYTES for safe analysis.
fn bound_content(content: &str) -> (String, bool) {
    if content.len() <= MAX_DETECTION_BYTES {
        (content.to_string(), false)
    } else {
        // Find a safe UTF-8 boundary
        let mut end = MAX_DETECTION_BYTES;
        while end > 0 && !content.is_char_boundary(end) {
            end -= 1;
        }
        (content[..end].to_string(), true)
    }
}

fn ensure_supported(lang: &str) -> &str {
    if SUPPORTED_LANGUAGES.contains(&lang) {
        lang
    } else {
        "text"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Phase 1: Extension / Filename ────────────────────────

    #[test]
    fn detect_by_extension_json() {
        assert_eq!(detect_language("", Some("data.json")), Some("json"));
    }

    #[test]
    fn detect_by_extension_typescript() {
        assert_eq!(detect_language("", Some("app.ts")), Some("typescript"));
    }

    #[test]
    fn detect_by_filename_dockerfile() {
        assert_eq!(detect_language("", Some("Dockerfile")), Some("dockerfile"));
    }

    #[test]
    fn detect_by_filename_bashrc() {
        assert_eq!(detect_language("", Some(".bashrc")), Some("shell"));
    }

    // ── Phase 2: Shebang ─────────────────────────────────────

    #[test]
    fn detect_python_shebang() {
        assert_eq!(
            detect_language("#!/usr/bin/env python3\nimport os\n", None),
            Some("python")
        );
    }

    #[test]
    fn detect_node_shebang() {
        assert_eq!(
            detect_language("#!/usr/bin/env node\nconsole.log('hi')\n", None),
            Some("javascript")
        );
    }

    // ── Phase 3: Structural ──────────────────────────────────

    #[test]
    fn detect_json_object() {
        assert_eq!(
            detect_language(r#"{"name": "test", "version": "1.0"}"#, None),
            Some("json")
        );
    }

    #[test]
    fn detect_html_doctype() {
        assert_eq!(
            detect_language("<!DOCTYPE html>\n<html><body></body></html>", None),
            Some("html")
        );
    }

    #[test]
    fn detect_xml_pi() {
        assert_eq!(
            detect_language("<?xml version=\"1.0\"?>\n<root/>", None),
            Some("xml")
        );
    }

    #[test]
    fn detect_dockerfile() {
        let content = "FROM python:3.11\nRUN pip install flask\nCOPY . /app";
        assert_eq!(detect_language(content, None), Some("dockerfile"));
    }

    #[test]
    fn detect_csv() {
        let content = "name,age,city\nAlice,30,NYC\nBob,25,LA\nCharlie,35,Chicago";
        assert_eq!(detect_language(content, None), Some("csv"));
    }

    #[test]
    fn detect_markdown() {
        let content = "# Hello World\n\nSome text with a [link](http://example.com).\n\n## Section\n\n- Item 1\n- Item 2";
        assert_eq!(detect_language(content, None), Some("markdown"));
    }

    #[test]
    fn detect_yaml() {
        let content = "name: my-app\nversion: 1.0.0\ndependencies:\n  - flask\n  - gunicorn";
        assert_eq!(detect_language(content, None), Some("yaml"));
    }

    #[test]
    fn detect_toml() {
        let content = "[package]\nname = \"my-app\"\nversion = \"0.1.0\"\nedition = \"2021\"";
        assert_eq!(detect_language(content, None), Some("toml"));
    }

    // ── Phase 4: Heuristic ───────────────────────────────────

    #[test]
    fn detect_python_content() {
        let content = r#"
import os

class MyApp:
    def __init__(self):
        self.name = "test"

    def run(self):
        print("running")
"#;
        assert_eq!(detect_language(content, None), Some("python"));
    }

    #[test]
    fn detect_rust_content() {
        let content = r#"
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Config {
    pub name: String,
}

pub fn process(config: &Config) -> Result<(), String> {
    println!("Processing: {}", config.name);
    Ok(())
}
"#;
        assert_eq!(detect_language(content, None), Some("rust"));
    }

    #[test]
    fn detect_go_content() {
        let content = r#"
package main

import "fmt"

func main() {
    result, err := compute(42)
    if err != nil {
        fmt.Println("error:", err)
    }
    fmt.Println(result)
}
"#;
        assert_eq!(detect_language(content, None), Some("go"));
    }

    #[test]
    fn detect_javascript_es_modules() {
        let content = r#"
import express from 'express';

const app = express();
app.get('/', (req, res) => {
    res.send('Hello');
});

export default app;
"#;
        assert_eq!(detect_language(content, None), Some("javascript"));
    }

    #[test]
    fn detect_typescript_types() {
        let content = r#"
interface User {
    name: string;
    age: number;
    active: boolean;
}

type Result<T> = { data: T } | { error: string };

const getUser = async (id: number): Promise<User> => {
    return { name: "Alice", age: 30, active: true };
};
"#;
        assert_eq!(detect_language(content, None), Some("typescript"));
    }

    #[test]
    fn detect_sql_content() {
        let content = r#"
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    name VARCHAR(255) NOT NULL
);

SELECT u.name, COUNT(o.id)
FROM users u
LEFT JOIN orders o ON u.id = o.user_id
GROUP BY u.name;
"#;
        assert_eq!(detect_language(content, None), Some("sql"));
    }

    // ── Edge Cases ───────────────────────────────────────────

    #[test]
    fn empty_content_and_no_filename() {
        assert_eq!(detect_language("", None), None);
    }

    #[test]
    fn whitespace_only() {
        assert_eq!(detect_language("   \n\n  \t  ", None), None);
    }

    #[test]
    fn extension_takes_priority_over_content() {
        // Even though content looks like Python, .rs extension wins
        assert_eq!(
            detect_language("def hello():\n    pass", Some("main.rs")),
            Some("rust")
        );
    }

    #[test]
    fn bom_is_stripped() {
        assert_eq!(
            detect_language("\u{FEFF}{\"key\": \"value\"}", None),
            Some("json")
        );
    }
}
