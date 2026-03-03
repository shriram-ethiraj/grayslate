import { snippetCompletion } from "@codemirror/autocomplete";
import type { CompletionContext, CompletionResult, Completion } from "@codemirror/autocomplete";
import { iconExists, loadIcon, renderSVG } from "@iconify/iconify";

export interface AutocompleteItem {
    snippet: string;
    label: string;
    type?: string;
    /** Can be an Iconify icon name like 'lucide:heading-1' or raw inline HTML <svg>...</svg> */
    iconData?: string;
    detailText?: string;
}

export interface AutocompleteConfig {
    triggerRegex: RegExp;
    validForRegex: RegExp;
    items: AutocompleteItem[];
}

export function createAutocompleteRenderer(iconData?: string, detailText?: string) {
    if (!iconData && !detailText) return undefined;

    return (completion: Completion) => {
        const wrap = document.createElement("div");
        wrap.className = "flex items-center gap-2 w-full";

        if (iconData) {
            const iconContainer = document.createElement("div");
            // Remove hardcoded text colors here; let them inherit from the parent <li> which handles hover colors
            iconContainer.className = "flex items-center justify-center w-5 h-5 opacity-70";

            if (iconData.startsWith("<")) {
                // Render as raw HTML directly
                iconContainer.innerHTML = iconData;

                // Enforce consistent sizing internally
                const svgNode = iconContainer.querySelector('svg');
                if (svgNode) {
                    svgNode.setAttribute("width", "18");
                    svgNode.setAttribute("height", "18");
                }
            } else {
                // Must be an iconify icon name!
                if (iconExists(iconData)) {
                    const svgEl = renderSVG(iconData, { width: "18", height: "18" });
                    if (svgEl) iconContainer.appendChild(svgEl);
                } else {
                    // Start loading, render as empty for now, then dynamically inject!
                    loadIcon(iconData).then(() => {
                        const svgEl = renderSVG(iconData, { width: "18", height: "18" });
                        if (svgEl && iconContainer.isConnected) {
                            iconContainer.appendChild(svgEl);
                        }
                    });
                }
            }

            wrap.appendChild(iconContainer);
        }

        if (detailText) {
            const detailSpan = document.createElement("span");
            // Remove hardcoded text colors here; let them inherit from the parent <li>
            detailSpan.className = "text-sm font-medium opacity-80";
            detailSpan.textContent = detailText;
            wrap.appendChild(detailSpan);
        }

        return wrap;
    };
}

export function createAutocompleteProvider(config: AutocompleteConfig) {
    const completions: Completion[] = config.items.map(item => {
        return snippetCompletion(item.snippet, {
            label: item.label,
            type: item.type || "text",
            info: createAutocompleteRenderer(item.iconData, item.detailText)
        });
    });

    return function provider(context: CompletionContext): CompletionResult | null {
        const word = context.matchBefore(config.triggerRegex);

        if (!word) {
            return null;
        }

        // Only offer completion if explicit or if we actually matched the pattern
        if (word.from === word.to && !context.explicit) {
            return null;
        }

        return {
            from: word.from,
            options: completions,
            validFor: config.validForRegex,
        };
    };
}
