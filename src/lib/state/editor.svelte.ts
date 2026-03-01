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
    | "shell"
    | "dockerfile"
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
    loader: {
        visible: boolean;
        message: string;
        subMessage: string;
        /** 0-100. Use -1 for indeterminate (pulsing bar). */
        progress: number;
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
        progress: -1,
    },
});

/** Show the editor-area loader overlay with optional sub-message and progress. */
export function showEditorLoader(message: string, subMessage = "", progress = -1) {
    editorState.loader.visible = true;
    editorState.loader.message = message;
    editorState.loader.subMessage = subMessage;
    editorState.loader.progress = progress;
}

/** Update loader progress and labels without toggling visibility. */
export function updateEditorLoader(message: string, subMessage = "", progress = -1) {
    editorState.loader.message = message;
    editorState.loader.subMessage = subMessage;
    editorState.loader.progress = progress;
}

/** Hide the editor-area loader overlay. */
export function hideEditorLoader() {
    editorState.loader.visible = false;
    editorState.loader.message = "";
    editorState.loader.subMessage = "";
    editorState.loader.progress = -1;
}
