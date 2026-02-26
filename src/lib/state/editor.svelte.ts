export type FileType =
    | "text"
    | "csv"
    | "markdown"
    | "json"
    | "javascript"
    | "python"
    | "auto"; // plus other supported languages

export const editorState = $state<{
    fileType: FileType;
    csv: {
        showTable: boolean;
        serializing: boolean;
    };
    markdown: {
        showPreview: boolean;
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
});
