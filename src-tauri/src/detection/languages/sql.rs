use regex::Regex;
use std::sync::LazyLock;

use super::{wp, LanguageDefinition};

/// Structural detection for SQL content.
pub(crate) fn is_likely_sql(trimmed: &str, _was_sliced: bool) -> bool {
    let first = trimmed.as_bytes().first().copied().unwrap_or(0);
    if first == b'<' || first == b'{' || first == b'[' {
        return false;
    }

    let lines: Vec<&str> = trimmed
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.len() < 2 {
        return false;
    }

    let non_comment: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|l| !l.starts_with("--") && !l.starts_with("/*"))
        .collect();
    if non_comment.is_empty() {
        return false;
    }

    static CODE_GUARD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^\s*(import\s|from\s+[\w.]+\s+import|export\s|def\s|class\s|func\s|fn\s|package\s|const\s|let\s|var\s|pub\s)").unwrap()
    });
    let code_count = non_comment.iter().filter(|l| CODE_GUARD.is_match(l)).count();
    if code_count as f64 / non_comment.len() as f64 > 0.05 {
        return false;
    }

    static JSX_GUARD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"<[A-Z]\w+|className=|"use (client|server)"|</[A-Z]\w+"#).unwrap()
    });
    if non_comment.iter().any(|l| JSX_GUARD.is_match(l)) {
        return false;
    }

    static CSS_GUARD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^\s*[\w.\-\[\]#:>~+*]+\s*\{|^\s*@(media|keyframes|import|layer|mixin|include)\b|:\s*(var\(--|inherit|initial|none)").unwrap()
    });
    if non_comment.iter().any(|l| CSS_GUARD.is_match(l)) {
        return false;
    }

    static ERB_GUARD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"<%[=\-]?\s|%>|^\s*end\s*$|\bdo\s*\|").unwrap()
    });
    if non_comment.iter().filter(|l| ERB_GUARD.is_match(l)).count() >= 1 {
        return false;
    }

    static RUST_GUARD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*(fn\s+\w+|let\s+mut\s|impl\s+\w|pub\s+(fn|struct|enum|mod)|use\s+\w+::\w+|#\[derive)").unwrap()
    });
    if RUST_GUARD.is_match(trimmed) {
        return false;
    }

    static GO_MOD_GUARD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(module\s+\w+[\w./\-]*|require\s+\(|go\s+\d+\.\d+)").unwrap()
    });
    if GO_MOD_GUARD.is_match(trimmed) {
        return false;
    }

    static MD_GUARD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(#{1,6}\s|\*\*|```|\|.+\|.+\||\[.+?\]\(.+?\)|^\s*[\-*+]\s+\S)").unwrap()
    });
    if MD_GUARD.is_match(trimmed) {
        return false;
    }

    let arrow_count = non_comment.iter().filter(|l| l.contains("=>")).count();
    if arrow_count >= 1 {
        return false;
    }

    let mut score = 0i32;

    static DDL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^\s*(CREATE|ALTER|DROP)\s+(TABLE|INDEX|VIEW|DATABASE|SCHEMA|PROCEDURE|FUNCTION|TRIGGER|SEQUENCE)\b").unwrap()
    });
    let ddl_count = non_comment.iter().filter(|l| DDL.is_match(l)).count();
    score += (ddl_count as i32) * 5;

    static DML: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^\s*(SELECT\s+(DISTINCT\s+)?(TOP\s+\d+\s+)?(\*|\w+\s*(,|\s+FROM))|INSERT\s+INTO|UPDATE\s+\w+\s+SET|DELETE\s+FROM|MERGE\s+INTO)\b").unwrap()
    });
    let dml_count = non_comment.iter().filter(|l| DML.is_match(l)).count();
    score += (dml_count as i32) * 3;

    static CLAUSES: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\b(FROM\s+\w+|WHERE\s+\w+|GROUP\s+BY|ORDER\s+BY|HAVING\s+|JOIN\s+\w+|ON\s+\w+\.\w+|LIMIT\s+\d|OFFSET\s+\d)\b").unwrap()
    });
    if CLAUSES.is_match(trimmed) {
        score += 2;
    }

    static TYPES: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\b(VARCHAR|INTEGER|BIGINT|DECIMAL|BOOLEAN|TIMESTAMP|PRIMARY\s+KEY|FOREIGN\s+KEY|NOT\s+NULL|DEFAULT\s+\S|UNIQUE\b|REFERENCES\s+\w+)\b").unwrap()
    });
    if TYPES.is_match(trimmed) {
        score += 3;
    }

    let sql_comment_count = lines.iter().filter(|l| l.starts_with("--")).count();
    if sql_comment_count >= 1 && score >= 3 {
        score += 2;
    }

    let semi_count = non_comment.iter().filter(|l| l.ends_with(';')).count();
    if semi_count >= 2 {
        score += 1;
    }

    score >= 6
}

pub fn definition() -> LanguageDefinition {
    LanguageDefinition {
        name: "sql",
        extensions: &[".sql"],
        filenames: &[],
        filename_patterns: &[],
        shebangs: &[],
        structural_priority: Some(110),
        structural_detect: Some(is_likely_sql),
        patterns: &[
            wp!(r"(?mi)^\s*SELECT\s+(DISTINCT\s+)?\*", 5),
            wp!(r"(?mi)^\s*SELECT\s+\w+\s*(,|\s+FROM)\s", 4),
            wp!(r"(?i)\bFROM\s+\w+", 2),
            wp!(r"(?i)\bWHERE\s+\w+\s*(=|<|>|!=|IS\s|IN\s|LIKE\s|BETWEEN\s)", 3),
            wp!(r"(?i)\b(INNER|LEFT|RIGHT|FULL|CROSS)\s+JOIN\b", 5),
            wp!(r"(?i)\bINSERT\s+INTO\s+\w+", 4),
            wp!(r"(?i)\bCREATE\s+(TABLE|INDEX|VIEW|DATABASE|PROCEDURE|FUNCTION)\b", 5),
            wp!(r"(?i)\bALTER\s+TABLE\s+\w+", 5),
            wp!(r"(?i)\bDROP\s+(TABLE|INDEX|VIEW|DATABASE)\b", 4),
            wp!(r"(?i)\bGROUP\s+BY\b", 3),
            wp!(r"(?i)\bORDER\s+BY\b", 2),
            wp!(r"(?i)\bHAVING\s+", 3),
            wp!(r"(?i)\bUNION\s+(ALL\s+)?SELECT\b", 4),
            wp!(r"(?i)\bPRIMARY\s+KEY\b", 3),
            wp!(r"(?i)\b(VARCHAR|INTEGER|BIGINT|DECIMAL|BOOLEAN|TIMESTAMP)\b", 3),
            wp!(r"(?i)\bNOT\s+NULL\b", 2),
            wp!(r"(?i)\bDEFAULT\s+", 1),
            // SQL line comments
            wp!(r"(?m)^\s*--\s", 2),
        ],
        anti_patterns: &[
            wp!(r"(?m)\bclass\s+\w+", -5),
            // SQL uses stronger heading/fence weights than the -3 common default,
            // so we override them here. The auto-applied ones will stack, giving
            // a total of -8 for headings and -8 for fences — correct for SQL.
            wp!(r"(?m)^```", -5),
            wp!(r"(?m)^#{1,6}\s+\S", -5),
            wp!(r#"(?m)^\s*import\s+[\w\{*].*\s+from\s+['"`]"#, -5),
            wp!(r"(?m)^\s*def\s+\w+\s*\(", -3),
            wp!(r"(?m)^\s*from\s+[\w.]+\s+import\s", -3),
            wp!(r#"(?m)^\s*"use (client|server)""#, -5),
            wp!(r"<[A-Z]\w+", -5),
            wp!(r"className=", -5),
            wp!(r"(?m)^\s*[\w.\-\[\]#]+\s*\{", -3),
            wp!(r"(?m)^\s*@(media|keyframes|layer)\b", -5),
        ],
        uses_hash_comments: false,
        keywords: &[
            "select", "insert", "update", "delete", "create", "alter",
            "drop", "table", "view", "index", "database", "schema",
            "join", "inner", "outer", "left", "right", "cross",
            "where", "having", "group", "order", "union", "intersect",
            "except", "exists", "between", "like", "distinct",
            "primary", "foreign", "key", "constraint", "references",
            "trigger", "procedure", "function", "cursor", "grant",
            "revoke", "commit", "rollback", "transaction", "begin",
        ],
        builtins: &[
            "varchar", "integer", "bigint", "smallint", "decimal",
            "numeric", "float", "double", "boolean", "timestamp",
            "date", "time", "char", "text", "blob", "clob",
            "serial", "autoincrement", "identity",
        ],
        family: None,
        exclusive_patterns: &[],
    }
}
