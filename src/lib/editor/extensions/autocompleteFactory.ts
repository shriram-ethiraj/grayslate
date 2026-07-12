import { snippetCompletion } from "@codemirror/autocomplete";
import type { CompletionContext, CompletionResult, Completion } from "@codemirror/autocomplete";

export interface AutocompleteItem {
    snippet: string;
    label: string;
    type?: string;
    /** Raw inline SVG markup, e.g. imported via `~icons/lucide/heading-1?raw` */
    iconData?: string;
    detailText?: string;
}

export interface AutocompleteConfig {
    triggerRegex: RegExp;
    validForRegex: RegExp;
    items: AutocompleteItem[];
}

interface AutocompleteCompletion extends Completion {
    iconData?: string;
}

function sizeAutocompleteSvg(svgNode: SVGElement | null) {
    if (!svgNode) return;
    svgNode.setAttribute("width", "18");
    svgNode.setAttribute("height", "18");
}

function createAutocompleteIcon(iconData?: string) {
    if (!iconData) return null;

    const iconContainer = document.createElement("div");
    iconContainer.className = "cm-autocomplete-option-icon";

    iconContainer.innerHTML = iconData;
    sizeAutocompleteSvg(iconContainer.querySelector("svg"));
    return iconContainer;
}

export const autocompleteDisplayConfig = {
    icons: false,
    addToOptions: [
        {
            position: 20,
            render(completion: Completion) {
                return createAutocompleteIcon((completion as AutocompleteCompletion).iconData);
            },
        },
    ],
};

export function createAutocompleteProvider(config: AutocompleteConfig) {
    const completions: AutocompleteCompletion[] = config.items.map(item => {
        const completion = snippetCompletion(item.snippet, {
            label: item.label,
            detail: item.detailText,
            type: item.type || "text",
        });

        return {
            ...completion,
            iconData: item.iconData,
        };
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
