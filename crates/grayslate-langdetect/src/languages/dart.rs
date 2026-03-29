use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition()-> LanguageDefinition {
    LanguageDefinition {
        name: "dart",
        extensions: &[".dart"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: None,
        structural_detect: None,
        keywords: &[
            "var", "final", "late", "required", "covariant", "deferred",
            "factory", "external", "part", "library", "export",
            "on", "show", "hide", "sync", "async", "yield",
            "rethrow", "assert", "typedef", "mixin", "with",
            "extension", "abstract", "sealed", "base", "get", "set",
        ],
        builtins: &[
            "print", "int", "double", "bool", "string", "list",
            "map", "set", "future", "stream", "widget",
            "statefulwidget", "statelesswidget", "buildcontext",
            "scaffold", "appbar", "column", "row", "center",
            "container", "text", "edgeinsetsall", "sizedbox",
            "navigator", "materialapp", "themedata",
        ],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Code],
        anchors: &[
            wp!(r#"(?m)^\s*import\s+['"]package:"#, 5),
            wp!(r"\bWidget\s+build\s*\(", 5),
            wp!(r"\b(StatelessWidget|StatefulWidget|State<\w+>)\b", 5),
            wp!(r"\blate\s+(final\s+)?\w+\s+\w+", 4),
            wp!(r"\brequired\s+this\.\w+", 4),
            // import 'dart:core' / 'dart:io' — Dart core libraries
            wp!(r#"(?m)^\s*import\s+['"]dart:"#, 5),
            // setState(() { }) — Flutter state management
            wp!(r"\bsetState\s*\(\s*\(\s*\)\s*\{", 5),
            // mixin declaration — Dart-specific
            wp!(r"(?m)^\s*(abstract\s+)?mixin\s+\w+", 4),
            // factory constructor — Dart-specific
            wp!(r"(?m)^\s*factory\s+\w+", 4),
            // Dart 3 class modifiers: sealed/base/final class
            wp!(r"(?m)^\s*(sealed|base)\s+class\s+\w+", 4),
        ],
        hints: &[
            wp!(r"\bFuture<\w+>", 3),
            wp!(r"(?m)^\s*void\s+main\s*\(\)\s*(async\s*)?\{", 3),
            wp!(r"\bfinal\s+\w+\s*=", 2),
            wp!(r"\b@override\b", 2),
            // BuildContext — Flutter framework type
            wp!(r"\bBuildContext\b", 3),
            // Stream< — Dart async streams
            wp!(r"\bStream<\w+>", 3),
            // extension on — Dart extension methods
            wp!(r"(?m)^\s*extension\s+\w+\s+on\s+", 3),
            // part of — Dart library system
            wp!(r"(?m)^\s*part\s+of\s+", 3),
            // Navigator.push — Flutter navigation
            wp!(r"\bNavigator\.\w+", 2),
            // @immutable annotation
            wp!(r"@immutable\b", 2),
        ],
        disqualifiers: &[],
    }
}
