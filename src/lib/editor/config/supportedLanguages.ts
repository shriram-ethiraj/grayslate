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
    icon: LanguageIcon | null;
}

const rawLanguages: Language[] = [
    { value: "auto", label: "Auto Detect", icon: null },
    { value: "text", label: "Plain text", icon: FileText },
    { value: "json", label: "JSON", icon: SiJson },
    { value: "javascript", label: "JavaScript", icon: SiJavascript },
    { value: "typescript", label: "TypeScript", icon: SiTypescript },
    { value: "python", label: "Python", icon: SiPython },
    { value: "html", label: "HTML", icon: SiHtml5 },
    { value: "css", label: "CSS", icon: SiCss },
    { value: "yaml", label: "YAML", icon: SiYaml },
    { value: "c", label: "C", icon: SiC },
    { value: "cpp", label: "C++", icon: SiCplusplus },
    { value: "java", label: "Java", icon: SiOpenjdk },
    { value: "go", label: "Go", icon: SiGo },
    { value: "xml", label: "XML", icon: FileCode },
    { value: "csv", label: "CSV", icon: SiGooglesheets },
    { value: "markdown", label: "Markdown", icon: SiMarkdown },
    { value: "shell", label: "Shell", icon: Terminal },
    { value: "dockerfile", label: "Dockerfile", icon: Container },
    { value: "svelte", label: "Svelte", icon: SiSvelte },
    { value: "vue", label: "Vue", icon: SiVuedotjs },
    { value: "rust", label: "Rust", icon: SiRust },
    { value: "clojure", label: "Clojure", icon: SiClojure },
    { value: "sql", label: "SQL", icon: FileCode },
    { value: "php", label: "PHP", icon: SiPhp },
    { value: "sass", label: "Sass", icon: SiSass },
    { value: "scss", label: "SCSS", icon: SiSass },
    { value: "jinja", label: "Jinja", icon: SiJinja },
    { value: "angular", label: "Angular", icon: SiAngular },
    { value: "nginx", label: "Nginx", icon: SiNginx },
    { value: "powershell", label: "PowerShell", icon: SiPowershell },
    { value: "ruby", label: "Ruby", icon: SiRuby },
    { value: "swift", label: "Swift", icon: SiSwift },
    { value: "toml", label: "TOML", icon: SiToml },
    { value: "kotlin", label: "Kotlin", icon: SiKotlin },
    { value: "objectivec", label: "Objective-C", icon: SiC },
    { value: "objectivecpp", label: "Objective-C++", icon: SiCplusplus },
    { value: "csharp", label: "C#", icon: SiCsharp },
    { value: "scala", label: "Scala", icon: SiScala },
    { value: "dart", label: "Dart", icon: SiDart },
];

/**
 * The list of supported languages, automatically sorted alphabetically
 * while keeping "Auto Detect" and "Plain text" at the top.
 */
export const languages: Language[] = [
    ...rawLanguages.slice(0, 2),
    ...rawLanguages.slice(2).sort((a, b) => a.label.localeCompare(b.label)),
];
