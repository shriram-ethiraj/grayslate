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
pub mod disambiguation;
pub mod extension;
pub mod family;
pub mod features;
pub mod heuristic;
pub mod languages;
pub mod scoring;
pub mod shebang;
pub mod structural;
pub mod treesitter;

/// Max bytes analysed — keeps detection < 10 ms even for huge pastes.
const MAX_DETECTION_BYTES: usize = 50_000;

/// Languages the editor can handle — auto-derived from per-language definitions.
/// IDs outside this set fall back to "text".
pub(crate) use languages::SUPPORTED_LANGUAGES;

/// Detect the language of a document from its content and/or filename.
///
/// Returns a language ID string (e.g. "python", "json", "rust") or `None`
/// when detection is uncertain.
///
/// Uses the family-first pipeline (v2) with legacy heuristic as fallback.
///
/// # Arguments
/// * `content` — The document text to analyse (can be empty for extension-only)
/// * `filename` — Optional filename or full path (e.g. "Dockerfile", "config.yml")
pub fn detect_language(content: &str, filename: Option<&str>) -> Option<&'static str> {
    detect_language_v2(content, filename)
}

/// Slice content to MAX_DETECTION_BYTES for safe analysis.
///
/// Returns a `Cow::Borrowed` slice when the content is already within the
/// limit — avoids a full heap copy for the common case of small documents.
fn bound_content(content: &str) -> (std::borrow::Cow<'_, str>, bool) {
    if content.len() <= MAX_DETECTION_BYTES {
        (std::borrow::Cow::Borrowed(content), false)
    } else {
        // Find a safe UTF-8 boundary
        let mut end = MAX_DETECTION_BYTES;
        while end > 0 && !content.is_char_boundary(end) {
            end -= 1;
        }
        (std::borrow::Cow::Owned(content[..end].to_string()), true)
    }
}

fn ensure_supported(lang: &str) -> &str {
    if SUPPORTED_LANGUAGES.contains(&lang) {
        lang
    } else {
        "text"
    }
}

/// Family-first detection pipeline (v2).
///
/// Pipeline phases:
///   Phase 0 — Deterministic anchors (extension, shebang, strong structural)
///   Phase 1 — Content family classification (prose/code/data/markup/shell/config)
///   Phase 2 — Family-gated candidate scoring (anchors + hints)
///   Phase 3 — Neighbor disambiguation (differentiators + tree-sitter)
///   Phase 4 — Confidence gate (abstain if unsure)
///
/// Falls back to the legacy pipeline (heuristic + structural) when the new
/// pipeline abstains — this allows incremental migration of languages.
pub fn detect_language_v2(content: &str, filename: Option<&str>) -> Option<&'static str> {
    // Phase 0a — file extension / filename (same as v1)
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
    let trimmed = bounded
        .strip_prefix('\u{FEFF}')
        .unwrap_or(&*bounded)
        .trim();
    if trimmed.is_empty() {
        return None;
    }

    // Phase 0b — shebang (same as v1)
    if let Some(first_line) = trimmed.lines().next() {
        if first_line.starts_with("#!") {
            if let Some(result) = shebang::detect_by_shebang(first_line) {
                return Some(result);
            }
        }
    }

    // Phase 0c — strong structural probes (near-deterministic)
    // These have very low false-positive rates: JSON, PHP, Svelte, Vue,
    // HTML, XML, Dockerfile, CSV. They fire before the family classifier.
    if let Some(result) = structural::detect_strong_structural(trimmed, was_sliced) {
        return Some(result);
    }

    // Phase 1 — content family classification
    let feats = features::extract_features(trimmed);
    let family_result = family::classify_family(&feats);

    // Phase 2 — family-gated candidate scoring
    let families: Vec<family::ContentFamily> = if family_result.is_confident() {
        // Confident: use only the top family
        family_result.top().map(|s| vec![s.family]).unwrap_or_default()
    } else {
        // Ambiguous: use top-2 families
        family_result.top_two().iter().map(|s| s.family).collect()
    };

    // Phase 2a — soft structural probes (family-gated).
    // These detectors (Markdown, YAML, SQL, TOML, SCSS, Sass, Prompt)
    // have higher false-positive rates, so they only fire when the
    // family classifier agrees. E.g., markdown won't fire on Code content.
    // When families is empty (classifier abstained), all soft detectors
    // are allowed — the classifier has no opinion to gate with.
    if let Some(result) = structural::detect_soft_structural(trimmed, was_sliced, &families) {
        return Some(result);
    }

    // Phase 2b — family-gated language scoring
    if !families.is_empty() {
        let candidates = scoring::score_candidates(trimmed, &families);

        if !candidates.is_empty() {
            // Phase 3 — neighbor disambiguation
            if let Some(winner) = disambiguation::disambiguate(trimmed, &candidates) {
                return Some(ensure_supported(winner));
            }
        }

        // If the top family is Prose and no language candidates matched,
        // the content is natural language — return None (abstain).
        if families.first() == Some(&family::ContentFamily::Prose) {
            return None;
        }
    }

    // Fallback to legacy pipeline when new pipeline abstains.
    // This ensures existing detection still works during incremental migration.
    if trimmed.len() >= 5 {
        let prose = structural::strip_code_blocks(trimmed);
        let scoring_input = if prose.len() < trimmed.len() { &prose } else { trimmed };
        let (winner, runner_up) = heuristic::detect_by_scoring_with_runner_up(scoring_input);
        if let Some(best) = winner {
            let validated = treesitter::validate_winner(scoring_input, best, runner_up);
            return Some(ensure_supported(validated));
        }
    }

    None
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

    // ── Regression Tests: False Positive Prevention ──────────

    #[test]
    fn python_from_imports_not_dockerfile() {
        // Python `from x import y` was matching Dockerfile's FROM instruction
        // because the regex was case-insensitive.
        let content = r#"from fastapi import FastAPI
from typing import Optional
from pydantic import BaseModel

app = FastAPI()

@app.get("/")
def read_root():
    return {"Hello": "World"}
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("dockerfile"), "Python imports must not be detected as Dockerfile");
        assert_eq!(result, Some("python"));
    }

    #[test]
    fn python_with_from_imports_py310() {
        // Minimal Python file that was being misdetected as Dockerfile
        let content = r#"from app.main import app
from app.models import Item

def test_create_item():
    pass
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("dockerfile"));
    }

    #[test]
    fn go_copyright_header_not_toml() {
        // Go files with copyright headers + const blocks were detected as TOML
        let content = r#"// Copyright 2014 Manu Martinez-Almeida. All rights reserved.
// Use of this source code is governed by a MIT style
// license that can be found in the LICENSE file.

package gin

import (
	"fmt"
	"net/http"
)

const (
	DebugMode   = "debug"
	ReleaseMode = "release"
	TestMode    = "test"
)

var DefaultWriter io.Writer = os.Stdout
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("toml"), "Go code must not be detected as TOML");
        assert_eq!(result, Some("go"));
    }

    #[test]
    fn tsx_displayname_not_toml() {
        // TSX component files with displayName assignments were detected as TOML
        let content = r#"import * as React from "react"
import { cn } from "@/lib/utils"

const Button = React.forwardRef<
  HTMLButtonElement,
  React.ButtonHTMLAttributes<HTMLButtonElement>
>(({ className, ...props }, ref) => (
  <button ref={ref} className={cn("btn", className)} {...props} />
))
Button.displayName = "Button"

export { Button }
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("toml"), "TSX must not be detected as TOML");
    }

    #[test]
    fn markdown_with_code_blocks_still_markdown() {
        // Markdown docs with embedded code were being rejected by code anti-signals
        let content = r#"# FastAPI Features

Here's how to use it:

```python
from fastapi import FastAPI

app = FastAPI()

@app.get("/")
def read_root():
    return {"Hello": "World"}
```

## Another Section

You can also use:

```typescript
interface User {
    name: string;
    age: number;
}
```

- Feature 1
- Feature 2
"#;
        assert_eq!(
            detect_language(content, None),
            Some("markdown"),
            "Markdown with fenced code blocks should still be detected as markdown"
        );
    }

    #[test]
    fn real_dockerfile_still_detected() {
        // After making Dockerfile case-sensitive, real Dockerfiles must still work
        let content = "FROM python:3.11-slim\nWORKDIR /app\nCOPY requirements.txt .\nRUN pip install -r requirements.txt\nCOPY . .\nCMD [\"python\", \"main.py\"]";
        assert_eq!(detect_language(content, None), Some("dockerfile"));
    }

    #[test]
    fn dockerfile_with_arg() {
        let content = "ARG PYTHON_VERSION=3.11\nFROM python:${PYTHON_VERSION}-slim\nRUN pip install flask\nCOPY . /app";
        assert_eq!(detect_language(content, None), Some("dockerfile"));
    }

    #[test]
    fn dockerfile_multistage() {
        let content = "FROM node:18 AS builder\nWORKDIR /app\nCOPY package.json .\nRUN npm install\nFROM node:18-slim\nCOPY --from=builder /app /app\nCMD [\"node\", \"index.js\"]";
        assert_eq!(detect_language(content, None), Some("dockerfile"));
    }

    #[test]
    fn sql_ddl_detected() {
        let content = r#"-- Users table
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    created_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_users_email ON users(email);
"#;
        assert_eq!(detect_language(content, None), Some("sql"));
    }

    #[test]
    fn sql_not_detected_in_markdown() {
        // SQL keywords inside markdown code blocks should not trigger SQL detection
        let content = r#"# Database Guide

Use this query:

```sql
SELECT * FROM users WHERE active = true;
```

## More Info

- Read the docs
- Check the API reference
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("sql"), "SQL inside markdown fences should not override markdown");
        assert_eq!(result, Some("markdown"));
    }

    #[test]
    fn go_sum_not_clojure() {
        // go.sum files have parentheses-heavy content that was triggering Clojure
        let content = r#"github.com/gin-gonic/gin v1.9.1 h1:4idEA...
github.com/gin-gonic/gin v1.9.1/go.mod h1:...
github.com/go-playground/assert/v2 v2.2.0 h1:...
github.com/go-playground/locales v0.14.1 h1:...
github.com/go-playground/validator/v10 v10.14.0 h1:...
golang.org/x/crypto v0.9.0 h1:...
golang.org/x/net v0.10.0 h1:...
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("clojure"), "go.sum must not be detected as Clojure");
    }

    #[test]
    fn typescript_with_type_annotations() {
        let content = r#"
import { useState } from "react"

interface Props {
    title: string
    count: number
    items: string[]
}

export function Counter({ title, count, items }: Props) {
    const [value, setValue] = useState<number>(count)
    return <div>{title}: {value}</div>
}
"#;
        assert_eq!(detect_language(content, None), Some("typescript"));
    }

    #[test]
    fn toml_cargo_still_detected() {
        let content = r#"[package]
name = "my-app"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
"#;
        assert_eq!(detect_language(content, None), Some("toml"));
    }

    #[test]
    fn yaml_with_multiline_strings() {
        let content = r#"name: CI Pipeline
on:
  push:
    branches: [main]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run tests
        run: |
          npm install
          npm test
"#;
        assert_eq!(detect_language(content, None), Some("yaml"));
    }

    #[test]
    fn tsx_select_component_not_sql() {
        // React Select component imports should not trigger SQL detection
        let content = r#""use client"

import { Label } from "@/registry/new-york-v4/ui/label"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
} from "@/registry/new-york-v4/ui/select"

const themes = [
  { name: "Default", value: "default" },
  { name: "Blue", value: "blue" },
]

export function ThemeSelector() {
  return (
    <div className="flex items-center gap-2">
      <Label htmlFor="theme">Theme</Label>
      <Select defaultValue="default">
        <SelectTrigger>
          <SelectValue placeholder="Select a theme" />
        </SelectTrigger>
      </Select>
    </div>
  )
}
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("sql"), "React Select component must not be detected as SQL");
        assert_ne!(result, Some("toml"), "TSX component must not be detected as TOML");
    }

    #[test]
    fn python_test_file_not_toml() {
        // Python test files with dict assertions were falsely detected as TOML
        let content = r#"import importlib
from unittest.mock import patch

import pytest
from fastapi.testclient import TestClient

@pytest.fixture(name="client")
def get_client(request):
    mod = importlib.import_module(f"docs_src.body.{request.param}")
    client = TestClient(mod.app)
    return client

def test_body_float(client):
    response = client.post("/items/", json={"name": "Foo", "price": 50.5})
    assert response.status_code == 200
    assert response.json() == {
        "name": "Foo",
        "price": 50.5,
        "description": None,
        "tax": None,
    }
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("toml"), "Python test file must not be detected as TOML");
        assert_eq!(result, Some("python"));
    }

    #[test]
    fn rust_compiler_error_not_clojure() {
        // .stderr Rust compiler error output contains `:keyword`-like paths
        let content = r#"x next/dynamic requires at least one argument
   ,-[input.js:3:1]
 2 |
 3 | const DynamicComponent = dynamic()
   :                          ^^^^^^^
   `----
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("clojure"), "Rust compiler errors must not be detected as Clojure");
    }

    #[test]
    fn css_file_not_sql() {
        // CSS with "text-" class names and select elements triggered SQL detection
        let content = r#".container {
  display: flex;
  flex-direction: column;
}

.text-muted-foreground {
  color: var(--muted-foreground);
}

select {
  border: 1px solid var(--border);
  border-radius: 0.5rem;
}

@media (min-width: 768px) {
  .container {
    max-width: 768px;
  }
}
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("sql"), "CSS file must not be detected as SQL");
        assert_eq!(result, Some("css"));
    }

    #[test]
    fn go_mod_not_sql() {
        // go.mod has "module" and "require" that could resemble SQL patterns
        let content = r#"module github.com/gin-gonic/gin

go 1.21

require (
	github.com/bytedance/sonic v1.9.1
	github.com/gin-contrib/sse v0.1.0
	github.com/go-playground/validator/v10 v10.14.0
	github.com/goccy/go-json v0.10.2
	github.com/mattn/go-isatty v0.0.19
	github.com/pelletier/go-toml/v2 v2.0.8
	golang.org/x/net v0.10.0
	google.golang.org/protobuf v1.30.0
)
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("sql"), "go.mod must not be detected as SQL");
    }

    // ── Cross-family penalty integration tests ───────────────────────────

    #[test]
    fn cpp_tensorflow_header_integration() {
        // Full integration test: content-only detection of a TensorFlow-style
        // C++ header. Must not be TypeScript, C#, PHP, or Perl.
        let content = r#"
#ifndef TENSORFLOW_CORE_KERNELS_CONV_OPS_H_
#define TENSORFLOW_CORE_KERNELS_CONV_OPS_H_

#include "tensorflow/core/framework/op_kernel.h"
#include "tensorflow/core/framework/tensor.h"

namespace tensorflow {
namespace internal {

template<typename Device, typename T>
class Conv2DOp : public OpKernel {
 public:
  explicit Conv2DOp(OpKernelConstruction* context) : OpKernel(context) {
    OP_REQUIRES_OK(context, context->GetAttr("strides", &strides_));
  }

  void Compute(OpKernelContext* context) override {
    const Tensor& input = context->input(0);
    const Tensor& filter = context->input(1);
    OP_REQUIRES(context, input.dims() == 4,
                errors::InvalidArgument("input must be 4-dimensional"));
  }

 private:
  std::vector<int32> strides_;
  Padding padding_;
};

}  // namespace internal
}  // namespace tensorflow

#endif  // TENSORFLOW_CORE_KERNELS_CONV_OPS_H_
"#;
        let result = detect_language(content, None);
        assert!(
            result == Some("cpp") || result == Some("c"),
            "TensorFlow C++ header detected as {:?} instead of C/C++",
            result
        );
    }

    #[test]
    fn cpp_cc_file_not_csharp() {
        // C++ .cc file content should not be detected as C#.
        let content = r#"
#include "mylib/utils.h"
#include <iostream>
#include <memory>

namespace mylib {

void ProcessData(const std::string& input) {
  auto result = std::make_unique<Result>();
  result->status = Status::OK;
  if (input.empty()) {
    LOG(WARNING) << "Empty input";
    return;
  }
  for (const auto& item : Parse(input)) {
    result->items.push_back(item);
  }
}

}  // namespace mylib
"#;
        let result = detect_language(content, None);
        assert!(
            result == Some("cpp") || result == Some("c"),
            "C++ source detected as {:?} instead of C/C++",
            result
        );
        assert_ne!(result, Some("csharp"));
        assert_ne!(result, Some("typescript"));
    }

    // ── Email detection tests ────────────────────────────────────────────

    #[test]
    fn email_rfc_headers() {
        let content = "\
From: alice@example.com\n\
To: bob@example.com\n\
Subject: Q3 Budget Review\n\
Date: Mon, 15 Jan 2024 10:30:00 -0500\n\
\n\
Hi Bob,\n\
\n\
Please find the Q3 budget review attached.\n\
\n\
Best regards,\n\
Alice";
        assert_eq!(detect_language(content, None), Some("email"));
    }

    #[test]
    fn email_greeting_closing_informal() {
        // The user's original failing sample — was misdetected as "cmd"
        let content = "\
Hi team,\n\
\n\
Quick update on the search improvements:\n\
\n\
* Basic indexing is done\n\
* Filters are partially working (need to fix edge cases)\n\
* Performance is still inconsistent for large datasets\n\
\n\
I'll continue working on optimization today and share another update tomorrow.\n\
\n\
Let me know if anything urgent needs to be prioritized.\n\
\n\
Thanks,\n\
John";
        let result = detect_language(content, None);
        assert_eq!(result, Some("email"), "Informal email detected as {:?}", result);
    }

    #[test]
    fn email_greeting_without_formal_closing() {
        // "Hi X" + "Let me know" phrasing, no explicit closing line
        let content = "\
Hi team,\n\
\n\
Quick update on the search improvements:\n\
\n\
* Basic indexing is done\n\
* Filters are partially working (need to fix edge cases)\n\
* Performance is still inconsistent for large datasets\n\
\n\
I'll continue working on optimization today and share another update tomorrow.\n\
\n\
Let me know if anything urgent needs to be prioritize";
        let result = detect_language(content, None);
        assert_eq!(result, Some("email"), "Casual email detected as {:?}", result);
    }

    #[test]
    fn email_reply_thread() {
        let content = "\
On Mon, Jan 15, 2024, Alice Smith wrote:\n\
> Hi team,\n\
> Here's the latest update on the project.\n\
>\n\
> Best,\n\
> Alice\n\
\n\
Thanks for the update! I have a few comments below.\n\
\n\
> * Basic indexing is done\n\
\n\
Great progress on this.\n\
\n\
Regards,\n\
Bob";
        assert_eq!(detect_language(content, None), Some("email"));
    }

    #[test]
    fn email_formal_dear() {
        let content = "\
Dear Dr. Johnson,\n\
\n\
I am writing to follow up on our conversation regarding the research proposal.\n\
The committee has reviewed your submission and would like to schedule a meeting.\n\
\n\
Please let me know your availability for next week.\n\
\n\
Sincerely,\n\
Prof. Sarah Chen";
        assert_eq!(detect_language(content, None), Some("email"));
    }

    #[test]
    fn email_outlook_forward() {
        let content = "\
Hi Mike,\n\
\n\
FYI see the original message below.\n\
\n\
--- Original Message ---\n\
From: support@vendor.com\n\
Sent: Friday, January 12, 2024\n\
To: procurement@company.com\n\
Subject: Invoice #12345\n\
\n\
Please find attached invoice for your recent order.\n\
\n\
Thank you,\n\
Vendor Support Team";
        assert_eq!(detect_language(content, None), Some("email"));
    }

    #[test]
    fn email_not_detected_for_code() {
        let content = "from fastapi import FastAPI\nfrom typing import Optional\n\napp = FastAPI()\n\n@app.get(\"/\")\ndef read_root():\n    return {\"Hello\": \"World\"}\n";
        let result = detect_language(content, None);
        assert_ne!(result, Some("email"), "Python code must not be email");
    }

    #[test]
    fn email_not_detected_for_batch() {
        let content = "@echo off\nsetlocal\nset PROJECT=myapp\nfor /F %%i in ('dir /b *.txt') do (\n    echo Processing %%i\n)\nendlocal\n";
        let result = detect_language(content, None);
        assert_ne!(result, Some("email"), "Batch script must not be email");
    }

    // ── Prompt detection tests ───────────────────────────────────────────

    #[test]
    fn prompt_you_are_role() {
        let content = "\
You are a senior software engineer specializing in distributed systems.\n\
\n\
Review the following code and identify:\n\
1. Potential race conditions\n\
2. Memory leaks\n\
3. Error handling gaps\n\
\n\
Format your response as a numbered list with severity ratings.";
        assert_eq!(detect_language(content, None), Some("prompt"));
    }

    #[test]
    fn prompt_act_as() {
        let content = "\
Act as a technical writer. Write clear, concise API documentation for the\n\
following REST endpoints. Use markdown format with code examples.\n\
\n\
Guidelines:\n\
- Include request/response examples\n\
- Document error codes\n\
- Add authentication requirements";
        assert_eq!(detect_language(content, None), Some("prompt"));
    }

    #[test]
    fn prompt_system_user_labels() {
        let content = "\
System: You are a helpful coding assistant. Answer questions about Python.\n\
\n\
User: How do I read a CSV file in Python?\n\
\n\
Assistant: You can use the built-in csv module or pandas.";
        assert_eq!(detect_language(content, None), Some("prompt"));
    }

    #[test]
    fn prompt_template_variables() {
        let content = "\
You are a {{role}} assistant.\n\
\n\
Given the following context:\n\
{{context}}\n\
\n\
Answer the user's question:\n\
{{question}}\n\
\n\
Respond in {{format}} format.";
        assert_eq!(detect_language(content, None), Some("prompt"));
    }

    #[test]
    fn prompt_instruction_sections() {
        let content = "\
Context: You are reviewing a pull request for a web application.\n\
\n\
Instructions:\n\
1. Check for security vulnerabilities\n\
2. Verify error handling\n\
3. Review naming conventions\n\
\n\
Rules:\n\
- Do not suggest style changes\n\
- Focus on correctness\n\
- Be concise\n\
\n\
Output: Provide your review as a markdown list.";
        assert_eq!(detect_language(content, None), Some("prompt"));
    }

    #[test]
    fn prompt_chatml_delimiters() {
        let content = "\
<|system|>\n\
You are a helpful assistant that answers questions about programming.\n\
<|user|>\n\
What is the difference between TCP and UDP?\n\
<|assistant|>";
        assert_eq!(detect_language(content, None), Some("prompt"));
    }

    #[test]
    fn prompt_i_want_you_to() {
        let content = "\
I want you to act as a data scientist. Analyze the dataset I provide\n\
and generate insights. Always include statistical significance tests.\n\
\n\
Format as JSON with the following structure:\n\
- summary: brief overview\n\
- insights: list of findings\n\
- recommendations: actionable next steps";
        assert_eq!(detect_language(content, None), Some("prompt"));
    }

    #[test]
    fn prompt_few_shot_examples() {
        let content = "\
You are a sentiment classifier.\n\
\n\
Example 1:\n\
Input: I love this product!\n\
Output: positive\n\
\n\
Example 2:\n\
Input: This is terrible.\n\
Output: negative\n\
\n\
Now classify:\n\
Input: {{text}}";
        assert_eq!(detect_language(content, None), Some("prompt"));
    }

    #[test]
    fn prompt_not_detected_for_code() {
        let content = "import os\n\nclass MyApp:\n    def __init__(self):\n        self.name = \"test\"\n\n    def run(self):\n        print(\"running\")\n";
        let result = detect_language(content, None);
        assert_ne!(result, Some("prompt"), "Python code must not be prompt");
    }

    #[test]
    fn prompt_not_detected_for_email() {
        let content = "\
From: alice@example.com\n\
To: bob@example.com\n\
Subject: Meeting tomorrow\n\
\n\
Hi Bob,\n\
Can we reschedule the meeting to 3pm?\n\
\n\
Thanks,\n\
Alice";
        let result = detect_language(content, None);
        assert_ne!(result, Some("prompt"), "Email must not be prompt");
    }

    // ── CMD still works after adding prose languages ─────────────────────

    #[test]
    fn cmd_still_detected_with_prose_langs() {
        let content = "@echo off\nsetlocal\nset PROJECT=myapp\necho Building %PROJECT%\nif exist build rmdir /s /q build\nfor /F %%i in ('dir /b *.txt') do echo %%i\ngoto :eof\n:cleanup\nendlocal\n";
        assert_eq!(detect_language(content, None), Some("cmd"));
    }

    // ── Prose false-positive regression suite ────────────────────────────
    // Real user samples that were misdetected as programming languages.

    #[test]
    fn prose_multi_question_prompt_not_scala() {
        let content = "I am designing a language detection system for a code editor, currently using extension + heuristics, but failing for mixed content files (yaml with embedded json/bash). How would you design a robust detection pipeline?\n\ncompare regex-based parsing vs AST-based parsing for syntax highlighting in a lightweight editor, considering performance constraints\n\ngiven this repo structure, suggest how to classify file types and naming conventions reliably\n\nI want to build a prompt system that adapts to user intent dynamically, how should I structure prompt templates and context injection?\n\nThis is still plain text\nI have a fastapi service, response time is high for one endpoint, here is the code, can you identify bottlenecks and suggest fixes\n\ngiven this dataset schema, write optimized queries for reporting dashboard, assume millions of rows\n\nthis yaml file is for github actions but failing, can you debug and fix it\n\nI am building a code editor, need language detection logic, here are examples, how should I approach it";
        let r = detect_language(content, None);
        assert!(
            r == Some("prompt") || r == None,
            "Multi-question prompt was detected as {:?}, expected prompt or None",
            r
        );
    }

    #[test]
    fn prose_informal_email_not_kotlin() {
        let content = "Hi,\n\nI was going through the changes and had a few doubts.\n\nSome parts look good but not fully sure about the naming consistency (especially in config files). Also YAML detection still seems off in some cases (multi-doc maybe?).\n\nNot urgent, but we should probably clean this before finalizing.\n\nLet's discuss when you're free.";
        let r = detect_language(content, None);
        assert!(
            r == Some("email") || r == None,
            "Informal email was detected as {:?}, expected email or None",
            r
        );
    }

    #[test]
    fn prose_short_multi_prompt_not_scala() {
        let content = "I am designing a language detection system for a code editor, currently using extension + heuristics, but failing for mixed content files (yaml with embedded json/bash). How would you design a robust detection pipeline?\n\ncompare regex-based parsing vs AST-based parsing for syntax highlighting in a lightweight editor, considering performance constraints\n\ngiven this repo structure, suggest how to classify file types and naming conventions reliably\n\nI want to build a prompt system that adapts to user intent dynamically, how should I structure prompt templates and context injection?";
        let r = detect_language(content, None);
        assert!(
            r == Some("prompt") || r == None,
            "Short multi-prompt was detected as {:?}, expected prompt or None",
            r
        );
    }

    #[test]
    fn prose_casual_questions_not_batch() {
        let content = "this code works but sometimes fails not sure why can you check\n\nyaml detection is not working properly esp for multi doc and json inside it\n\nneed help optimizing this, its slow when data is large\n\nthis query is taking too long maybe indexing issue?\n\nnot sure if this is correct approach what do you think";
        let r = detect_language(content, None);
        assert!(
            r != Some("cmd") && r != Some("shell") && r != Some("powershell"),
            "Casual questions were detected as {:?}, expected None/prompt",
            r
        );
    }

    #[test]
    fn prose_structured_prompt_not_yaml() {
        let content = "I'm building a code editor with language detection based on file extension + heuristics, but it fails for mixed content files (like YAML with embedded JSON or bash).\n\nCurrent approach:\n- extension-based fallback\n- regex scanning first 200 lines\n\nProblems:\n- misclassifies GitHub Actions as generic YAML\n- breaks on multi-doc YAML\n- slow for large files\n\nCan you suggest a better architecture that balances performance and accuracy? Ideally something incremental, not full parsing.";
        let r = detect_language(content, None);
        assert!(
            r == Some("prompt") || r == None,
            "Structured prompt was detected as {:?}, expected prompt or None",
            r
        );
    }

    #[test]
    fn prose_postgres_prompt_not_batch() {
        let content = "We have a Postgres DB (~50M rows), one query is slow (~3-5s).\n\nQuery joins 3 tables, filters on date range + status, and sorts by created_at desc.\n\nIndexes exist but not helping much.\n\nCan you:\n1. analyze possible bottlenecks\n2. suggest index strategy\n3. rewrite query if needed\n4. explain how to verify improvements";
        let r = detect_language(content, None);
        assert!(
            r == Some("prompt") || r == None,
            "Postgres prompt was detected as {:?}, expected prompt or None",
            r
        );
    }

    #[test]
    fn prose_api_design_not_yaml() {
        let content = "I'm designing an API for a search system where users can run queries, save results, and later evaluate selected items.\n\nConstraints:\n- search results can be large (1000+ items)\n- evaluation is user-driven (subset of results)\n- results currently not persisted\n\nShould I:\n- persist all search results?\n- store only selected items?\n- or re-run queries on demand?\n\nCan you compare tradeoffs and suggest a scalable design?";
        let r = detect_language(content, None);
        assert!(
            r == Some("prompt") || r == None,
            "API design prompt was detected as {:?}, expected prompt or None",
            r
        );
    }

    #[test]
    fn prose_yaml_classifier_not_yaml() {
        let content = "I'm trying to detect YAML file types (k8s, github actions, docker compose, etc.) based on content.\n\nProblem:\n- many files share common keys like `version`, `services`, `jobs`\n- some files mix formats (YAML + JSON + shell)\n\nExample:\n- GitHub Actions has `jobs`\n- GitLab CI also has `jobs`\n- Docker compose has `services`\n\nHow would you design a reliable classifier without relying on file name?";
        let r = detect_language(content, None);
        assert!(
            r == Some("prompt") || r == None,
            "YAML classifier prompt was detected as {:?}, expected prompt or None",
            r
        );
    }

    // ── Generated prose tests: varied complexities ───────────────────────
    // Cover emails, prompts, informal notes, and technical prose to ensure
    // none are misdetected as code or data-format languages.

    #[test]
    fn prose_one_liner_question() {
        let content = "how do I fix this error in my code";
        let r = detect_language(content, None);
        assert!(
            r == None || r == Some("prompt"),
            "One-liner question detected as {:?}",
            r
        );
    }

    #[test]
    fn prose_short_informal_note() {
        let content = "tried restarting but didn't help, still getting the same issue with the build, maybe it's a dependency conflict?";
        let r = detect_language(content, None);
        assert!(
            r == None || r == Some("prompt"),
            "Short informal note detected as {:?}",
            r
        );
    }

    #[test]
    fn prose_meeting_notes() {
        let content = "Team sync notes:\n\n- discussed migration timeline\n- agreed on using Postgres for the new service\n- need to finalize the API contract by Friday\n- Dave will handle the CI/CD setup\n\nAction items:\n- review PR #234\n- update docs with new endpoints\n- schedule follow-up for next Tuesday";
        let r = detect_language(content, None);
        assert!(
            r != Some("yaml") && r != Some("scala") && r != Some("cmd"),
            "Meeting notes detected as {:?}",
            r
        );
    }

    #[test]
    fn prose_formal_email_with_agenda() {
        let content = "Dear team,\n\nPlease find below the agenda for tomorrow's meeting:\n\n1. Q3 budget review\n2. Hiring update for the engineering team\n3. Release planning for v2.0\n\nCould everyone prepare their status updates beforehand? It would help us stay on track.\n\nLooking forward to seeing everyone.\n\nBest regards,\nSarah";
        let r = detect_language(content, None);
        assert_eq!(r, Some("email"), "Formal email with agenda detected as {:?}", r);
    }

    #[test]
    fn prose_slack_style_message() {
        let content = "hey, quick question - is the staging env up? I'm trying to test the new auth flow but getting 502s. not sure if it's my changes or something else. can someone take a look when they get a chance?";
        let r = detect_language(content, None);
        assert!(
            r == None || r == Some("prompt"),
            "Slack-style message detected as {:?}",
            r
        );
    }

    #[test]
    fn prose_bug_report() {
        let content = "Bug report:\n\nWhen I open a large JSON file (>10MB) and switch to another tab, the editor freezes for about 3 seconds. This doesn't happen with smaller files.\n\nSteps to reproduce:\n1. Open a 15MB JSON file\n2. Wait for it to load\n3. Click on a different tab\n4. Observe the freeze\n\nExpected: smooth tab switch\nActual: 3s freeze with high CPU\n\nThis started after the last update. I'm on Windows 11, using version 0.9.2.";
        let r = detect_language(content, None);
        assert!(
            r != Some("yaml") && r != Some("json") && r != Some("cmd") && r != Some("scala"),
            "Bug report detected as {:?}",
            r
        );
    }

    #[test]
    fn prose_code_review_feedback() {
        let content = "I reviewed the changes and have a few suggestions:\n\nThe error handling in the auth module looks incomplete. What happens if the token refresh fails? Right now it silently swallows the error and the user stays logged in with an expired token.\n\nAlso, the naming is inconsistent - sometimes you use camelCase, sometimes snake_case. Let's pick one and stick with it.\n\nThe test coverage for the new endpoints is good though. Nice work on the edge cases.";
        let r = detect_language(content, None);
        assert!(
            r == None || r == Some("email") || r == Some("prompt"),
            "Code review feedback detected as {:?}",
            r
        );
    }

    #[test]
    fn prose_technical_design_discussion() {
        let content = "I've been thinking about how to handle rate limiting for our API.\n\nOption A: Use a token bucket algorithm with Redis. Pros - distributed, battle-tested. Cons - adds Redis dependency.\n\nOption B: In-memory sliding window. Pros - simple, no external deps. Cons - doesn't work across multiple instances.\n\nOption C: Use the API gateway's built-in rate limiting. Pros - zero code. Cons - less granular control.\n\nI'm leaning toward Option A since we already have Redis for caching. What do you all think?";
        let r = detect_language(content, None);
        assert!(
            r != Some("scala") && r != Some("kotlin") && r != Some("cmd") && r != Some("yaml"),
            "Technical design discussion detected as {:?}",
            r
        );
    }

    #[test]
    fn prose_very_short_fragments() {
        let content = "check this\nfixed it\npushed\nlooks good to me\nmerge when ready";
        let r = detect_language(content, None);
        assert!(
            r == None || r == Some("prompt") || r == Some("email"),
            "Very short fragments detected as {:?}",
            r
        );
    }

    #[test]
    fn prose_mixed_tech_terms_not_code() {
        let content = "We need to set up monitoring for the new service. I suggest using Prometheus for metrics collection and Grafana for dashboards. The existing alerting rules should also be extended to cover the new endpoints.\n\nFor logging, we can continue with our current ELK stack. Just make sure to add structured logging with proper correlation IDs.\n\nLet me know if you need help with any of this.";
        let r = detect_language(content, None);
        assert!(
            r != Some("scala") && r != Some("kotlin") && r != Some("cmd") && r != Some("yaml"),
            "Tech terms in prose detected as {:?}",
            r
        );
    }

    #[test]
    fn prose_email_with_code_reference() {
        let content = "Hi,\n\nI noticed the `getUserById` function is being called without error handling in several places. Could you add try-catch blocks around those calls?\n\nAlso, the `config.json` file has a typo in the database URL. I've fixed it in my branch but wanted to flag it.\n\nThanks,\nAlex";
        let r = detect_language(content, None);
        assert_eq!(
            r,
            Some("email"),
            "Email referencing code detected as {:?}",
            r
        );
    }

    #[test]
    fn prose_long_paragraph_essay() {
        let content = "The evolution of programming languages over the past decades has been remarkable. From the early days of assembly and COBOL to modern languages like Rust and TypeScript, each generation has brought new abstractions and safety guarantees. What's particularly interesting is how the pendulum swings between simplicity and expressiveness. Languages like Go chose deliberate simplicity, while Scala and Haskell embraced rich type systems. Neither approach is universally better - the right choice depends on the team, the project, and the constraints you're working within.";
        let r = detect_language(content, None);
        assert!(
            r == None || r == Some("prompt"),
            "Long essay paragraph detected as {:?}",
            r
        );
    }

    #[test]
    fn prose_todo_list_not_yaml() {
        let content = "Things to do:\n- fix the login bug\n- update the README\n- add tests for the new API\n- review Maria's PR\n- deploy to staging\n\nNice to have:\n- refactor the utils module\n- add dark mode support";
        let r = detect_language(content, None);
        assert!(
            r != Some("yaml"),
            "Todo list detected as {:?}",
            r
        );
    }

    // ── Ensure real YAML still works with prose guard ────────────────────

    #[test]
    fn yaml_docker_compose_still_detected() {
        let content = "version: '3.8'\nservices:\n  web:\n    image: nginx:latest\n    ports:\n      - '80:80'\n  db:\n    image: postgres:15\n    environment:\n      - POSTGRES_PASSWORD=secret";
        assert_eq!(detect_language(content, None), Some("yaml"));
    }

    #[test]
    fn yaml_github_actions_still_detected() {
        let content = "name: CI\non:\n  push:\n    branches: [main]\n  pull_request:\n    branches: [main]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - run: npm test";
        assert_eq!(detect_language(content, None), Some("yaml"));
    }

    #[test]
    fn yaml_k8s_service_still_detected() {
        let content = "apiVersion: v1\nkind: Service\nmetadata:\n  name: my-service\n  namespace: production\nspec:\n  selector:\n    app: my-app\n  ports:\n    - port: 80\n      targetPort: 8080\n  type: ClusterIP";
        assert_eq!(detect_language(content, None), Some("yaml"));
    }

    // ── V2 Pipeline Tests ────────────────────────────────────

    #[test]
    fn v2_extension_still_works() {
        assert_eq!(detect_language_v2("", Some("data.json")), Some("json"));
        assert_eq!(detect_language_v2("", Some("app.py")), Some("python"));
    }

    #[test]
    fn v2_shebang_still_works() {
        assert_eq!(
            detect_language_v2("#!/usr/bin/env python3\nprint('hello')", None),
            Some("python")
        );
    }

    #[test]
    fn v2_structural_still_works() {
        // JSON structural detection
        assert_eq!(
            detect_language_v2("{\"name\": \"test\", \"version\": \"1.0\"}", None),
            Some("json")
        );
    }

    #[test]
    fn v2_prose_does_not_detect_as_code() {
        // The original reported bugs — v2 should handle these correctly
        // even during migration (falls back to legacy which has the prose guards)
        let prose = "I am designing a language detection system for a code editor, \
                      currently using extension + heuristics, but failing for mixed \
                      content files (yaml with embedded json/bash). How would you \
                      design a robust detection pipeline?";
        let result = detect_language_v2(prose, None);
        // Should either be None or "prompt" — never a code language
        assert!(
            result.is_none() || result == Some("prompt"),
            "v2: prose should not be detected as code, got {:?}",
            result
        );
    }

    #[test]
    fn v2_code_still_detected_via_fallback() {
        // During migration, code detection falls back to legacy heuristic
        let python = "import os\nimport sys\n\ndef main():\n    path = os.getcwd()\n    \
                       if path == '/tmp':\n        sys.exit(1)\n    print(path)\n\nmain()";
        assert_eq!(detect_language_v2(python, None), Some("python"));
    }

    #[test]
    fn v2_email_not_code() {
        let email = "Hi John,\n\nI've been thinking about the project and I'm not sure \
                      if we should proceed. The naming consistency is off and YAML detection \
                      still seems flaky.\n\nLet's discuss when you're free.\n\nThanks,\nSarah";
        let result = detect_language_v2(email, None);
        assert!(
            result.is_none() || result == Some("email") || result == Some("prompt"),
            "v2: email should not be detected as a code language, got {:?}",
            result
        );
    }

    // ── V2 Pipeline: Family-Gated Scoring Tests ─────────────────────────

    #[test]
    fn v2_python_via_family_scoring() {
        let python = r#"
import os
import sys

def main():
    path = os.getcwd()
    if path == '/tmp':
        sys.exit(1)
    print(path)

if __name__ == "__main__":
    main()
"#;
        let result = detect_language_v2(python, None);
        assert_eq!(result, Some("python"), "v2: Python should be detected via family scoring");
    }

    #[test]
    fn v2_rust_via_family_scoring() {
        let rust = r#"
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Config {
    pub name: String,
}

pub fn process(config: &Config) -> Result<(), String> {
    let mut map = HashMap::new();
    map.insert("key", config.name.clone());
    println!("Processing: {}", config.name);
    Ok(())
}
"#;
        let result = detect_language_v2(rust, None);
        assert_eq!(result, Some("rust"), "v2: Rust should be detected via family scoring");
    }

    #[test]
    fn v2_go_via_family_scoring() {
        let go = r#"
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
        let result = detect_language_v2(go, None);
        assert_eq!(result, Some("go"), "v2: Go should be detected via family scoring");
    }

    #[test]
    fn v2_typescript_via_family_scoring() {
        let ts = r#"
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
        let result = detect_language_v2(ts, None);
        assert_eq!(result, Some("typescript"), "v2: TypeScript should be detected via family scoring");
    }

    #[test]
    fn v2_java_via_family_scoring() {
        let java = r#"
import java.util.ArrayList;
import java.util.List;

public class Main {
    public static void main(String[] args) {
        List<String> items = new ArrayList<>();
        items.add("hello");
        System.out.println(items);
    }
}
"#;
        let result = detect_language_v2(java, None);
        assert_eq!(result, Some("java"), "v2: Java should be detected via family scoring");
    }

    #[test]
    fn v2_kotlin_via_family_scoring() {
        let kotlin = r#"
data class User(val name: String, val age: Int)

fun main() {
    val users = listOf(
        User("Alice", 30),
        User("Bob", 25),
    )
    users.filter { it.age > 18 }
        .forEach { println(it.name) }
}
"#;
        let result = detect_language_v2(kotlin, None);
        assert_eq!(result, Some("kotlin"), "v2: Kotlin should be detected via family scoring");
    }

    #[test]
    fn v2_scala_via_family_scoring() {
        let scala = r#"
case class User(name: String, age: Int)

sealed trait Shape
case class Circle(radius: Double) extends Shape
case class Rectangle(w: Double, h: Double) extends Shape

object Main extends App {
    implicit val ordering: Ordering[User] = Ordering.by(_.age)
    val users = List(User("Alice", 30), User("Bob", 25))
    users.sorted.foreach(println)
}
"#;
        let result = detect_language_v2(scala, None);
        assert_eq!(result, Some("scala"), "v2: Scala should be detected via family scoring");
    }

    #[test]
    fn v2_cpp_via_family_scoring() {
        let cpp = r#"
#include <iostream>
#include <vector>
#include <string>

using namespace std;

template<typename T>
void printVec(const vector<T>& vec) {
    for (const auto& item : vec) {
        cout << item << endl;
    }
}

int main() {
    auto ptr = make_unique<string>("hello");
    cout << *ptr << endl;
    return 0;
}
"#;
        let result = detect_language_v2(cpp, None);
        assert_eq!(result, Some("cpp"), "v2: C++ should be detected via family scoring");
    }

    // ── V2 Pipeline: Rival Disambiguation Tests ─────────────────────────

    #[test]
    fn v2_js_vs_ts_detects_js() {
        // Pure JS with require/module.exports — no TypeScript type annotations
        let js = r#"
const express = require('express');
const path = require('path');

const app = express();
const PORT = process.env.PORT || 3000;

app.get('/', (req, res) => {
    res.send('Hello World');
});

module.exports = app;
"#;
        let result = detect_language_v2(js, None);
        assert_eq!(result, Some("javascript"), "v2: JS with require/exports should detect as JS, not TS");
    }

    #[test]
    fn v2_js_vs_ts_detects_ts() {
        // TypeScript with interface, type annotations
        let ts = r#"
interface Config {
    port: number;
    host: string;
    debug: boolean;
}

type Handler = (req: Request, res: Response) => void;

declare module 'express' {
    interface Application {
        customMethod(): void;
    }
}

const config: Config = { port: 3000, host: "localhost", debug: true };
"#;
        let result = detect_language_v2(ts, None);
        assert_eq!(result, Some("typescript"), "v2: TS with interface/type/declare should detect as TS");
    }

    #[test]
    fn v2_c_vs_cpp_detects_c() {
        // Pure C with no C++ features
        let c = r#"
#include <stdio.h>
#include <stdlib.h>

#define MAX_SIZE 1024

typedef struct {
    int id;
    char name[64];
} Record;

int main(int argc, char* argv[]) {
    Record* r = malloc(sizeof(Record));
    r->id = 1;
    printf("Record: %d\n", r->id);
    free(r);
    return 0;
}
"#;
        let result = detect_language_v2(c, None);
        assert!(
            result == Some("c") || result == Some("cpp"),
            "v2: Pure C should detect as C (or C++), got {:?}", result
        );
    }

    #[test]
    fn v2_c_vs_cpp_detects_cpp() {
        // C++ with templates, std::, cout, namespaces
        let cpp = r#"
#include <iostream>
#include <vector>
#include <memory>

namespace mylib {

template<typename T>
class Container {
public:
    void add(T item) { items_.push_back(std::move(item)); }
    
    void print() const {
        for (const auto& item : items_) {
            std::cout << item << std::endl;
        }
    }

private:
    std::vector<T> items_;
};

}  // namespace mylib
"#;
        let result = detect_language_v2(cpp, None);
        assert_eq!(result, Some("cpp"), "v2: C++ with templates/std::/cout should detect as C++");
    }

    #[test]
    fn v2_java_vs_kotlin_detects_java() {
        // Pure Java with System.out, @Override, public static
        let java = r#"
import java.util.HashMap;
import java.util.Map;

public class UserService {
    private final Map<String, User> users = new HashMap<>();

    public void addUser(String name) throws IllegalArgumentException {
        if (name == null) {
            throw new IllegalArgumentException("Name cannot be null");
        }
        users.put(name, new User(name));
        System.out.println("Added user: " + name);
    }

    @Override
    public String toString() {
        return "UserService{users=" + users.size() + "}";
    }
}
"#;
        let result = detect_language_v2(java, None);
        assert_eq!(result, Some("java"), "v2: Java with System.out/@Override should detect as Java");
    }

    #[test]
    fn v2_java_vs_kotlin_detects_kotlin() {
        // Kotlin with fun, data class, companion object, null safety
        let kotlin = r#"
data class User(val name: String, val age: Int)

class UserService {
    companion object {
        private val logger = LoggerFactory.getLogger(UserService::class.java)
    }

    private val users = mutableListOf<User>()

    fun addUser(name: String) {
        val user = User(name, 0)
        users.add(user)
        logger.info("Added: ${user.name}")
    }

    fun findUser(name: String): User? = users.firstOrNull { it.name == name }

    suspend fun loadUsers(): List<User> {
        return withContext(Dispatchers.IO) {
            repository.getAll()
        }
    }
}
"#;
        let result = detect_language_v2(kotlin, None);
        assert_eq!(result, Some("kotlin"), "v2: Kotlin with fun/companion/data class should detect as Kotlin");
    }

    #[test]
    fn v2_java_vs_scala_detects_scala() {
        // Scala with case class, sealed trait, implicit, pattern matching
        let scala = r#"
case class Config(host: String, port: Int)

sealed trait Result[+A]
case class Success[A](value: A) extends Result[A]
case class Failure(error: String) extends Result[Nothing]

object Main extends App {
    implicit val ordering: Ordering[Config] = Ordering.by(_.port)
    
    def process(config: Config): Result[String] = config match {
        case Config(host, port) if port > 0 => Success(s"$host:$port")
        case _ => Failure("invalid config")
    }
}
"#;
        let result = detect_language_v2(scala, None);
        assert_eq!(result, Some("scala"), "v2: Scala with case class/sealed trait/implicit should detect as Scala");
    }

    #[test]
    fn v2_python_vs_ruby_detects_ruby() {
        // Ruby with do..end, @instance_vars, def..end, symbols
        let ruby = r#"
class UserController < ApplicationController
  before_action :set_user, only: [:show, :update, :destroy]

  def index
    @users = User.where(active: true).order(:name)
    render json: @users
  end

  def create
    @user = User.new(user_params)
    if @user.save
      render json: @user, status: :created
    else
      render json: @user.errors, status: :unprocessable_entity
    end
  end

  private

  def set_user
    @user = User.find(params[:id])
  end

  def user_params
    params.require(:user).permit(:name, :email, :role)
  end
end
"#;
        let result = detect_language_v2(ruby, None);
        assert_eq!(result, Some("ruby"), "v2: Ruby with do..end/@ vars should detect as Ruby");
    }

    // ── V2 Pipeline: Shell Family Tests ─────────────────────────────────

    #[test]
    fn v2_shell_script() {
        let shell = r#"#!/bin/bash
set -euo pipefail

export DEPLOY_ENV="${1:-staging}"

for service in $(cat services.txt); do
    echo "Deploying $service to $DEPLOY_ENV..."
    docker-compose -f "docker/$service.yml" up -d
    if [ $? -ne 0 ]; then
        echo "FAILED: $service" >&2
        exit 1
    fi
done
echo "All services deployed."
"#;
        let result = detect_language_v2(shell, None);
        assert_eq!(result, Some("shell"), "v2: bash script should detect as shell");
    }

    #[test]
    fn v2_powershell_script() {
        let ps = r#"
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$Environment
)

$items = Get-ChildItem -Path $PSScriptRoot -Filter "*.ps1"
foreach ($item in $items) {
    Write-Host "Processing: $($item.Name)"
    & $item.FullName -Environment $Environment
}

Write-Output "Done processing $($items.Count) scripts"
"#;
        let result = detect_language_v2(ps, None);
        assert_eq!(result, Some("powershell"), "v2: PowerShell script should detect as powershell");
    }

    #[test]
    fn v2_cmd_batch() {
        let cmd = r#"@echo off
setlocal enabledelayedexpansion

set PROJECT=myapp
set /p VERSION=<version.txt

echo Building %PROJECT% version %VERSION%

if exist build (
    rmdir /s /q build
)

for /F %%i in ('dir /b *.txt') do (
    echo Processing %%i
    copy %%i build\%PROJECT%\%%i
)

goto :eof

:cleanup
endlocal
"#;
        let result = detect_language_v2(cmd, None);
        assert_eq!(result, Some("cmd"), "v2: CMD batch should detect as cmd");
    }

    // ── V2 Pipeline: Prose Rejection Tests ──────────────────────────────

    #[test]
    fn v2_prose_with_code_keywords_rejected() {
        // Text full of words that happen to be programming keywords
        let prose = "Let me set up a meeting to discuss where we should import the data from. \
                     I want to define the scope and select the right approach for our class of problems. \
                     We need to consider if this is the right time to make these changes and how \
                     we can implement them without breaking existing functionality.";
        let result = detect_language_v2(prose, None);
        assert!(
            result.is_none() || result == Some("email") || result == Some("prompt"),
            "v2: prose with code keywords should not detect as code, got {:?}",
            result
        );
    }

    #[test]
    fn v2_short_question_rejected() {
        let q = "what does this function do and should I refactor it?";
        let result = detect_language_v2(q, None);
        assert!(
            result.is_none() || result == Some("prompt"),
            "v2: short question should not detect as code, got {:?}",
            result
        );
    }

    #[test]
    fn v2_meeting_notes_not_yaml() {
        // Meeting notes with contractions and questions — clearly prose, not YAML.
        // Note: Without enough prose signals, YAML structural detector may fire
        // before the family classifier runs (Phase 0c vs Phase 1).
        let notes = "Team sync - Jan 15\n\n\
                     - discussed the API redesign; it's looking promising\n\
                     - agreed to use GraphQL for the new endpoints\n\
                     - need to finalize schema by Friday - we're behind schedule\n\
                     - performance testing starts next week, isn't it?\n\n\
                     Action items:\n\
                     - review PR #456 (who's handling this?)\n\
                     - update architecture docs - they're outdated\n\
                     - set up monitoring dashboards";
        let result = detect_language_v2(notes, None);
        assert_ne!(result, Some("yaml"), "v2: meeting notes with contractions should not be YAML, got {:?}", result);
    }

    // ── Ruby vs Markdown ─────────────────────────────────────

    #[test]
    fn ruby_gem_version_not_markdown() {
        let src = "# frozen_string_literal: true\n\nmodule Rails\n  # Returns the currently loaded version of \\Rails as a +Gem::Version+.\n  def self.gem_version\n    Gem::Version.new VERSION::STRING\n  end\n\n  module VERSION\n    MAJOR = 8\n    MINOR = 2\n    TINY  = 0\n    PRE   = \"alpha\"\n\n    STRING = [MAJOR, MINOR, TINY, PRE].compact.join(\".\")\n  end\nend\n";
        let result = detect_language(src, None);
        assert_ne!(result, Some("markdown"), "Ruby gem_version should not be detected as markdown, got {:?}", result);
    }

    #[test]
    fn ruby_deprecator_not_markdown() {
        // Minimal Ruby module file — was misdetected as markdown.
        let src = "# frozen_string_literal: true\n\nmodule ActionCable\n  def self.deprecator\n    @deprecator ||= ActiveSupport::Deprecation.new(\"8.1\", \"ActionCable\")\n  end\nend\n";
        let result = detect_language(src, None);
        assert_ne!(result, Some("markdown"), "Ruby deprecator file should not be detected as markdown, got {:?}", result);
    }

    #[test]
    fn ruby_gemspec_not_markdown() {
        let src = "# frozen_string_literal: true\n\nGem::Specification.new do |s|\n  s.name = \"rails\"\n  s.version = \"8.2.0\"\n  s.summary = \"Full-stack web framework.\"\nend\n";
        let result = detect_language(src, None);
        assert_ne!(result, Some("markdown"), "Ruby gemspec should not be detected as markdown, got {:?}", result);
    }

    // ── Audit regression: Kotlin content detection ──────────

    #[test]
    fn kotlin_data_class_not_java() {
        let content = r#"
package okhttp3

data class User(val name: String, val age: Int)

fun main() {
    val users = listOf(
        User("Alice", 30),
        User("Bob", 25),
    )
    users.filter { it.age > 18 }
        .forEach { println(it.name) }
}
"#;
        let result = detect_language(content, None);
        assert_eq!(result, Some("kotlin"), "Kotlin data class detected as {:?}", result);
    }

    #[test]
    fn kotlin_extension_function_not_java() {
        // Real pattern from ktor repo: extension functions with receivers
        let content = r#"
package ktorbuild

import org.gradle.api.Project
import org.gradle.api.Task

internal fun Project.registerPackageJsonAggregationTasks() {
    val target = "js"
    tasks.register("aggregateTask") {
        dependsOn(tasks.named { it.startsWith(target) })
    }
}

fun Project.wirePackageJsonAggregationTasks() {
    tasks.named { it == "kotlinPackageJsonUmbrella" }
        .configureEach { dependsOnPackageJsonAggregation("js") }
}

private fun Task.dependsOnPackageJsonAggregation(target: String) {
    dependsOn("${target}PackageJsonAggregation")
}
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("java"), "Kotlin extension functions must not be detected as Java, got {:?}", result);
        assert_ne!(result, Some("go"), "Kotlin extension functions must not be detected as Go, got {:?}", result);
    }

    #[test]
    fn kotlin_with_java_imports_not_java() {
        // Real pattern from okhttp: Kotlin file importing java.* packages
        let content = r#"
package okhttp3

import java.io.Closeable
import java.util.concurrent.CopyOnWriteArraySet
import java.util.logging.Level
import java.util.logging.Logger
import kotlin.reflect.KClass

object OkHttpDebugLogging {
    private val loggers = CopyOnWriteArraySet<Logger>()

    fun logRecords(): Sequence<LogRecord> {
        return logRecords.asSequence()
    }

    fun enable(loggerClass: KClass<*>): Closeable {
        val logger = Logger.getLogger(loggerClass.java.name)
        logger.level = Level.FINEST
        return Closeable { logger.level = null }
    }
}
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("java"), "Kotlin with java.* imports must not be detected as Java, got {:?}", result);
    }

    #[test]
    fn kotlin_gradle_dsl_not_css() {
        // Real pattern from ktor/okhttp: .kts Gradle build files
        let content = r#"
plugins {
    `kotlin-dsl`
}

dependencies {
    implementation("org.jetbrains.kotlin:kotlin-gradle-plugin:1.9.0")
    implementation("org.jetbrains.kotlin:kotlin-serialization:1.9.0")
}

kotlin {
    jvmToolchain(21)
    compilerOptions {
        freeCompilerArgs.add("-Xcontext-receivers")
    }
}

tasks.validatePlugins {
    enableStricterValidation.set(true)
}
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("css"), "Kotlin Gradle DSL must not be detected as CSS, got {:?}", result);
    }

    #[test]
    fn kotlin_file_annotation_not_css() {
        // Real pattern: Kotlin @file: annotations + typealias
        let content = r#"
@file:OptIn(ExperimentalWasmDsl::class)

import org.jetbrains.kotlin.gradle.ExperimentalWasmDsl
import org.jetbrains.kotlin.gradle.dsl.KotlinMultiplatformExtension

private typealias KotlinSourceSets = NamedDomainObjectContainer<KotlinSourceSet>

val KotlinSourceSets.posixMain: KotlinSourceSetProvider by KotlinSourceSetProvider
val KotlinSourceSets.posixTest: KotlinSourceSetProvider by KotlinSourceSetProvider

@JvmInline
value class KotlinSourceSetProvider(val name: String)

fun KotlinMultiplatformExtension.configureSourceSets() {
    sourceSets.posixMain.dependsOn(commonMain)
}
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("css"), "Kotlin @file: annotation must not be detected as CSS, got {:?}", result);
        assert_ne!(result, Some("typescript"), "Kotlin @file: annotation must not be detected as TypeScript, got {:?}", result);
    }

    #[test]
    fn kotlin_coroutines_detected() {
        let content = r#"
package io.ktor.server

import kotlinx.coroutines.*

suspend fun handleRequest(call: ApplicationCall) {
    coroutineScope {
        val deferred = async {
            fetchData(call.request.uri)
        }
        launch {
            logRequest(call)
        }
        call.respond(deferred.await())
    }
}
"#;
        let result = detect_language(content, None);
        assert_eq!(result, Some("kotlin"), "Kotlin coroutines detected as {:?}", result);
    }

    #[test]
    fn kotlin_simple_fun_val_detected() {
        // Minimal Kotlin file with just fun and val — must not fall to Java/Go/CSS
        let content = r#"
package mockwebserver3

import java.util.concurrent.TimeUnit

fun main() {
    val timeout = TimeUnit.SECONDS.toMillis(30)
    println("Timeout: $timeout")
}
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("java"), "Simple Kotlin fun/val must not be Java, got {:?}", result);
        assert_ne!(result, Some("css"), "Simple Kotlin fun/val must not be CSS, got {:?}", result);
    }

    // ── Audit regression: Scala content detection ───────────

    #[test]
    fn scala_case_class_not_java() {
        let content = r#"
package org.scalatra

case class Route(method: HttpMethod, path: String, action: () => Any)

sealed trait HttpMethod
case object Get extends HttpMethod
case object Post extends HttpMethod

object RouteRegistry {
    def apply(): RouteRegistry = new RouteRegistry()
}
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("java"), "Scala case class must not be detected as Java, got {:?}", result);
    }

    #[test]
    fn scala_trait_extends_not_java() {
        // Real pattern from scalatra: trait with extends and match
        let content = r#"
package org.scalatra

import javax.servlet.http.HttpServletRequest

trait SslRequirement extends Handler with ServletApiImplicits {
    abstract override def handle(req: HttpServletRequest, res: HttpServletResponse): Unit = {
        if (!req.isSecure) {
            val oldUri = req.uri
            val port = securePortMap.lift(oldUri.getPort) getOrElse 443
            res.sendRedirect(oldUri.toString)
        }
    }
}
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("java"), "Scala trait+extends must not be Java, got {:?}", result);
        assert_ne!(result, Some("go"), "Scala trait+extends must not be Go, got {:?}", result);
    }

    #[test]
    fn scala_match_case_not_java() {
        // Real pattern from scalatra: pattern matching
        let content = r#"
package org.scalatra

import scala.collection.concurrent.TrieMap
import java.util.concurrent.ConcurrentHashMap

class RouteRegistry {
    private[this] val _methodRoutes: ConcurrentMap[HttpMethod, Seq[Route]] =
        new ConcurrentHashMap[HttpMethod, Seq[Route]]().asScala

    def matchingMethods(requestPath: String): Set[HttpMethod] = {
        _methodRoutes.keys.filter { method =>
            method match {
                case Head => _methodRoutes.getOrElse(Head, Vector.empty).nonEmpty
                case m    => _methodRoutes.getOrElse(m, Vector.empty).nonEmpty
            }
        }.toSet
    }
}
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("java"), "Scala match/case must not be Java, got {:?}", result);
    }

    #[test]
    fn scala_simple_extends_not_kotlin() {
        // Minimal Scala file — `extends` instead of Kotlin's `:`
        let content = r#"
package org.scalatra

class ScalatraException(message: String) extends Exception(message)

class NotFoundException(message: String) extends ScalatraException(message)
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("kotlin"), "Simple Scala extends must not be Kotlin, got {:?}", result);
    }

    #[test]
    fn sbt_build_file_not_go() {
        // Real pattern from scalatra: build.sbt with SBT DSL
        let content = r#"
import Dependencies._

val unusedOptions = Seq("-Ywarn-unused:imports")

lazy val scalatraSettings = Seq(
    organization := "org.scalatra",
    crossScalaVersions := Seq("2.13.18", "3.6.4"),
    scalacOptions ++= Seq("-deprecation", "-unchecked"),
    Def.settings(
        publishTo := sonatypePublishToBundle.value
    )
)

ThisBuild / version := "3.2.0-SNAPSHOT"
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("go"), "SBT build file must not be Go, got {:?}", result);
        assert_ne!(result, Some("kotlin"), "SBT build file must not be Kotlin, got {:?}", result);
    }

    // ── Audit regression: Clojure content detection ─────────

    #[test]
    fn clojure_defproject_not_css() {
        // Real pattern from compojure: project.clj
        let content = r#"
(defproject compojure "1.7.2"
  :description "A concise routing library for Ring"
  :url "https://github.com/weavejester/compojure"
  :license {:name "Eclipse Public License"
            :url "http://www.eclipse.org/legal/epl-v10.html"}
  :dependencies [[org.clojure/clojure "1.9.0"]
                  [org.clojure/tools.macro "0.1.5"]
                  [clout "2.2.1"]
                  [medley "1.1.0"]
                  [ring/ring-core "1.7.1"]
                  [ring/ring-codec "1.1.1"]]
  :plugins [[lein-codox "0.10.3"]])
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("css"), "Clojure defproject must not be CSS, got {:?}", result);
    }

    #[test]
    fn clojure_ns_defn_detected() {
        let content = r#"
(ns compojure.coercions-test
  (:require [clojure.test :refer :all]
            [compojure.coercions :refer :all]))

(deftest test-as-int
  (is (= (as-int "1") 1))
  (is (= (as-int "foo") nil)))

(deftest test-as-uuid
  (is (= (as-uuid "de305d54-75b4-431b-adb2-eb6b9e546014")
         #uuid "de305d54-75b4-431b-adb2-eb6b9e546014")))
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("text"), "Clojure ns+deftest must not be text, got {:?}", result);
        assert_ne!(result, Some("css"), "Clojure ns+deftest must not be CSS, got {:?}", result);
    }

    // ── Audit regression: Go content detection ──────────────

    #[test]
    fn go_short_version_file_not_text() {
        // Real pattern from gin: version.go — very short Go file
        let content = r#"// Copyright 2018 Gin Core Team. All rights reserved.
// Use of this source code is governed by a MIT style
// license that can be found in the LICENSE file.

package gin

// Version is the current gin framework's version.
const Version = "v1.12.0"
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("text"), "Short Go version file must not be text, got {:?}", result);
    }

    #[test]
    fn go_test_file_not_prompt() {
        // Real pattern from gin: auth_test.go
        let content = r#"
package gin

import (
    "encoding/base64"
    "net/http"
    "testing"

    "github.com/stretchr/testify/assert"
)

func TestBasicAuth(t *testing.T) {
    pairs := processAccounts(Accounts{
        "admin": "password",
        "foo":   "bar",
    })
    assert.Len(t, pairs, 2)
}

func TestBasicAuthFails(t *testing.T) {
    assert.Panics(t, func() {
        processAccounts(Accounts{
            "":    "password",
        })
    })
}
"#;
        let result = detect_language(content, None);
        assert_eq!(result, Some("go"), "Go test file detected as {:?}", result);
    }

    #[test]
    fn go_mod_not_ruby() {
        // Real pattern from gin: go.mod
        let content = r#"module github.com/gin-gonic/gin

go 1.25.0

require (
    github.com/bytedance/sonic v1.13.2
    github.com/gin-contrib/sse v1.1.0
    github.com/go-playground/validator/v10 v10.26.0
    github.com/pelletier/go-toml/v2 v2.2.4
    github.com/stretchr/testify v1.10.0
)

require (
    github.com/davecgh/go-spew v1.1.1 // indirect
    github.com/pmezard/go-difflib v1.0.0 // indirect
    gopkg.in/yaml.v3 v3.0.1 // indirect
)
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("ruby"), "go.mod must not be detected as Ruby, got {:?}", result);
    }

    #[test]
    fn go_struct_type_detected() {
        let content = r#"
package gin

type Engine struct {
    RouterGroup
    pool             sync.Pool
    trees            methodTrees
    maxParams        uint16
}

type RouterGroup struct {
    Handlers HandlersChain
    basePath string
    engine   *Engine
    root     bool
}

func (engine *Engine) ServeHTTP(w http.ResponseWriter, req *http.Request) {
    c := engine.pool.Get().(*Context)
    c.writermem.reset(w)
    c.Request = req
    engine.handleHTTPRequest(c)
    engine.pool.Put(c)
}
"#;
        let result = detect_language(content, None);
        assert_eq!(result, Some("go"), "Go struct+method detected as {:?}", result);
    }

    // ── Audit regression: Cross-language non-confusion ──────

    #[test]
    fn java_still_detected_correctly() {
        // Make sure real Java is still detected as Java after adding anti-patterns
        let content = r#"
import java.util.ArrayList;
import java.util.List;

public class Main {
    public static void main(String[] args) {
        List<String> items = new ArrayList<>();
        for (String item : items) {
            System.out.println(item);
        }
    }

    @Override
    public String toString() {
        return "Main";
    }
}
"#;
        let result = detect_language(content, None);
        assert_eq!(result, Some("java"), "Real Java detected as {:?}", result);
    }

    #[test]
    fn css_still_detected_correctly() {
        // Make sure real CSS is still detected correctly after adding anti-patterns
        let content = r#"
.container {
    display: flex;
    margin: 0 auto;
    padding: 16px;
}

#header {
    background: #333;
    color: white;
}

@media (max-width: 768px) {
    .container {
        flex-direction: column;
    }
}
"#;
        let result = detect_language(content, None);
        assert_eq!(result, Some("css"), "Real CSS detected as {:?}", result);
    }

    #[test]
    fn kotlin_not_scala_when_using_fun() {
        // Ensure `fun` keyword distinguishes Kotlin from Scala (which uses `def`)
        let content = r#"
package io.ktor.server

import io.ktor.server.application.*

fun Application.configureRouting() {
    routing {
        get("/") {
            call.respondText("Hello World!")
        }
    }
}

fun Application.configureSerialization() {
    install(ContentNegotiation) {
        json()
    }
}
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("scala"), "Kotlin with fun must not be Scala, got {:?}", result);
        assert_ne!(result, Some("java"), "Kotlin with fun must not be Java, got {:?}", result);
    }

    #[test]
    fn scala_not_kotlin_when_using_def() {
        // Ensure `def` keyword + trait distinguishes Scala from Kotlin
        let content = r#"
package org.scalatra

import javax.servlet.http.HttpServletRequest
import scala.collection.mutable

trait ScalatraBase extends Handler {
    def get(path: String)(action: => Any): Route
    def post(path: String)(action: => Any): Route

    override def handle(request: HttpServletRequest): Unit = {
        val matchedRoutes = routes.matchingMethods(request.getPathInfo)
        matchedRoutes match {
            case Some(route) => route.action()
            case None => pass()
        }
    }
}
"#;
        let result = detect_language(content, None);
        assert_ne!(result, Some("kotlin"), "Scala with def+trait must not be Kotlin, got {:?}", result);
        assert_ne!(result, Some("java"), "Scala with def+trait must not be Java, got {:?}", result);
    }
}
