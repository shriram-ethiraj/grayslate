import type { Component } from "svelte";
import type { IconProps } from "@lucide/svelte";
import { FileText, FileCode, Terminal, Container } from "@lucide/svelte";

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

export type LanguageIcon = Component<IconProps> | Component;

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
];

/**
 * The list of supported languages, automatically sorted alphabetically
 * while keeping "Auto Detect" and "Plain text" at the top.
 */
export const languages: Language[] = [
    ...rawLanguages.slice(0, 2),
    ...rawLanguages.slice(2).sort((a, b) => a.label.localeCompare(b.label)),
];
