use super::LanguageDefinition;
use super::ContentFamily;
use regex::Regex;
use std::sync::LazyLock;

/// Strip fenced code blocks (` ``` `...` ``` `) and indented code blocks (4+ spaces
/// after a blank line) so embedded code snippets don't trigger anti-signals
/// against markdown detection or heuristic language scoring.
///
/// Keeps fence markers themselves (they're a markdown signal).
pub(crate) fn strip_code_blocks(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut in_fence = false;
    let mut prev_blank = false;
    let mut in_indent_block = false;

    for line in content.lines() {
        let trimmed_line = line.trim();

        if trimmed_line.starts_with("```") {
            in_fence = !in_fence;
            in_indent_block = false;
            result.push_str(line);
            result.push('\n');
            prev_blank = false;
            continue;
        }

        if in_fence {
            prev_blank = false;
            continue;
        }

        let is_indented = line.starts_with("    ") || line.starts_with('\t');
        if trimmed_line.is_empty() {
            if in_indent_block {
                prev_blank = true;
                continue;
            }
            prev_blank = true;
            result.push('\n');
            continue;
        }

        if is_indented && (prev_blank || in_indent_block) {
            in_indent_block = true;
            prev_blank = false;
            continue;
        }

        in_indent_block = false;
        prev_blank = false;
        result.push_str(line);
        result.push('\n');
    }
    result
}

fn has_markdown_frontmatter(trimmed: &str) -> bool {
    if !trimmed.starts_with("---") {
        return false;
    }
    let lines: Vec<&str> = trimmed.lines().collect();
    let close_idx = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, l)| l.trim() == "---")
        .map(|(i, _)| i);
    let close_idx = match close_idx {
        Some(i) if i > 0 => i,
        _ => return false,
    };
    let after_frontmatter: String = lines[close_idx + 1..].join("\n");
    let after = after_frontmatter.trim();

    if after.is_empty() {
        return true;
    }

    static COMPONENT_GUARD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)(^\s*<(script|style|Layout|Component|Fragment)\b|^\s*import\s+[\w\{].*from\s+['"]|^\s*export\s+(const|default|function|let)\s|<[A-Z]\w+[\s/>])"#).unwrap()
    });
    if COMPONENT_GUARD.is_match(after) {
        return false;
    }

    true
}

pub(crate) fn is_likely_markdown(trimmed: &str, _was_sliced: bool) -> bool {
    let first = trimmed.as_bytes().first().copied().unwrap_or(0);
    if first == b'<' || first == b'{' || first == b'[' {
        return false;
    }

    if has_markdown_frontmatter(trimmed) {
        return true;
    }

    static MDX_HEADING: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^#{1,6}\s+\S").unwrap());
    static MDX_IMPORT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?m)^\s*import\s+[\w\{*].*\s+from\s+['"`]"#).unwrap());
    if MDX_HEADING.is_match(trimmed) && MDX_IMPORT.is_match(trimmed) {
        return true;
    }

    let prose_only = strip_code_blocks(trimmed);

    static CODE_ANTI: LazyLock<Vec<(Regex, i32)>> = LazyLock::new(|| vec![
        (Regex::new(r"(?m)^/\*\*?\s").unwrap(), 3),
        (Regex::new(r"(?m)^\s*(import|export)\s+").unwrap(), 3),
        (Regex::new(r"(?m)^\s*(const|let|var)\s+\w+\s*[=:]").unwrap(), 2),
        (Regex::new(r"(?m)^\s*function\s+\w*\s*\(").unwrap(), 2),
        (Regex::new(r"(?m)^\s*(interface|type|enum)\s+\w+").unwrap(), 3),
        // Weight 3 (was 2): any file with a `class` declaration at line-start is code,
        // not markdown. Raising to 3 pushes code_score to the higher-threshold zone so
        // a lone `# comment` line cannot tip the score over the markdown threshold.
        (Regex::new(r"(?m)^\s*class\s+\w+").unwrap(), 3),
        (Regex::new(r"(?m)=>\s*[\{(\n]").unwrap(), 2),
        // `def` without requiring parens — catches Ruby `def self.method` and `def foo`.
        (Regex::new(r"(?m)^\s*def\s+[\w.]+").unwrap(), 3),
        (Regex::new(r#"(?m)^\s*#include\s*[<"]"#).unwrap(), 3),
        (Regex::new(r"(?m);\s*$").unwrap(), 1),
        (Regex::new(r"(?m)^\s*async\s+(function|\w+\s*[=(])").unwrap(), 2),
        (Regex::new(r"(?m)^\s*[a-zA-Z_][\w.\-]*\s*:\s+[^h\s]").unwrap(), 4),
        // PEP 263 / Emacs file-local variable encoding header (e.g. `# -*- coding: utf-8 -*-`).
        // These lines start with `#` so they match the markdown heading regex, causing
        // Python files that open with this header to be misidentified as markdown.
        (Regex::new(r"(?m)^#.*coding\s*[:=]\s*[-\w]+").unwrap(), 3),
        // Ruby `module Foo` — uppercase module names are code, not markdown prose.
        (Regex::new(r"(?m)^\s*module\s+[A-Z]\w*").unwrap(), 3),
        // Ruby/Lua `end` on its own line — block terminators are a strong code signal.
        (Regex::new(r"(?m)^\s*end\s*$").unwrap(), 2),
        // Ruby magic comment pragma (e.g. `# frozen_string_literal: true`).
        // Starts with `#` so it matches the markdown heading regex.
        (Regex::new(r"(?m)^#\s*frozen_string_literal:").unwrap(), 3),
        // Method chaining: 3+ dot-separated identifiers at line start
        // (e.g. `Rails.application.config.filter_parameters`).
        // Universally code (Ruby/Java/Python/JS), never appears in prose.
        (Regex::new(r"(?m)^\s*[a-zA-Z_]\w*(?:\.\w+){2,}").unwrap(), 3),
    ]);

    let mut code_score = 0i32;
    for (re, weight) in CODE_ANTI.iter() {
        if re.is_match(&prose_only) {
            code_score += weight;
        }
    }

    if code_score >= 6 {
        return false;
    }

    static MD_SIGNALS: LazyLock<Vec<(Regex, i32)>> = LazyLock::new(|| vec![
        (Regex::new(r"(?m)^#{1,6}\s+\S").unwrap(), 3),
        (Regex::new(r"\[.+?\]\(.+?\)").unwrap(), 2),
        (Regex::new(r"!\[.*?\]\(.+?\)").unwrap(), 2),
        (Regex::new(r"(?m)^\s*[\-*+]\s+\S").unwrap(), 1),
        (Regex::new(r"(?m)^\s*\d+\.\s+\S").unwrap(), 1),
        (Regex::new(r"(?m)^\s*>\s+").unwrap(), 1),
        (Regex::new(r"\*\*.+?\*\*").unwrap(), 1),
        (Regex::new(r"(?m)^```").unwrap(), 2),
        (Regex::new(r"(?m)^\|.+\|.+\|").unwrap(), 2),
        (Regex::new(r"(?m)^---\s*$").unwrap(), 1),
    ]);

    let mut score = 0i32;
    for (re, weight) in MD_SIGNALS.iter() {
        if re.is_match(trimmed) {
            score += weight;
        }
    }

    let threshold = if code_score >= 3 { 8 } else { 3 };
    score >= threshold
}

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "markdown",
        extensions: &[".md", ".markdown", ".mdx"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: Some(80),
        structural_detect: Some(is_likely_markdown),
        patterns: &[],
        anti_patterns: &[],
        uses_hash_comments: false,
        keywords: &[],
        builtins: &[],
        family: None,
        exclusive_patterns: &[],
        // ── Family-gated fields ──────────────────────────────
        content_families: &[ContentFamily::Markup, ContentFamily::Prose, ContentFamily::Code],
        anchors: &[],
        hints: &[],
        rivals: &[],
        differentiators: &[],
        disqualifiers: &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_coding_header_not_markdown() {
        // Regression: `# -*- coding: utf-8 -*-` matched the markdown heading
        // regex AND all def/self signals were stripped as "indented code blocks",
        // causing this Python file to be misidentified as markdown.
        let src = "# -*- coding: utf-8 -*-\n\nfrom enum import Enum\n\n\nclass DefaultCategories(Enum):\n\n    HOUSING = 0\n    FOOD = 1\n\n\nclass Categorizer(object):\n\n    def __init__(self, m):\n        self.m = m\n\n    def categorize(self, t):\n        return self.m.get(t.seller)\n";
        assert!(!is_likely_markdown(src, false), "Python class file should not be markdown");
    }

    #[test]
    fn real_markdown_still_detected() {
        let src = "# Getting Started\n\nSome paragraph.\n\n## Installation\n\n- Step one\n- Step two\n\n> Note: check the docs\n";
        assert!(is_likely_markdown(src, false), "Real markdown should be detected");
    }

    #[test]
    fn python_class_only_not_markdown() {
        let src = "class Foo:\n    pass\n\nclass Bar(Foo):\n    def method(self):\n        return 42\n";
        assert!(!is_likely_markdown(src, false), "Plain Python class should not be markdown");
    }

    #[test]
    fn ruby_gem_version_not_markdown() {
        // Regression: `# frozen_string_literal: true` matched the heading regex,
        // causing Ruby files to be misidentified as markdown.
        let src = "# frozen_string_literal: true\n\nmodule Rails\n  # Returns the currently loaded version of \\Rails as a +Gem::Version+.\n  def self.gem_version\n    Gem::Version.new VERSION::STRING\n  end\n\n  module VERSION\n    MAJOR = 8\n    MINOR = 2\n    TINY  = 0\n    PRE   = \"alpha\"\n\n    STRING = [MAJOR, MINOR, TINY, PRE].compact.join(\".\")\n  end\nend\n";
        assert!(!is_likely_markdown(src, false), "Ruby gem_version file should not be markdown");
    }

    #[test]
    fn ruby_simple_module_not_markdown() {
        // A minimal Ruby file with just a module and no def — still not markdown.
        let src = "# frozen_string_literal: true\n\nmodule Rails\n  VERSION = \"8.2.0\"\nend\n";
        assert!(!is_likely_markdown(src, false), "Ruby module file should not be markdown");
    }

    #[test]
    fn ruby_class_with_methods_not_markdown() {
        let src = "# frozen_string_literal: true\n\nclass Foo\n  def initialize(bar)\n    @bar = bar\n  end\n\n  def to_s\n    @bar.to_s\n  end\nend\n";
        assert!(!is_likely_markdown(src, false), "Ruby class file should not be markdown");
    }

    #[test]
    fn ruby_without_pragma_not_markdown() {
        // Ruby file without frozen_string_literal — module + end still block markdown.
        let src = "module MyApp\n  module Config\n    TIMEOUT = 30\n  end\nend\n";
        assert!(!is_likely_markdown(src, false), "Ruby module without pragma should not be markdown");
    }

    #[test]
    fn markdown_with_ruby_code_block_still_detected() {
        // Markdown that contains a Ruby fenced code block should still be markdown.
        let src = "# Installation\n\nAdd to your Gemfile:\n\n```ruby\nrequire 'my_gem'\nmodule Foo\n  def bar\n  end\nend\n```\n\nThen run `bundle install`.\n";
        assert!(is_likely_markdown(src, false), "Markdown with Ruby code block should still be markdown");
    }

    #[test]
    fn ruby_config_with_hash_comments_not_markdown() {
        // Regression: Ruby initializer file with `# comment` lines matched the
        // markdown heading regex. Method chaining (Rails.application.config.*)
        // is a strong code signal that should block markdown detection.
        let src = "# Be sure to restart your server when you modify this file.\n\n# Configure parameters to be filtered from the log file. Use this to limit dissemination of\n# sensitive information. See the ActiveSupport::ParameterFilter documentation for supported\n# notations and behaviors.\nRails.application.config.filter_parameters += [\n  :passw, :secret, :token, :_key, :crypt, :salt, :certificate, :otp, :ssn\n]\n";
        assert!(!is_likely_markdown(src, false), "Ruby config file with method chaining should not be markdown");
    }
}
