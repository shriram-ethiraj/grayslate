use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "python",
        extensions: &[".py", ".pyi", ".pyw"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[r"\bpython[23w]?\b"],
        structural_priority: None,
        structural_detect: None,
        keywords: &[
            "elif", "except", "lambda", "nonlocal", "pass", "raise", "yield",
            "assert", "del", "global", "with", "async", "await", "def",
        ],
        builtins: &[
            "enumerate", "isinstance", "issubclass", "classmethod", "staticmethod",
            "property", "delattr", "getattr", "hasattr", "setattr", "callable",
            "frozenset", "memoryview", "bytearray", "reversed", "breakpoint",
            "__init__", "__name__", "__main__", "__all__", "__file__",
            "__version__", "__author__", "__doc__", "__dict__", "__slots__",
        ],
        // ── Family-gated fields ───────────────────────────────
        content_families: &[ContentFamily::Code, ContentFamily::Config],
        anchors: &[
            wp!(r"(?m)^\s*elif\s+", 5),
            wp!(r#"if\s+__name__\s*==\s*['"]__main__['"]"#, 5),
            wp!(r"(?m)^>>> ", 5),
            // def __init__(self — Python constructor
            wp!(r"def\s+__init__\s*\(self", 5),
            // from __future__ import — Python-exclusive future imports
            wp!(r"(?m)^\s*from\s+__future__\s+import\s", 5),
            wp!(r"(?m)^\s*def\s+\w+\s*\(self[\s,)]", 4),
            wp!(r"(?m)^\s*async\s+def\s+\w+", 4),
            wp!(r"(?m)^__all__\s*=\s*[\[\(]", 4),
            // -> Type: — return type annotation
            wp!(r"->\s*\w+\s*:", 4),
            // except ValueError as e: — Python exception binding
            wp!(r"\bexcept\s+\w+\s+as\s+\w+\s*:", 4),
            // import module — Python bare import (not from..import)
            wp!(r"(?m)^\s*import\s+\w+", 4),
            // from module import — Python import style
            wp!(r"(?m)^\s*from\s+\w[\w.]*\s+import\s", 4),
            // class Foo: or class Foo(Base): — Python class declaration
            wp!(r"(?m)^\s*class\s+\w+\s*[\(:]", 4),
            // Function parameter type annotations: def foo(x: str, y: int)
            wp!(r"def\s+\w+\s*\([^)]*:\s*(str|int|float|bool|list|dict|tuple|set|None|Any|Optional)\b", 4),
        ],
        hints: &[
            wp!(r"(?m)^\s*def\s+\w+\s*\(", 3),
            wp!(r"\bself\.\w+", 3),
            wp!(r#"f['"][^'"]*\{[^}]+\}"#, 3),
            // @property / @staticmethod / @classmethod — Python built-in decorators
            wp!(r"@(property|staticmethod|classmethod)\b", 3),
            // yield from — Python delegated generator
            wp!(r"\byield\s+from\b", 3),
            wp!(r"\b__\w+__\b", 2),
            wp!(r"(?m)^\s*@\w+(\.\w+)*(\(.*\))?\s*$", 2),
            wp!(r"(?m)^\s*(try|except|finally)\s*:", 2),
            // with open() — Python file context manager
            wp!(r"\bwith\s+open\s*\(", 2),
            // := walrus operator (Python 3.8+)
            wp!(r":=", 2),
            // print() function call
            wp!(r"\bprint\s*\(", 2),
            // Python triple-quote docstrings
            wp!(r#"(?m)^\s*"""|\s*'''"#, 2),
            // None, True, False — Python-specific boolean literals
            wp!(r"\bNone\b", 2),
        ],
        disqualifiers: &[],
    }
}
