import { snippetCompletion } from "@codemirror/autocomplete";
import type { CompletionContext, CompletionResult, Completion } from "@codemirror/autocomplete";
import { createElement, type IconNode } from "lucide";

export interface AutocompleteItem {
    snippet: string;
    label: string;
    type?: string;
    iconHtml?: string;
    iconNode?: IconNode;
    detailText?: string;
}

export interface AutocompleteConfig {
    triggerRegex: RegExp;
    validForRegex: RegExp;
    items: AutocompleteItem[];
}

export function createAutocompleteRenderer(iconHtml?: string, iconNode?: IconNode, detailText?: string) {
    if (!iconHtml && !iconNode && !detailText) return undefined;

    return (completion: Completion) => {
        const wrap = document.createElement("div");
        wrap.className = "flex items-center gap-2 w-full";

        if (iconHtml || iconNode) {
            const iconContainer = document.createElement("div");
            // Remove hardcoded text colors here; let them inherit from the parent <li> which handles hover colors
            iconContainer.className = "flex items-center justify-center w-5 h-5 opacity-70";

            if (iconNode) {
                const svg = createElement(iconNode);
                svg.setAttribute("width", "18");
                svg.setAttribute("height", "18");
                // svg.setAttribute("class", "lucide");
                iconContainer.appendChild(svg);
            } else if (iconHtml) {
                iconContainer.innerHTML = iconHtml;
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
            info: createAutocompleteRenderer(item.iconHtml, item.iconNode, item.detailText)
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
