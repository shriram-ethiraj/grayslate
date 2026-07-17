import { $, expect } from "@wdio/globals";
import {
  clickTestId,
  invokeInApp,
  openExternalFixture,
  waitForDetectedLanguage,
  waitForLanguageMode,
} from "../helpers/app.js";

interface LanguageCase {
  language: string;
  filename?: string;
  content: string;
  extension: string;
}

interface SuggestResult {
  filename: string;
  detectedLanguage: string;
}

const languageCases: LanguageCase[] = [
  { language: "rust", filename: "sample.rs", content: "pub struct Widget;", extension: "rs" },
  { language: "c", filename: "sample.c", content: "int main(void) { return 0; }", extension: "c" },
  { language: "cpp", filename: "sample.cpp", content: "namespace demo { class Widget {}; }", extension: "cpp" },
  { language: "csharp", filename: "sample.cs", content: "public class Widget {}", extension: "cs" },
  { language: "java", filename: "sample.java", content: "public class Widget {}", extension: "java" },
  { language: "kotlin", filename: "sample.kt", content: "data class Widget(val id: Int)", extension: "kt" },
  { language: "scala", filename: "sample.scala", content: "case class Widget(id: Int)", extension: "scala" },
  { language: "dart", filename: "sample.dart", content: "class Widget {}", extension: "dart" },
  { language: "swift", filename: "sample.swift", content: "struct Widget { let id: Int }", extension: "swift" },
  { language: "objectivec", filename: "sample.m", content: "@interface Widget : NSObject @end", extension: "m" },
  {
    language: "objectivecpp",
    filename: "sample.mm",
    content: "#import <Foundation/Foundation.h>\n@interface Widget : NSObject @end\nnamespace demo { template <typename T> class NativeWidget { std::vector<T> values; }; }",
    extension: "mm",
  },
  { language: "go", filename: "sample.go", content: "package main\nfunc main() {}", extension: "go" },
  { language: "python", filename: "sample.py", content: "def widget():\n    return 1", extension: "py" },
  { language: "ruby", filename: "sample.rb", content: "class Widget\nend", extension: "rb" },
  { language: "perl", filename: "sample.pl", content: "sub widget { return 1; }", extension: "pl" },
  { language: "php", filename: "sample.php", content: "<?php class Widget {}", extension: "php" },
  { language: "javascript", filename: "sample.js", content: "export function widget() {}", extension: "js" },
  { language: "typescript", filename: "sample.ts", content: "export interface Widget { id: number }", extension: "ts" },
  { language: "svelte", filename: "sample.svelte", content: "<script>let value = 1;</script><p>{value}</p>", extension: "svelte" },
  { language: "vue", filename: "sample.vue", content: "<template><main>Widget</main></template>", extension: "vue" },
  {
    language: "angular",
    content: "import { Component, OnInit } from '@angular/core';\n@Component({ selector: 'app-widget', templateUrl: './widget.html' })\nexport class WidgetComponent implements OnInit { ngOnInit(): void {} }",
    extension: "angular",
  },
  { language: "html", filename: "sample.html", content: "<!doctype html><title>Widget</title>", extension: "html" },
  { language: "css", filename: "sample.css", content: ".widget { color: red; }", extension: "css" },
  { language: "scss", filename: "sample.scss", content: "$color: red; .widget { color: $color; }", extension: "scss" },
  { language: "sass", filename: "sample.sass", content: "$color: red\n.widget\n  color: $color", extension: "sass" },
  { language: "json", filename: "sample.json", content: "{\"widget\":true}", extension: "json" },
  { language: "yaml", filename: "sample.yaml", content: "name: widget\nenabled: true", extension: "yaml" },
  { language: "toml", filename: "sample.toml", content: "[package]\nname = \"widget\"", extension: "toml" },
  { language: "xml", filename: "sample.xml", content: "<?xml version=\"1.0\"?><widget />", extension: "xml" },
  { language: "sql", filename: "sample.sql", content: "SELECT id FROM widgets;", extension: "sql" },
  { language: "markdown", filename: "sample.md", content: "# Widget\n\nDescription", extension: "md" },
  { language: "csv", filename: "sample.csv", content: "id,name\n1,Widget", extension: "csv" },
  { language: "shell", filename: "sample.sh", content: "#!/bin/sh\necho widget", extension: "sh" },
  { language: "powershell", filename: "sample.ps1", content: "function Get-Widget { Write-Output 'widget' }", extension: "ps1" },
  { language: "cmd", filename: "sample.bat", content: "@echo off\necho widget", extension: "bat" },
  { language: "dockerfile", filename: "Dockerfile", content: "FROM alpine:latest\nRUN echo widget", extension: "dockerfile" },
  { language: "nginx", filename: "nginx.conf", content: "server { listen 8080; }", extension: "conf" },
  { language: "jinja", filename: "sample.j2", content: "{% for item in items %}{{ item }}{% endfor %}", extension: "j2" },
  { language: "clojure", filename: "sample.clj", content: "(defn widget [] 1)", extension: "clj" },
  { language: "email", content: "Subject: Project update\n\nHi team,\nPlease review the update.\nRegards,\nAlex", extension: "txt" },
  { language: "prompt", content: "You are a code reviewer. Summarize the findings as JSON.", extension: "txt" },
  { language: "text", filename: "sample.ini", content: "A short plain note about the project.", extension: "txt" },
];

describe("Act 3 — language recognition", () => {
  it("covers every detector language and canonical naming extension through real IPC", async () => {
    for (const testCase of languageCases) {
      const detected = await invokeInApp<string | null>("detect_language", {
        content: testCase.content,
        filename: testCase.filename,
      });
      expect(detected).toBe(testCase.language);

      const suggestion = await invokeInApp<SuggestResult>("suggest_slate_name", {
        content: testCase.content,
        languageHint: testCase.language,
      });
      expect(suggestion.detectedLanguage).toBe(testCase.language);
      expect(suggestion.filename.endsWith(`.${testCase.extension}`)).toBe(true);
      if (testCase.language === "email") expect(suggestion.filename).toContain("-email.txt");
      if (testCase.language === "prompt") expect(suggestion.filename).toContain("-prompt.txt");
    }

    expect(await invokeInApp<string | null>("detect_language", {
      content: "value item thing",
      filename: undefined,
    })).toBeNull();
  });

  it("manually overrides the editor language and returns to auto detection", async () => {
    await openExternalFixture("sample.py");
    await waitForDetectedLanguage("python");
    await waitForLanguageMode("python");

    await clickTestId("language-mode");
    await (await $("[data-testid='language-picker-dialog']")).waitForDisplayed();
    await clickTestId("language-item-json");
    await waitForLanguageMode("json");

    await clickTestId("language-mode");
    await clickTestId("language-item-auto");
    await waitForLanguageMode("auto");
    await waitForDetectedLanguage("python");
  });
});
