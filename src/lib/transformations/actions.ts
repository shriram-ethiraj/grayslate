import type { Component } from "svelte";
import type { FileType } from "$lib/state/editor.svelte";
import type { ChunkedTextEvent } from "$lib/ipc";
import type { IndentConfig } from "$lib/editor/components/IndentationPicker.svelte";
import CarbonUrl from '~icons/carbon/url';
import CarbonSecurity from '~icons/carbon/security';
import LucideCaseUpper from '~icons/lucide/case-upper';
import LucideCaseLower from '~icons/lucide/case-lower';
import LucideLabCaseCamel from '~icons/lucide-lab/case-camel';
import LucideLabCaseSnake from '~icons/lucide-lab/case-snake';
import LucideLabCaseKebab from '~icons/lucide-lab/case-kebab';
import FluentTextWordCount20Filled from '~icons/fluent/text-word-count-20-filled';
import FluentTextCaseTitle20Filled from '~icons/fluent/text-case-title-20-filled';
import FluentCut20Filled from '~icons/fluent/cut-20-filled';
import FluentArrowSort20Filled from '~icons/fluent/arrow-sort-20-filled';
import FluentArrowSwap20Filled from '~icons/fluent/arrow-swap-20-filled';
import LucideListCheck from '~icons/lucide/list-check';
import FluentCodeText20Filled from '~icons/fluent/code-text-20-filled';
import MaterialSymbolsCompressRounded from '~icons/material-symbols/compress-rounded';
import LucideSortDesc from '~icons/lucide/sort-desc';
import PepiconsPopDuplicateOff from '~icons/pepicons-pop/duplicate-off';
import LucideBinary from '~icons/lucide/binary';
import MdiHexadecimal from '~icons/mdi/hexadecimal';
import MdiDecimal from '~icons/mdi/decimal';
import MaterialSymbolsTransformRounded from '~icons/material-symbols/transform-rounded';
import FluentTextCollapse20Filled from '~icons/fluent/text-collapse-20-filled';

export type JsonTransformationActionId =
    | "json.format"
    | "json.minify"
    | "json.validate"
    | "json.lines-to-array"
    | "json.array-to-lines"
    | "json.sort-keys"
    | "json.to-typescript"
    | "json.to-csv"
    | "json.to-yaml"
    | "json.keys-camel-case"
    | "json.keys-snake-case"
    | "json.keys-kebab-case"
    | "json.keys-title-case"
    | "json.keys-sponge-case";

export type CsvTransformationActionId = "csv.to-json";

export type SqlTransformationActionId = "sql.format";

export type FormatTransformationActionId =
    | "javascript.format"
    | "typescript.format"
    | "css.format"
    | "html.format"
    | "svelte.format"
    | "yaml.format"
    | "markdown.format"
    | "toml.format";

export type YamlTransformationActionId = "yaml.to-json";

export type TextTransformationActionId =
    | "text.trim-trailing-whitespace"
    | "text.collapse-blank-lines"
    | "text.trim"
    | "text.uppercase"
    | "text.lowercase"
    | "text.reverse-lines"
    | "text.reverse-string"
    | "text.markdown-quote"
    | "text.rot13"
    | "text.add-slashes"
    | "text.remove-slashes"
    | "text.sort-lines"
    | "text.remove-duplicate-lines"
    | "text.collapse-lines"
    | "text.camel-case"
    | "text.snake-case"
    | "text.kebab-case"
    | "text.title-case"
    | "text.sponge-case";

export type UrlTransformationActionId =
    | "url.encode"
    | "url.decode"
    | "url.query-to-json"
    | "url.json-to-query";

export type SecurityTransformationActionId =
    | "security.url-defang"
    | "security.url-refang";

export type EncodingTransformationActionId =
    | "encoding.base64-encode"
    | "encoding.base64-decode"
    | "encoding.base64url-encode"
    | "encoding.base64url-decode"
    | "encoding.html-encode"
    | "encoding.html-decode"
    | "encoding.gzip-to-base64"
    | "encoding.gzip-from-base64"
    | "encoding.jwt-decode";

export type HashTransformationActionId =
    | "hash.sha-256"
    | "hash.sha-512"
    | "hash.sha-1"
    | "hash.md5"
    | "checksum.crc32";

export type TimeTransformationActionId =
    | "time.unix-seconds-to-rfc3339"
    | "time.unix-milliseconds-to-rfc3339"
    | "time.rfc3339-to-unix-seconds"
    | "time.rfc3339-to-unix-milliseconds";

export type XmlTransformationActionId =
    | "xml.format"
    | "xml.minify"
    | "xml.validate";

export type GenerateTransformationActionId =
    | "generate.uuid-v4"
    | "generate.uuid-v7";

export type ConvertTransformationActionId =
    | "convert.ascii-to-hex"
    | "convert.hex-to-ascii"
    | "convert.decimal-to-binary"
    | "convert.binary-to-decimal"
    | "convert.decimal-to-hex"
    | "convert.hex-to-decimal";

export type StatsTransformationActionId =
    | "stats.count-characters"
    | "stats.count-lines"
    | "stats.count-words";

export type TransformationActionId =
    | JsonTransformationActionId
    | CsvTransformationActionId
    | SqlTransformationActionId
    | FormatTransformationActionId
    | YamlTransformationActionId
    | TextTransformationActionId
    | UrlTransformationActionId
    | SecurityTransformationActionId
    | EncodingTransformationActionId
    | HashTransformationActionId
    | TimeTransformationActionId
    | XmlTransformationActionId
    | GenerateTransformationActionId
    | ConvertTransformationActionId
    | StatsTransformationActionId;

export type TransformationMessageLevel = "success" | "error" | "info";

export type TransformationActionDefinition = {
    id: TransformationActionId;
    title: string;
    description: string;
    category: string;
    keywords: string[];
    fileTypes: FileType[];
    supportsSelection: boolean;
    /** How a replacement result is applied. Generators insert at the cursor. */
    applyMode?: "replace" | "insert";
    /** Optional override icon. Falls back to the file-type icon when unset. */
    icon?: Component;
    /**
     * For format-converting transformations, the language mode to apply to the
     * editor immediately after a successful full-document transform. Undefined
     * for transformations that do not change the document type.
     */
    outputLanguage?: string;
};

export type ExecuteTransformationRequest = {
    actionId: TransformationActionId;
    text: string;
    /** Per-invocation ID used to cancel the request via `cancel_transformation`. */
    requestId: number;
    /** Action-specific parameters (e.g. indentation config for formatting transforms). */
    params?: {
        indentConfig?: IndentConfig;
    };
};

/** Progress update sent by Rust during long-running transformations via the IPC channel. */
export type TransformationProgressEvent = {
    type: "progress";
    /** Items processed so far (bytes or rows depending on the operation). */
    current: number;
    /** Total items to process (same unit as `current`). */
    total: number;
};

/** One slice of the result text. Accumulate all chunks in order before use. */
export type TransformationChunkEvent = ChunkedTextEvent;

/** Union of all events sent via the transformation IPC channel. */
export type TransformationChannelEvent =
    | TransformationProgressEvent
    | TransformationChunkEvent;

export type ExecuteTransformationResponse =
    | {
        kind: "replace-text";
        chunkCount: number;
        message?: string;
        level?: TransformationMessageLevel;
    }
    | {
        kind: "show-message";
        message: string;
        level: TransformationMessageLevel;
    };

export const transformationActions: TransformationActionDefinition[] = [
    {
        id: "json.format",
        title: "Format JSON",
        description: "Pretty-print JSON with consistent indentation while preserving comments and trailing commas.",
        category: "JSON",
        keywords: ["json", "format", "pretty", "indent"],
        fileTypes: ["json"],
        supportsSelection: true,
        icon: FluentCodeText20Filled
    },
    {
        id: "json.minify",
        title: "Minify JSON",
        description: "Remove unnecessary whitespace while preserving comments and trailing commas.",
        category: "JSON",
        keywords: ["json", "minify", "compact", "compress"],
        fileTypes: ["json"],
        supportsSelection: true,
        icon: MaterialSymbolsCompressRounded
    },
    {
        id: "json.validate",
        title: "Validate JSON",
        description: "Check whether the JSON is valid with support for comments and trailing commas.",
        category: "JSON",
        keywords: ["json", "validate", "lint", "check"],
        fileTypes: ["json"],
        supportsSelection: true,
        icon: LucideListCheck,
    },
    {
        id: "json.lines-to-array",
        title: "JSON Lines to JSON Array",
        description: "Parse each nonblank line as JSON and combine the values into one formatted array.",
        category: "JSON",
        keywords: ["json", "jsonl", "ndjson", "lines", "array", "convert"],
        fileTypes: ["json", "text"],
        supportsSelection: true,
        outputLanguage: "json",
        icon: MaterialSymbolsTransformRounded,
    },
    {
        id: "json.array-to-lines",
        title: "JSON Array to JSON Lines",
        description: "Convert a top-level JSON array to one compact JSON value per line.",
        category: "JSON",
        keywords: ["json", "jsonl", "ndjson", "lines", "array", "convert"],
        fileTypes: ["json"],
        supportsSelection: true,
        outputLanguage: "text",
        icon: MaterialSymbolsTransformRounded,
    },
    {
        id: "json.sort-keys",
        title: "Sort JSON Keys",
        description: "Recursively sort strict-JSON object keys while preserving array order.",
        category: "JSON",
        keywords: ["json", "sort", "keys", "canonical", "alphabetical"],
        fileTypes: ["json"],
        supportsSelection: true,
        icon: FluentArrowSort20Filled,
    },
    {
        id: "json.to-typescript",
        title: "JSON to TypeScript",
        description: "Infer TypeScript declarations from strict JSON using Root as the top-level name.",
        category: "Convert",
        keywords: ["json", "typescript", "interface", "type", "generate", "convert"],
        fileTypes: ["json"],
        supportsSelection: true,
        outputLanguage: "typescript",
        icon: MaterialSymbolsTransformRounded,
    },
    {
        id: "sql.format",
        title: "Format SQL",
        description: "Pretty-print SQL queries with consistent indentation and uppercase keywords.",
        category: "SQL",
        keywords: ["sql", "format", "pretty", "indent", "query"],
        fileTypes: ["sql"],
        supportsSelection: true,
        icon: FluentCodeText20Filled,
    },
    // ── Code Formatting ────────────────────────────────────────────────────
    {
        id: "javascript.format",
        title: "Format JavaScript",
        description: "Pretty-print JavaScript with consistent indentation.",
        category: "JavaScript",
        keywords: ["javascript", "js", "format", "pretty", "indent"],
        fileTypes: ["javascript"],
        supportsSelection: true,
        icon: FluentCodeText20Filled,
    },
    {
        id: "typescript.format",
        title: "Format TypeScript",
        description: "Pretty-print TypeScript with consistent indentation.",
        category: "TypeScript",
        keywords: ["typescript", "ts", "format", "pretty", "indent"],
        fileTypes: ["typescript"],
        supportsSelection: true,
        icon: FluentCodeText20Filled,
    },
    {
        id: "css.format",
        title: "Format CSS",
        description: "Pretty-print CSS with consistent indentation.",
        category: "CSS",
        keywords: ["css", "style", "format", "pretty", "indent"],
        fileTypes: ["css"],
        supportsSelection: true,
        icon: FluentCodeText20Filled,
    },
    {
        id: "html.format",
        title: "Format HTML",
        description: "Pretty-print HTML with consistent indentation.",
        category: "HTML",
        keywords: ["html", "markup", "format", "pretty", "indent"],
        fileTypes: ["html"],
        supportsSelection: true,
        icon: FluentCodeText20Filled,
    },
    {
        id: "svelte.format",
        title: "Format Svelte",
        description: "Pretty-print Svelte markup, script, and style blocks with consistent indentation.",
        category: "Svelte",
        keywords: ["svelte", "format", "pretty", "indent"],
        fileTypes: ["svelte"],
        supportsSelection: true,
        icon: FluentCodeText20Filled,
    },
    {
        id: "yaml.format",
        title: "Format YAML",
        description: "Pretty-print YAML with consistent indentation.",
        category: "YAML",
        keywords: ["yaml", "yml", "format", "pretty", "indent"],
        fileTypes: ["yaml"],
        supportsSelection: true,
        icon: FluentCodeText20Filled,
    },
    {
        id: "markdown.format",
        title: "Format Markdown",
        description: "Pretty-print Markdown prose with consistent formatting.",
        category: "Markdown",
        keywords: ["markdown", "md", "format", "pretty"],
        fileTypes: ["markdown"],
        supportsSelection: true,
        icon: FluentCodeText20Filled,
    },
    {
        id: "toml.format",
        title: "Format TOML",
        description: "Pretty-print TOML with consistent indentation.",
        category: "TOML",
        keywords: ["toml", "format", "pretty", "indent"],
        fileTypes: ["toml"],
        supportsSelection: true,
        icon: FluentCodeText20Filled,
    },
    {
        id: "xml.format",
        title: "Format XML",
        description: "Pretty-print well-formed XML while preserving text and CDATA content.",
        category: "XML",
        keywords: ["xml", "format", "pretty", "indent", "markup"],
        fileTypes: ["xml"],
        supportsSelection: true,
        icon: FluentCodeText20Filled,
    },
    {
        id: "xml.minify",
        title: "Minify XML",
        description: "Remove formatting-only whitespace from well-formed XML.",
        category: "XML",
        keywords: ["xml", "minify", "compact", "compress", "markup"],
        fileTypes: ["xml"],
        supportsSelection: true,
        icon: MaterialSymbolsCompressRounded,
    },
    {
        id: "xml.validate",
        title: "Validate XML",
        description: "Check XML well-formedness without resolving DTDs, schemas, or external entities.",
        category: "XML",
        keywords: ["xml", "validate", "well formed", "lint", "check"],
        fileTypes: ["xml"],
        supportsSelection: true,
        icon: LucideListCheck,
    },
    // ── JSON Key Case Conversion ─────────────────────────────────────────────
    {
        id: "json.keys-camel-case",
        title: "Keys to camelCase",
        description: "Recursively convert all JSON object key names to camelCase.",
        category: "Case Conversion",
        keywords: ["json", "keys", "case", "camel", "camelcase", "rename"],
        fileTypes: ["json"],
        supportsSelection: true,
        icon: LucideLabCaseCamel,
    },
    {
        id: "json.keys-snake-case",
        title: "Keys to snake_case",
        description: "Recursively convert all JSON object key names to snake_case.",
        category: "Case Conversion",
        keywords: ["json", "keys", "case", "snake", "snake_case", "rename"],
        fileTypes: ["json"],
        supportsSelection: true,
        icon: LucideLabCaseSnake,
    },
    {
        id: "json.keys-kebab-case",
        title: "Keys to kebab-case",
        description: "Recursively convert all JSON object key names to kebab-case.",
        category: "Case Conversion",
        keywords: ["json", "keys", "case", "kebab", "kebab-case", "rename"],
        fileTypes: ["json"],
        supportsSelection: true,
        icon: LucideLabCaseKebab,
    },
    {
        id: "json.keys-title-case",
        title: "Keys to Title Case",
        description: "Recursively convert all JSON object key names to Title Case.",
        category: "Case Conversion",
        keywords: ["json", "keys", "case", "title", "capitalize", "rename"],
        fileTypes: ["json"],
        supportsSelection: true,
        icon: FluentTextCaseTitle20Filled,
    },
    {
        id: "json.keys-sponge-case",
        title: "Keys to sPoNgE cAsE",
        description: "Recursively convert all JSON object key names to sPoNgE cAsE.",
        category: "Case Conversion",
        keywords: ["json", "keys", "case", "sponge", "mock", "alternate", "rename"],
        fileTypes: ["json"],
        supportsSelection: true,
    },
    {
        id: "text.trim-trailing-whitespace",
        title: "Trim Trailing Whitespace",
        description: "Remove trailing spaces and tabs from each line.",
        category: "Plain Text",
        keywords: ["text", "trim", "whitespace", "spaces", "tabs"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: FluentCut20Filled
    },
    {
        id: "text.collapse-blank-lines",
        title: "Collapse Blank Lines",
        description: "Reduce repeated blank lines to a single empty line.",
        category: "Plain Text",
        keywords: ["text", "spacing", "blank", "lines", "cleanup"],
        fileTypes: ["text"],
        supportsSelection: false,
        icon: FluentTextCollapse20Filled
    },
    {
        id: "csv.to-json",
        title: "CSV to JSON",
        description: "Convert CSV to a JSON array of objects, using the first row as headers. Delimiter is auto-detected.",
        category: "Convert",
        keywords: ["csv", "json", "convert", "array", "objects", "table"],
        fileTypes: ["csv"],
        supportsSelection: false,
        outputLanguage: "json",
        icon: MaterialSymbolsTransformRounded,
    },
    {
        id: "json.to-csv",
        title: "JSON to CSV",
        description: "Convert a JSON array of objects to CSV, using object keys as headers.",
        category: "Convert",
        keywords: ["json", "csv", "convert", "table", "array", "objects"],
        fileTypes: ["json"],
        supportsSelection: false,
        outputLanguage: "csv",
        icon: MaterialSymbolsTransformRounded,
    },
    {
        id: "json.to-yaml",
        title: "JSON to YAML",
        description: "Convert JSON to YAML. Accepts the same JSON-with-comments and trailing-comma input supported by other JSON actions.",
        category: "Convert",
        keywords: ["json", "yaml", "yml", "convert", "markup"],
        fileTypes: ["json"],
        supportsSelection: false,
        outputLanguage: "yaml",
        icon: MaterialSymbolsTransformRounded,
    },
    {
        id: "yaml.to-json",
        title: "YAML to JSON",
        description: "Convert YAML to pretty-printed JSON.",
        category: "Convert",
        keywords: ["yaml", "yml", "json", "convert", "markup"],
        fileTypes: ["yaml"],
        supportsSelection: false,
        outputLanguage: "json",
        icon: MaterialSymbolsTransformRounded,
    },
    // ── Plain Text ──────────────────────────────────────────────────────────
    {
        id: "text.trim",
        title: "Trim Whitespace",
        description: "Remove all leading and trailing whitespace from the entire document.",
        category: "Plain Text",
        keywords: ["text", "trim", "strip", "whitespace"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: FluentCut20Filled
    },
    {
        id: "text.uppercase",
        title: "Uppercase",
        description: "Convert all text to uppercase letters.",
        category: "Plain Text",
        keywords: ["text", "uppercase", "caps", "upper"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: LucideCaseUpper,
    },
    {
        id: "text.lowercase",
        title: "Lowercase",
        description: "Convert all text to lowercase letters.",
        category: "Plain Text",
        keywords: ["text", "lowercase", "lower"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: LucideCaseLower,
    },
    {
        id: "text.reverse-lines",
        title: "Reverse Lines",
        description: "Reverse the order of lines in the document.",
        category: "Plain Text",
        keywords: ["text", "reverse", "lines", "order", "flip"],
        fileTypes: ["text"],
        supportsSelection: false,
        icon: FluentArrowSort20Filled
    },
    {
        id: "text.reverse-string",
        title: "Reverse String",
        description: "Reverse the characters in the document, preserving Unicode grapheme clusters.",
        category: "Plain Text",
        keywords: ["text", "reverse", "string", "characters", "unicode"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: FluentArrowSwap20Filled
    },
    {
        id: "text.markdown-quote",
        title: "Add Markdown Quote",
        description: "Prefix every line with '> ' to format the text as a Markdown blockquote.",
        category: "Plain Text",
        keywords: ["text", "markdown", "quote", "blockquote"],
        fileTypes: ["text", "markdown"],
        supportsSelection: true,
    },
    {
        id: "text.rot13",
        title: "ROT13",
        description: "Apply ROT13 substitution cipher to ASCII letters. Applying it twice restores the original.",
        category: "Plain Text",
        keywords: ["text", "rot13", "cipher", "obfuscate", "encode"],
        fileTypes: ["text"],
        supportsSelection: true,
    },
    {
        id: "text.add-slashes",
        title: "Add Slashes",
        description: "Escape single quotes, double quotes, and backslashes with a backslash.",
        category: "Plain Text",
        keywords: ["text", "escape", "slashes", "quotes", "backslash"],
        fileTypes: ["text"],
        supportsSelection: true,
    },
    {
        id: "text.remove-slashes",
        title: "Remove Slashes",
        description: "Unescape backslash-escaped single quotes, double quotes, and backslashes.",
        category: "Plain Text",
        keywords: ["text", "unescape", "slashes", "quotes", "backslash"],
        fileTypes: ["text"],
        supportsSelection: true,
    },
    {
        id: "text.sort-lines",
        title: "Sort Lines",
        description: "Sort all lines alphabetically in ascending order.",
        category: "Plain Text",
        keywords: ["text", "sort", "lines", "alphabetical", "order"],
        fileTypes: ["text"],
        supportsSelection: false,
        icon: LucideSortDesc,
    },
    {
        id: "text.remove-duplicate-lines",
        title: "Remove Duplicate Lines",
        description: "Remove repeated lines, keeping the first occurrence of each. Reports how many were removed.",
        category: "Plain Text",
        keywords: ["text", "deduplicate", "unique", "lines", "duplicates"],
        fileTypes: ["text"],
        supportsSelection: false,
        icon: PepiconsPopDuplicateOff,
    },
    {
        id: "text.collapse-lines",
        title: "Collapse Lines",
        description: "Join all lines into a single line separated by spaces.",
        category: "Plain Text",
        keywords: ["text", "collapse", "join", "lines", "single"],
        fileTypes: ["text"],
        supportsSelection: false,
        icon: FluentTextCollapse20Filled
    },
    // ── Case Conversion ──────────────────────────────────────────────────────
    {
        id: "text.camel-case",
        title: "camelCase",
        description: "Convert each line to camelCase (first word lowercase, subsequent words capitalized).",
        category: "Case Conversion",
        keywords: ["text", "case", "camel", "camelcase", "identifier"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: LucideLabCaseCamel,
    },
    {
        id: "text.snake-case",
        title: "snake_case",
        description: "Convert each line to snake_case (words separated by underscores, all lowercase).",
        category: "Case Conversion",
        keywords: ["text", "case", "snake", "snake_case", "identifier"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: LucideLabCaseSnake,
    },
    {
        id: "text.kebab-case",
        title: "kebab-case",
        description: "Convert each line to kebab-case (words separated by hyphens, all lowercase).",
        category: "Case Conversion",
        keywords: ["text", "case", "kebab", "kebab-case", "identifier", "slug"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: LucideLabCaseKebab,
    },
    {
        id: "text.title-case",
        title: "Title Case",
        description: "Convert each line to Title Case (first letter of each word capitalized).",
        category: "Case Conversion",
        keywords: ["text", "case", "title", "capitalize", "words"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: FluentTextCaseTitle20Filled,
    },
    {
        id: "text.sponge-case",
        title: "sPoNgE cAsE",
        description: "AlTeRnAtE tHe CaSe Of AlPhAbEtIc ChArAcTeRs.",
        category: "Case Conversion",
        keywords: ["text", "case", "sponge", "mock", "mocking", "alternate"],
        fileTypes: ["text"],
        supportsSelection: true
    },
    // ── URL ──────────────────────────────────────────────────────────────────
    {
        id: "url.encode",
        title: "URL Encode",
        description: "Percent-encode all characters except unreserved ASCII characters (letters, digits, -, _, ., ~).",
        category: "URL",
        keywords: ["url", "encode", "percent", "escape", "uri"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: CarbonUrl,
    },
    {
        id: "url.decode",
        title: "URL Decode",
        description: "Decode percent-encoded URL characters back to their original form.",
        category: "URL",
        keywords: ["url", "decode", "percent", "unescape", "uri"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: CarbonUrl,
    },
    {
        id: "url.query-to-json",
        title: "Query String to JSON",
        description: "Convert a URL query string to JSON, preserving repeated keys as arrays.",
        category: "URL",
        keywords: ["url", "query", "parameters", "json", "parse", "convert"],
        fileTypes: ["text"],
        supportsSelection: true,
        outputLanguage: "json",
        icon: CarbonUrl,
    },
    {
        id: "url.json-to-query",
        title: "JSON to Query String",
        description: "Convert a flat JSON object of scalar values or scalar arrays to a URL query string.",
        category: "URL",
        keywords: ["url", "query", "parameters", "json", "stringify", "convert"],
        fileTypes: ["json"],
        supportsSelection: true,
        outputLanguage: "text",
        icon: CarbonUrl,
    },
    // ── Security ─────────────────────────────────────────────────────────────
    {
        id: "security.url-defang",
        title: "URL Defang",
        description: "Make URLs safe to share in reports by replacing 'http' with 'hXXp', '://' with '[://]', and '.' with '[.]'.",
        category: "Security",
        keywords: ["security", "defang", "url", "ioc", "threat", "safe"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: CarbonSecurity,
    },
    {
        id: "security.url-refang",
        title: "URL Refang",
        description: "Restore a defanged URL back to its original form by reversing 'hXXp', '[://]', and '[.]' substitutions.",
        category: "Security",
        keywords: ["security", "refang", "url", "ioc", "threat", "restore"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: CarbonSecurity,
    },
    // ── Encoding ─────────────────────────────────────────────────────────────
    {
        id: "encoding.base64-encode",
        title: "Base64 Encode",
        description: "Encode the document as standard Base64.",
        category: "Encoding",
        keywords: ["encoding", "base64", "encode", "binary"],
        fileTypes: ["text"],
        supportsSelection: true,
    },
    {
        id: "encoding.base64-decode",
        title: "Base64 Decode",
        description: "Decode a Base64-encoded string back to its original UTF-8 text.",
        category: "Encoding",
        keywords: ["encoding", "base64", "decode", "binary"],
        fileTypes: ["text"],
        supportsSelection: true,
    },
    {
        id: "encoding.base64url-encode",
        title: "Base64URL Encode",
        description: "Encode UTF-8 text as URL-safe Base64 without padding.",
        category: "Encoding",
        keywords: ["encoding", "base64url", "base64", "url safe", "encode"],
        fileTypes: ["text"],
        supportsSelection: true,
    },
    {
        id: "encoding.base64url-decode",
        title: "Base64URL Decode",
        description: "Decode padded or unpadded URL-safe Base64 to UTF-8 text.",
        category: "Encoding",
        keywords: ["encoding", "base64url", "base64", "url safe", "decode"],
        fileTypes: ["text"],
        supportsSelection: true,
    },
    {
        id: "encoding.html-encode",
        title: "HTML Entity Encode",
        description: "Encode HTML-sensitive characters while preserving other Unicode text.",
        category: "Encoding",
        keywords: ["html", "entity", "entities", "encode", "escape"],
        fileTypes: ["text", "html", "xml"],
        supportsSelection: true,
    },
    {
        id: "encoding.html-decode",
        title: "HTML Entity Decode",
        description: "Decode standard named and numeric HTML entities.",
        category: "Encoding",
        keywords: ["html", "entity", "entities", "decode", "unescape"],
        fileTypes: ["text", "html", "xml"],
        supportsSelection: true,
    },
    {
        id: "encoding.gzip-to-base64",
        title: "GZip Text to Base64",
        description: "Gzip-compress UTF-8 text and return standard Base64.",
        category: "Encoding",
        keywords: ["gzip", "compress", "base64", "encoding"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: MaterialSymbolsCompressRounded,
    },
    {
        id: "encoding.gzip-from-base64",
        title: "Base64 GZip to Text",
        description: "Decode Base64, decompress gzip data, and return UTF-8 text.",
        category: "Encoding",
        keywords: ["gzip", "decompress", "base64", "decoding"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: MaterialSymbolsCompressRounded,
    },
    {
        id: "encoding.jwt-decode",
        title: "Decode JWT (Unverified)",
        description: "Decode JWT header and payload JSON without verifying the signature.",
        category: "Encoding",
        keywords: ["jwt", "token", "decode", "claims", "api", "unverified"],
        fileTypes: ["text"],
        supportsSelection: true,
        outputLanguage: "json",
        icon: CarbonSecurity,
    },
    // ── Hashes and checksums ────────────────────────────────────────────────
    {
        id: "hash.sha-256",
        title: "SHA-256 Hash",
        description: "Hash the exact UTF-8 input bytes and output lowercase hexadecimal.",
        category: "Hash",
        keywords: ["hash", "sha", "sha256", "digest", "checksum"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: CarbonSecurity,
    },
    {
        id: "hash.sha-512",
        title: "SHA-512 Hash",
        description: "Hash the exact UTF-8 input bytes and output lowercase hexadecimal.",
        category: "Hash",
        keywords: ["hash", "sha", "sha512", "digest", "checksum"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: CarbonSecurity,
    },
    {
        id: "checksum.crc32",
        title: "CRC32 Checksum",
        description: "Compute CRC32 over the exact UTF-8 input bytes.",
        category: "Hash",
        keywords: ["crc", "crc32", "checksum", "integrity"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: CarbonSecurity,
    },
    {
        id: "hash.sha-1",
        title: "SHA-1 Hash (Legacy)",
        description: "Generate a legacy SHA-1 digest for compatibility and non-security integrity checks.",
        category: "Hash",
        keywords: ["hash", "sha", "sha1", "legacy", "digest"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: CarbonSecurity,
    },
    {
        id: "hash.md5",
        title: "MD5 Hash (Legacy)",
        description: "Generate a legacy MD5 digest for compatibility and non-security integrity checks.",
        category: "Hash",
        keywords: ["hash", "md5", "legacy", "digest", "checksum"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: CarbonSecurity,
    },
    // ── Time ────────────────────────────────────────────────────────────────
    {
        id: "time.unix-seconds-to-rfc3339",
        title: "Unix Seconds to RFC 3339 UTC",
        description: "Convert one Unix-seconds integer to an unambiguous UTC timestamp.",
        category: "Time",
        keywords: ["time", "date", "unix", "epoch", "seconds", "rfc3339", "iso"],
        fileTypes: ["text"],
        supportsSelection: true,
    },
    {
        id: "time.unix-milliseconds-to-rfc3339",
        title: "Unix Milliseconds to RFC 3339 UTC",
        description: "Convert one Unix-milliseconds integer to an unambiguous UTC timestamp.",
        category: "Time",
        keywords: ["time", "date", "unix", "epoch", "milliseconds", "rfc3339", "iso"],
        fileTypes: ["text"],
        supportsSelection: true,
    },
    {
        id: "time.rfc3339-to-unix-seconds",
        title: "RFC 3339 to Unix Seconds",
        description: "Convert one timezone-qualified RFC 3339 timestamp to Unix seconds.",
        category: "Time",
        keywords: ["time", "date", "unix", "epoch", "seconds", "rfc3339", "iso"],
        fileTypes: ["text"],
        supportsSelection: true,
    },
    {
        id: "time.rfc3339-to-unix-milliseconds",
        title: "RFC 3339 to Unix Milliseconds",
        description: "Convert one timezone-qualified RFC 3339 timestamp to Unix milliseconds.",
        category: "Time",
        keywords: ["time", "date", "unix", "epoch", "milliseconds", "rfc3339", "iso"],
        fileTypes: ["text"],
        supportsSelection: true,
    },
    // ── Generators ──────────────────────────────────────────────────────────
    {
        id: "generate.uuid-v4",
        title: "Insert UUID v4",
        description: "Replace the selection or insert one random UUID at the cursor.",
        category: "Generate",
        keywords: ["generate", "uuid", "guid", "v4", "random", "identifier"],
        fileTypes: ["text"],
        supportsSelection: true,
        applyMode: "insert",
    },
    {
        id: "generate.uuid-v7",
        title: "Insert UUID v7",
        description: "Replace the selection or insert one time-ordered UUID at the cursor.",
        category: "Generate",
        keywords: ["generate", "uuid", "guid", "v7", "time ordered", "identifier"],
        fileTypes: ["text"],
        supportsSelection: true,
        applyMode: "insert",
    },
    // ── Convert ──────────────────────────────────────────────────────────────
    {
        id: "convert.ascii-to-hex",
        title: "ASCII to Hex",
        description: "Encode every byte of the text as two uppercase hexadecimal digits.",
        category: "Convert",
        keywords: ["convert", "ascii", "hex", "hexadecimal", "encode"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: MdiHexadecimal,
    },
    {
        id: "convert.hex-to-ascii",
        title: "Hex to ASCII",
        description: "Decode pairs of hexadecimal digits back to UTF-8 text. Spaces between pairs are ignored.",
        category: "Convert",
        keywords: ["convert", "hex", "ascii", "hexadecimal", "decode"],
        fileTypes: ["text"],
        supportsSelection: true,
    },
    {
        id: "convert.decimal-to-binary",
        title: "Decimal to Binary",
        description: "Convert each line containing a decimal integer to its binary representation. Non-numeric lines are passed through unchanged.",
        category: "Convert",
        keywords: ["convert", "decimal", "binary", "number", "radix"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: LucideBinary
    },
    {
        id: "convert.binary-to-decimal",
        title: "Binary to Decimal",
        description: "Convert each line containing a binary integer to its decimal representation. Non-numeric lines are passed through unchanged.",
        category: "Convert",
        keywords: ["convert", "binary", "decimal", "number", "radix"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: MdiDecimal
    },
    {
        id: "convert.decimal-to-hex",
        title: "Decimal to Hex",
        description: "Convert each line containing a decimal integer to its uppercase hexadecimal representation. Non-numeric lines are passed through unchanged.",
        category: "Convert",
        keywords: ["convert", "decimal", "hex", "hexadecimal", "number", "radix"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: MdiHexadecimal,
    },
    {
        id: "convert.hex-to-decimal",
        title: "Hex to Decimal",
        description: "Convert each line containing a hexadecimal integer (with or without 0x prefix) to its decimal representation. Non-numeric lines are passed through unchanged.",
        category: "Convert",
        keywords: ["convert", "hex", "hexadecimal", "decimal", "number", "radix"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: MdiDecimal
    },
    // ── Stats ────────────────────────────────────────────────────────────────
    {
        id: "stats.count-characters",
        title: "Count Characters",
        description: "Count the total number of Unicode characters (codepoints) in the document or selection.",
        category: "Stats",
        keywords: ["stats", "count", "characters", "length", "size"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: FluentTextWordCount20Filled,
    },
    {
        id: "stats.count-lines",
        title: "Count Lines",
        description: "Count the number of lines in the document or selection.",
        category: "Stats",
        keywords: ["stats", "count", "lines", "rows"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: FluentTextWordCount20Filled,
    },
    {
        id: "stats.count-words",
        title: "Count Words",
        description: "Count the number of whitespace-delimited words in the document or selection.",
        category: "Stats",
        keywords: ["stats", "count", "words", "tokens"],
        fileTypes: ["text"],
        supportsSelection: true,
        icon: FluentTextWordCount20Filled,
    },
];

export function getTransformationActionsForFileType(
    fileType: FileType,
): TransformationActionDefinition[] {
    return transformationActions.filter((action) => action.fileTypes.includes(fileType));
}

export function hasActionsForFileType(fileType: FileType): boolean {
    return transformationActions.some((action) => action.fileTypes.includes(fileType));
}

export function getTransformationAction(
    actionId: TransformationActionId,
): TransformationActionDefinition | undefined {
    return transformationActions.find((action) => action.id === actionId);
}
