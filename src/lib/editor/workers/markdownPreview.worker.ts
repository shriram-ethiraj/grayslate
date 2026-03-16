import { renderMarkdownToHtml } from "../components/markdown/renderMarkdown";
import type {
    MarkdownPreviewWorkerRequest,
    MarkdownPreviewWorkerResponse,
} from "./markdownPreviewProtocol";

function postResponse(response: MarkdownPreviewWorkerResponse): void {
    self.postMessage(response);
}

self.onmessage = (event: MessageEvent<MarkdownPreviewWorkerRequest>) => {
    const message = event.data;

    if (message.type !== "render") {
        return;
    }

    try {
        postResponse({
            type: "result",
            requestId: message.requestId,
            html: renderMarkdownToHtml(message.content),
        });
    } catch (error) {
        postResponse({
            type: "error",
            requestId: message.requestId,
            error:
                error instanceof Error
                    ? error.message
                    : "Unknown markdown preview render failure",
        });
    }
};
