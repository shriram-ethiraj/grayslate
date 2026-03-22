import type { Component } from "svelte";
import FileText from "~icons/lucide/file-text";
import FileCode from "~icons/lucide/file-code";
import Terminal from "~icons/lucide/terminal";
import Container from "~icons/lucide/container";

import SiJavascript from "~icons/simple-icons/javascript";
import SiTypescript from "~icons/simple-icons/typescript";
import SiPython from "~icons/simple-icons/python";
import SiHtml5 from "~icons/simple-icons/html5";
import SiCss from "~icons/simple-icons/css";
import SiYaml from "~icons/simple-icons/yaml";
import SiC from "~icons/simple-icons/c";
import SiCplusplus from "~icons/simple-icons/cplusplus";
import SiGo from "~icons/simple-icons/go";
import SiMarkdown from "~icons/simple-icons/markdown";
import SiJson from "~icons/simple-icons/json";
import SiOpenjdk from "~icons/simple-icons/openjdk";
import SiGooglesheets from "~icons/simple-icons/googlesheets";
import SiSvelte from "~icons/simple-icons/svelte";
import SiVuedotjs from "~icons/simple-icons/vuedotjs";
import SiRust from "~icons/simple-icons/rust";
import SiClojure from "~icons/simple-icons/clojure";
import SiPhp from "~icons/simple-icons/php";
import SiRuby from "~icons/simple-icons/ruby";
import SiSwift from "~icons/simple-icons/swift";
import SiKotlin from "~icons/simple-icons/kotlin";
import SiDart from "~icons/simple-icons/dart";
import SiCsharp from "~icons/simple-icons/csharp";
import SiScala from "~icons/simple-icons/scala";
import SiAngular from "~icons/simple-icons/angular";
import SiNginx from "~icons/simple-icons/nginx";
import SiToml from "~icons/simple-icons/toml";
import SiSass from "~icons/simple-icons/sass";
import SiJinja from "~icons/simple-icons/jinja";
import SiPowershell from "~icons/simple-icons/powershell";
import SiPerl from "~icons/simple-icons/perl";
import RiWindowsFill from '~icons/ri/windows-fill';

export type LanguageIcon = Component;

export interface Language {
    value: string;
    label: string;
    token: string;
    icon: LanguageIcon | null;
}

// Canonical meta object per language — shared across all extension aliases.
const JAVASCRIPT:   Language = { value: "javascript",   label: "JavaScript",   token: "JS",      icon: SiJavascript };
const TYPESCRIPT:   Language = { value: "typescript",   label: "TypeScript",   token: "TS",      icon: SiTypescript };
const PYTHON:       Language = { value: "python",       label: "Python",       token: "PY",      icon: SiPython };
const HTML:         Language = { value: "html",         label: "HTML",         token: "HTML",    icon: SiHtml5 };
const CSS:          Language = { value: "css",          label: "CSS",          token: "CSS",     icon: SiCss };
const YAML:         Language = { value: "yaml",         label: "YAML",         token: "YAML",    icon: SiYaml };
const C:            Language = { value: "c",            label: "C",            token: "C",       icon: SiC };
const CPP:          Language = { value: "cpp",          label: "C++",          token: "C++",     icon: SiCplusplus };
const JAVA:         Language = { value: "java",         label: "Java",         token: "JAVA",    icon: SiOpenjdk };
const GO:           Language = { value: "go",           label: "Go",           token: "GO",      icon: SiGo };
const XML:          Language = { value: "xml",          label: "XML",          token: "XML",     icon: FileCode };
const CSV:          Language = { value: "csv",          label: "CSV",          token: "CSV",     icon: SiGooglesheets };
const MARKDOWN:     Language = { value: "markdown",     label: "Markdown",     token: "MD",      icon: SiMarkdown };
const SHELL:        Language = { value: "shell",        label: "Shell",        token: "SH",      icon: Terminal };
const CMD:          Language = { value: "cmd",          label: "Batch",        token: "BAT",     icon: RiWindowsFill };
const DOCKERFILE:   Language = { value: "dockerfile",   label: "Dockerfile",   token: "DOCKER",  icon: Container };
const SVELTE:       Language = { value: "svelte",       label: "Svelte",       token: "SVELTE",  icon: SiSvelte };
const VUE:          Language = { value: "vue",          label: "Vue",          token: "VUE",     icon: SiVuedotjs };
const RUST:         Language = { value: "rust",         label: "Rust",         token: "RUST",    icon: SiRust };
const CLOJURE:      Language = { value: "clojure",      label: "Clojure",      token: "CLOJURE", icon: SiClojure };
const SQL:          Language = { value: "sql",          label: "SQL",          token: "SQL",     icon: FileCode };
const PHP:          Language = { value: "php",          label: "PHP",          token: "PHP",     icon: SiPhp };
const SASS:         Language = { value: "sass",         label: "Sass",         token: "SASS",    icon: SiSass };
const SCSS:         Language = { value: "scss",         label: "SCSS",         token: "SCSS",    icon: SiSass };
const JINJA:        Language = { value: "jinja",        label: "Jinja",        token: "JINJA",   icon: SiJinja };
const ANGULAR:      Language = { value: "angular",      label: "Angular",      token: "ANGULAR", icon: SiAngular };
const NGINX:        Language = { value: "nginx",        label: "Nginx",        token: "NGINX",   icon: SiNginx };
const POWERSHELL:   Language = { value: "powershell",   label: "PowerShell",   token: "PS1",     icon: SiPowershell };
const RUBY:         Language = { value: "ruby",         label: "Ruby",         token: "RUBY",    icon: SiRuby };
const SWIFT:        Language = { value: "swift",        label: "Swift",        token: "SWIFT",   icon: SiSwift };
const TOML:         Language = { value: "toml",         label: "TOML",         token: "TOML",    icon: SiToml };
const KOTLIN:       Language = { value: "kotlin",       label: "Kotlin",       token: "KOTLIN",  icon: SiKotlin };
const OBJECTIVEC:   Language = { value: "objectivec",   label: "Objective-C",  token: "OBJ-C",   icon: SiC };
const OBJECTIVECPP: Language = { value: "objectivecpp", label: "Objective-C++",token: "OBJ-C++", icon: SiCplusplus };
const CSHARP:       Language = { value: "csharp",       label: "C#",           token: "C#",      icon: SiCsharp };
const SCALA:        Language = { value: "scala",        label: "Scala",        token: "SCALA",   icon: SiScala };
const DART:         Language = { value: "dart",         label: "Dart",         token: "DART",    icon: SiDart };
const JSON:         Language = { value: "json",         label: "JSON",         token: "JSON",    icon: SiJson };
const PERL:         Language = { value: "perl",         label: "Perl",         token: "PERL",    icon: SiPerl };

// All known languages
const ALL_LANGUAGES: Language[] = [
    JAVASCRIPT, TYPESCRIPT, PYTHON, HTML, CSS, YAML,
    C, CPP, JAVA, GO, XML, CSV, MARKDOWN, SHELL, CMD, DOCKERFILE,
    SVELTE, VUE, RUST, CLOJURE, SQL, PHP, SASS, SCSS, JINJA,
    ANGULAR, NGINX, POWERSHELL, RUBY, SWIFT, TOML, KOTLIN,
    OBJECTIVEC, OBJECTIVECPP, CSHARP, SCALA, DART, JSON, PERL,
];

/**
 * All known languages sorted alphabetically for the language picker.
 * "Auto Detect" and "Plain text" are pinned at the top.
 */
export const languages: Language[] = [
    { value: "auto", label: "Auto Detect", token: "AUTO", icon: null },
    { value: "text", label: "Plain text",  token: "TXT",  icon: FileText },
    ...ALL_LANGUAGES.sort((a, b) => a.label.localeCompare(b.label)),
];
