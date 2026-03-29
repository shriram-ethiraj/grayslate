/// Content family classification.
///
/// Classifies document content into broad families (Prose, Code, Data, etc.)
/// using the language-agnostic features from `features.rs`. This is the key
/// innovation in the detection pipeline — it gates which language candidates
/// are even considered, preventing prose from competing with code languages.
///
/// The classifier is a deterministic decision tree, not ML. It is inspectable,
/// debuggable, and does not require training data.
use super::features::ContentFeatures;

// ── Content families ────────────────────────────────────────────────────

/// Broad content family — determines which language candidates are considered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentFamily {
    /// Natural language text: emails, notes, essays, meeting notes, chat.
    Prose,
    /// Structured data formats: JSON, CSV, YAML, TOML, XML.
    StructuredData,
    /// Markup/template languages: HTML, Markdown, Svelte, Vue.
    Markup,
    /// Programming languages: Python, Rust, Java, etc.
    Code,
    /// Shell scripts: Bash, Zsh, CMD, PowerShell.
    ShellScript,
    /// Configuration files: dotfiles, INI, .env, Nginx, Dockerfile.
    Config,
}

/// A family classification with confidence score.
#[derive(Debug, Clone)]
pub struct FamilyScore {
    pub family: ContentFamily,
    pub confidence: f64,
}

/// Result of family classification.
#[derive(Debug, Clone)]
pub struct FamilyResult {
    /// Ranked families, highest confidence first.
    pub scores: Vec<FamilyScore>,
}

impl FamilyResult {
    /// The top-ranked family, if any scores are present.
    pub fn top(&self) -> Option<&FamilyScore> {
        self.scores.first()
    }

    /// Whether the classification is confident (top score clearly ahead).
    pub fn is_confident(&self) -> bool {
        match (self.scores.first(), self.scores.get(1)) {
            (Some(first), Some(second)) => first.confidence - second.confidence > 0.25,
            (Some(first), None) => first.confidence > 0.5,
            _ => false,
        }
    }
}

// ── Classifier ──────────────────────────────────────────────────────────

/// Classify document content into content families.
///
/// Returns a ranked list of families with confidence scores. The family
/// classifier uses only language-agnostic features — it knows nothing about
/// specific programming languages, so adding new languages never changes
/// the classifier's behaviour.
pub fn classify_family(features: &ContentFeatures) -> FamilyResult {
    let mut scores: Vec<(ContentFamily, f64)> = Vec::new();

    // Score each family independently
    let prose_score = score_prose(features);
    let code_score = score_code(features);
    let data_score = score_structured_data(features);
    let markup_score = score_markup(features);
    let shell_score = score_shell(features);
    let config_score = score_config(features);

    if prose_score > 0.0 {
        scores.push((ContentFamily::Prose, prose_score));
    }
    if code_score > 0.0 {
        scores.push((ContentFamily::Code, code_score));
    }
    if data_score > 0.0 {
        scores.push((ContentFamily::StructuredData, data_score));
    }
    if markup_score > 0.0 {
        scores.push((ContentFamily::Markup, markup_score));
    }
    if shell_score > 0.0 {
        scores.push((ContentFamily::ShellScript, shell_score));
    }
    if config_score > 0.0 {
        scores.push((ContentFamily::Config, config_score));
    }

    // Sort by confidence (descending)
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    FamilyResult {
        scores: scores
            .into_iter()
            .map(|(family, confidence)| FamilyScore { family, confidence })
            .collect(),
    }
}

// ── Per-family scoring functions ────────────────────────────────────────

/// Score how prose-like the content is.
///
/// High-confidence prose indicators:
/// - High stopword ratio (> 0.3)
/// - Contractions present
/// - Question marks
/// - Greetings/closings (email-like)
/// - High pronoun density
/// - High average words per line (8–20 typical for prose)
fn score_prose(f: &ContentFeatures) -> f64 {
    // Minimum content needed
    if f.total_words < 5 {
        return 0.0;
    }

    let mut score: f64 = 0.0;

    // Stopword ratio is the strongest prose signal
    if f.stopword_ratio > 0.35 {
        score += 0.4;
    } else if f.stopword_ratio > 0.25 {
        score += 0.2;
    }

    // Contractions are nearly conclusive for natural language
    if f.contraction_count >= 2 {
        score += 0.25;
    } else if f.contraction_count >= 1 {
        score += 0.15;
    }

    // Questions
    if f.question_ratio > 0.15 {
        score += 0.15;
    } else if f.question_ratio > 0.0 {
        score += 0.08;
    }

    // Greetings and closings
    if f.greeting_present {
        score += 0.15;
    }
    if f.closing_present {
        score += 0.1;
    }

    // Pronoun density
    if f.pronoun_ratio > 0.05 {
        score += 0.15;
    } else if f.pronoun_ratio > 0.02 {
        score += 0.08;
    }

    // Average words per line: prose tends to have 8+ words per line
    if f.avg_words_per_line > 10.0 {
        score += 0.1;
    } else if f.avg_words_per_line > 7.0 {
        score += 0.05;
    }

    // Negative signals: code-like features suppress prose score
    if f.semicolon_ratio > 0.15 {
        score -= 0.3;
    }
    if f.brace_ratio > 0.25 {
        score -= 0.2;
    }
    if f.import_line_count >= 2 {
        score -= 0.25;
    }
    if f.function_def_count >= 1 {
        score -= 0.2;
    }
    if f.operator_density > 0.3 {
        score -= 0.15;
    }

    score.max(0.0)
}

/// Score how code-like the content is.
fn score_code(f: &ContentFeatures) -> f64 {
    if f.non_empty_lines < 2 {
        return 0.0;
    }

    let mut score: f64 = 0.0;

    // Import lines — strong code signal
    if f.import_line_count >= 3 {
        score += 0.35;
    } else if f.import_line_count >= 1 {
        score += 0.2;
    }

    // Function definitions
    if f.function_def_count >= 2 {
        score += 0.3;
    } else if f.function_def_count >= 1 {
        score += 0.2;
    }

    // Semicolons at end of lines
    if f.semicolon_ratio > 0.3 {
        score += 0.25;
    } else if f.semicolon_ratio > 0.1 {
        score += 0.15;
    }

    // Braces
    if f.brace_ratio > 0.2 {
        score += 0.2;
    } else if f.brace_ratio > 0.1 {
        score += 0.1;
    }

    // Programming operators
    if f.operator_density > 0.3 {
        score += 0.15;
    } else if f.operator_density > 0.1 {
        score += 0.08;
    }

    // Function/method call expressions — universal code signal that works for
    // brace-less languages (Ruby, Python, Lua) where braces/semicolons are absent.
    if f.call_expression_count >= 3 {
        score += 0.2;
    } else if f.call_expression_count >= 1 {
        score += 0.1;
    }

    // Block-end keywords (end, fi, done) — code signal for Ruby/Lua/Shell
    if f.block_end_count >= 2 {
        score += 0.15;
    } else if f.block_end_count >= 1 {
        score += 0.08;
    }

    // Balanced brackets suggest structured code
    if f.bracket_balance && f.brace_ratio > 0.05 {
        score += 0.05;
    }

    // Negative: high prose signals suppress code
    if f.stopword_ratio > 0.35 && f.contraction_count >= 2 {
        score -= 0.3;
    }
    if f.greeting_present && f.closing_present {
        score -= 0.2;
    }
    if f.avg_words_per_line > 12.0 && f.contraction_count > 0 {
        score -= 0.15;
    }

    score.max(0.0)
}

/// Score how structured-data-like the content is (JSON, YAML, CSV, TOML, XML).
fn score_structured_data(f: &ContentFeatures) -> f64 {
    let mut score: f64 = 0.0;

    // Key-value ratio is the primary signal
    if f.kv_ratio > 0.6 {
        score += 0.4;
    } else if f.kv_ratio > 0.4 {
        score += 0.25;
    } else if f.kv_ratio > 0.2 {
        score += 0.1;
    }

    // Nesting depth suggests structured data
    if f.nesting_depth >= 3 {
        score += 0.2;
    } else if f.nesting_depth >= 1 {
        score += 0.1;
    }

    // Balanced brackets with high kv_ratio
    if f.bracket_balance && f.kv_ratio > 0.3 {
        score += 0.1;
    }

    // Section headers (TOML [section], INI [section])
    if f.section_header_count >= 2 {
        score += 0.15;
    }

    // Brace-heavy content with low code signals → could be JSON
    if f.brace_ratio > 0.3 && f.function_def_count == 0 && f.import_line_count == 0 {
        score += 0.15;
    }

    // Negative: prose signals suppress data score
    if f.contraction_count >= 2 {
        score -= 0.2;
    }
    if f.pronoun_ratio > 0.04 {
        score -= 0.15;
    }
    if f.question_ratio > 0.1 {
        score -= 0.1;
    }
    // Negative: high semicolon + brace ratio is code (CSS, Nginx), not data.
    // Pure data formats (JSON, YAML, TOML) rarely have semicolons.
    if f.semicolon_ratio > 0.2 && f.brace_ratio > 0.3 {
        score -= 0.35;
    }

    score.max(0.0)
}

/// Score how markup-like the content is (HTML, XML, Markdown, Svelte, Vue).
fn score_markup(f: &ContentFeatures) -> f64 {
    let mut score: f64 = 0.0;

    // Tag ratio is the primary signal
    if f.tag_ratio > 0.3 {
        score += 0.5;
    } else if f.tag_ratio > 0.15 {
        score += 0.35;
    } else if f.tag_ratio > 0.05 {
        score += 0.15;
    }

    // Tags + balanced brackets
    if f.tag_ratio > 0.1 && f.bracket_balance {
        score += 0.1;
    }

    score.max(0.0)
}

/// Score how shell-script-like the content is.
fn score_shell(f: &ContentFeatures) -> f64 {
    let mut score: f64 = 0.0;

    // Pipes
    if f.pipe_count >= 3 {
        score += 0.3;
    } else if f.pipe_count >= 1 {
        score += 0.15;
    }

    // Environment variable expansions
    if f.env_expansion_count >= 3 {
        score += 0.25;
    } else if f.env_expansion_count >= 1 {
        score += 0.1;
    }

    // Redirects
    if f.redirect_count >= 2 {
        score += 0.15;
    } else if f.redirect_count >= 1 {
        score += 0.08;
    }

    // Combination: pipes + env vars is strong shell signal
    if f.pipe_count >= 1 && f.env_expansion_count >= 1 {
        score += 0.1;
    }

    // Negative: strong code signals suppress shell
    if f.import_line_count >= 2 {
        score -= 0.2;
    }
    if f.function_def_count >= 2 {
        score -= 0.15;
    }

    // Negative: high semicolons suggest code/config, not shell.
    // Shell scripts rarely end lines with `;` except in one-liners.
    if f.semicolon_ratio > 0.3 {
        score -= 0.25;
    }

    score.max(0.0)
}

/// Score how config-file-like the content is (INI, .env, Dockerfile, Nginx).
fn score_config(f: &ContentFeatures) -> f64 {
    let mut score: f64 = 0.0;

    // Key-value lines at moderate ratio
    if f.kv_ratio > 0.5 {
        score += 0.3;
    } else if f.kv_ratio > 0.3 {
        score += 0.15;
    }

    // Section headers are characteristic of config files
    if f.section_header_count >= 1 {
        score += 0.2;
    }

    // Flat structure (shallow nesting) is more config-like than data-like
    if f.nesting_depth <= 1 && f.kv_ratio > 0.4 {
        score += 0.1;
    }

    // Environment variables suggest config
    if f.env_expansion_count >= 2 {
        score += 0.1;
    }

    // Directive-block pattern: braces + semicolons but no imports/function defs
    // (typical of Nginx, Apache, etc.)
    if f.brace_ratio > 0.3 && f.semicolon_ratio > 0.15
        && f.import_line_count == 0 && f.function_def_count == 0 {
        score += 0.35;
    }

    // Negative: imports, function defs, and prose signals
    if f.import_line_count >= 2 {
        score -= 0.2;
    }
    if f.contraction_count >= 2 {
        score -= 0.15;
    }
    // Negative: high stopword ratio → more likely prose than config
    if f.stopword_ratio > 0.25 {
        score -= 0.2;
    }

    score.max(0.0)
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::extract_features;

    fn classify(content: &str) -> FamilyResult {
        let features = extract_features(content);
        classify_family(&features)
    }

    fn top_family(content: &str) -> Option<ContentFamily> {
        classify(content).top().map(|s| s.family)
    }

    // ── Prose detection ─────────────────────────────────────

    #[test]
    fn email_is_prose() {
        let content = "Hi John,\n\nI've been thinking about the project and I'm not sure \
                        if we should proceed with the current approach. What do you think?\n\n\
                        Let's discuss when you're free.\n\nThanks,\nSarah";
        assert_eq!(top_family(content), Some(ContentFamily::Prose));
    }

    #[test]
    fn technical_discussion_is_prose() {
        let content = "I am designing a language detection system for a code editor, \
                        currently using extension + heuristics, but failing for mixed \
                        content files (yaml with embedded json/bash). How would you \
                        design a robust detection pipeline?";
        assert_eq!(top_family(content), Some(ContentFamily::Prose));
    }

    #[test]
    fn informal_note_is_prose() {
        let content = "this code works but sometimes fails not sure why can you check\n\n\
                        yaml detection is not working properly esp for multi doc and json inside it\n\n\
                        need help optimizing this, its slow when data is large";
        assert_eq!(top_family(content), Some(ContentFamily::Prose));
    }

    #[test]
    fn meeting_notes_not_code() {
        // Meeting notes look like YAML (key: value + list items) but are prose.
        // It's acceptable for the classifier to be unsure, but it should never
        // return Code as the top family.
        let content = "Team sync notes:\n- discussed migration timeline\n\
                        - agreed on Postgres over MySQL\n- John will handle auth module\n\
                        - next meeting: Friday\n\nAction items:\n\
                        - Sarah: finish API endpoints\n- Mike: review database schema";
        let result = classify(content);
        if let Some(top) = result.top() {
            assert_ne!(top.family, ContentFamily::Code,
                "meeting notes should not be classified as code (got {:?} with confidence {})",
                top.family, top.confidence);
        }
        // It's fine for no family to be returned (abstention)
    }

    #[test]
    fn essay_paragraph_is_prose() {
        let content = "The evolution of programming languages from low-level assembly to modern \
                        high-level abstractions represents one of the most significant \
                        developments in computing. From the early days of FORTRAN and COBOL, \
                        through the structured programming revolution, to today's multi-paradigm \
                        languages, each generation has brought new ideas about how humans should \
                        communicate with machines.";
        assert_eq!(top_family(content), Some(ContentFamily::Prose));
    }

    // ── Code detection ──────────────────────────────────────

    #[test]
    fn python_is_code() {
        let content = "import os\nimport sys\n\ndef main():\n    path = os.getcwd()\n    \
                        if path == '/tmp':\n        sys.exit(1)\n    print(path)\n\nmain()";
        assert_eq!(top_family(content), Some(ContentFamily::Code));
    }

    #[test]
    fn rust_is_code() {
        let content = "use std::io;\nuse std::collections::HashMap;\n\n\
                        fn main() {\n    let mut map = HashMap::new();\n    \
                        map.insert(\"key\", 42);\n    \
                        println!(\"{:?}\", map);\n}";
        assert_eq!(top_family(content), Some(ContentFamily::Code));
    }

    #[test]
    fn javascript_is_code() {
        let content = "const express = require('express');\n\
                        const app = express();\n\n\
                        app.get('/', (req, res) => {\n    \
                        res.json({ message: 'hello' });\n});\n\n\
                        app.listen(3000);";
        assert_eq!(top_family(content), Some(ContentFamily::Code));
    }

    #[test]
    fn java_is_code() {
        let content = "import java.util.List;\nimport java.util.ArrayList;\n\n\
                        public class Main {\n    public static void main(String[] args) {\n        \
                        List<String> items = new ArrayList<>();\n        \
                        items.add(\"hello\");\n        \
                        System.out.println(items.size());\n    }\n}";
        assert_eq!(top_family(content), Some(ContentFamily::Code));
    }

    // ── Structured data detection ───────────────────────────

    #[test]
    fn yaml_is_structured_data() {
        let content = "name: my-app\nversion: 1.0.0\n\ndependencies:\n  express: ^4.0\n  \
                        lodash: ^4.17\n\nscripts:\n  start: node index.js\n  test: jest\n  \
                        build: webpack --mode production";
        let result = classify(content);
        let top = result.top().expect("should classify");
        assert!(
            top.family == ContentFamily::StructuredData || top.family == ContentFamily::Config,
            "YAML should be data or config, got {:?}", top.family
        );
    }

    #[test]
    fn toml_is_structured_data() {
        let content = "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
                        [dependencies]\nserde = { version = \"1.0\", features = [\"derive\"] }\n\
                        tokio = { version = \"1\", features = [\"full\"] }";
        let result = classify(content);
        let top = result.top().expect("should classify");
        assert!(
            top.family == ContentFamily::StructuredData || top.family == ContentFamily::Config,
            "TOML should be data or config, got {:?}", top.family
        );
    }

    // ── Markup detection ────────────────────────────────────

    #[test]
    fn html_is_markup() {
        let content = "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n  <title>Test</title>\n\
                        </head>\n<body>\n  <div class=\"container\">\n    <h1>Hello</h1>\n    \
                        <p>World</p>\n  </div>\n</body>\n</html>";
        assert_eq!(top_family(content), Some(ContentFamily::Markup));
    }

    // ── Shell detection ─────────────────────────────────────

    #[test]
    fn shell_script_is_shell() {
        let content = "#!/bin/bash\nset -euo pipefail\n\nDATA_DIR=${HOME}/data\n\
                        find $DATA_DIR -name '*.log' | grep error | sort > /tmp/errors.txt\n\
                        cat /tmp/errors.txt | wc -l\necho \"Done: $DATA_DIR\"";
        assert_eq!(top_family(content), Some(ContentFamily::ShellScript));
    }

    // ── Ambiguous / edge cases ──────────────────────────────

    #[test]
    fn very_short_input_no_confident_family() {
        let result = classify("hello world");
        // Very short input shouldn't produce a confident result
        assert!(!result.is_confident() || result.top().map(|t| t.family) == Some(ContentFamily::Prose));
    }

    #[test]
    fn sql_like_prose_is_not_code() {
        // This sentence uses SQL keywords but is clearly prose
        let content = "We need to select data from the main database where the status \
                        is active and order the results by creation date. This should \
                        help us understand the distribution of users across regions.";
        let result = classify(content);
        let top = result.top().expect("should classify");
        assert_eq!(top.family, ContentFamily::Prose,
            "SQL-like prose should be Prose, not {:?}", top.family);
    }

    #[test]
    fn real_sql_is_code() {
        let content = "SELECT u.name, u.email, COUNT(o.id) as order_count\n\
                        FROM users u\n\
                        JOIN orders o ON u.id = o.user_id\n\
                        WHERE o.status = 'completed'\n\
                        GROUP BY u.name, u.email\n\
                        HAVING COUNT(o.id) > 5\n\
                        ORDER BY order_count DESC;";
        let result = classify(content);
        let top = result.top().expect("should classify");
        // SQL is a borderline case — it could be Code or StructuredData.
        // The important thing is it's NOT Prose.
        assert_ne!(top.family, ContentFamily::Prose,
            "Real SQL should not be Prose");
    }

    #[test]
    fn prompt_is_prose() {
        let content = "You are a helpful assistant. Given the following code, \
                        identify bugs and suggest fixes. Be concise and focus on \
                        the most critical issues first. If you're not sure about \
                        something, say so rather than guessing.";
        let result = classify(content);
        let top = result.top().expect("should classify");
        assert_eq!(top.family, ContentFamily::Prose,
            "AI prompt should be Prose, not {:?}", top.family);
    }
}
