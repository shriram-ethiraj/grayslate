import {
    siJavascript,
    siTypescript,
    siPython,
    siHtml5,
    siCss,
    siYaml,
    siC,
    siCplusplus,
    siGo,
    siMarkdown,
    siJson,
    siOpenjdk,
    siGooglesheets,
} from "simple-icons";
import type { SimpleIcon } from "simple-icons";
import type { Component } from "svelte";
import { FileText, FileCode } from "@lucide/svelte";
import type { IconProps } from "@lucide/svelte";

export type LanguageIcon = SimpleIcon | Component<IconProps>;

export interface Language {
    value: string;
    label: string;
    icon: LanguageIcon | null;
}

export const languages: Language[] = [
    { value: "auto",       label: "Auto Detect", icon: null },
    { value: "text",       label: "Plain text",  icon: FileText },
    { value: "json",       label: "JSON",        icon: siJson },
    { value: "javascript", label: "JavaScript",  icon: siJavascript },
    { value: "typescript", label: "TypeScript",  icon: siTypescript },
    { value: "python",     label: "Python",      icon: siPython },
    { value: "html",       label: "HTML",        icon: siHtml5 },
    { value: "css",        label: "CSS",         icon: siCss },
    { value: "yaml",       label: "YAML",        icon: siYaml },
    { value: "c",          label: "C",           icon: siC },
    { value: "cpp",        label: "C++",         icon: siCplusplus },
    { value: "java",       label: "Java",        icon: siOpenjdk },
    { value: "go",         label: "Go",          icon: siGo },
    { value: "xml",        label: "XML",         icon: FileCode },
    { value: "csv",        label: "CSV",         icon: siGooglesheets },
    { value: "markdown",   label: "Markdown",    icon: siMarkdown },
];
