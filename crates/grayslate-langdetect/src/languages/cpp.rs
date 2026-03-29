use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "cpp",
        extensions: &[".cpp", ".cxx", ".cc", ".hpp", ".hxx", ".hh"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        keywords: &[
            "constexpr", "consteval", "constinit", "decltype", "noexcept",
            "nullptr", "template", "typename", "virtual", "override",
            "final", "explicit", "mutable", "namespace", "using",
            "concept", "requires", "co_await", "co_return", "co_yield",
            "static_cast", "dynamic_cast", "reinterpret_cast", "const_cast",
            "static_assert", "thread_local", "alignas", "alignof",
        ],
        builtins: &[
            "cout", "cin", "cerr", "endl", "string", "vector", "map",
            "set", "deque", "list", "queue", "stack", "pair",
            "unique_ptr", "shared_ptr", "weak_ptr", "optional",
            "variant", "mutex", "future", "promise", "tuple",
            "array", "unordered_map", "unordered_set", "bitset",
        ],
        // ── New family-gated fields ──────────────────────────
        content_families: &[ContentFamily::Code],
        anchors: &[
            wp!(r"\bstd::\w+", 5),
            wp!(r"\bcout\s*<<", 5),
            wp!(r"\bcin\s*>>", 5),
            wp!(r"(?m)#include\s*<(iostream|string|vector|map|set|algorithm|memory|functional|fstream|sstream|numeric|chrono|thread|mutex|atomic|cassert|cstdint)>", 5),
            wp!(r"\busing\s+namespace\s+std\b", 5),
            wp!(r"(?m)\btemplate\s*<", 4),
            wp!(r"\bnullptr\b", 4),
            wp!(r"\b(unique_ptr|shared_ptr|weak_ptr)<", 4),
            wp!(r"\b(static_cast|reinterpret_cast|dynamic_cast|const_cast)\s*<", 4),
            // C++ lambdas: [capture](params){ body }
            wp!(r"\[[\w&=, ]*\]\s*\([^)]*\)\s*(\{|->)", 4),
            // Move semantics
            wp!(r"\bstd::(move|forward)\s*\(", 4),
            // Modern C++ concepts/requires
            wp!(r"\bconcept\s+\w+", 4),
            wp!(r"\brequires\s+", 3),
            // Class with inheritance — C++ exclusive (C has no class)
            wp!(r"(?m)\bclass\s+\w+\s*:\s*(public|private|protected)\s+", 5),
            // namespace with braces — strong C++ signal
            wp!(r"(?m)\bnamespace\s+\w+\s*\{", 4),
            // C++ includes with .h that are C++ wrappers (absl/, llvm/, tensorflow/)
            wp!(r#"(?m)#include\s*"[^"]*\.(h|hpp|hxx)""#, 4),
        ],
        hints: &[
            wp!(r"(?m)\bclass\s+\w+\s*[:\{]", 2),
            wp!(r"\bconstexpr\b", 3),
            wp!(r"\bvirtual\s+", 2),
            wp!(r"\bnew\s+\w+", 2),
            wp!(r"\bdelete\s+\w+", 2),
            wp!(r"\w+::\w+", 2),
            // Trailing return type: auto foo() -> int
            wp!(r"\bauto\s+\w+\s*\([^)]*\)\s*->", 3),
            // consteval / constinit — C++20
            wp!(r"\b(consteval|constinit)\b", 3),
            // enum class — C++ scoped enum
            wp!(r"\benum\s+class\s+\w+", 3),
            // Range-based for: for (auto& x : vec)
            wp!(r"\bfor\s*\(\s*(const\s+)?auto\s*&?\s+\w+\s*:", 3),
            // try/catch with std::exception
            wp!(r"\bcatch\s*\(\s*(const\s+)?std::", 3),
            // #endif (header guard — present in C++ headers too)
            wp!(r"(?m)^#endif\b", 2),
            // #pragma once
            wp!(r"(?m)#pragma\s+once\b", 2),
            // override / final keywords
            wp!(r"\b(override|final)\b", 2),
        ],
        disqualifiers: &[],
    }
}
