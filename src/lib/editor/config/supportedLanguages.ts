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

export type LanguageIcon = Component;

export interface Language {
    value: string;
    label: string;
    token: string;
    icon: LanguageIcon | null;
}

const rawLanguages: Language[] = [
    { value: "auto", label: "Auto Detect", token: "AUTO", icon: null },
    { value: "text", label: "Plain text", token: "TXT", icon: FileText },
    { value: "json", label: "JSON", token: "JSON", icon: SiJson },
    { value: "javascript", label: "JavaScript", token: "JS", icon: SiJavascript },
    { value: "typescript", label: "TypeScript", token: "TS", icon: SiTypescript },
    { value: "python", label: "Python", token: "PY", icon: SiPython },
    { value: "html", label: "HTML", token: "HTML", icon: SiHtml5 },
    { value: "css", label: "CSS", token: "CSS", icon: SiCss },
    { value: "yaml", label: "YAML", token: "YAML", icon: SiYaml },
    { value: "c", label: "C", token: "C", icon: SiC },
    { value: "cpp", label: "C++", token: "C++", icon: SiCplusplus },
    { value: "java", label: "Java", token: "JAVA", icon: SiOpenjdk },
    { value: "go", label: "Go", token: "GO", icon: SiGo },
    { value: "xml", label: "XML", token: "XML", icon: FileCode },
    { value: "csv", label: "CSV", token: "CSV", icon: SiGooglesheets },
    { value: "markdown", label: "Markdown", token: "MD", icon: SiMarkdown },
    { value: "shell", label: "Shell", token: "SH", icon: Terminal },
    { value: "dockerfile", label: "Dockerfile", token: "DOCKER", icon: Container },
    { value: "svelte", label: "Svelte", token: "SVELTE", icon: SiSvelte },
    { value: "vue", label: "Vue", token: "VUE", icon: SiVuedotjs },
    { value: "rust", label: "Rust", token: "RUST", icon: SiRust },
    { value: "clojure", label: "Clojure", token: "CLOJURE", icon: SiClojure },
    { value: "sql", label: "SQL", token: "SQL", icon: FileCode },
    { value: "php", label: "PHP", token: "PHP", icon: SiPhp },
    { value: "sass", label: "Sass", token: "SASS", icon: SiSass },
    { value: "scss", label: "SCSS", token: "SCSS", icon: SiSass },
    { value: "jinja", label: "Jinja", token: "JINJA", icon: SiJinja },
    { value: "angular", label: "Angular", token: "ANGULAR", icon: SiAngular },
    { value: "nginx", label: "Nginx", token: "NGINX", icon: SiNginx },
    { value: "powershell", label: "PowerShell", token: "PS1", icon: SiPowershell },
    { value: "ruby", label: "Ruby", token: "RUBY", icon: SiRuby },
    { value: "swift", label: "Swift", token: "SWIFT", icon: SiSwift },
    { value: "toml", label: "TOML", token: "TOML", icon: SiToml },
    { value: "kotlin", label: "Kotlin", token: "KOTLIN", icon: SiKotlin },
    { value: "objectivec", label: "Objective-C", token: "OBJ-C", icon: SiC },
    { value: "objectivecpp", label: "Objective-C++", token: "OBJ-C++", icon: SiCplusplus },
    { value: "csharp", label: "C#", token: "C#", icon: SiCsharp },
    { value: "scala", label: "Scala", token: "SCALA", icon: SiScala },
    { value: "dart", label: "Dart", token: "DART", icon: SiDart },
];

/**
 * The list of supported languages, automatically sorted alphabetically
 * while keeping "Auto Detect" and "Plain text" at the top.
 */
export const languages: Language[] = [
    ...rawLanguages.slice(0, 2),
    ...rawLanguages.slice(2).sort((a, b) => a.label.localeCompare(b.label)),
];
