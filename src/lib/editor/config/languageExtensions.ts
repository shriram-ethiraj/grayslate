import type { Extension } from "@codemirror/state";

type LanguageExtensionLoader = () => Promise<Extension>;

const extensionPromises = new Map<string, Promise<Extension>>();

function canonicalLanguageId(language: string): string {
    switch (language) {
        case "jsonl": return "json";
        case "c": return "cpp";
        case "cmd": return "shell";
        default: return language;
    }
}

const loaders: Record<string, LanguageExtensionLoader> = {
    json: async () => {
        const [language, inlay, fold, keyPath] = await Promise.all([
            import("@codemirror/lang-json"),
            import("$lib/editor/extensions/jsonInlayHints"),
            import("$lib/editor/extensions/jsonFoldWidget"),
            import("$lib/editor/extensions/jsonKeyPath"),
        ]);
        return [language.json(), inlay.jsonInlayHints, fold.jsonFoldWidget, keyPath.jsonKeyPath];
    },
    javascript: async () => (await import("@codemirror/lang-javascript")).javascript({ jsx: true }),
    typescript: async () => (await import("@codemirror/lang-javascript")).javascript({
        typescript: true,
        jsx: true,
    }),
    python: async () => (await import("@codemirror/lang-python")).python(),
    html: async () => (await import("@codemirror/lang-html")).html(),
    css: async () => (await import("@codemirror/lang-css")).css(),
    yaml: async () => (await import("@codemirror/lang-yaml")).yaml(),
    cpp: async () => (await import("@codemirror/lang-cpp")).cpp(),
    java: async () => (await import("@codemirror/lang-java")).java(),
    go: async () => (await import("@codemirror/lang-go")).go(),
    xml: async () => (await import("@codemirror/lang-xml")).xml(),
    svelte: async () => (await import("@replit/codemirror-lang-svelte")).svelte(),
    vue: async () => (await import("@codemirror/lang-vue")).vue(),
    rust: async () => (await import("@codemirror/lang-rust")).rust(),
    clojure: async () => (await import("@nextjournal/lang-clojure")).clojure(),
    sql: async () => (await import("@codemirror/lang-sql")).sql(),
    php: async () => (await import("@codemirror/lang-php")).php(),
    sass: async () => (await import("@codemirror/lang-sass")).sass({ indented: true }),
    scss: async () => (await import("@codemirror/lang-sass")).sass(),
    jinja: async () => (await import("@codemirror/lang-jinja")).jinja(),
    angular: async () => (await import("@codemirror/lang-angular")).angular(),
    csv: async () => {
        const [rainbow, tooltip] = await Promise.all([
            import("$lib/editor/extensions/csvRainbowHighlight"),
            import("$lib/editor/extensions/csvCellHeaderTooltip"),
        ]);
        return [rainbow.csvRainbowHighlight, tooltip.csvCellHeaderTooltip];
    },
    markdown: async () => {
        const [language, autocomplete, provider, display] = await Promise.all([
            import("@codemirror/lang-markdown"),
            import("@codemirror/autocomplete"),
            import("$lib/editor/components/markdown/markdownAutocomplete"),
            import("$lib/editor/extensions/autocompleteFactory"),
        ]);
        return [
            language.markdown(),
            autocomplete.autocompletion({
                ...display.autocompleteDisplayConfig,
                override: [provider.markdownAutocompleteProvider],
            }),
        ];
    },
    shell: async () => {
        const [language, mode] = await Promise.all([
            import("@codemirror/language"),
            import("@codemirror/legacy-modes/mode/shell"),
        ]);
        return language.StreamLanguage.define(mode.shell);
    },
    dockerfile: async () => {
        const [language, mode] = await Promise.all([
            import("@codemirror/language"),
            import("@codemirror/legacy-modes/mode/dockerfile"),
        ]);
        return language.StreamLanguage.define(mode.dockerFile);
    },
    nginx: async () => {
        const [language, mode] = await Promise.all([
            import("@codemirror/language"),
            import("@codemirror/legacy-modes/mode/nginx"),
        ]);
        return language.StreamLanguage.define(mode.nginx);
    },
    powershell: async () => {
        const [language, mode] = await Promise.all([
            import("@codemirror/language"),
            import("@codemirror/legacy-modes/mode/powershell"),
        ]);
        return language.StreamLanguage.define(mode.powerShell);
    },
    perl: async () => {
        const [language, mode] = await Promise.all([
            import("@codemirror/language"),
            import("@codemirror/legacy-modes/mode/perl"),
        ]);
        return language.StreamLanguage.define(mode.perl);
    },
    ruby: async () => {
        const [language, mode] = await Promise.all([
            import("@codemirror/language"),
            import("@codemirror/legacy-modes/mode/ruby"),
        ]);
        return language.StreamLanguage.define(mode.ruby);
    },
    swift: async () => {
        const [language, mode] = await Promise.all([
            import("@codemirror/language"),
            import("@codemirror/legacy-modes/mode/swift"),
        ]);
        return language.StreamLanguage.define(mode.swift);
    },
    toml: async () => {
        const [language, mode] = await Promise.all([
            import("@codemirror/language"),
            import("@codemirror/legacy-modes/mode/toml"),
        ]);
        return language.StreamLanguage.define(mode.toml);
    },
    kotlin: () => loadCLikeMode("kotlin"),
    objectivec: () => loadCLikeMode("objectiveC"),
    objectivecpp: () => loadCLikeMode("objectiveCpp"),
    csharp: () => loadCLikeMode("csharp"),
    scala: () => loadCLikeMode("scala"),
    dart: () => loadCLikeMode("dart"),
};

type CLikeModeName = "kotlin" | "objectiveC" | "objectiveCpp" | "csharp" | "scala" | "dart";

async function loadCLikeMode(modeName: CLikeModeName): Promise<Extension> {
    const [language, modes] = await Promise.all([
        import("@codemirror/language"),
        import("@codemirror/legacy-modes/mode/clike"),
    ]);
    return language.StreamLanguage.define(modes[modeName]);
}

/** Load and cache only the active document's language support in this webview. */
export function loadLanguageExtension(language: string): Promise<Extension> {
    const languageId = canonicalLanguageId(language);
    const loader = loaders[languageId];
    if (!loader) return Promise.resolve([]);

    const cached = extensionPromises.get(languageId);
    if (cached) return cached;

    const loading = loader().catch((error: unknown) => {
        extensionPromises.delete(languageId);
        throw error;
    });
    extensionPromises.set(languageId, loading);
    return loading;
}
