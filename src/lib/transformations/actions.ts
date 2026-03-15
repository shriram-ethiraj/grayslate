import type { FileType } from "$lib/state/editor.svelte";

export type JsonTransformationActionId =
    | "json.format"
    | "json.minify"
    | "json.validate"
    | "json.to-csv"
    | "json.to-yaml";

export type CsvTransformationActionId = "csv.to-json";

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

export type UrlTransformationActionId = "url.encode" | "url.decode";

export type SecurityTransformationActionId =
    | "security.url-defang"
    | "security.url-refang";

export type EncodingTransformationActionId =
    | "encoding.base64-encode"
    | "encoding.base64-decode";

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
    | YamlTransformationActionId
    | TextTransformationActionId
    | UrlTransformationActionId
    | SecurityTransformationActionId
    | EncodingTransformationActionId
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
};

export type ExecuteTransformationResponse =
    | {
        kind: "replace-text";
        text: string;
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
    },
    {
        id: "json.minify",
        title: "Minify JSON",
        description: "Remove unnecessary whitespace while preserving comments and trailing commas.",
        category: "JSON",
        keywords: ["json", "minify", "compact", "compress"],
        fileTypes: ["json"],
        supportsSelection: true,
    },
    {
        id: "json.validate",
        title: "Validate JSON",
        description: "Check whether the JSON is valid with support for comments and trailing commas.",
        category: "JSON",
        keywords: ["json", "validate", "lint", "check"],
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
    },
    {
        id: "text.collapse-blank-lines",
        title: "Collapse Blank Lines",
        description: "Reduce repeated blank lines to a single empty line.",
        category: "Plain Text",
        keywords: ["text", "spacing", "blank", "lines", "cleanup"],
        fileTypes: ["text"],
        supportsSelection: false,
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
    },
    {
        id: "text.uppercase",
        title: "Uppercase",
        description: "Convert all text to uppercase letters.",
        category: "Plain Text",
        keywords: ["text", "uppercase", "caps", "upper"],
        fileTypes: ["text"],
        supportsSelection: true,
    },
    {
        id: "text.lowercase",
        title: "Lowercase",
        description: "Convert all text to lowercase letters.",
        category: "Plain Text",
        keywords: ["text", "lowercase", "lower"],
        fileTypes: ["text"],
        supportsSelection: true,
    },
    {
        id: "text.reverse-lines",
        title: "Reverse Lines",
        description: "Reverse the order of lines in the document.",
        category: "Plain Text",
        keywords: ["text", "reverse", "lines", "order", "flip"],
        fileTypes: ["text"],
        supportsSelection: false,
    },
    {
        id: "text.reverse-string",
        title: "Reverse String",
        description: "Reverse the characters in the document, preserving Unicode grapheme clusters.",
        category: "Plain Text",
        keywords: ["text", "reverse", "string", "characters", "unicode"],
        fileTypes: ["text"],
        supportsSelection: true,
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
    },
    {
        id: "text.remove-duplicate-lines",
        title: "Remove Duplicate Lines",
        description: "Remove repeated lines, keeping the first occurrence of each. Reports how many were removed.",
        category: "Plain Text",
        keywords: ["text", "deduplicate", "unique", "lines", "duplicates"],
        fileTypes: ["text"],
        supportsSelection: false,
    },
    {
        id: "text.collapse-lines",
        title: "Collapse Lines",
        description: "Join all lines into a single line separated by spaces.",
        category: "Plain Text",
        keywords: ["text", "collapse", "join", "lines", "single"],
        fileTypes: ["text"],
        supportsSelection: false,
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
    },
    {
        id: "text.snake-case",
        title: "snake_case",
        description: "Convert each line to snake_case (words separated by underscores, all lowercase).",
        category: "Case Conversion",
        keywords: ["text", "case", "snake", "snake_case", "identifier"],
        fileTypes: ["text"],
        supportsSelection: true,
    },
    {
        id: "text.kebab-case",
        title: "kebab-case",
        description: "Convert each line to kebab-case (words separated by hyphens, all lowercase).",
        category: "Case Conversion",
        keywords: ["text", "case", "kebab", "kebab-case", "identifier", "slug"],
        fileTypes: ["text"],
        supportsSelection: true,
    },
    {
        id: "text.title-case",
        title: "Title Case",
        description: "Convert each line to Title Case (first letter of each word capitalized).",
        category: "Case Conversion",
        keywords: ["text", "case", "title", "capitalize", "words"],
        fileTypes: ["text"],
        supportsSelection: true,
    },
    {
        id: "text.sponge-case",
        title: "sPoNgE cAsE",
        description: "AlTeRnAtE tHe CaSe Of AlPhAbEtIc ChArAcTeRs.",
        category: "Case Conversion",
        keywords: ["text", "case", "sponge", "mock", "mocking", "alternate"],
        fileTypes: ["text"],
        supportsSelection: true,
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
    },
    {
        id: "url.decode",
        title: "URL Decode",
        description: "Decode percent-encoded URL characters back to their original form.",
        category: "URL",
        keywords: ["url", "decode", "percent", "unescape", "uri"],
        fileTypes: ["text"],
        supportsSelection: true,
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
    },
    {
        id: "security.url-refang",
        title: "URL Refang",
        description: "Restore a defanged URL back to its original form by reversing 'hXXp', '[://]', and '[.]' substitutions.",
        category: "Security",
        keywords: ["security", "refang", "url", "ioc", "threat", "restore"],
        fileTypes: ["text"],
        supportsSelection: true,
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
    // ── Convert ──────────────────────────────────────────────────────────────
    {
        id: "convert.ascii-to-hex",
        title: "ASCII to Hex",
        description: "Encode every byte of the text as two uppercase hexadecimal digits.",
        category: "Convert",
        keywords: ["convert", "ascii", "hex", "hexadecimal", "encode"],
        fileTypes: ["text"],
        supportsSelection: true,
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
    },
    {
        id: "convert.binary-to-decimal",
        title: "Binary to Decimal",
        description: "Convert each line containing a binary integer to its decimal representation. Non-numeric lines are passed through unchanged.",
        category: "Convert",
        keywords: ["convert", "binary", "decimal", "number", "radix"],
        fileTypes: ["text"],
        supportsSelection: true,
    },
    {
        id: "convert.decimal-to-hex",
        title: "Decimal to Hex",
        description: "Convert each line containing a decimal integer to its uppercase hexadecimal representation. Non-numeric lines are passed through unchanged.",
        category: "Convert",
        keywords: ["convert", "decimal", "hex", "hexadecimal", "number", "radix"],
        fileTypes: ["text"],
        supportsSelection: true,
    },
    {
        id: "convert.hex-to-decimal",
        title: "Hex to Decimal",
        description: "Convert each line containing a hexadecimal integer (with or without 0x prefix) to its decimal representation. Non-numeric lines are passed through unchanged.",
        category: "Convert",
        keywords: ["convert", "hex", "hexadecimal", "decimal", "number", "radix"],
        fileTypes: ["text"],
        supportsSelection: true,
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
    },
    {
        id: "stats.count-lines",
        title: "Count Lines",
        description: "Count the number of lines in the document or selection.",
        category: "Stats",
        keywords: ["stats", "count", "lines", "rows"],
        fileTypes: ["text"],
        supportsSelection: true,
    },
    {
        id: "stats.count-words",
        title: "Count Words",
        description: "Count the number of whitespace-delimited words in the document or selection.",
        category: "Stats",
        keywords: ["stats", "count", "words", "tokens"],
        fileTypes: ["text"],
        supportsSelection: true,
    },
];

export function getTransformationActionsForFileType(
    fileType: FileType,
): TransformationActionDefinition[] {
    return transformationActions.filter((action) => action.fileTypes.includes(fileType));
}

export function getTransformationAction(
    actionId: TransformationActionId,
): TransformationActionDefinition | undefined {
    return transformationActions.find((action) => action.id === actionId);
}
