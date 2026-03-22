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
pub mod languages;
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
        .unwrap_or(&*bounded)
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
    //   Strip markdown code blocks (fenced and indented) so that embedded code
    //   examples don't trigger false-positive language signals.
    //   4a — tree-sitter validation of the heuristic winner
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
}
