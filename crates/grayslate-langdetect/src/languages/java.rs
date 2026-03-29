use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "java",
        extensions: &[".java"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        keywords: &[
            "synchronized", "strictfp", "transient", "volatile", "native",
            "extends", "implements", "throws", "package", "final",
            "abstract", "static", "private", "protected", "public",
        ],
        builtins: &[
            "system", "override", "deprecated", "suppresswarnings",
            "arraylist", "hashmap", "hashset", "linkedlist", "treemap",
            "iterator", "comparable", "runnable", "serializable",
            "inputstream", "outputstream", "bufferedreader",
        ],
        // ── Family-gated fields ───────────────────────────────
        content_families: &[ContentFamily::Code],
        anchors: &[
            wp!(r"\bpublic\s+static\s+void\s+main", 5),
            wp!(r"\bSystem\.out\.print(ln)?\s*\(", 5),
            wp!(r"(?m)\bimport\s+java\.\w+", 5),
            wp!(r"(?m)\bimport\s+javax\.\w+", 5),
            wp!(r"@Override\b", 4),
            wp!(r"(?m)\bthrows\s+\w+", 4),
            wp!(r"\binstanceof\s+", 4),
            // Java array declarations: `String[] args`, `int[] values`
            wp!(r"\bString\s*\[\s*\]\s+\w+", 4),
            // @FunctionalInterface — Java-only annotation
            wp!(r"@FunctionalInterface\b", 4),
            // package declaration
            wp!(r"(?m)^\s*package\s+[\w.]+;", 4),
            // Java access modifier + class/interface
            wp!(r"(?m)\bpublic\s+(class|interface|enum)\s+\w+", 4),
        ],
        hints: &[
            wp!(r"(?m)\bpublic\s+class\s+\w+", 3),
            wp!(r"(?m)\bprivate\s+(final\s+)?\w+\s+\w+", 2),
            wp!(r"(?m)\bprotected\s+", 2),
            wp!(r"\bimplements\s+\w+", 2),
            wp!(r"\bnew\s+ArrayList<", 2),
            wp!(r"\bsynchronized\s*[(\{]", 2),
            wp!(r"@SuppressWarnings\b", 2),
            // `final Type name =` — Java constant/variable pattern
            wp!(r"\bfinal\s+\w+\s+\w+\s*=", 2),
            // Java class literal: `MyClass.class`
            wp!(r"\w+\.class\b", 2),
            // import org.* / com.* — JVM ecosystem
            wp!(r"(?m)\bimport\s+(org|com|io)\.\w+", 3),
            // Java-style generic: List<Type>, Map<K,V>
            wp!(r"\b(List|Map|Set|Optional|Stream)<", 2),
        ],
        disqualifiers: &[
            // Kotlin signals
            wp!(r"\bfun\s+\w+", -5),
            wp!(r"\bdata\s+class\s+\w+", -5),
            // Scala signals
            wp!(r"\bimplicit\s+(val|def|class)\b", -5),
        ],
    }
}