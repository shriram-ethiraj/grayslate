import type { Extension, Range } from "@codemirror/state";
import { ViewPlugin, Decoration, EditorView, WidgetType } from "@codemirror/view";
import { syntaxTree } from "@codemirror/language";
import type { ViewUpdate, DecorationSet } from "@codemirror/view";

class ArrayIndexWidget extends WidgetType {
    constructor(private readonly index: number) {
        super();
    }

    eq(other: ArrayIndexWidget) {
        return other.index === this.index;
    }

    toDOM() {
        const span = document.createElement("span");
        span.className = "cm-json-array-index";
        span.textContent = `${this.index}`;

        return span;
    }
}

const jsonInlayHintsTheme: Extension = EditorView.baseTheme({
    ".cm-json-array-index": {
        display: "inline-flex",
        alignItems: "center",
        boxSizing: "border-box",
        overflow: "hidden",
        whiteSpace: "nowrap",
        marginRight: "0.55em",
        padding: "0.08em 0.42em",
        borderRadius: "0.4em",
        backgroundClip: "padding-box",
        color: "var(--cm-hint-color, #888)",
        backgroundColor: "var(--cm-hint-bg, rgba(128, 128, 128, 0.1))",
        fontFamily: "inherit",
        fontSize: "0.84em",
        fontWeight: "500",
        lineHeight: "1.2",
        verticalAlign: "baseline",
        userSelect: "none",
        pointerEvents: "auto",
        cursor: "default",
        transition: "background-color 0.15s ease, color 0.15s ease",
    },
    ".cm-json-array-index:hover": {
        backgroundColor: "var(--cm-hint-bg-hover, rgba(128, 128, 128, 0.2))",
    },
});

const jsonInlayHintsPlugin = ViewPlugin.fromClass(class {
    decorations: DecorationSet;

    constructor(view: EditorView) {
        this.decorations = this.buildDecorations(view);
    }

    update(update: ViewUpdate) {
        if (update.docChanged || update.viewportChanged || syntaxTree(update.state) !== syntaxTree(update.startState)) {
            this.decorations = this.buildDecorations(update.view);
        }
    }

    buildDecorations(view: EditorView) {
        const widgets: Range<Decoration>[] = [];
        const tree = syntaxTree(view.state);

        for (let { from, to } of view.visibleRanges) {
            tree.iterate({
                from,
                to,
                enter: (node) => {
                    if (node.name === "Array") {
                        let index = 0;
                        let child = node.node.firstChild;
                        let widgetsCreated = 0;
                        while (child && widgetsCreated < 200) {
                            const name = child.name;
                            // Check if this child represents a value inside the array
                            if (name !== "[" && name !== "]" && name !== "," && name !== "⚠") {
                                // Add decoration if this element falls completely or partially in viewport
                                if (child.from >= from && child.from <= to) {
                                    widgets.push(Decoration.widget({
                                        widget: new ArrayIndexWidget(index),
                                        side: -1
                                    }).range(child.from));
                                    widgetsCreated++;
                                }
                                index++;
                            }
                            child = child.nextSibling;
                        }
                    }
                }
            });
        }

        return Decoration.set(widgets, true);
    }
}, {
    decorations: v => v.decorations
});

export const jsonInlayHints: Extension = [jsonInlayHintsPlugin, jsonInlayHintsTheme];
