export type FileType =
    | "text"
    | "csv"
    | "markdown"
    | "json"
    | "javascript"
    | "typescript"
    | "python"
    | "html"
    | "css"
    | "yaml"
    | "c"
    | "cpp"
    | "java"
    | "go"
    | "xml"
    | "auto";

export const editorState = $state<{
    fileType: FileType;
    csv: {
        showTable: boolean;
        serializing: boolean;
    };
    markdown: {
        showPreview: boolean;
    };
    /** Shared overlay loader for the editor content area. */
    loader: {
        visible: boolean;
        message: string;
        subMessage: string;
    };
}>({
    fileType: "text",
    csv: {
        showTable: false,
        serializing: false,
    },
    markdown: {
        showPreview: true,
    },
    loader: {
        visible: false,
        message: "",
        subMessage: "",
    },
});

/** Show the editor-area loader overlay with an optional sub-message. */
export function showEditorLoader(message: string, subMessage = "") {
    editorState.loader.visible = true;
    editorState.loader.message = message;
    editorState.loader.subMessage = subMessage;
}

/** Hide the editor-area loader overlay. */
export function hideEditorLoader() {
    editorState.loader.visible = false;
    editorState.loader.message = "";
    editorState.loader.subMessage = "";
}
