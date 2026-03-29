use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition()-> LanguageDefinition {
    LanguageDefinition {
        name: "clojure",
        extensions: &[".clj", ".cljs", ".cljc", ".edn"],
        filenames: &["deps.edn"],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        keywords: &[
            "defn", "defmacro", "defonce", "defprotocol", "defstruct",
            "defmulti", "defmethod", "deftype", "defrecord", "ns",
            "require", "recur", "binding", "doseq", "dotimes",
            "cond", "when", "if-let", "when-let", "loop", "fn", "let",
            "defproject", "deftest",
        ],
        builtins: &[
            "conj", "assoc", "dissoc", "merge", "first", "rest",
            "seq", "vec", "map", "filter", "reduce", "range",
            "repeat", "cycle", "take", "drop", "partition",
            "sort", "reverse", "count", "into", "atom", "deref",
            "swap", "reset", "comp", "partial", "juxt",
        ],
        content_families: &[ContentFamily::Code],
        anchors: &[
            wp!(r"(?m)^\s*\(ns\s+[\w.\-]+", 5),
            wp!(r"\(defn\s+\w+", 5),
            wp!(r"\(defproject\s+\w+", 5),
            wp!(r"\(deftest\s+\w+", 5),
            wp!(r#"\(require\s+'"#, 4),
            wp!(r"\(:require\s+\[", 4),
            wp!(r"\(defmacro\s+\w+", 5),
            wp!(r"\(defrecord\s+\w+", 5),
            wp!(r"\(defprotocol\s+\w+", 5),
        ],
        hints: &[
            wp!(r"\(def\s+\w+", 3),
            wp!(r"\(let\s+\[", 3),
            wp!(r"\(cond\s", 3),
            wp!(r"\(assoc\s", 3),
            wp!(r"\(-> ", 3),
            wp!(r"\(->> ", 3),
            wp!(r#"\(import\s+'"#, 3),
            wp!(r"[\s(]:\w[\w\-]*[\s)]", 2),
            wp!(r"#\(", 2),
            wp!(r"@\w+", 2),
            wp!(r"\(defmulti\s+\w+", 3),
            wp!(r"\(reduce\s+", 2),
        ],
        disqualifiers: &[
            wp!(r"(?m)[.#][\w\-]+\s*\{[^}]*(color|margin|padding|display)\s*:", -5),
            wp!(r"@media\s*[\s(]", -5),
            wp!(r"<(div|span|p|a|form|input|table|tr|td|ul|li|h[1-6])\b", -5),
            wp!(r"(?m)^\s*(pub\s+)?(fn|struct|enum|mod|trait|impl|use)\s", -5),
            wp!(r"(?m)^\s*(const|let|var)\s+\w+\s*=", -5),
        ],
    }
}