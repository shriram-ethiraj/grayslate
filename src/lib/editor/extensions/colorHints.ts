import { ViewPlugin, Decoration, WidgetType } from "@codemirror/view";
import type { EditorView, DecorationSet, ViewUpdate } from "@codemirror/view";
import type { Range } from "@codemirror/state";

/**
 * Matches color literals across all supported languages:
 *   - Hex:  #RGB  #RGBA  #RRGGBB  #RRGGBBAA
 *   - Functional: rgb() / rgba() / hsl() / hsla()
 *     (both legacy comma and modern space/slash syntax)
 */
const COLOR_RE =
    /#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{4}|[0-9a-fA-F]{3})\b|rgba?\s*\(\s*[\d.%]+\s*[,\s]\s*[\d.%]+\s*[,\s]\s*[\d.%]+(?:\s*[,/]\s*[\d.%]+)?\s*\)|hsla?\s*\(\s*[\d.]+(?:deg|rad|grad|turn)?\s*[,\s]\s*[\d.%]+%?\s*[,\s]\s*[\d.%]+%?(?:\s*[,/]\s*[\d.%]+)?\s*\)/gi;

let sharedCtx: CanvasRenderingContext2D | null = null;
function getCanvasCtx() {
    if (typeof document === "undefined") return null;
    if (!sharedCtx) {
        sharedCtx = document.createElement("canvas").getContext("2d", { willReadFrequently: true });
    }
    return sharedCtx;
}

const colorCache = new Map<string, string | null>();

/**
 * Release all cached color validation results and the shared canvas context.
 * Call on file switch to avoid cross-file cache pollution and free the canvas.
 */
export function clearColorCache(): void {
    colorCache.clear();
    if (sharedCtx) {
        sharedCtx = null;
    }
}

/**
 * Validates a matched color string by asking the browser to parse it.
 * Returns null if the browser rejects it (fillStyle stays at the default).
 */
function toCSSColor(raw: string): string | null {
    if (colorCache.has(raw)) return colorCache.get(raw)!;

    const ctx = getCanvasCtx();
    if (!ctx) return raw;

    ctx.fillStyle = "#000";
    ctx.fillStyle = raw;
    const normalised = raw.trim().toLowerCase();

    let result: string | null = raw;
    if (ctx.fillStyle === "#000000" && normalised !== "#000" && normalised !== "#000000") {
        result = null;
    }

    colorCache.set(raw, result);
    if (colorCache.size > 2000) {
        colorCache.clear();
    }
    return result;
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

class ColorSwatchWidget extends WidgetType {
    constructor(private readonly color: string) {
        super();
    }

    eq(other: ColorSwatchWidget) {
        return other.color === this.color;
    }

    toDOM() {
        const swatch = document.createElement("span");
        swatch.className = "cm-color-swatch";
        swatch.setAttribute("aria-label", `Color: ${this.color}`);

        Object.assign(swatch.style, {
            display: "inline-block",
            width: "0.8em",
            height: "0.8em",
            borderRadius: "2px",
            marginRight: "4px",
            verticalAlign: "middle",
            position: "relative",
            top: "-0.05em",
            // Resolves to #e2e8f0 (light) / #30343d (dark) via the app's CSS variables.
            border: "1px solid var(--border)",
            pointerEvents: "none",
            userSelect: "none",
            flexShrink: "0",
            // overflow:hidden clips the color overlay to the rounded corners
            // without needing border-radius on the child element.
            overflow: "hidden",
            // Neutral checkerboard so alpha transparency is visible on both themes.
            backgroundImage:
                "linear-gradient(45deg, #b0b0b0 25%, transparent 25%)," +
                "linear-gradient(-45deg, #b0b0b0 25%, transparent 25%)," +
                "linear-gradient(45deg, transparent 75%, #b0b0b0 75%)," +
                "linear-gradient(-45deg, transparent 75%, #b0b0b0 75%)",
            backgroundSize: "6px 6px",
            backgroundPosition: "0 0, 0 3px, 3px -3px, -3px 0px",
            backgroundColor: "#e8e8e8",
        });

        const overlay = document.createElement("span");
        Object.assign(overlay.style, {
            position: "absolute",
            inset: "0",
            backgroundColor: this.color,
        });

        swatch.appendChild(overlay);
        return swatch;
    }

    ignoreEvent() {
        return true;
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/**
 * CodeMirror ViewPlugin that renders an inline color swatch before every
 * recognized color literal in the visible viewport. Works across all
 * languages — no syntax tree dependency.
 */
export const colorHints = ViewPlugin.fromClass(
    class {
        decorations: DecorationSet;

        constructor(view: EditorView) {
            this.decorations = this.buildDecorations(view);
        }

        update(update: ViewUpdate) {
            if (update.docChanged || update.viewportChanged) {
                this.decorations = this.buildDecorations(update.view);
            }
        }

        buildDecorations(view: EditorView): DecorationSet {
            const widgets: Range<Decoration>[] = [];

            for (const { from, to } of view.visibleRanges) {
                const text = view.state.doc.sliceString(from, to);
                COLOR_RE.lastIndex = 0;

                let match: RegExpExecArray | null;
                while ((match = COLOR_RE.exec(text)) !== null) {
                    const cssColor = toCSSColor(match[0]);
                    if (!cssColor) continue;

                    // side: -1 positions the swatch immediately before the token.
                    widgets.push(
                        Decoration.widget({
                            widget: new ColorSwatchWidget(cssColor),
                            side: -1,
                        }).range(from + match.index)
                    );
                }
            }

            return Decoration.set(widgets, true);
        }
    },
    { decorations: (v) => v.decorations }
);
