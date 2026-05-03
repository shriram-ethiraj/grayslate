/// Structural signal detection (two-tier).
///
/// **Strong detectors** (Phase 0c) are near-deterministic and run before the
/// family classifier. They have very low false-positive rates.
///
///   JSON(5) → PHP(10) → Svelte(20) → Vue(30) → HTML(40) → XML(50)
///   → Dockerfile(60) → CSV(70)
///
/// **Soft detectors** (Phase 2a) run AFTER the family classifier and are
/// gated by content family. This prevents, e.g., markdown from eating code
/// files that start with `# comments`, or YAML from matching prose.
///
///   Markdown(80) → SCSS(90) → Sass(91) → TOML(100) → SQL(110)
///   → Prompt(115) → YAML(120)
use super::family::ContentFamily;
use super::languages::{STRONG_STRUCTURAL, SOFT_STRUCTURAL};

/// Phase 0c — strong structural probes (near-deterministic).
///
/// These detectors have very low false-positive rates and run before
/// the family classifier gates anything.
pub fn detect_strong_structural(trimmed: &str, was_sliced: bool) -> Option<&'static str> {
    for entry in STRONG_STRUCTURAL.iter() {
        if (entry.detect)(trimmed, was_sliced) {
            if cfg!(debug_assertions) {
                eprintln!("[Lang Detect]   [Phase 0c] Strong probe: \"{}\" matched ✓", entry.name);
            }
            return Some(entry.name);
        }
    }
    if cfg!(debug_assertions) {
        let names: Vec<&str> = STRONG_STRUCTURAL.iter().map(|e| e.name).collect();
        eprintln!("[Lang Detect]   [Phase 0c] Strong probes checked: [{}] — none matched", names.join(", "));
    }
    None
}

/// Phase 2a — soft structural probes (family-gated).
///
/// Only fires detectors whose declared content families overlap with
/// the families the classifier identified. Returns `None` if no soft
/// detector matches within the allowed families.
pub fn detect_soft_structural(
    trimmed: &str,
    was_sliced: bool,
    families: &[ContentFamily],
) -> Option<&'static str> {
    if cfg!(debug_assertions) {
        let fam_str: Vec<&str> = families.iter().map(|f| match f {
            ContentFamily::Prose => "Prose",
            ContentFamily::Code => "Code",
            ContentFamily::StructuredData => "Data",
            ContentFamily::Markup => "Markup",
            ContentFamily::ShellScript => "Shell",
            ContentFamily::Config => "Config",
        }).collect();
        eprintln!("[Lang Detect]   [Phase 2a] Scanning soft structural signals (allowed families: [{}])", fam_str.join(","));
    }

    for entry in SOFT_STRUCTURAL.iter() {
        // Family gate: skip detectors that don't match the classified families.
        // When families is empty (classifier abstained), allow all detectors.
        let gated = !families.is_empty()
            && !entry.content_families.iter().any(|f| families.contains(f));

            if cfg!(debug_assertions) {
                let entry_fams: Vec<&str> = entry.content_families.iter().map(|f| match f {
                    ContentFamily::Prose => "Prose",
                    ContentFamily::Code => "Code",
                    ContentFamily::StructuredData => "Data",
                    ContentFamily::Markup => "Markup",
                    ContentFamily::ShellScript => "Shell",
                    ContentFamily::Config => "Config",
                }).collect();
                if gated {
                    eprintln!(
                        "[Lang Detect]   [Phase 2a] \"{}\": skipped (needs family [{}], current content doesn't match)",
                        entry.name, entry_fams.join(","),
                    );
                }
            }

        if gated {
            continue;
        }

        if (entry.detect)(trimmed, was_sliced) {
            if cfg!(debug_assertions) {
                eprintln!("[Lang Detect]   [Phase 2a] \"{}\" ✓ matched", entry.name);
            }
            return Some(entry.name);
        } else if cfg!(debug_assertions) {
            eprintln!("[Lang Detect]   [Phase 2a] \"{}\" ✗ no match", entry.name);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::family::ContentFamily;

    /// Test helper: runs both strong and soft structural detection with all families.
    fn detect_structural(content: &str, was_sliced: bool) -> Option<&'static str> {
        let trimmed = content.trim();
        if let Some(lang) = detect_strong_structural(trimmed, was_sliced) {
            return Some(lang);
        }
        let all_families = vec![
            ContentFamily::Code, ContentFamily::StructuredData, ContentFamily::Markup,
            ContentFamily::Config, ContentFamily::ShellScript, ContentFamily::Prose,
        ];
        detect_soft_structural(trimmed, was_sliced, &all_families)
    }

    #[test]
    fn json_object() {
        let content = r#"{"name": "test", "version": "1.0"}"#;
        assert_eq!(detect_structural(content, false), Some("json"));
    }

    #[test]
    fn json_array() {
        let content = r#"[1, 2, 3, 4]"#;
        assert_eq!(detect_structural(content, false), Some("json"));
    }

    #[test]
    fn html_doc() {
        let content = "<!DOCTYPE html>\n<html>\n<head><title>Test</title></head>\n<body></body>\n</html>";
        assert_eq!(detect_structural(content, false), Some("html"));
    }

    #[test]
    fn xml_doc() {
        let content = "<?xml version=\"1.0\"?>\n<root><item>test</item></root>";
        assert_eq!(detect_structural(content, false), Some("xml"));
    }

    #[test]
    fn csv_basic() {
        let content = "name,age,city\nAlice,30,NYC\nBob,25,LA\nCharlie,35,Chicago";
        assert_eq!(detect_structural(content, false), Some("csv"));
    }

    #[test]
    fn yaml_doc() {
        let content = "name: test\nversion: 1.0\ndependencies:\n  - foo\n  - bar\nitems:\n  key: value";
        assert_eq!(detect_structural(content, false), Some("yaml"));
    }

    #[test]
    fn toml_doc() {
        let content = "[package]\nname = \"test\"\nversion = \"1.0\"\n\n[dependencies]\nfoo = \"1.0\"";
        assert_eq!(detect_structural(content, false), Some("toml"));
    }

    #[test]
    fn markdown_doc() {
        let content = "# Title\n\nSome paragraph text here.\n\n## Section\n\n- bullet one\n- bullet two\n\n> blockquote";
        assert_eq!(detect_structural(content, false), Some("markdown"));
    }

    #[test]
    fn dockerfile_basic() {
        let content = "FROM node:18\nWORKDIR /app\nCOPY . .\nRUN npm install\nCMD [\"node\", \"index.js\"]";
        assert_eq!(detect_structural(content, false), Some("dockerfile"));
    }

    #[test]
    fn php_basic() {
        let content = "<?php\necho \"Hello World\";\n?>";
        assert_eq!(detect_structural(content, false), Some("php"));
    }

    #[test]
    fn svelte_component() {
        let content = "<script>\n  let count = 0;\n</script>\n<button on:click={() => count++}>{count}</button>\n<style>\n  button { color: red; }\n</style>";
        assert_eq!(detect_structural(content, false), Some("svelte"));
    }

    #[test]
    fn vue_template() {
        let content = "<template>\n  <div v-if=\"show\">\n    <button @click=\"toggle\">Toggle</button>\n  </div>\n</template>\n<script setup>\nconst show = ref(true);\n</script>";
        assert_eq!(detect_structural(content, false), Some("vue"));
    }

    #[test]
    fn scss_with_variables() {
        let content = "$primary: #333;\n$font-size: 16px;\n\nbody {\n  color: $primary;\n  font-size: $font-size;\n}";
        assert_eq!(detect_structural(content, false), Some("scss"));
    }

    #[test]
    fn not_csv_yaml() {
        let content = "name: test\nversion: 1.0\nauthor: someone\nlicense: MIT";
        assert_ne!(detect_structural(content, false), Some("csv"));
    }
}