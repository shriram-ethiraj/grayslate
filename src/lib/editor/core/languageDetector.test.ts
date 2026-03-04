/**
 * languageDetector.test.ts
 *
 * Unified, table-driven test suite for the language detector.
 *
 * Run: pnpm dlx tsx src/lib/editor/core/languageDetector.test.ts
 */

/* eslint-disable no-console */

let passed = 0;
let failed = 0;
const failures: string[] = [];

type Difficulty = "easy" | "medium" | "hard";

type DetectCase = {
    label: string;
    expected: string | null;
    content: string;
    filename?: string;
    difficulty: Difficulty;
};

type DetectPhase = {
    title: string;
    cases: DetectCase[];
};

function assert(label: string, actual: string | null, expected: string | null) {
    if (actual === expected) {
        passed++;
    } else {
        failed++;
        const msg = `  ✗ ${label}\n    expected: ${expected}\n    actual:   ${actual}`;
        failures.push(msg);
        console.error(msg);
    }
}

function runPhase(
    phase: DetectPhase,
    detect: (content: string, filename?: string) => string | null,
) {
    console.log(`\n── ${phase.title} ──`);
    for (const testCase of phase.cases) {
        const actual = detect(testCase.content, testCase.filename);
        assert(`[${testCase.difficulty}] ${testCase.label}`, actual, testCase.expected);
    }
}

const fileCase = (
    label: string,
    filename: string,
    expected: string,
    difficulty: Difficulty = "easy",
): DetectCase => ({
    label,
    expected,
    filename,
    content: "",
    difficulty,
});

const textCase = (
    label: string,
    content: string,
    expected: string | null,
    difficulty: Difficulty = "medium",
    filename?: string,
): DetectCase => ({
    label,
    content,
    expected,
    filename,
    difficulty,
});

async function main() {
    const { languageDetector } = await import("./languageDetector");

    const phase1ExtensionCases: DetectCase[] = [
        fileCase("JSON extension", "data.json", "json"),
        fileCase("JSONC extension", "tsconfig.jsonc", "json"),
        fileCase("YAML extension", "config.yaml", "yaml"),
        fileCase("YML extension", "config.yml", "yaml"),
        fileCase("XML extension", "pom.xml", "xml"),
        fileCase("SVG extension", "icon.svg", "xml"),
        fileCase("HTML extension", "index.html", "html"),
        fileCase("Markdown extension", "README.md", "markdown"),
        fileCase("JS extension", "app.js", "javascript"),
        fileCase("TS extension", "app.ts", "typescript"),
        fileCase("TSX extension", "App.tsx", "typescript"),
        fileCase("Python extension", "main.py", "python"),
        fileCase("CSS extension", "styles.css", "css"),
        fileCase("SCSS extension", "styles.scss", "scss"),
        fileCase("Sass extension", "styles.sass", "sass"),
        fileCase("C extension", "main.c", "c"),
        fileCase("C++ extension", "main.cpp", "cpp"),
        fileCase("Java extension", "Main.java", "java"),
        fileCase("Go extension", "main.go", "go"),
        fileCase("CSV extension", "data.csv", "csv"),
        fileCase("TSV extension", "data.tsv", "csv"),
        fileCase("Shell extension", "deploy.sh", "shell"),
        fileCase("Svelte extension", "App.svelte", "svelte"),
        fileCase("Vue extension", "App.vue", "vue"),
        fileCase("Rust extension", "main.rs", "rust"),
        fileCase("Clojure extension", "core.clj", "clojure"),
        fileCase("ClojureScript extension", "app.cljs", "clojure"),
        fileCase("EDN extension", "deps.edn", "clojure"),
        fileCase("Dockerfile name", "Dockerfile", "dockerfile"),
        fileCase("Dockerfile lowercase", "dockerfile", "dockerfile"),
        fileCase(".bashrc name", ".bashrc", "shell"),
        fileCase(".zshrc name", ".zshrc", "shell"),
        fileCase("SQL extension", "query.sql", "sql"),
        fileCase("PHP extension", "index.php", "php"),
        fileCase("PHP7 extension", "app.php7", "php"),
        fileCase("PHTML extension", "view.phtml", "php"),
        fileCase("Jinja extension", "template.j2", "jinja"),
        fileCase("Jinja2 extension", "base.jinja2", "jinja"),
        fileCase("PowerShell extension", "script.ps1", "powershell"),
        fileCase("PSM1 extension", "module.psm1", "powershell"),
        fileCase("Ruby extension", "app.rb", "ruby"),
        fileCase("Swift extension", "main.swift", "swift"),
        fileCase("TOML extension", "config.toml", "toml"),
        fileCase("Kotlin extension", "Main.kt", "kotlin"),
        fileCase("Kotlin script extension", "build.gradle.kts", "kotlin"),
        fileCase("C# extension", "Program.cs", "csharp"),
        fileCase("Scala extension", "App.scala", "scala"),
        fileCase("Dart extension", "main.dart", "dart"),
        fileCase("Obj-C++ extension", "bridge.mm", "objectivecpp"),
        fileCase("Nginx conf pattern", "nginx.conf", "nginx"),
        fileCase("Nginx prefix conf", "nginx-proxy.conf", "nginx"),
        fileCase("Gemfile name", "Gemfile", "ruby"),
        fileCase("Rakefile name", "Rakefile", "ruby"),
        fileCase("Cargo.toml name", "Cargo.toml", "toml"),
        textCase("Ruby shebang", "#!/usr/bin/env ruby\nputs 'hello'", "ruby", "medium"),
        textCase("PHP shebang", "#!/usr/bin/env php\n<?php echo 'hello'; ?>", "php", "medium"),
    ];

    const phase2ShebangCases: DetectCase[] = [
        textCase("Python shebang", "#!/usr/bin/env python3\nprint('hi')", "python", "easy"),
        textCase("Node shebang", "#!/usr/bin/env node\nconsole.log('hi')", "javascript", "easy"),
        textCase("Bash shebang", "#!/bin/bash\necho hello", "shell", "easy"),
        textCase("Sh shebang", "#!/bin/sh\necho hello", "shell", "easy"),
        textCase("Deno shebang", "#!/usr/bin/env deno\nconsole.log('hi')", "typescript", "medium"),
        textCase("Zsh shebang", "#!/usr/bin/env zsh\necho hello", "shell", "medium"),
    ];

    const phase3StructuralCases: DetectCase[] = [
        textCase("Simple JSON object", '{"test": 1}', "json", "easy"),
        textCase("JSON array", "[1, 2, 3]", "json", "easy"),
        textCase("Nested JSON", '{"a": {"b": [1, 2]}}', "json", "medium"),
        textCase("JSONC with comments", `{
  // A comment
  "compilerOptions": {
    "target": "es2020", /* inline comment */
    "module": "esnext"
  }
}`, "json", "hard"),

        textCase("DOCTYPE html", "<!DOCTYPE html>\n<html>\n<head></head>\n<body></body>\n</html>", "html", "easy"),
        textCase("HTML with multiple tags", "<div>\n<span>Hello</span>\n<script>alert(1)</script>\n<style>body{}</style>\n</div>", "html", "medium"),

        textCase("Svelte snippet", `
<script lang="ts">
  let count = $state(0);
</script>
{#if count > 0}
  <p>{count}</p>
{:else}
  <p>Zero</p>
{/if}
<button on:click={() => count++}>Click</button>
`, "svelte", "medium"),
        textCase("Vue SFC snippet", `
<template>
  <div v-if="show">
    <input v-model="name" />
    <button @click="submit">Send</button>
  </div>
</template>
<script setup>
import { ref } from 'vue';
const name = ref('');
</script>
`, "vue", "medium"),

        textCase("XML declaration", '<?xml version="1.0" encoding="UTF-8"?>\n<root>\n  <item>Test</item>\n</root>', "xml", "easy"),
        textCase("XML with namespace prefix", '<ns:root>\n  <ns:child>value</ns:child>\n</ns:root>', "xml", "medium"),
        textCase("XML NOT markdown", '<configuration>\n  <appSettings>\n    <add key="debug" value="true" />\n  </appSettings>\n</configuration>', "xml", "hard"),

        textCase("Simple Dockerfile", "FROM node:18-alpine\nWORKDIR /app\nCOPY package*.json ./\nRUN npm install\nCOPY . .\nEXPOSE 3000\nCMD [\"node\", \"server.js\"]", "dockerfile", "easy"),
        textCase("Multi-stage Dockerfile", "FROM node:18 AS builder\nWORKDIR /app\nCOPY . .\nRUN npm run build\n\nFROM nginx:alpine\nCOPY --from=builder /app/dist /usr/share/nginx/html", "dockerfile", "medium"),

        textCase("Simple CSV", "name,age,city\nAlice,30,NYC\nBob,25,LA\nCharlie,35,Chicago", "csv", "easy"),
        textCase("TSV", "name\tage\tcity\nAlice\t30\tNYC\nBob\t25\tLA", "csv", "medium"),
        textCase("Semicolon-delimited", "name;age;city\nAlice;30;NYC\nBob;25;LA", "csv", "hard"),

        textCase("YAML with ---", "---\nname: my-project\nversion: 1.0.0\ndescription: A test project", "yaml", "easy"),
        textCase("YAML nested keys", "server:\n  host: localhost\n  port: 8080\n  debug: true\ndatabase:\n  url: postgres://localhost/db", "yaml", "medium"),
        textCase("GitHub Actions YAML", "name: CI\non: [push, pull_request]\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4", "yaml", "hard"),

        textCase("Sass indented syntax", "$font-stack: Helvetica, sans-serif\n$primary-color: #333\n\nbody\n  font: 100% $font-stack\n  color: $primary-color", "sass", "easy"),
        textCase("SCSS braces syntax", "$font-stack: Helvetica, sans-serif;\n$primary-color: #333;\n\nbody {\n  font: 100% $font-stack;\n  color: $primary-color;\n}", "scss", "easy"),
        textCase("SCSS @mixin with braces", "@mixin flex-center {\n  display: flex;\n  align-items: center;\n  justify-content: center;\n}\n.card {\n  @include flex-center;\n}", "scss", "medium"),
        textCase("Sass @mixin indented", "@mixin respond-to($bp)\n  @media (min-width: $bp)\n    @content\n\n.hero\n  @include respond-to(768px)\n    font-size: 2rem", "sass", "medium"),
        textCase("SCSS @use + @forward", "@use 'sass:math';\n@forward 'variables';\n\n$ratio: math.div(16, 9);\n\n.video {\n  aspect-ratio: $ratio;\n}", "scss", "hard"),
        textCase("SCSS @extend chain", ".message {\n  border: 1px solid #ccc;\n  padding: 10px;\n}\n.success {\n  @extend .message;\n  border-color: green;\n}", "scss", "hard"),

        textCase("Markdown with headings", "# My Project\n\nA great project.\n\n## Features\n\n- Fast\n- Reliable\n- Easy to use", "markdown", "easy"),
        textCase("Markdown links and bold", "# Getting Started\n\nVisit [our docs](https://example.com).\n\n**Important:** Read the README first.", "markdown", "medium"),
        textCase("Markdown with table", "# Data Table\n\n| Name | Age | City |\n|------|-----|------|\n| Alice | 30 | NYC |\n| Bob | 25 | LA |", "markdown", "hard"),
    ];

    const phase4LanguageCases: Array<{
        languageLabel: string;
        expected: string;
        variations: Array<{ difficulty: Difficulty; label: string; content: string }>;
    }> = [
        {
            languageLabel: "Python",
            expected: "python",
            variations: [
                { difficulty: "easy", label: "Function", content: "def add(a, b):\n    return a + b" },
                { difficulty: "medium", label: "Class with self", content: "class User:\n    def __init__(self, name):\n        self.name = name\n\n    def greet(self):\n        print(self.name)" },
                { difficulty: "hard", label: "Decorator + context manager", content: "from pathlib import Path\n\n@staticmethod\ndef load(path):\n    with open(path, 'r') as handle:\n        return handle.read()\n\nif __name__ == '__main__':\n    print(load('notes.txt'))" },
            ],
        },
        {
            languageLabel: "JavaScript",
            expected: "javascript",
            variations: [
                { difficulty: "easy", label: "Const + function", content: "const value = 1;\nfunction inc(n) {\n    return n + 1;\n}\nconsole.log(inc(value));" },
                { difficulty: "medium", label: "CommonJS module", content: "const fs = require('fs');\nmodule.exports = function read(file) {\n    return fs.readFileSync(file, 'utf8');\n};" },
                { difficulty: "hard", label: "Async pipeline", content: "const loadAll = async (ids) => {\n    const rows = await Promise.all(ids.map((id) => fetch('/api/' + id).then((r) => r.json())));\n    return rows;\n};\nloadAll([1, 2]).catch(console.error);" },
            ],
        },
        {
            languageLabel: "TypeScript",
            expected: "typescript",
            variations: [
                { difficulty: "easy", label: "Interface", content: "interface User {\n    name: string;\n}\nconst user: User = { name: 'a' };" },
                { difficulty: "medium", label: "Type alias + enum", content: "type Id = string | number;\nenum Role { Admin, User }\nconst current: Id = '42';\nconst role = Role.Admin as Role;" },
                { difficulty: "hard", label: "Generics + utility types", content: "declare function pick<T extends object, K extends keyof T>(obj: T, key: K): Pick<T, K>;\nconst source: Record<string, number> = { a: 1 };\nconst value = pick(source, 'a');" },
            ],
        },
        {
            languageLabel: "CSS",
            expected: "css",
            variations: [
                { difficulty: "easy", label: "Class selector", content: ".card {\n    margin: 1rem;\n    padding: 1rem;\n}" },
                { difficulty: "medium", label: "Media query + custom property", content: ":root { --accent: #0ea5e9; }\n@media (max-width: 768px) {\n    .card { color: var(--accent); }\n}" },
                { difficulty: "hard", label: "Animation + pseudo", content: "@keyframes fadeIn {\n    from { opacity: 0; }\n    to { opacity: 1; }\n}\n.button:hover::before {\n    content: '';\n    animation: fadeIn 200ms ease-in;\n}" },
            ],
        },
        {
            languageLabel: "Shell",
            expected: "shell",
            variations: [
                { difficulty: "easy", label: "Export + echo", content: "export APP_ENV=dev\necho \"env=$APP_ENV\"" },
                { difficulty: "medium", label: "If + loop", content: "if [[ -d \"$HOME\" ]]; then\n    echo \"home ok\"\nfi\nfor file in *.txt; do\n    echo \"$file\"\ndone" },
                { difficulty: "hard", label: "Case + substitution", content: "case \"$1\" in\n  start)\n    echo \"starting $(date)\"\n    ;;\n  *)\n    echo \"unknown\"\n    ;;\nesac" },
            ],
        },
        {
            languageLabel: "Java",
            expected: "java",
            variations: [
                { difficulty: "easy", label: "Main class", content: "public class Main {\n    public static void main(String[] args) {\n        System.out.println(\"hi\");\n    }\n}" },
                { difficulty: "medium", label: "Imports + list", content: "import java.util.List;\nimport java.util.ArrayList;\nclass Demo {\n    void run() {\n        List<String> names = new ArrayList<>();\n    }\n}" },
                { difficulty: "hard", label: "Override + throws", content: "import javax.sql.DataSource;\nclass Repo extends BaseRepo {\n    @Override\n    public String load() throws Exception {\n        return \"ok\";\n    }\n}" },
            ],
        },
        {
            languageLabel: "Go",
            expected: "go",
            variations: [
                { difficulty: "easy", label: "Package main", content: "package main\n\nimport \"fmt\"\n\nfunc main() {\n    fmt.Println(\"hi\")\n}" },
                { difficulty: "medium", label: "Goroutine + channel", content: "package main\n\nfunc main() {\n    ch := make(chan int)\n    go func() { ch <- 1 }()\n    <-ch\n}" },
                { difficulty: "hard", label: "Receiver + defer", content: "package api\n\nimport (\n    \"fmt\"\n)\n\ntype S struct{}\n\nfunc (s *S) Run() {\n    defer fmt.Println(\"done\")\n    fmt.Println(\"run\")\n}" },
            ],
        },
        {
            languageLabel: "C",
            expected: "c",
            variations: [
                { difficulty: "easy", label: "stdio main", content: "#include <stdio.h>\nint main() {\n    printf(\"ok\\n\");\n    return 0;\n}" },
                { difficulty: "medium", label: "malloc + free", content: "#include <stdlib.h>\nvoid run() {\n    char *p = malloc(16);\n    if (p == NULL) return;\n    free(p);\n}" },
                { difficulty: "hard", label: "typedef struct", content: "#define LIMIT 10\ntypedef struct Node {\n    int value;\n} Node;\nsize_t size_of_node() {\n    return sizeof(Node);\n}" },
            ],
        },
        {
            languageLabel: "C++",
            expected: "cpp",
            variations: [
                { difficulty: "easy", label: "iostream", content: "#include <iostream>\nint main() {\n    std::cout << \"hi\" << std::endl;\n    return 0;\n}" },
                { difficulty: "medium", label: "vector + auto", content: "#include <vector>\n#include <string>\nint main() {\n    auto names = std::vector<std::string>{\"a\", \"b\"};\n    return 0;\n}" },
                { difficulty: "hard", label: "template + virtual", content: "template <typename T>\nclass Box {\npublic:\n    virtual T get() const = 0;\n};\nvoid use_ptr(Box<int>* box) {\n    auto value = box->get();\n}" },
            ],
        },
        {
            languageLabel: "Rust",
            expected: "rust",
            variations: [
                { difficulty: "easy", label: "println macro", content: "fn main() {\n    println!(\"hello\");\n}" },
                { difficulty: "medium", label: "Struct + impl", content: "pub struct Point { x: i32, y: i32 }\nimpl Point {\n    pub fn norm(&self) -> i32 {\n        self.x + self.y\n    }\n}" },
                { difficulty: "hard", label: "Trait + match + Result", content: "use std::io::Result;\ntrait Render { fn draw(&self); }\nfn run(flag: bool) -> Result<()> {\n    match flag {\n        true => println!(\"ok\"),\n        false => println!(\"skip\"),\n    }\n    Ok(())\n}" },
            ],
        },
        {
            languageLabel: "Clojure",
            expected: "clojure",
            variations: [
                { difficulty: "easy", label: "ns + defn", content: "(ns demo.core)\n(defn greet [name] (str \"hi \" name))" },
                { difficulty: "medium", label: "let + threading", content: "(defn normalize [xs]\n  (let [items (map inc xs)]\n    (-> items\n        (assoc 0 99))))" },
                { difficulty: "hard", label: "cond + reduce + keyword", content: "(defn classify [x]\n  (cond\n    (< x 0) :neg\n    (= x 0) :zero\n    :else :pos))\n(reduce + [1 2 3])" },
            ],
        },
        {
            languageLabel: "SQL",
            expected: "sql",
            variations: [
                { difficulty: "easy", label: "Select", content: "SELECT id, name FROM users WHERE active = 1;" },
                { difficulty: "medium", label: "DDL", content: "CREATE TABLE logs (id INTEGER PRIMARY KEY, message VARCHAR(255) NOT NULL);" },
                { difficulty: "hard", label: "Union + grouping", content: "SELECT city FROM users\nUNION ALL SELECT city FROM leads\nGROUP BY city\nHAVING COUNT(*) > 1\nORDER BY city;" },
            ],
        },
        {
            languageLabel: "PHP",
            expected: "php",
            variations: [
                { difficulty: "easy", label: "Opening tag", content: "<?php\n$name = 'World';\necho $name;" },
                { difficulty: "medium", label: "Namespace + class", content: "<?php\nnamespace App\\Services;\nuse App\\Models\\User;\nclass UserService {\n    public function find() {\n        return new User();\n    }\n}" },
                { difficulty: "hard", label: "Superglobals + array", content: "<?php\nfunction inputName() {\n    if (isset($_POST['name'])) {\n        return $_POST['name'];\n    }\n    return array('fallback')[0];\n}\necho inputName();" },
            ],
        },
        {
            languageLabel: "Ruby",
            expected: "ruby",
            variations: [
                { difficulty: "easy", label: "Method + puts", content: "def greet(name)\n  puts name\nend" },
                { difficulty: "medium", label: "Class + attr_accessor", content: "class User\n  attr_accessor :name\n  def initialize(name)\n    @name = name\n  end\nend" },
                { difficulty: "hard", label: "Module + require_relative + block", content: "require_relative 'helpers'\nmodule Repo\n  def self.fetch(ids)\n    ids.map do |id|\n      puts id\n    end unless ids.nil?\n  end\nend" },
            ],
        },
        {
            languageLabel: "Swift",
            expected: "swift",
            variations: [
                { difficulty: "easy", label: "Import + func", content: "import Foundation\nfunc greet() {\n    print(\"hello\")\n}" },
                { difficulty: "medium", label: "Class + IBOutlet", content: "import UIKit\nclass ViewController: UIViewController {\n    @IBOutlet weak var label: UILabel!\n    func render() {\n        guard let text = Optional(\"ok\") else { return }\n        label.text = text\n    }\n}" },
                { difficulty: "hard", label: "Protocol + extension", content: "protocol Cacheable {}\nextension String: Cacheable {}\nfunc resolve(value: String?) {\n    if let unwrapped = value {\n        print(unwrapped)\n    }\n}" },
            ],
        },
        {
            languageLabel: "Kotlin",
            expected: "kotlin",
            variations: [
                { difficulty: "easy", label: "Main + val", content: "fun main() {\n    val name = \"A\"\n    println(name)\n}" },
                { difficulty: "medium", label: "Data + sealed classes", content: "data class User(val name: String)\nsealed class Result {\n    data class Success(val value: String): Result()\n}\nval items = listOf(User(\"A\"))" },
                { difficulty: "hard", label: "Companion + when", content: "package demo.core\nclass Parser {\n    companion object {\n        fun create(): Parser = Parser()\n    }\n}\nfun toText(value: Int): String = when(value) {\n    0 -> \"zero\"\n    else -> \"other\"\n}" },
            ],
        },
        {
            languageLabel: "C#",
            expected: "csharp",
            variations: [
                { difficulty: "easy", label: "Using + Main", content: "using System;\npublic class Program {\n    static void Main(string[] args) {\n        Console.WriteLine(\"hi\");\n    }\n}" },
                { difficulty: "medium", label: "Namespace + LINQ", content: "using System.Linq;\nusing System.Collections.Generic;\nnamespace Demo.App {\n    public class Query {\n        public IEnumerable<int> Even(List<int> values) {\n            return values.Where(v => v % 2 == 0);\n        }\n    }\n}" },
                { difficulty: "hard", label: "Attribute + async Task", content: "using System;\nusing System.Threading.Tasks;\n[Obsolete]\npublic class Runner {\n    public async Task<int> RunAsync() {\n        await Task.Delay(1);\n        return 1;\n    }\n}" },
            ],
        },
        {
            languageLabel: "Scala",
            expected: "scala",
            variations: [
                { difficulty: "easy", label: "Object + println", content: "object Main extends App {\n  val name = \"x\"\n  println(name)\n}" },
                { difficulty: "medium", label: "Case class", content: "package demo.core\ncase class User(name: String, age: Int)\nobject Repo {\n  val users = List(User(\"A\", 1))\n}" },
                { difficulty: "hard", label: "Sealed trait + implicit + match", content: "sealed trait Event\ncase class Created(id: Int) extends Event\nobject Handler {\n  implicit val ordering: Ordering[Int] = Ordering.Int\n  def apply(event: Event): String = event match {\n    case Created(id) =>\n      s\"$id\"\n  }\n}" },
            ],
        },
        {
            languageLabel: "Dart",
            expected: "dart",
            variations: [
                { difficulty: "easy", label: "Main function", content: "void main() {\n  final name = 'World';\n  print(name);\n}" },
                { difficulty: "medium", label: "Flutter widget", content: "import 'package:flutter/material.dart';\nclass MyApp extends StatelessWidget {\n  @override\n  Widget build(BuildContext context) {\n    return const Placeholder();\n  }\n}" },
                { difficulty: "hard", label: "Late + required + Future", content: "class User {\n  late final String id;\n  User({required this.id});\n}\nFuture<String> load() async {\n  return 'ok';\n}" },
            ],
        },
        {
            languageLabel: "PowerShell",
            expected: "powershell",
            variations: [
                { difficulty: "easy", label: "Verb-Noun function", content: "function Get-UserName {\n    param([string]$Name)\n    Write-Host $Name\n}" },
                { difficulty: "medium", label: "CmdletBinding + pipeline", content: "function Get-Accounts {\n    [CmdletBinding()]\n    param()\n    Get-Process | Where-Object { $_.Name -ne '' } | Select-Object Name\n}" },
                { difficulty: "hard", label: "PSVersion + operators", content: "$version = $PSVersionTable.PSVersion\nif ($version.Major -ge 7) {\n    Set-Item -Path Env:MODE -Value 'modern'\n}\nInvoke-Command -ScriptBlock { Write-Output 'done' }" },
            ],
        },
        {
            languageLabel: "TOML",
            expected: "toml",
            variations: [
                { difficulty: "easy", label: "Simple table", content: "[app]\nname = \"grayslate\"\nversion = \"0.1.0\"" },
                { difficulty: "medium", label: "Nested sections", content: "[server]\nhost = \"localhost\"\nport = 8080\n\n[server.tls]\nenabled = true" },
                { difficulty: "hard", label: "Inline tables + arrays", content: "[package]\nname = \"my-app\"\nfeatures = [\"csv\", \"markdown\"]\n\n[dependencies]\ntokio = { version = \"1\", features = [\"rt-multi-thread\", \"macros\"] }" },
            ],
        },
    ];

    const phase4Cases: DetectCase[] = phase4LanguageCases.flatMap(({ languageLabel, expected, variations }) =>
        variations.map(variation =>
            textCase(
                `${languageLabel} — ${variation.label}`,
                variation.content,
                expected,
                variation.difficulty,
            ),
        ),
    );

    const edgeCases: DetectCase[] = [
        textCase("Empty string", "", null, "easy"),
        textCase("Whitespace only", "   \n\n  ", null, "easy"),
        textCase("Very short content", "hi", null, "easy"),
        textCase("Single number", "42", null, "easy"),

        textCase(
            "JS object literal NOT json",
            "const config = {\n  name: 'test',\n  version: '1.0'\n};\nmodule.exports = config;",
            "javascript",
            "medium",
        ),
        textCase(
            "TS NOT JS",
            "interface Config { port: number; host: string; }\nconst config: Config = { port: 3000, host: \"localhost\" };\nexport type Mode = \"dev\" | \"prod\";",
            "typescript",
            "medium",
        ),
        textCase(
            "CSS NOT HTML",
            ".container { display: flex; }\n@media (max-width: 768px) { .container { flex-direction: column; } }\n:root { --primary: #3b82f6; }",
            "css",
            "medium",
        ),
        textCase(
            "Markdown frontmatter NOT yaml",
            "---\ntitle: Test\ndate: 2024-01-01\n---\n\n# Hello World\n\nThis is a blog post with **bold** text.",
            "markdown",
            "hard",
        ),
        textCase("BOM prefix handling", "\uFEFF" + '{"key": "value"}', "json", "hard"),

        // Sass/SCSS must NOT be misclassified as TOML or YAML
        textCase(
            "Sass vars NOT toml",
            "$spacing: 8px\n$radius: 4px\n\n.card\n  padding: $spacing\n  border-radius: $radius",
            "sass",
            "hard",
        ),
        textCase(
            "SCSS vars NOT yaml",
            "$primary: #0ea5e9;\n$secondary: #64748b;\n\n.button {\n  background: $primary;\n  color: $secondary;\n}",
            "scss",
            "hard",
        ),
    ];

    const phases: DetectPhase[] = [
        { title: "Phase 1: Extension Detection", cases: phase1ExtensionCases },
        { title: "Phase 2: Shebang Detection", cases: phase2ShebangCases },
        { title: "Phase 3: Structural Detection", cases: phase3StructuralCases },
        { title: "Phase 4: Heuristic Scoring", cases: phase4Cases },
        { title: "Edge Cases", cases: edgeCases },
    ];

    for (const phase of phases) {
        runPhase(phase, languageDetector.detect.bind(languageDetector));
    }

    // ── Phase 5: Real Project File Detection (content-only) ──────────
    // Walk the project tree, read each file's content, and verify our
    // detector can identify the language WITHOUT seeing the filename.
    {
        const fs = await import("fs");
        const pathMod = await import("path");

        /** Map file extensions to the expected language ID for content-only detection. */
        const EXT_TO_LANG: Record<string, string> = {
            ".json": "json",
            ".yaml": "yaml", ".yml": "yaml",
            ".toml": "toml",
            ".xml": "xml", ".svg": "xml",
            ".html": "html", ".htm": "html",
            ".css": "css",
            ".scss": "scss", ".sass": "sass",
            ".js": "javascript", ".mjs": "javascript", ".cjs": "javascript",
            ".ts": "typescript", ".tsx": "typescript", ".mts": "typescript",
            ".svelte": "svelte",
            ".vue": "vue",
            ".py": "python",
            ".rs": "rust",
            ".go": "go",
            ".java": "java",
            ".c": "c",
            ".cpp": "cpp", ".cxx": "cpp", ".cc": "cpp",
            ".rb": "ruby",
            ".php": "php",
            ".swift": "swift",
            ".kt": "kotlin", ".kts": "kotlin",
            ".cs": "csharp",
            ".scala": "scala",
            ".dart": "dart",
            ".sh": "shell", ".bash": "shell", ".zsh": "shell",
            ".ps1": "powershell",
            ".sql": "sql",
            ".md": "markdown",
            ".dockerfile": "dockerfile",
            ".clj": "clojure", ".cljs": "clojure",
        };

        /**
         * Extensions where content-only detection commonly produces an
         * acceptable alternative (e.g. .h → "c" or "cpp", .js → "javascript"
         * or "typescript"). Key = extension, value = set of accepted IDs.
         */
        const ACCEPTABLE_ALTS: Record<string, Set<string>> = {
            ".h":   new Set(["c", "cpp"]),
            ".hpp": new Set(["c", "cpp"]),
            ".js":  new Set(["javascript", "typescript"]),
            ".mjs": new Set(["javascript", "typescript"]),
            ".cjs": new Set(["javascript", "typescript"]),
            ".ts":  new Set(["javascript", "typescript"]),
            ".tsx": new Set(["javascript", "typescript"]),
            ".mts": new Set(["javascript", "typescript"]),
        };

        /** Directories to skip entirely. */
        const SKIP_DIRS = new Set([
            "node_modules", ".svelte-kit", "build", "target",
            ".git", ".vscode", ".idea", "dist", ".agents",
        ]);

        /** Extensions that are binary or not meaningfully detectable. */
        const SKIP_EXTENSIONS = new Set([
            ".png", ".jpg", ".jpeg", ".gif", ".ico", ".webp", ".avif",
            ".woff", ".woff2", ".ttf", ".eot", ".otf",
            ".zip", ".gz", ".tar", ".br",
            ".exe", ".dll", ".so", ".dylib",
            ".lock", ".map",
            ".d",  // Rust dep-info files
        ]);

        /** Files to skip by exact name. */
        const SKIP_FILES = new Set([
            "pnpm-lock.yaml", "package-lock.json", "yarn.lock",
            "pnpm-workspace.yaml",
        ]);

        const MIN_CONTENT_LENGTH = 20; // bytes — skip trivially small files

        /** Recursively collect file paths under `dir`. */
        function walk(dir: string): string[] {
            const results: string[] = [];
            for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
                const name = entry.name;
                // Skip hidden files/dirs (starting with .)
                if (name.startsWith(".")) continue;
                const fullPath = pathMod.join(dir, name);
                if (entry.isDirectory()) {
                    if (SKIP_DIRS.has(name)) continue;
                    results.push(...walk(fullPath));
                } else if (entry.isFile()) {
                    results.push(fullPath);
                }
            }
            return results;
        }

        // Resolve project root (four levels up from this test file's dir)
        const { fileURLToPath } = await import("url");
        const thisDir = pathMod.dirname(fileURLToPath(import.meta.url));
        const projectRoot = pathMod.resolve(thisDir, "../../../..");
        const allFiles = walk(projectRoot);

        let projPassed = 0;
        let projFailed = 0;
        let projSkipped = 0;
        const projFailures: string[] = [];

        console.log(`\n── Phase 5: Real Project Files (content-only) ──`);

        for (const filePath of allFiles) {
            const basename = pathMod.basename(filePath);
            const ext = pathMod.extname(filePath).toLowerCase();
            const relPath = pathMod.relative(projectRoot, filePath).replace(/\\/g, "/");

            // Skip conditions
            if (SKIP_FILES.has(basename)) { projSkipped++; continue; }
            if (SKIP_EXTENSIONS.has(ext)) { projSkipped++; continue; }
            if (!ext && !["Dockerfile", "Makefile", "Gemfile", "Rakefile"].includes(basename)) {
                projSkipped++; continue;
            }

            const expectedLang = EXT_TO_LANG[ext]
                ?? (basename.toLowerCase() === "dockerfile" ? "dockerfile" : undefined);

            // No mapping for this extension — skip
            if (!expectedLang) { projSkipped++; continue; }

            let content: string;
            try {
                content = fs.readFileSync(filePath, "utf8");
            } catch {
                projSkipped++;
                continue;
            }

            if (content.length < MIN_CONTENT_LENGTH) { projSkipped++; continue; }

            // Detect based on content ONLY — no filename hint
            const detected = languageDetector.detect(content);
            const alts = ACCEPTABLE_ALTS[ext];
            const isMatch = detected === expectedLang
                || (alts !== undefined && detected !== null && alts.has(detected));

            if (isMatch) {
                projPassed++;
            } else {
                projFailed++;
                const msg = `  ✗ ${relPath}\n    expected: ${expectedLang}${alts ? ` (or ${[...alts].join("/")})` : ""}\n    actual:   ${detected}`;
                projFailures.push(msg);
                console.error(msg);
            }
        }

        console.log(`  Scanned ${allFiles.length} files — tested ${projPassed + projFailed}, skipped ${projSkipped}`);
        console.log(`  Content-only results: ${projPassed} passed, ${projFailed} failed`);

        // Merge into global counters
        passed += projPassed;
        failed += projFailed;
        failures.push(...projFailures);
    }

    console.log("\n════════════════════════════════════════");
    console.log(`Results: ${passed} passed, ${failed} failed, ${passed + failed} total`);

    if (failures.length > 0) {
        console.log("\nFailures:");
        for (const failure of failures) console.log(failure);
        process.exit(1);
    }

    console.log("All tests passed ✓");
    process.exit(0);
}

main().catch(err => {
    console.error(err);
    process.exit(1);
});
