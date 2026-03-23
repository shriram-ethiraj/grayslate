use std::collections::HashSet;

use super::NamingDefinition;
use crate::naming::model::MAX_TOKENS;

pub fn definition() -> NamingDefinition {
    NamingDefinition {
        name: "ruby",
        extension: "rb",
        extract: extract_ruby,
    }
}

/// Ruby extraction with module/class awareness and DSL detection.
///
/// Priority order (file-local symbols outrank module/library names):
///   1. DSL patterns: Rails migrations, Gemspec, Rake tasks — P10
///   2. `class` declarations — P9
///   3. RSpec `describe` / Sinatra route verbs — P8 (DSL entry points)
///   4. `def` methods — P7
///   5. Gemfile/config patterns — P7
///   6. `module` declarations — P5 (fallback context, like package)
fn extract_ruby(content: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    // DSL: Gemspec — multiple patterns for real-world shapes
    static GEMSPEC_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)Gem::Specification\.new\b.*?\|.*?\|\s*\n\s*\w+\.name\s*=\s*["']([^"']+)["']"#).unwrap()
    });
    // Also catch: Gem::Specification.new do |s| \n  s.name = 'foo'
    // and multiline with extra whitespace
    static GEMSPEC_ALT_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)\.name\s*=\s*["']([^"']+)["']"#).unwrap()
    });
    // Detect gemspec context (to qualify gemspec_alt matches)
    static GEMSPEC_CONTEXT_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"Gem::Specification\.new").unwrap()
    });
    // DSL: Rails migration
    static MIGRATION_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^class\s+(\w+)\s*<\s*ActiveRecord::Migration").unwrap()
    });
    // DSL: Rake task
    static RAKE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^(?:desc|task)\s+["':]([\w:]+)"#).unwrap()
    });
    // DSL: RSpec describe/context
    static RSPEC_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^(?:RSpec\.)?(?:describe|context)\s+["']?([A-Z][a-zA-Z0-9_:]*|[^"'\n]{3,40})["']?"#).unwrap()
    });
    // DSL: Sinatra-style route verbs
    static ROUTE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^[ \t]*(?:get|post|put|patch|delete|options|head)\s+['"/]([^'"{\s]*)"#).unwrap()
    });
    // DSL: Gemfile source + gem declarations (detect Gemfile context)
    static GEMFILE_CONTEXT_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^source\s+["']https?://rubygems"#).unwrap()
    });
    // DSL: RSpec.configure / configure block
    static RSPEC_CONFIGURE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^RSpec\.configure\b").unwrap()
    });

    static MODULE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^[ \t]*module\s+([A-Z][a-zA-Z0-9_:]*)").unwrap());
    static CLASS_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^[ \t]*class\s+([A-Z][a-zA-Z0-9_:]*)").unwrap()
    });
    static DEF_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^[ \t]*def\s+(self\.)?([a-zA-Z_][a-zA-Z0-9_?!]*)").unwrap()
    });

    const NOISE: &[&str] = &[
        "initialize", "setup", "run", "call", "to_s", "to_h", "to_a",
        "inspect", "hash", "eql?", "test", "self",
    ];

    struct Symbol { name: String, priority: u8 }
    let mut symbols: Vec<Symbol> = Vec::new();

    // DSL patterns (highest priority) — early return for unambiguous matches

    // Gemspec: try primary pattern first, then alternative with context
    if let Some(cap) = GEMSPEC_RE.captures(content) {
        return Some(cap[1].to_string());
    }
    if GEMSPEC_CONTEXT_RE.is_match(content) {
        if let Some(cap) = GEMSPEC_ALT_RE.captures(content) {
            return Some(cap[1].to_string());
        }
    }

    if let Some(cap) = MIGRATION_RE.captures(content) {
        return Some(cap[1].to_string());
    }

    // Gemfile detection
    if GEMFILE_CONTEXT_RE.is_match(content) {
        // Extract first non-trivial gem declaration for naming
        let gem_re = Regex::new(r#"(?m)^\s*gem\s+["']([a-zA-Z][\w-]*)["']"#).ok();
        if let Some(re) = gem_re {
            let gems: Vec<String> = re.captures_iter(content).take(3)
                .filter_map(|c| {
                    let name = c[1].to_string();
                    if name != "bundler" && name != "rake" { Some(name) } else { None }
                })
                .collect();
            if !gems.is_empty() {
                return Some(format!("gemfile-{}", gems[0]));
            }
            return Some("gemfile".to_string());
        }
    }

    // RSpec.configure → test configuration
    if RSPEC_CONFIGURE_RE.is_match(content) {
        symbols.push(Symbol { name: "rspec-config".to_string(), priority: 10 });
    }

    // Rake tasks
    if let Some(cap) = RAKE_RE.captures(content) {
        symbols.push(Symbol { name: cap[1].to_string(), priority: 10 });
    }

    // RSpec describe (P8)
    for cap in RSPEC_RE.captures_iter(content).take(2) {
        let name = cap[1].trim().to_string();
        let short = name.rsplit("::").next().unwrap_or(&name).to_string();
        if !short.is_empty() {
            symbols.push(Symbol { name: short, priority: 8 });
        }
    }

    // Sinatra routes (P8) — extract first meaningful route
    for cap in ROUTE_RE.captures_iter(content).take(3) {
        let route = cap[1].trim_matches('/').to_string();
        if !route.is_empty() && route != "*" {
            symbols.push(Symbol { name: route, priority: 8 });
        }
    }

    // Classes (P9)
    for cap in CLASS_RE.captures_iter(content).take(3) {
        let name = &cap[1];
        let short = name.rsplit("::").next().unwrap_or(name);
        if !short.is_empty() {
            symbols.push(Symbol { name: short.to_string(), priority: 9 });
        }
    }

    // Methods (P7)
    for cap in DEF_RE.captures_iter(content).take(4) {
        let name = cap[2].to_string();
        if !NOISE.contains(&name.as_str()) {
            symbols.push(Symbol { name, priority: 7 });
        }
    }

    // Modules — demoted to P5 (fallback context, like package in JVM)
    for cap in MODULE_RE.captures_iter(content).take(3) {
        let name = &cap[1];
        let short = name.rsplit("::").next().unwrap_or(name);
        if !short.is_empty() {
            symbols.push(Symbol { name: short.to_string(), priority: 5 });
        }
    }

    symbols.sort_by(|a, b| b.priority.cmp(&a.priority));

    let mut seen = HashSet::new();
    let mut tokens: Vec<String> = Vec::new();
    for sym in &symbols {
        if tokens.len() >= MAX_TOKENS { break; }
        if seen.insert(sym.name.clone()) {
            tokens.push(sym.name.clone());
        }
    }

    if tokens.is_empty() { None } else { Some(tokens.join("-")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::shared::slugify;

    fn name(src: &str) -> Option<String> {
        extract_ruby(src).and_then(|s| slugify(&s))
    }

    // --- Priority rebalance: class outranks module ---
    #[test]
    fn class_leads_over_module() {
        let src = "module Sinatra\n  class Base\n    def call(env)\n    end\n  end\nend";
        let n = name(src).unwrap();
        assert!(n.contains("base"), "class wins over module: {n}");
    }

    #[test]
    fn module_and_class() {
        let src = "module Authentication\n  class SessionManager\n    def create_session\n    end\n  end\nend";
        let n = name(src).unwrap();
        assert!(n.contains("session-manager"), "class wins: {n}");
    }

    #[test]
    fn migration() {
        let src = "class CreateUsers < ActiveRecord::Migration[7.0]\n  def change\n    create_table :users do |t|\n    end\n  end\nend";
        let n = name(src).unwrap();
        assert!(n.contains("create-users"), "got: {n}");
    }

    #[test]
    fn top_level_def() {
        let src = "def fibonacci(n)\n  return n if n <= 1\n  fibonacci(n-1) + fibonacci(n-2)\nend";
        let n = name(src).unwrap();
        assert!(n.contains("fibonacci"), "got: {n}");
    }

    // --- New: Gemspec alternative pattern ---
    #[test]
    fn gemspec_multiline() {
        let src = r#"Gem::Specification.new do |s|
  s.name        = 'sinatra'
  s.version     = Sinatra::VERSION
  s.description = "Classy web framework"
end"#;
        let n = name(src).unwrap();
        assert!(n.contains("sinatra"), "gemspec name extracted: {n}");
    }

    // --- New: Gemfile ---
    #[test]
    fn gemfile_detection() {
        let src = r#"source "https://rubygems.org"
gemspec
gem 'rack-test'
gem 'rspec'"#;
        let n = name(src).unwrap();
        assert!(n.contains("gemfile"), "gemfile detected: {n}");
    }

    // --- New: RSpec ---
    #[test]
    fn rspec_describe() {
        let src = r#"require 'spec_helper'
RSpec.describe UserService do
  it 'authenticates users' do
  end
end"#;
        let n = name(src).unwrap();
        assert!(n.contains("user-service"), "rspec describe: {n}");
    }

    #[test]
    fn rspec_configure() {
        let src = "require 'sinatra'\n\nRSpec.configure do |config|\n  config.include Sinatra::TestHelpers\nend";
        let n = name(src).unwrap();
        assert!(n.contains("rspec-config"), "rspec configure: {n}");
    }

    // --- New: Sinatra routes ---
    #[test]
    fn sinatra_route() {
        let src = "require 'sinatra'\n\nget '/hello' do\n  'Hello World'\nend\n\npost '/submit' do\n  'Submitted'\nend";
        let n = name(src).unwrap();
        assert!(n.contains("hello"), "sinatra route: {n}");
    }

    // --- New: module-only fallback ---
    #[test]
    fn module_only_when_no_class() {
        let src = "module Rack\n  module Protection\n    # require helpers\n  end\nend";
        let n = name(src).unwrap();
        assert!(n.contains("rack") || n.contains("protection"), "module fallback: {n}");
    }

    // --- Audit regression: class wins over generic module ---
    #[test]
    fn class_wins_over_sinatra_module() {
        let src = "module Sinatra\n  class Application < Base\n    def routes\n    end\n  end\nend";
        let n = name(src).unwrap();
        assert!(n.contains("application"), "class beats Sinatra module: {n}");
    }
}

