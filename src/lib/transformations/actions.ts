import type { FileType } from "$lib/state/editor.svelte";

export type JsonTransformationActionId =
    | "json.format"
    | "json.minify"
    | "json.validate";

export type TextTransformationActionId =
    | "text.trim-trailing-whitespace"
    | "text.collapse-blank-lines";

export type TransformationActionId =
    | JsonTransformationActionId
    | TextTransformationActionId;

export type TransformationMessageLevel = "success" | "error" | "info";

export type TransformationActionDefinition = {
    id: TransformationActionId;
    title: string;
    description: string;
    category: string;
    keywords: string[];
    fileTypes: FileType[];
    supportsSelection: boolean;
};

export type ExecuteTransformationRequest = {
    actionId: TransformationActionId;
    text: string;
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
