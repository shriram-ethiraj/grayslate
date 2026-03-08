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
import { markdown } from "@codemirror/lang-markdown";
import { jsonInlayHints } from "$lib/editor/extensions/jsonInlayHints";
import { jsonFoldWidget } from "$lib/editor/extensions/jsonFoldWidget";
import { jsonKeyPath } from "$lib/editor/extensions/jsonKeyPath";
import { svelte } from "@replit/codemirror-lang-svelte";
import { rust } from "@codemirror/lang-rust";
import { clojure } from "@nextjournal/lang-clojure";
import { sql } from "@codemirror/lang-sql";
import { php } from "@codemirror/lang-php";
import { sass } from "@codemirror/lang-sass";
import { jinja } from "@codemirror/lang-jinja";
import { angular } from "@codemirror/lang-angular";
import { vue } from "@codemirror/lang-vue";

import { markdownAutocompleteProvider } from "$lib/editor/components/markdown/markdownAutocomplete";
import { autocompletion } from "@codemirror/autocomplete";
import { csvRainbowHighlight } from "$lib/editor/extensions/csvRainbowHighlight";
import type { Extension } from "@codemirror/state";
import { StreamLanguage } from "@codemirror/language";
import { shell } from "@codemirror/legacy-modes/mode/shell";
import { dockerFile } from "@codemirror/legacy-modes/mode/dockerfile";
import { nginx } from "@codemirror/legacy-modes/mode/nginx";
import { powerShell } from "@codemirror/legacy-modes/mode/powershell";
import { ruby } from "@codemirror/legacy-modes/mode/ruby";
import { swift } from "@codemirror/legacy-modes/mode/swift";
import { toml } from "@codemirror/legacy-modes/mode/toml";
import {
    kotlin,
    objectiveC,
    objectiveCpp,
    csharp,
    scala,
    dart,
} from "@codemirror/legacy-modes/mode/clike";

export interface LanguageExtensionOptions {
    /**
     * When `true`, strips heavy viewport-driven decorations (inlay hints,
     * fold widgets, key-path highlights) that fire on every scroll shift.
     * Lezer grammar (syntax highlighting) is kept — it runs incrementally.
     */
    lightweight?: boolean;
}

/**
 * Returns the CodeMirror extension (or extension array) for the given
 * language identifier.  Returns an empty array for unknown / plain-text
 * languages so callers can always spread or pass the return value directly.
 */
export function getLanguageExtension(
    langId: string,
    options?: LanguageExtensionOptions,
): Extension | Extension[] {
    const lightweight = options?.lightweight ?? false;
    switch (langId) {
        case "json":
            return lightweight
                ? [json()]
                : [json(), jsonInlayHints, jsonFoldWidget, jsonKeyPath];
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
        case "svelte":
            return svelte();
        case "vue":
            return vue();
        case "rust":
            return rust();
        case "clojure":
            return clojure();
        case "csv":
            // No Lezer grammar — the rainbow column highlighter IS the
            // CSV syntax highlighting.  The codemirror-lang-csv grammar
            // would tag everything as `tags.string`, causing the theme's
            // green string colour to override the rainbow colours once
            // the async parse completes.
            //
            // Each extension is self-contained and independently
            // publishable — compose them here for the full experience.
            // For very large CSVs the rainbow highlighter is dropped
            // because it rebuilds decorations on every viewport shift.
            return lightweight ? [] : [csvRainbowHighlight];
        case "shell":
            return StreamLanguage.define(shell);
        case "dockerfile":
            return StreamLanguage.define(dockerFile);
        case "markdown":
            return [
                markdown(),
                autocompletion({
                    override: [markdownAutocompleteProvider],
                }),
            ];
        case "sql":
            return sql();
        case "php":
            return php();
        case "sass":
            return sass({ indented: true });
        case "scss":
            return sass();
        case "jinja":
            return jinja();
        case "angular":
            return angular();
        case "nginx":
            return StreamLanguage.define(nginx);
        case "powershell":
            return StreamLanguage.define(powerShell);
        case "ruby":
            return StreamLanguage.define(ruby);
        case "swift":
            return StreamLanguage.define(swift);
        case "toml":
            return StreamLanguage.define(toml);
        case "kotlin":
            return StreamLanguage.define(kotlin);
        case "objectivec":
            return StreamLanguage.define(objectiveC);
        case "objectivecpp":
            return StreamLanguage.define(objectiveCpp);
        case "csharp":
            return StreamLanguage.define(csharp);
        case "scala":
            return StreamLanguage.define(scala);
        case "dart":
            return StreamLanguage.define(dart);
        default:
            return [];
    }
}
