/// Content feature extraction for family classification.
///
/// Extracts language-agnostic statistical features from document content.
/// These features describe the *shape* of the document (prose-like, code-like,
/// data-like, markup-like) without knowing about any specific programming language.
///
/// The feature extractor is the foundation of the family classifier — it runs
/// once per detection call and its output gates which language candidates
/// are even considered.
use regex::Regex;
use std::sync::LazyLock;

// ── Regexes (compiled once) ─────────────────────────────────────────────

/// English contractions: it's, don't, can't, we're, I'm, etc.
static CONTRACTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b\w+'(m|s|re|ll|t|ve|d)\b").unwrap());

/// Common English stopwords (the, is, and, of, to, a, in, that, it, for, ...)
/// Matched as whole words, case-insensitive.
static STOPWORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(the|is|are|was|were|be|been|being|have|has|had|do|does|did|will|would|could|should|shall|may|might|can|must|a|an|and|but|or|not|no|so|if|then|than|that|this|these|those|it|its|of|in|on|at|to|for|with|from|by|about|as|into|through|during|before|after|above|below|between|out|up|down|off|over|under|again|further|also|just|very|too|more|most|other|each|some|such|only|own|same|all|both|few|many|any|every|much|what|which|who|how|when|where|why)\b").unwrap()
});

/// Personal pronouns (I, you, we, they, my, your, etc.)
static PRONOUN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(I|you|we|they|he|she|my|your|our|their|his|her|me|us|them|mine|yours|ours|theirs|myself|yourself|ourselves|themselves)\b")
        .unwrap()
});

/// Greeting patterns at line start.
static GREETING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?im)^\s*(hi|hey|hello|dear|greetings)\b").unwrap());

/// Closing/sign-off patterns.
static CLOSING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^\s*(thanks|thank you|regards|best|sincerely|cheers|take care)\b").unwrap()
});

/// Import/include/require/use statements at line start.
/// Also matches Lisp-style (:require and (require patterns.
static IMPORT_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\s*(import|from\s+\S+\s+import|#include|require|use|using|\(:require|\(require)\b").unwrap()
});

/// Function/method definitions.
/// Also matches Lisp-style (defn, (define, (defun patterns.
static FUNCTION_DEF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\s*\(?(def[nun]?|fn|func|function|fun|sub|proc|method)\s+\w+").unwrap()
});

/// Function/method call expressions: `foo(`, `obj.method(`, `Class.new(`.
/// Nearly universal in programming languages, absent from prose and data formats.
static CALL_EXPRESSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[a-zA-Z_]\w*\s*\(").unwrap()
});

/// Block-end keywords on their own line: `end`, `fi`, `done`, `esac`.
/// Used by Ruby, Lua, Elixir, Shell, etc. — never prose or data.
static BLOCK_END: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\s*(end|fi|done|esac)\s*$").unwrap()
});

/// Key-value lines: `key: value`, `key = value`, `key=value`
static KV_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\s*[\w][\w.\-/]*\s*[:=]\s*\S").unwrap()
});

/// HTML/XML tags: <tag>, </tag>, <tag attr="val">
static TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"</?[a-zA-Z][\w\-]*[^>]*>").unwrap());

/// Section headers: [section], [[array-of-tables]]
static SECTION_HEADER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*\[{1,2}[\w.\-/]+\]{1,2}\s*$").unwrap());

/// Environment variable expansions: $VAR, ${VAR}, $env:VAR, %VAR%
static ENV_EXPANSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\$\{?\w+\}?|\$env:\w+|%\w+%)").unwrap());

/// Pipe operators in shell: cmd1 | cmd2
static PIPE_OP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\|\s*\w").unwrap());

/// Redirect operators: >, >>, <, 2>&1
static REDIRECT_OP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[12]?>>?\s|\s<\s").unwrap());

/// Programming operators: ==, !=, >=, <=, &&, ||, ->, =>, ::
static CODE_OPERATOR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(==|!=|>=|<=|&&|\|\||->|=>|::|\+=|-=|\*=|/=|%=)").unwrap()
});

/// Lines starting with SQL clause-level keywords.
/// Used as a code-family signal: SQL has no imports, braces, or function defs
/// but its distinctive clause-per-line structure is strongly code-like.
static SQL_CODE_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(SELECT|FROM|WHERE|ORDER\s+BY|GROUP\s+BY|HAVING|INSERT\s+INTO|UPDATE\s+.*SET|DELETE\s+FROM|CREATE\s+(TABLE|INDEX|VIEW|DATABASE|SCHEMA)|ALTER\s+(TABLE|INDEX|VIEW|DATABASE|SCHEMA)|DROP\s+(TABLE|INDEX|VIEW|DATABASE|SCHEMA)|TRUNCATE|EXPLAIN|DESCRIBE|WITH\s+\w+\s+AS|MERGE)\b").unwrap()
});

/// Word-like tokens for counting.
static WORD_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[a-zA-Z']+").unwrap());

// ── Feature struct ──────────────────────────────────────────────────────

/// Language-agnostic statistical features of a document.
///
/// Every field describes the *shape* of the content without reference to
/// any specific programming language. The family classifier uses these
/// features to decide whether the document is prose, code, data, markup,
/// shell script, or ambiguous.
#[derive(Debug, Clone)]
pub struct ContentFeatures {
    // ── Prose signals ────────────────────────────────────────
    /// Fraction of word tokens that are common English stopwords (0.0–1.0).
    pub stopword_ratio: f64,
    /// Number of English contractions found (it's, don't, etc.).
    pub contraction_count: usize,
    /// Fraction of non-empty lines that end with a question mark.
    pub question_ratio: f64,
    /// Average number of word tokens per non-empty line.
    pub avg_words_per_line: f64,
    /// Whether a greeting pattern was found near the start.
    pub greeting_present: bool,
    /// Whether a closing/sign-off pattern was found.
    pub closing_present: bool,
    /// Fraction of word tokens that are personal pronouns (0.0–1.0).
    pub pronoun_ratio: f64,

    // ── Code signals ─────────────────────────────────────────
    /// Fraction of non-empty lines where SQL clause-level keywords start the line.
    pub sql_code_density: f64,
    /// Fraction of non-empty lines ending with `;`.
    pub semicolon_ratio: f64,
    /// Fraction of non-empty lines containing `{` or `}`.
    pub brace_ratio: f64,
    /// Number of import/include/require/use lines.
    pub import_line_count: usize,
    /// Number of function/method definition lines.
    pub function_def_count: usize,
    /// Average programming operators (==, !=, ->, =>, etc.) per non-empty line.
    pub operator_density: f64,
    /// Number of function/method call expressions (e.g., `foo(`, `obj.method(`).
    pub call_expression_count: usize,
    /// Number of block-end keywords on their own line (end, fi, done, esac).
    pub block_end_count: usize,

    // ── Data / Config signals ────────────────────────────────
    /// Fraction of non-empty lines matching `key: value` or `key = value`.
    pub kv_ratio: f64,
    /// Maximum indentation depth (in levels, not characters).
    pub nesting_depth: usize,
    /// Whether all bracket pairs ([], {}, ()) are balanced.
    pub bracket_balance: bool,
    /// Number of `[section]` or `[[table]]` headers.
    pub section_header_count: usize,

    // ── Markup signals ───────────────────────────────────────
    /// Fraction of characters that are inside HTML/XML tags.
    pub tag_ratio: f64,

    // ── Shell signals ────────────────────────────────────────
    /// Number of pipe operators (`|`).
    pub pipe_count: usize,
    /// Number of redirect operators (`>`, `>>`, `<`).
    pub redirect_count: usize,
    /// Number of environment variable expansions ($VAR, ${VAR}, %VAR%).
    pub env_expansion_count: usize,

    // ── Derived ──────────────────────────────────────────────
    /// Total number of non-empty lines analysed.
    pub non_empty_lines: usize,
    /// Total number of word tokens found.
    pub total_words: usize,
}

/// Extract language-agnostic features from document content.
///
/// The caller is responsible for bounding the input to MAX_DETECTION_BYTES
/// and stripping BOM/trimming. This function operates on whatever content
/// it receives.
pub fn extract_features(content: &str) -> ContentFeatures {
    let lines: Vec<&str> = content.lines().collect();
    let non_empty: Vec<&str> = lines.iter().filter(|l| !l.trim().is_empty()).copied().collect();
    let non_empty_count = non_empty.len();

    // Word tokens for stopword/pronoun analysis
    let all_words: Vec<&str> = WORD_TOKEN.find_iter(content).map(|m| m.as_str()).collect();
    let total_words = all_words.len();

    // ── Prose signals ────────────────────────────────────────
    let stopword_hits = STOPWORD.find_iter(content).count();
    let stopword_ratio = if total_words > 0 {
        stopword_hits as f64 / total_words as f64
    } else {
        0.0
    };

    let contraction_count = CONTRACTION.find_iter(content).count();

    let question_lines = non_empty.iter().filter(|l| l.trim().ends_with('?')).count();
    let question_ratio = if non_empty_count > 0 {
        question_lines as f64 / non_empty_count as f64
    } else {
        0.0
    };

    let words_per_line_sum: usize = non_empty
        .iter()
        .map(|l| WORD_TOKEN.find_iter(l).count())
        .sum();
    let avg_words_per_line = if non_empty_count > 0 {
        words_per_line_sum as f64 / non_empty_count as f64
    } else {
        0.0
    };

    let greeting_present = GREETING.is_match(content);
    let closing_present = CLOSING.is_match(content);

    let pronoun_hits = PRONOUN.find_iter(content).count();
    let pronoun_ratio = if total_words > 0 {
        pronoun_hits as f64 / total_words as f64
    } else {
        0.0
    };

    // ── Code signals ─────────────────────────────────────────
    let sql_code_lines = non_empty.iter().filter(|l| SQL_CODE_LINE.is_match(l)).count();
    let sql_code_density = if non_empty_count > 0 {
        sql_code_lines as f64 / non_empty_count as f64
    } else {
        0.0
    };

    let semicolon_lines = non_empty
        .iter()
        .filter(|l| l.trim().ends_with(';'))
        .count();
    let semicolon_ratio = if non_empty_count > 0 {
        semicolon_lines as f64 / non_empty_count as f64
    } else {
        0.0
    };

    let brace_lines = non_empty
        .iter()
        .filter(|l| {
            let t = l.trim();
            t.contains('{') || t.contains('}')
        })
        .count();
    let brace_ratio = if non_empty_count > 0 {
        brace_lines as f64 / non_empty_count as f64
    } else {
        0.0
    };

    let import_line_count = IMPORT_LINE.find_iter(content).count();
    let function_def_count = FUNCTION_DEF.find_iter(content).count();

    let operator_total = CODE_OPERATOR.find_iter(content).count();
    let operator_density = if non_empty_count > 0 {
        operator_total as f64 / non_empty_count as f64
    } else {
        0.0
    };

    let call_expression_count = non_empty
        .iter()
        .filter(|l| CALL_EXPRESSION.is_match(l))
        .count();
    let block_end_count = BLOCK_END.find_iter(content).count();

    // ── Data / Config signals ────────────────────────────────
    let kv_lines = non_empty.iter().filter(|l| KV_LINE.is_match(l)).count();
    let kv_ratio = if non_empty_count > 0 {
        kv_lines as f64 / non_empty_count as f64
    } else {
        0.0
    };

    // Nesting depth: measure max indentation in units of 2 spaces
    let nesting_depth = non_empty
        .iter()
        .map(|l| {
            let indent = l.len() - l.trim_start().len();
            indent / 2 // Rough level count
        })
        .max()
        .unwrap_or(0);

    let bracket_balance = check_bracket_balance(content);
    let section_header_count = SECTION_HEADER.find_iter(content).count();

    // ── Markup signals ───────────────────────────────────────
    let tag_chars: usize = TAG.find_iter(content).map(|m| m.as_str().len()).sum();
    let tag_ratio = if !content.is_empty() {
        tag_chars as f64 / content.len() as f64
    } else {
        0.0
    };

    // ── Shell signals ────────────────────────────────────────
    let pipe_count = PIPE_OP.find_iter(content).count();
    let redirect_count = REDIRECT_OP.find_iter(content).count();
    let env_expansion_count = ENV_EXPANSION.find_iter(content).count();

    ContentFeatures {
        stopword_ratio,
        contraction_count,
        question_ratio,
        avg_words_per_line,
        greeting_present,
        closing_present,
        pronoun_ratio,
        sql_code_density,
        semicolon_ratio,
        brace_ratio,
        import_line_count,
        function_def_count,
        operator_density,
        call_expression_count,
        block_end_count,
        kv_ratio,
        nesting_depth,
        bracket_balance,
        section_header_count,
        tag_ratio,
        pipe_count,
        redirect_count,
        env_expansion_count,
        non_empty_lines: non_empty_count,
        total_words,
    }
}

/// Check if all brackets are balanced: (), [], {}
fn check_bracket_balance(content: &str) -> bool {
    let mut parens = 0i32;
    let mut brackets = 0i32;
    let mut braces = 0i32;
    // Skip content inside strings (rough: toggle on ", ignore escaped \")
    let mut in_string = false;
    let mut prev_char = '\0';
    for ch in content.chars() {
        if ch == '"' && prev_char != '\\' {
            in_string = !in_string;
        }
        if !in_string {
            match ch {
                '(' => parens += 1,
                ')' => parens -= 1,
                '[' => brackets += 1,
                ']' => brackets -= 1,
                '{' => braces += 1,
                '}' => braces -= 1,
                _ => {}
            }
            // Early exit: negative means unmatched closer
            if parens < 0 || brackets < 0 || braces < 0 {
                return false;
            }
        }
        prev_char = ch;
    }
    parens == 0 && brackets == 0 && braces == 0
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prose_features() {
        let content = "Hi there,\n\nI've been thinking about the project and I'm not sure \
                        if we should proceed with the current approach. What do you think?\n\n\
                        Let's discuss when you're free.\n\nThanks,\nJohn";
        let f = extract_features(content);
        assert!(f.stopword_ratio > 0.25, "stopword_ratio={}", f.stopword_ratio);
        assert!(f.contraction_count >= 3, "contractions={}", f.contraction_count);
        assert!(f.greeting_present, "should detect greeting");
        assert!(f.closing_present, "should detect closing");
        assert!(f.pronoun_ratio > 0.03, "pronoun_ratio={}", f.pronoun_ratio);
        assert!(f.question_ratio > 0.0, "question_ratio={}", f.question_ratio);
        assert_eq!(f.semicolon_ratio, 0.0);
        assert_eq!(f.brace_ratio, 0.0);
        assert_eq!(f.import_line_count, 0);
        assert_eq!(f.function_def_count, 0);
    }

    #[test]
    fn code_features_python() {
        let content = "import os\nimport sys\n\ndef main():\n    path = os.getcwd()\n    \
                        if path == '/tmp':\n        sys.exit(1)\n    print(path)\n\nmain()";
        let f = extract_features(content);
        assert!(f.import_line_count >= 2, "imports={}", f.import_line_count);
        assert!(f.function_def_count >= 1, "func_defs={}", f.function_def_count);
        assert!(f.operator_density > 0.0, "operators={}", f.operator_density);
        assert!(f.stopword_ratio < 0.3, "stopword_ratio={}", f.stopword_ratio);
        assert_eq!(f.contraction_count, 0);
        assert!(!f.greeting_present);
        assert!(!f.closing_present);
    }

    #[test]
    fn code_features_rust() {
        let content = "use std::io;\n\nfn main() {\n    let mut input = String::new();\n    \
                        io::stdin().read_line(&mut input).unwrap();\n    \
                        println!(\"{}\", input.trim());\n}";
        let f = extract_features(content);
        assert!(f.import_line_count >= 1, "imports={}", f.import_line_count);
        assert!(f.function_def_count >= 1, "func_defs={}", f.function_def_count);
        assert!(f.brace_ratio > 0.1, "brace_ratio={}", f.brace_ratio);
        assert!(f.bracket_balance);
    }

    #[test]
    fn yaml_features() {
        let content = "name: my-app\nversion: 1.0.0\n\ndependencies:\n  - express\n  \
                        - lodash\n\nscripts:\n  start: node index.js\n  test: jest";
        let f = extract_features(content);
        assert!(f.kv_ratio > 0.4, "kv_ratio={}", f.kv_ratio);
        assert!(f.semicolon_ratio == 0.0);
        assert!(f.brace_ratio == 0.0);
        assert_eq!(f.import_line_count, 0);
        assert_eq!(f.contraction_count, 0);
    }

    #[test]
    fn html_features() {
        let content = "<!DOCTYPE html>\n<html>\n<head>\n  <title>Test</title>\n</head>\n\
                        <body>\n  <div class=\"container\">\n    <h1>Hello</h1>\n    \
                        <p>World</p>\n  </div>\n</body>\n</html>";
        let f = extract_features(content);
        assert!(f.tag_ratio > 0.15, "tag_ratio={}", f.tag_ratio);
        assert_eq!(f.contraction_count, 0);
        assert_eq!(f.import_line_count, 0);
    }

    #[test]
    fn shell_features() {
        let content = "#!/bin/bash\nset -euo pipefail\n\nDATA_DIR=${HOME}/data\n\
                        find $DATA_DIR -name '*.log' | grep error | sort > /tmp/errors.txt\n\
                        cat /tmp/errors.txt | wc -l\necho \"Done: $DATA_DIR processed\"";
        let f = extract_features(content);
        assert!(f.pipe_count >= 2, "pipes={}", f.pipe_count);
        assert!(f.redirect_count >= 1, "redirects={}", f.redirect_count);
        assert!(f.env_expansion_count >= 2, "env_expansions={}", f.env_expansion_count);
    }

    #[test]
    fn bracket_balance_correct() {
        assert!(check_bracket_balance("fn main() { let x = [1, 2, 3]; }"));
        assert!(check_bracket_balance("()[]{}"));
        assert!(check_bracket_balance(""));
    }

    #[test]
    fn bracket_balance_unmatched() {
        assert!(!check_bracket_balance("fn main() {"));
        assert!(!check_bracket_balance("let x = [1, 2"));
        assert!(!check_bracket_balance("((())"));
    }

    #[test]
    fn short_input_features() {
        let f = extract_features("hello");
        assert_eq!(f.non_empty_lines, 1);
        assert_eq!(f.total_words, 1);
        assert_eq!(f.import_line_count, 0);
        assert_eq!(f.function_def_count, 0);
    }

    #[test]
    fn empty_input_features() {
        let f = extract_features("");
        assert_eq!(f.non_empty_lines, 0);
        assert_eq!(f.total_words, 0);
        assert_eq!(f.stopword_ratio, 0.0);
    }

    #[test]
    fn technical_prose_features() {
        // Prose that mentions code concepts — should still read as prose
        let content = "I am designing a language detection system for a code editor, \
                        currently using extension + heuristics, but failing for mixed \
                        content files (yaml with embedded json/bash). How would you \
                        design a robust detection pipeline?";
        let f = extract_features(content);
        assert!(f.stopword_ratio > 0.2, "stopword_ratio={}", f.stopword_ratio);
        assert!(f.question_ratio > 0.0, "should have question");
        assert_eq!(f.semicolon_ratio, 0.0);
        assert_eq!(f.brace_ratio, 0.0);
        assert_eq!(f.import_line_count, 0);
        assert_eq!(f.function_def_count, 0);
    }

    #[test]
    fn json_features() {
        let content = "{\n  \"name\": \"test\",\n  \"version\": \"1.0\",\n  \
                        \"dependencies\": {\n    \"express\": \"^4.0\"\n  }\n}";
        let f = extract_features(content);
        assert!(f.brace_ratio > 0.2, "brace_ratio={}", f.brace_ratio);
        assert!(f.bracket_balance);
        assert_eq!(f.import_line_count, 0);
        assert_eq!(f.function_def_count, 0);
        // JSON "key": "value" lines don't match the kv regex due to quotes —
        // that's fine, JSON is detected structurally (Phase 0 parse), not by kv_ratio.
    }
}
