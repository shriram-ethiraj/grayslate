use super::{wp, LanguageDefinition};
use super::ContentFamily;

pub fn definition()-> LanguageDefinition {
    LanguageDefinition {
        name: "ruby",
        extensions: &[".rb"],
        filenames: &["gemfile", "rakefile"],
        filename_patterns: &[],
        shebangs: &[r"\bruby\b"],
        structural_priority: None,
        structural_detect: None,
        keywords: &[
            "def", "end", "module", "unless", "until", "begin",
            "ensure", "rescue", "raise", "yield", "elsif", "when",
            "undef", "alias", "defined", "redo", "retry",
            "prepend", "include", "extend", "attr_accessor",
            "attr_reader", "attr_writer", "puts", "require_relative",
        ],
        builtins: &[
            "proc", "lambda", "freeze", "frozen", "taint", "untaint",
            "respond_to", "send", "method_missing", "define_method",
            "class_eval", "instance_eval", "module_eval",
            "instance_variable_get", "instance_variable_set",
            "each", "map", "select", "reject", "inject", "collect",
        ],
        // ── Family-gated fields ───────────────────────────────
        content_families: &[ContentFamily::Code, ContentFamily::Config],
        anchors: &[
            wp!(r#"\brequire_relative\s+['""]"#, 5),
            wp!(r"\battr_(accessor|reader|writer)\s+:", 5),
            wp!(r"(?m)^=begin\b", 5),
            // class << self — Ruby singleton class
            wp!(r"\bclass\s*<<\s*self\b", 5),
            wp!(r"%(w|i|r|q|Q|x)\s*[\[\({<]", 4),
            wp!(r"\bdo\s*\|[\w,\s]+\|", 4),
            wp!(r"(?m)^\s*unless\s+", 4),
            // elsif — Ruby-specific keyword (not elif, not elseif)
            wp!(r"\belsif\b", 4),
            // rescue — Ruby exception handling
            wp!(r"\brescue\b", 4),
            // describe/context — RSpec DSL (but NOT "it" alone — too common in English)
            wp!(r#"\b(describe|context)\s+['"]"#, 4),
            // RSpec it block: it "does something" or it { ... }
            wp!(r#"\bit\s+['"\{]"#, 4),
            // frozen_string_literal pragma — Ruby-exclusive
            wp!(r"(?m)^#\s*frozen_string_literal:\s*(true|false)", 5),
            // Gem::Specification — Gemspec files
            wp!(r"Gem::Specification", 5),
            // raise ErrorClass — Ruby exception raising with class
            wp!(r"\braise\s+[A-Z]\w+", 4),
            // class Foo < Bar — Ruby class inheritance
            wp!(r"(?m)^\s*class\s+[A-Z]\w+\s*<\s*[A-Z]", 4),
        ],
        hints: &[
            wp!(r"(?m)^\s*def\s+\w+", 3),
            wp!(r"(?m)^\s*end\s*$", 3),
            wp!(r"(?m)^\s*module\s+\w+", 3),
            wp!(r#"\bputs\s+['"\w]"#, 3),
            wp!(r#"\brequire\s+['""]"#, 3),
            wp!(r"\.(each|map|select|reject|inject|collect)\s*(\{|\bdo\b)", 3),
            // has_many / belongs_to / validates — Rails ActiveRecord DSL
            wp!(r"\b(has_many|belongs_to|validates)\b", 3),
            // ||= — Ruby conditional assignment
            wp!(r"\|\|=", 3),
            // yield — Ruby block invocation
            wp!(r"\byield\b", 2),
            // <<~HEREDOC or <<HEREDOC — Ruby heredoc
            wp!(r"<<~?\w+", 2),
            // key: value — Ruby symbol-key hash syntax
            wp!(r"\w+:\s+", 1),
            // @instance_var — Ruby instance variables
            wp!(r"@[a-z_]\w+", 2),
            // Symbol literals :name
            wp!(r":\w+", 2),
        ],
        disqualifiers: &[
            // C/C++ preprocessor — never valid Ruby
            wp!(r"(?m)^#include\s", 1),
            wp!(r"(?m)^#define\s", 1),
            wp!(r"(?m)^#(?:ifndef|ifdef|endif)\b", 1),
        ],
    }
}
