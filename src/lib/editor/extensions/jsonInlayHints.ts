import { ViewPlugin, Decoration, EditorView } from "@codemirror/view";
import { syntaxTree } from "@codemirror/language";
import { WidgetType } from "@codemirror/view";
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

        // Styling matches VS Code / IntelliJ inlay hints style roughly
        span.style.color = "var(--cm-hint-color, #888)";
        span.style.fontSize = "0.9em";
        span.style.marginRight = "6px";
        span.style.padding = "0px 4px";
        span.style.borderRadius = "3px";
        span.style.backgroundColor = "var(--cm-hint-bg, rgba(128, 128, 128, 0.1))";
        span.style.userSelect = "none";
        span.style.pointerEvents = "none";
        span.style.fontFamily = "monospace";

        return span;
    }
}

export const jsonInlayHints = ViewPlugin.fromClass(class {
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
        const widgets: any[] = [];
        const tree = syntaxTree(view.state);

        for (let { from, to } of view.visibleRanges) {
            tree.iterate({
                from,
                to,
                enter: (node) => {
                    if (node.name === "Array") {
                        let index = 0;
                        let child = node.node.firstChild;
                        while (child) {
                            const name = child.name;
                            // Check if this child represents a value inside the array
                            if (name !== "[" && name !== "]" && name !== "," && name !== "⚠") {
                                // Add decoration if this element falls completely or partially in viewport
                                if (child.from >= from && child.from <= to) {
                                    widgets.push(Decoration.widget({
                                        widget: new ArrayIndexWidget(index),
                                        side: -1
                                    }).range(child.from));
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
