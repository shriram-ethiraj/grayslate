import { Marked, type Token } from "marked";

const BLOCK_TOKENS = new Set([
    "heading",
    "paragraph",
    "code",
    "blockquote",
    "list",
    "table",
    "hr",
    "html",
]);

function buildLineStarts(src: string): number[] {
    const starts = [0];
    for (let index = 0; index < src.length; index += 1) {
        if (src[index] === "\n") {
            starts.push(index + 1);
        }
    }
    return starts;
}

function offsetToLine(lineStarts: number[], offset: number): number {
    let low = 0;
    let high = lineStarts.length - 1;

    while (low < high) {
        const mid = (low + high + 1) >> 1;
        if (lineStarts[mid] <= offset) {
            low = mid;
        } else {
            high = mid - 1;
        }
    }

    return low + 1;
}

function escapeHtml(value: string): string {
    return value
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
}

export function renderMarkdownToHtml(src: string): string {
    if (!src) {
        return "";
    }

    const lineStarts = buildLineStarts(src);
    const tokenLineMap = new WeakMap<Token, number>();
    let searchOffset = 0;

    const markedInstance = new Marked();
    markedInstance.use({
        walkTokens(token) {
            if (token.raw && typeof token.raw === "string") {
                const index = src.indexOf(token.raw, searchOffset);
                if (index !== -1) {
                    tokenLineMap.set(token, offsetToLine(lineStarts, index));
                    if (BLOCK_TOKENS.has(token.type)) {
                        searchOffset = index + token.raw.length;
                    }
                }
            }
        },
        renderer: {
            heading(token) {
                const line = tokenLineMap.get(token);
                const text = this.parser.parseInline(token.tokens);
                const attr = line != null ? ` data-line="${line}"` : "";
                return `<h${token.depth}${attr}>${text}</h${token.depth}>\n`;
            },
            paragraph(token) {
                const line = tokenLineMap.get(token);
                const text = this.parser.parseInline(token.tokens);
                const attr = line != null ? ` data-line="${line}"` : "";
                return `<p${attr}>${text}</p>\n`;
            },
            code(token) {
                const line = tokenLineMap.get(token);
                const attr = line != null ? ` data-line="${line}"` : "";
                const langClass = token.lang ? ` class="language-${token.lang}"` : "";
                return `<pre${attr}><code${langClass}>${escapeHtml(token.text)}</code></pre>\n`;
            },
            blockquote(token) {
                const line = tokenLineMap.get(token);
                const body = this.parser.parse(token.tokens);
                const attr = line != null ? ` data-line="${line}"` : "";
                return `<blockquote${attr}>${body}</blockquote>\n`;
            },
            list(token) {
                const line = tokenLineMap.get(token);
                const tag = token.ordered ? "ol" : "ul";
                let body = "";

                for (const item of token.items) {
                    const itemLine = tokenLineMap.get(item);
                    const itemAttr = itemLine != null ? ` data-line="${itemLine}"` : "";
                    let itemBody = this.parser.parse(item.tokens);

                    if (item.task) {
                        const checked = item.checked
                            ? ' checked="" disabled=""'
                            : ' disabled=""';
                        itemBody = `<input type="checkbox"${checked}> ${itemBody}`;
                    }

                    body += `<li${itemAttr}>${itemBody}</li>\n`;
                }

                const attr = line != null ? ` data-line="${line}"` : "";
                const startAttr =
                    token.ordered && token.start !== 1 ? ` start="${token.start}"` : "";
                return `<${tag}${attr}${startAttr}>\n${body}</${tag}>\n`;
            },
            table(token) {
                const line = tokenLineMap.get(token);
                const attr = line != null ? ` data-line="${line}"` : "";

                let header = "<tr>";
                for (let index = 0; index < token.header.length; index += 1) {
                    const cell = token.header[index];
                    const align = token.align[index];
                    const alignAttr = align ? ` style="text-align:${align}"` : "";
                    const text = this.parser.parseInline(cell.tokens);
                    header += `<th${alignAttr}>${text}</th>`;
                }
                header += "</tr>\n";

                let body = "";
                for (const row of token.rows) {
                    body += "<tr>";
                    for (let index = 0; index < row.length; index += 1) {
                        const cell = row[index];
                        const align = token.align[index];
                        const alignAttr = align ? ` style="text-align:${align}"` : "";
                        const text = this.parser.parseInline(cell.tokens);
                        body += `<td${alignAttr}>${text}</td>`;
                    }
                    body += "</tr>\n";
                }

                return `<table${attr}>\n<thead>\n${header}</thead>\n<tbody>\n${body}</tbody>\n</table>\n`;
            },
            hr(token) {
                const line = tokenLineMap.get(token);
                const attr = line != null ? ` data-line="${line}"` : "";
                return `<hr${attr}>\n`;
            },
        },
    });

    return markedInstance.parse(src) as string;
}
