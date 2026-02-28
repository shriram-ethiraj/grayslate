/**
 * languageExtensions.ts
 *
 * Maps a language identifier string to the corresponding CodeMirror 6
 * extension set.  Keeping this in its own module means Editor.svelte does
 * not have to import every language package directly, and the mapping is
 * easily unit-testable in isolation.
 */

import { json } from "@codemirror/lang-json";
import { javascript } from "@codemirror/lang-javascript";
import { python } from "@codemirror/lang-python";
import { html } from "@codemirror/lang-html";
import { css } from "@codemirror/lang-css";
import { yaml } from "@codemirror/lang-yaml";
import { cpp } from "@codemirror/lang-cpp";
import { java } from "@codemirror/lang-java";
import { go } from "@codemirror/lang-go";
import { xml } from "@codemirror/lang-xml";
import { csv } from "codemirror-lang-csv";
import { markdown } from "@codemirror/lang-markdown";
import { jsonInlayHints } from "$lib/editor/extensions/jsonInlayHints";
import { jsonFoldWidget } from "$lib/editor/extensions/jsonFoldWidget";
import { jsonKeyPath } from "$lib/editor/extensions/jsonKeyPath";
import { markdownAutocompleteProvider } from "$lib/editor/components/markdown/markdownAutocomplete";
import { autocompletion } from "@codemirror/autocomplete";
import type { Extension } from "@codemirror/state";

/**
 * Returns the CodeMirror extension (or extension array) for the given
 * language identifier.  Returns an empty array for unknown / plain-text
 * languages so callers can always spread or pass the return value directly.
 */
export function getLanguageExtension(langId: string): Extension | Extension[] {
    switch (langId) {
        case "json":
            return [json(), jsonInlayHints, jsonFoldWidget, jsonKeyPath];
        case "javascript":
            return javascript({ jsx: true });
        case "typescript":
            return javascript({ typescript: true, jsx: true });
        case "python":
            return python();
        case "html":
            return html();
        case "css":
            return css();
        case "yaml":
            return yaml();
        // cpp() covers both C and C++ syntax
        case "c":
        case "cpp":
            return cpp();
        case "java":
            return java();
        case "go":
            return go();
        case "xml":
            return xml();
        case "csv":
            return csv();
        case "shell":
        case "dockerfile":
            return [];  // Plain-text mode (no CM extension yet)
        case "markdown":
            return [
                markdown(),
                autocompletion({
                    override: [markdownAutocompleteProvider],
                }),
            ];
        default:
            return [];
    }
}
