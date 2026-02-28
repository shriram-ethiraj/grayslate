/**
 * languageDetector.test.ts
 *
 * Comprehensive test suite for the language detector.
 *
 * Run:  pnpm dlx tsx src/lib/editor/core/languageDetector.test.ts
 *
 * Tests cover the full cascade: extension → shebang → structural → scoring.
 */

/* eslint-disable no-console */

// ════════════════════════════════════════════════════════════════
// Test Harness
// ════════════════════════════════════════════════════════════════

let passed = 0;
let failed = 0;
const failures: string[] = [];

function assert(
    label: string,
    actual: string | null,
    expected: string | null,
) {
    if (actual === expected) {
        passed++;
    } else {
        failed++;
        const msg = `  ✗ ${label}\n    expected: ${expected}\n    actual:   ${actual}`;
        failures.push(msg);
        console.error(msg);
    }
}

async function main() {
const { languageDetector } = await import("./languageDetector");

// ════════════════════════════════════════════════════════════════
// Phase 1 — Extension Detection
// ════════════════════════════════════════════════════════════════

console.log("\n── Phase 1: Extension Detection ──");

assert("JSON extension",       languageDetector.detect("", "data.json"),       "json");
assert("JSONC extension",      languageDetector.detect("", "tsconfig.jsonc"),   "json");
assert("YAML extension",       languageDetector.detect("", "config.yaml"),      "yaml");
assert("YML extension",        languageDetector.detect("", "config.yml"),       "yaml");
assert("XML extension",        languageDetector.detect("", "pom.xml"),          "xml");
assert("SVG extension",        languageDetector.detect("", "icon.svg"),         "xml");
assert("HTML extension",       languageDetector.detect("", "index.html"),       "html");
assert("Markdown extension",   languageDetector.detect("", "README.md"),        "markdown");
assert("JS extension",         languageDetector.detect("", "app.js"),           "javascript");
assert("TS extension",         languageDetector.detect("", "app.ts"),           "typescript");
assert("TSX extension",        languageDetector.detect("", "App.tsx"),          "typescript");
assert("Python extension",     languageDetector.detect("", "main.py"),          "python");
assert("CSS extension",        languageDetector.detect("", "styles.css"),       "css");
assert("SCSS extension",       languageDetector.detect("", "styles.scss"),      "css");
assert("C extension",          languageDetector.detect("", "main.c"),           "c");
assert("C++ extension",        languageDetector.detect("", "main.cpp"),         "cpp");
assert("Java extension",       languageDetector.detect("", "Main.java"),        "java");
assert("Go extension",         languageDetector.detect("", "main.go"),          "go");
assert("CSV extension",        languageDetector.detect("", "data.csv"),         "csv");
assert("TSV extension",        languageDetector.detect("", "data.tsv"),         "csv");
assert("Shell extension",      languageDetector.detect("", "deploy.sh"),        "shell");
assert("Dockerfile name",      languageDetector.detect("", "Dockerfile"),       "dockerfile");
assert("Dockerfile lowercase", languageDetector.detect("", "dockerfile"),       "dockerfile");
assert(".bashrc name",         languageDetector.detect("", ".bashrc"),           "shell");
assert(".zshrc name",          languageDetector.detect("", ".zshrc"),            "shell");

// ════════════════════════════════════════════════════════════════
// Phase 2 — Shebang Detection
// ════════════════════════════════════════════════════════════════

console.log("\n── Phase 2: Shebang Detection ──");

assert("Python shebang",       languageDetector.detect("#!/usr/bin/env python3\nprint('hi')"), "python");
assert("Node shebang",         languageDetector.detect("#!/usr/bin/env node\nconsole.log('hi')"), "javascript");
assert("Bash shebang",         languageDetector.detect("#!/bin/bash\necho hello"), "shell");
assert("Sh shebang",           languageDetector.detect("#!/bin/sh\necho hello"), "shell");
assert("Deno shebang",         languageDetector.detect("#!/usr/bin/env deno\nconsole.log('hi')"), "typescript");
assert("Zsh shebang",          languageDetector.detect("#!/usr/bin/env zsh\necho hello"), "shell");

// ════════════════════════════════════════════════════════════════
// Phase 3a — JSON Detection
// ════════════════════════════════════════════════════════════════

console.log("\n── Phase 3a: JSON Detection ──");

assert("Simple JSON object",   languageDetector.detect('{"test": 1}'),                  "json");
assert("JSON array",           languageDetector.detect('[1, 2, 3]'),                     "json");
assert("Nested JSON",          languageDetector.detect('{"a": {"b": [1, 2]}}'),          "json");
assert("Pretty JSON",          languageDetector.detect('{\n  "name": "project",\n  "version": "1.0.0"\n}'), "json");
assert("Empty JSON object",    languageDetector.detect('{}'),                             "json");
assert("Empty JSON array",     languageDetector.detect('[]'),                             "json");

assert("JSONL", languageDetector.detect(
    '{"id": 1, "name": "Alice"}\n{"id": 2, "name": "Bob"}\n{"id": 3, "name": "Charlie"}'
), "json");

assert("package.json style", languageDetector.detect(`{
  "name": "my-project",
  "version": "1.0.0",
  "dependencies": {
    "svelte": "^5.0.0",
    "typescript": "~5.6.2"
  }
}`), "json");

assert("JSONC with comments", languageDetector.detect(`{
  // A comment
  "compilerOptions": {
    "target": "es2020", /* inline comment */
    "module": "esnext"
  }
}`), "json");

// ════════════════════════════════════════════════════════════════
// Phase 3b — HTML Detection
// ════════════════════════════════════════════════════════════════

console.log("\n── Phase 3b: HTML Detection ──");

assert("DOCTYPE html", languageDetector.detect("<!DOCTYPE html>\n<html>\n<head></head>\n<body></body>\n</html>"), "html");
assert("DOCTYPE case insensitive", languageDetector.detect("<!doctype html>\n<html></html>"), "html");
assert("<html> tag", languageDetector.detect("<html>\n<head><title>Test</title></head>\n<body><div>Hello</div></body>\n</html>"), "html");
assert("HTML with multiple tags", languageDetector.detect("<div>\n<span>Hello</span>\n<script>alert(1)</script>\n<style>body{}</style>\n</div>"), "html");

// ════════════════════════════════════════════════════════════════
// Phase 3c — XML Detection  (THE BIG FIX)
// ════════════════════════════════════════════════════════════════

console.log("\n── Phase 3c: XML Detection ──");

assert("XML declaration", languageDetector.detect('<?xml version="1.0" encoding="UTF-8"?>\n<root>\n  <item>Test</item>\n</root>'), "xml");

assert("XML with xmlns", languageDetector.detect('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">\n  <circle cx="50" cy="50" r="40"/>\n</svg>'), "xml");

assert("XML without declaration", languageDetector.detect('<project>\n  <modelVersion>4.0.0</modelVersion>\n  <groupId>com.example</groupId>\n  <artifactId>demo</artifactId>\n</project>'), "xml");

assert("XML comment then tags", languageDetector.detect('<!-- Configuration file -->\n<config>\n  <setting name="debug" value="true"/>\n</config>'), "xml");

assert("XML with namespace prefix", languageDetector.detect('<ns:root>\n  <ns:child>value</ns:child>\n</ns:root>'), "xml");

assert("RSS feed", languageDetector.detect('<?xml version="1.0"?>\n<rss version="2.0">\n  <channel>\n    <title>Feed</title>\n  </channel>\n</rss>'), "xml");

// ── THE CRITICAL BUG FIX ──
// XML must NOT be detected as Markdown (the original bug)
assert("XML NOT detected as markdown", languageDetector.detect('<configuration>\n  <appSettings>\n    <add key="debug" value="true" />\n  </appSettings>\n</configuration>'), "xml");

assert("XML NOT detected as YAML", languageDetector.detect('<?xml version="1.0"?>\n<settings>\n  <item key="name">value</item>\n</settings>'), "xml");

// ════════════════════════════════════════════════════════════════
// Phase 3d — Dockerfile Detection
// ════════════════════════════════════════════════════════════════

console.log("\n── Phase 3d: Dockerfile Detection ──");

assert("Simple Dockerfile", languageDetector.detect(
    "FROM node:18-alpine\nWORKDIR /app\nCOPY package*.json ./\nRUN npm install\nCOPY . .\nEXPOSE 3000\nCMD [\"node\", \"server.js\"]"
), "dockerfile");

assert("Multi-stage Dockerfile", languageDetector.detect(
    "FROM node:18 AS builder\nWORKDIR /app\nCOPY . .\nRUN npm run build\n\nFROM nginx:alpine\nCOPY --from=builder /app/dist /usr/share/nginx/html"
), "dockerfile");

assert("Dockerfile with ARG first", languageDetector.detect(
    "ARG VERSION=latest\nFROM ubuntu:${VERSION}\nRUN apt-get update\nCMD [\"/bin/bash\"]"
), "dockerfile");

assert("Dockerfile with comments", languageDetector.detect(
    "# Build stage\nFROM golang:1.21 AS build\nRUN go build -o /app\n\n# Runtime\nFROM alpine:3.18\nCOPY --from=build /app /app\nENTRYPOINT [\"/app\"]"
), "dockerfile");

// ════════════════════════════════════════════════════════════════
// Phase 3e — CSV / TSV Detection
// ════════════════════════════════════════════════════════════════

console.log("\n── Phase 3e: CSV / TSV Detection ──");

assert("Simple CSV", languageDetector.detect("name,age,city\nAlice,30,NYC\nBob,25,LA\nCharlie,35,Chicago"), "csv");

assert("CSV with headers", languageDetector.detect(
    "id,first_name,last_name,email\n1,John,Doe,john@example.com\n2,Jane,Smith,jane@example.com"
), "csv");

assert("TSV (tab-separated)", languageDetector.detect(
    "name\tage\tcity\nAlice\t30\tNYC\nBob\t25\tLA"
), "csv");

assert("Semicolon-delimited", languageDetector.detect(
    "name;age;city\nAlice;30;NYC\nBob;25;LA"
), "csv");

assert("CSV with nested quotes", languageDetector.detect(
    'id,category\n979594,"ANZSIC06 divisions A-S (excluding classes K6330, L6711...)"\n979595,"Sales, government..., ANZSIC06..."'
), "csv");

// ════════════════════════════════════════════════════════════════
// Phase 3f — YAML Detection
// ════════════════════════════════════════════════════════════════

console.log("\n── Phase 3f: YAML Detection ──");

assert("YAML with ---", languageDetector.detect(
    "---\nname: my-project\nversion: 1.0.0\ndescription: A test project"
), "yaml");

assert("YAML key-value pairs", languageDetector.detect(
    "name: my-project\nversion: 1.0.0\nauthor: John Doe\nlicense: MIT\ndescription: A sample project"
), "yaml");

assert("YAML with nested keys", languageDetector.detect(
    "server:\n  host: localhost\n  port: 8080\n  debug: true\ndatabase:\n  url: postgres://localhost/db"
), "yaml");

assert("YAML with list items", languageDetector.detect(
    "dependencies:\n  - svelte\n  - typescript\n  - vite\ndevDependencies:\n  - vitest\n  - prettier"
), "yaml");

assert("GitHub Actions YAML", languageDetector.detect(
    "name: CI\non: [push, pull_request]\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4"
), "yaml");

// YAML should NOT misfire on Python or JS
assert("Python NOT detected as YAML", languageDetector.detect(
    "def hello():\n    print('hello world')\n\nif __name__ == '__main__':\n    hello()"
), "python");

// ════════════════════════════════════════════════════════════════
// Phase 3g — Markdown Detection
// ════════════════════════════════════════════════════════════════

console.log("\n── Phase 3g: Markdown Detection ──");

assert("Markdown with headings and lists", languageDetector.detect(
    "# My Project\n\nA great project.\n\n## Features\n\n- Fast\n- Reliable\n- Easy to use\n\n## Installation\n\n```bash\nnpm install my-project\n```"
), "markdown");

assert("Markdown with links and bold", languageDetector.detect(
    "# Getting Started\n\nVisit [our docs](https://example.com) for more info.\n\n**Important:** Read the README first.\n\n1. Clone the repo\n2. Run `npm install`\n3. Start coding"
), "markdown");

assert("Markdown with frontmatter", languageDetector.detect(
    "---\ntitle: My Blog Post\ndate: 2024-01-01\ntags: [tech, code]\n---\n\n# My Blog Post\n\nThis is the content of my blog post.\n\n## Section 1\n\nSome text here."
), "markdown");

assert("Markdown with table", languageDetector.detect(
    "# Data Table\n\n| Name | Age | City |\n|------|-----|------|\n| Alice | 30 | NYC |\n| Bob | 25 | LA |"
), "markdown");

// ════════════════════════════════════════════════════════════════
// Phase 4 — Heuristic Scoring (Programming Languages)
// ════════════════════════════════════════════════════════════════

console.log("\n── Phase 4: Heuristic Scoring ──");

assert("Python code", languageDetector.detect(`
import os
from pathlib import Path

def process_files(directory):
    for path in Path(directory).iterdir():
        if path.is_file():
            print(f"Processing {path}")

if __name__ == "__main__":
    process_files("/tmp")
`), "python");

assert("JavaScript code", languageDetector.detect(`
const express = require('express');
const app = express();

app.get('/', (req, res) => {
    res.json({ message: 'Hello World' });
});

module.exports = app;
`), "javascript");

assert("TypeScript code", languageDetector.detect(`
interface User {
    name: string;
    age: number;
    email: string;
}

type UserRole = 'admin' | 'user' | 'guest';

function getUser(id: number): Promise<User> {
    return fetch(\`/api/users/\${id}\`).then(r => r.json());
}

export const defaultRole: UserRole = 'user';
`), "typescript");

assert("CSS code", languageDetector.detect(`
.container {
    display: flex;
    justify-content: center;
    align-items: center;
    padding: 1rem;
}

#header {
    background: var(--primary-color);
    color: white;
}

@media (max-width: 768px) {
    .container {
        flex-direction: column;
    }
}
`), "css");

assert("Shell script (no shebang)", languageDetector.detect(`
export PATH="/usr/local/bin:$PATH"

if [[ -d "$HOME/.config" ]]; then
    echo "Config directory exists"
fi

for file in *.txt; do
    echo "Processing $file"
done
`), "shell");

assert("Java code", languageDetector.detect(`
import java.util.List;
import java.util.ArrayList;

public class Main {
    public static void main(String[] args) {
        List<String> items = new ArrayList<>();
        items.add("Hello");
        System.out.println(items);
    }
}
`), "java");

assert("Go code", languageDetector.detect(`
package main

import (
    "fmt"
    "net/http"
)

func handler(w http.ResponseWriter, r *http.Request) {
    fmt.Fprintf(w, "Hello, World!")
}

func main() {
    http.HandleFunc("/", handler)
    http.ListenAndServe(":8080", nil)
}
`), "go");

assert("C code", languageDetector.detect(`
#include <stdio.h>
#include <stdlib.h>

int main(int argc, char *argv[]) {
    char *buffer = malloc(1024);
    if (buffer == NULL) {
        printf("Failed to allocate memory\\n");
        return 1;
    }
    printf("Hello, World!\\n");
    free(buffer);
    return 0;
}
`), "c");

assert("C++ code", languageDetector.detect(`
#include <iostream>
#include <vector>
#include <string>

using namespace std;

int main() {
    vector<string> names = {"Alice", "Bob", "Charlie"};
    for (const auto& name : names) {
        cout << "Hello, " << name << endl;
    }
    return 0;
}
`), "cpp");

// ════════════════════════════════════════════════════════════════
// Edge Cases & Regression Tests
// ════════════════════════════════════════════════════════════════

console.log("\n── Edge Cases ──");

assert("Empty string",         languageDetector.detect(""),         null);
assert("Whitespace only",      languageDetector.detect("   \n\n  "), null);
assert("Very short content",   languageDetector.detect("hi"),       null);
assert("Single number",        languageDetector.detect("42"),       null);

// Ensure JS object literal is NOT detected as JSON
assert("JS object literal NOT json", languageDetector.detect(
    "const config = {\n  name: 'test',\n  version: '1.0'\n};\nmodule.exports = config;"
), "javascript");

// Markdown frontmatter is NOT YAML
assert("Markdown frontmatter is NOT yaml", languageDetector.detect(
    "---\ntitle: Test\ndate: 2024-01-01\n---\n\n# Hello World\n\nThis is a blog post with **bold** text."
), "markdown");

// BOM handling
assert("BOM prefix handling", languageDetector.detect(
    "\uFEFF" + '{"key": "value"}'
), "json");

// ════════════════════════════════════════════════════════════════
// Report
// ════════════════════════════════════════════════════════════════

console.log("\n════════════════════════════════════════");
console.log(`Results: ${passed} passed, ${failed} failed, ${passed + failed} total`);

if (failures.length > 0) {
    console.log("\nFailures:");
    for (const f of failures) console.log(f);
    process.exit(1);
} else {
    console.log("All tests passed ✓");
    process.exit(0);
}

} // end main()

main().catch(err => { console.error(err); process.exit(1); });
