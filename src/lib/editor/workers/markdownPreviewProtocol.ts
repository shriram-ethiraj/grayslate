export type MarkdownPreviewWorkerRequest = {
    type: "render";
    requestId: number;
    content: string;
};

export type MarkdownPreviewWorkerResponse =
    | {
          type: "result";
          requestId: number;
          html: string;
      }
    | {
          type: "error";
          requestId: number;
          error: string;
      };
