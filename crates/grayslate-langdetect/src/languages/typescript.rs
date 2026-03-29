use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "typescript",
        extensions: &[".ts", ".tsx", ".mts", ".cts"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[r"\bdeno\b"],
        structural_priority: None,
        structural_detect: None,
        keywords: &[
            "interface", "type", "declare", "readonly", "enum",
            "satisfies", "keyof", "infer", "implements",
        ],
        builtins: &[
            "readonly", "partial", "record", "pick", "omit", "required",
            "exclude", "extract", "nonnullable", "returntype", "instancetype",
            "parameters", "awaited", "uppercase", "lowercase", "capitalize",
        ],
        // ── New family-gated fields ──────────────────────────
        content_families: &[ContentFamily::Code, ContentFamily::Config, ContentFamily::StructuredData],
        anchors: &[
            // Type annotation syntax — exclusive to TS in JS family
            wp!(r":\s*(string|number|boolean|void|any|never|unknown|undefined)\b", 4),
            wp!(r"(?m)\binterface\s+\w+", 4),
            wp!(r"(?m)\btype\s+\w+\s*=\s*", 4),
            wp!(r"(?m)\bdeclare\s+(const|function|class|module|type|interface)", 5),
            wp!(r"(?m)^///\s*<reference\s", 5),
            wp!(r"\b(keyof|infer|satisfies)\s+", 5),
            wp!(r"\bas\s+(string|number|any|unknown|[A-Z]\w+)\b", 4),
            wp!(r"\bas\s+const\b", 4),
            // Utility types — TS-exclusive generics
            wp!(r"\b(Readonly|Partial|Record|Pick|Omit|Required)<", 4),
            wp!(r"\b(Exclude|Extract|NonNullable|ReturnType|InstanceType|Parameters)<", 4),
            // ── TS-only module syntax (never valid in JS) ──
            wp!(r"(?m)^\s*import\s+type\s+", 5),
            wp!(r"(?m)^\s*export\s+type\s*\{", 5),
            wp!(r"import\s*\{[^}]*\btype\s+\w+", 4),
            // ── TS-only keywords (never valid in plain JS) ──
            wp!(r"(?m)\benum\s+\w+\s*\{", 4),
            wp!(r"(?m)\bnamespace\s+\w+\s*\{", 4),
            wp!(r"(?m)\babstract\s+class\s+\w+", 4),
            // Non-null assertion operator: expr!.prop (TS-only)
            wp!(r"\w+!\.\w+", 4),
            // Function parameter with type: (param: Type)
            wp!(r"\(\s*\w+\s*:\s*[A-Z]\w+", 4),
            // ── Variable-level type annotations (TS-exclusive) ──
            // const x: SomeType, let y: React.FC, var z: Props
            wp!(r"(?m)\b(const|let|var)\s+\w+\s*:\s*[A-Z][\w.]+", 4),
            // ── React TypeScript types (TS-only, not runtime values) ──
            wp!(r"\bReact\.(FC|FunctionComponent|ComponentType|ReactElement|ReactNode|PropsWithChildren|Dispatch|SetStateAction)\b", 4),
            // JSX.Element return type (TS-only)
            wp!(r"\bJSX\.Element\b", 4),
        ],
        hints: &[
            wp!(r"\bPromise<", 2),
            // Generic type parameter syntax: <T>, <T extends U>
            wp!(r"<[A-Z]\w*(?:\s+extends\s+\w+)?>", 3),
            // Optional property: name?: type
            wp!(r"\w+\?\s*:\s*(string|number|boolean|any|\w+)", 3),
            // Union types with primitives
            wp!(r"\|\s*(string|number|boolean|null|undefined)\b", 3),
            // Typed array: : Type[]
            wp!(r":\s*\w+\[\]", 2),
            // Generic function: function name<T>
            wp!(r"(?m)\bfunction\s+\w+\s*<\w+", 3),
            // React with TS generics (React.FC<Props>, React.ComponentProps<...>)
            wp!(r"\bReact\.(FC|FunctionComponent|ComponentProps|ComponentType|ReactNode)<", 3),
            // implements keyword (TS-specific in JS world)
            wp!(r"\bimplements\s+\w+", 3),
            // readonly modifier
            wp!(r"\breadonly\s+\w+", 2),
            // Generic function call: useState<Type>(), createContext<Type>()
            wp!(r"\b\w+<[A-Z]\w+(?:\s*,\s*[A-Z]\w+)*>\s*\(", 3),
            // Return type annotation: ): Type or ): Promise<Type>
            wp!(r"\)\s*:\s*[A-Z]\w+", 2),
            // Angle bracket type assertion (legacy syntax): <Type>expr
            wp!(r"<(string|number|boolean|any|[A-Z]\w+)>\w+", 2),
            // Generic constraint: extends keyword in generics
            wp!(r"\bextends\s+(string|number|boolean|object|[A-Z]\w+)\b", 2),
        ],
        disqualifiers: &[
            // C/C++ preprocessor — never valid TypeScript
            wp!(r"(?m)^#include\s", 1),
            wp!(r"(?m)^#define\s", 1),
            wp!(r"(?m)^#(?:ifndef|ifdef|endif)\b", 1),
        ],
    }
}
