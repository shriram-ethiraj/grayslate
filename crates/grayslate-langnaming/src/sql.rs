use super::{prose::extract_yake, shared::slugify};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

// ── Types ───────────────────────────────────────────────────────────────

struct CteInfo {
    name: String,
    body: String,
}

// ── Static regex patterns ───────────────────────────────────────────────

static LINE_COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"--[^\n]*").unwrap());
static BLOCK_COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)/\*.*?\*/").unwrap());

/// Matches: `alias AS (`  — captures the alias name.
static CTE_ALIAS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*(\w+)\s+AS\s*\(").unwrap());

static CREATE_TARGET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)\bCREATE\s+(?:OR\s+REPLACE\s+)?(?:TABLE|VIEW|FUNCTION|PROCEDURE|INDEX)\s+(?:IF\s+NOT\s+EXISTS\s+)?(?:\w+\.)?(\w+|"[^"]+")"#,
    )
    .unwrap()
});
static INSERT_TARGET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\bINSERT\s+INTO\s+(?:\w+\.)?(\w+|"[^"]+")"#).unwrap()
});
static UPDATE_TARGET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\bUPDATE\s+(?:\w+\.)?(\w+|"[^"]+")"#).unwrap()
});

static FROM_TABLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\bFROM\s+(?:\w+\.)?(\w+|"[^"]+")"#).unwrap()
});
static JOIN_TABLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)\b(?:(?:LEFT|RIGHT|INNER|CROSS|FULL|OUTER|NATURAL)\s+)*JOIN\s+(?:\w+\.)?(\w+|"[^"]+")"#,
    )
    .unwrap()
});

/// Matches WHERE/AND column before a comparison operator or keyword.
static WHERE_COLUMN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:WHERE|AND)\s+(?:\w+\.)?(\w+)\s*(?:[=<>!]+|(?:LIKE|IN|BETWEEN|IS)\b)",
    )
    .unwrap()
});

static GROUP_BY_START: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bGROUP\s+BY\b").unwrap());
static GROUP_BY_END: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:HAVING|ORDER\s+BY|LIMIT|UNION|EXCEPT|INTERSECT)\b").unwrap()
});

static ORDER_BY_START: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bORDER\s+BY\b").unwrap());
static ORDER_BY_END: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:LIMIT|OFFSET|FETCH|UNION|EXCEPT|INTERSECT)\b").unwrap()
});

// ── Public entry ────────────────────────────────────────────────────────

/// Regex-based SQL content extraction.
///
/// Signal priority (higher = more semantically meaningful for naming):
///   10 – First CTE name, CREATE TABLE/VIEW/FUNCTION
///    9 – Last CTE name (the "output" concept in multi-CTE pipelines)
///    8 – Primary FROM table, INSERT/UPDATE target
///    7 – GROUP BY column (aggregation dimension)
///    6 – ORDER BY column (sorting intent)
///    5 – WHERE filter column (filtering context)
///    4 – JOINed tables
///
/// After ranking, a prefix-aware word deduplication pass ensures no individual
/// word (or prefix-relative near-duplicate like "month"/"monthly") appears
/// twice in the final stem.
pub(crate) fn extract_sql(content: &str) -> Option<String> {
    let cleaned = strip_comments(content);
    let mut signals: Vec<(usize, u8, String)> = Vec::new();
    let mut order = 0usize;

    // DDL signals (P10).
    for cap in CREATE_TARGET.captures_iter(&cleaned) {
        push_signal(&mut signals, &mut order, 10, &unquote(&cap[1]));
    }

    // DML signals (P8 target).
    for cap in INSERT_TARGET.captures_iter(&cleaned) {
        push_signal(&mut signals, &mut order, 8, &unquote(&cap[1]));
    }
    for cap in UPDATE_TARGET.captures_iter(&cleaned) {
        push_signal(&mut signals, &mut order, 8, &unquote(&cap[1]));
    }

    // CTE + query extraction.
    let (ctes, body) = parse_ctes(&cleaned);
    let cte_names: HashSet<String> =
        ctes.iter().map(|c| c.name.to_ascii_lowercase()).collect();

    process_ctes(&ctes, &cte_names, &mut signals, &mut order);

    let flat = flatten_subqueries(&body);
    extract_query_signals(&flat, &cte_names, &mut signals, &mut order);

    if signals.is_empty() {
        return extract_yake(content);
    }

    build_stem(signals)
}

// ── Comment stripping ───────────────────────────────────────────────────

fn strip_comments(sql: &str) -> String {
    let no_block = BLOCK_COMMENT.replace_all(sql, " ");
    LINE_COMMENT.replace_all(&no_block, " ").into_owned()
}

// ── Identifier unquoting ────────────────────────────────────────────────

fn unquote(name: &str) -> String {
    let s = name.trim();
    if s.len() >= 2 {
        if (s.starts_with('"') && s.ends_with('"'))
            || (s.starts_with('`') && s.ends_with('`'))
        {
            return s[1..s.len() - 1].to_string();
        }
        if s.starts_with('[') && s.ends_with(']') {
            return s[1..s.len() - 1].to_string();
        }
    }
    s.to_string()
}

// ── Subquery flattening ─────────────────────────────────────────────────

/// Replace all parenthesized content with spaces so that regex only sees
/// top-level SQL clauses. This prevents capturing FROM/JOIN/WHERE inside
/// subqueries, function calls, or window expressions.
fn flatten_subqueries(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut depth = 0u32;
    let mut in_sq = false;
    let mut in_dq = false;
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];

        if in_sq {
            if b == b'\'' {
                // Handle escaped '' inside strings.
                if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    out.push(if depth == 0 { b } else { b' ' });
                    out.push(if depth == 0 { b'\'' } else { b' ' });
                    i += 2;
                    continue;
                }
                in_sq = false;
            }
            out.push(if depth == 0 { b } else { b' ' });
            i += 1;
            continue;
        }

        if in_dq {
            if b == b'"' {
                in_dq = false;
            }
            out.push(if depth == 0 { b } else { b' ' });
            i += 1;
            continue;
        }

        match b {
            b'\'' => {
                in_sq = true;
                out.push(if depth == 0 { b } else { b' ' });
            }
            b'"' => {
                in_dq = true;
                out.push(if depth == 0 { b } else { b' ' });
            }
            b'(' => {
                depth += 1;
                out.push(b' ');
            }
            b')' => {
                depth = depth.saturating_sub(1);
                out.push(b' ');
            }
            _ => out.push(if depth == 0 { b } else { b' ' }),
        }
        i += 1;
    }

    String::from_utf8(out).unwrap_or_default()
}

// ── CTE parsing ─────────────────────────────────────────────────────────

/// Find the matching `)` for an `(` at `open`, handling nested parens and
/// string literals. Returns `None` if unbalanced.
fn find_matching_paren(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 1u32;
    let mut in_sq = false;
    let mut in_dq = false;
    let mut i = open + 1;

    while i < bytes.len() {
        let b = bytes[i];

        if in_sq {
            if b == b'\'' {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    i += 2;
                    continue;
                }
                in_sq = false;
            }
            i += 1;
            continue;
        }
        if in_dq {
            if b == b'"' {
                in_dq = false;
            }
            i += 1;
            continue;
        }

        match b {
            b'\'' => in_sq = true,
            b'"' => in_dq = true,
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Parse `WITH [RECURSIVE] alias AS (...), alias AS (...) ...` and return
/// the list of CTEs plus the remaining query body.
fn parse_ctes(sql: &str) -> (Vec<CteInfo>, String) {
    // Find leading WITH [RECURSIVE].
    let trimmed = sql.trim_start();
    let upper = trimmed.to_ascii_uppercase();
    let after_with = if let Some(rest) = strip_keyword(&upper, trimmed, "WITH") {
        let rest = rest.trim_start();
        let upper_rest = rest.to_ascii_uppercase();
        strip_keyword(&upper_rest, rest, "RECURSIVE")
            .map(|r| r.trim_start())
            .unwrap_or(rest)
    } else {
        return (Vec::new(), sql.to_string());
    };

    // The offset into `sql` where CTE aliases begin.
    let base_offset = sql.len() - after_with.len();
    let mut pos = base_offset;
    let mut ctes = Vec::new();

    loop {
        let remaining = &sql[pos..];
        let cap = match CTE_ALIAS.captures(remaining) {
            Some(c) => c,
            None => break,
        };
        let name = cap[1].to_string();
        // The `(` is at the end of the overall match minus 1.
        let open_paren = pos + cap.get(0).unwrap().end() - 1;
        let close_paren = match find_matching_paren(sql.as_bytes(), open_paren) {
            Some(p) => p,
            None => break,
        };

        ctes.push(CteInfo {
            name,
            body: sql[open_paren + 1..close_paren].to_string(),
        });

        pos = close_paren + 1;

        // Skip whitespace + optional comma.
        let tail = sql[pos..].trim_start();
        if tail.starts_with(',') {
            let comma_abs = sql.len() - tail.len();
            pos = comma_abs + 1;
        } else {
            pos = sql.len() - tail.len();
            break;
        }
    }

    (ctes, sql[pos..].to_string())
}

/// If `upper` starts with `keyword` followed by a word boundary, return
/// the corresponding slice of `original` after the keyword.
fn strip_keyword<'a>(upper: &str, original: &'a str, keyword: &str) -> Option<&'a str> {
    if upper.starts_with(keyword) {
        let after = &upper[keyword.len()..];
        if after.is_empty() || !after.as_bytes()[0].is_ascii_alphanumeric() {
            return Some(&original[keyword.len()..]);
        }
    }
    None
}

// ── CTE processing ─────────────────────────────────────────────────────

fn process_ctes(
    ctes: &[CteInfo],
    cte_names: &HashSet<String>,
    signals: &mut Vec<(usize, u8, String)>,
    order: &mut usize,
) {
    for (index, cte) in ctes.iter().enumerate() {
        let priority = if index == 0 {
            10
        } else if index + 1 == ctes.len() {
            9
        } else {
            0
        };

        let is_generic = is_generic_cte_name(&cte.name);
        let descriptor = describe_body(&cte.body, cte_names);
        let is_redundant = descriptor
            .as_deref()
            .is_some_and(|d| alias_is_redundant(&cte.name, d));
        let keep_alias = !is_generic && !is_redundant && priority > 0;

        if keep_alias {
            push_signal(signals, order, priority, &cte.name);
        }

        if !keep_alias {
            // Fall back to inner query signals.
            let flat = flatten_subqueries(&cte.body);
            extract_query_signals(&flat, cte_names, signals, order);
        }
    }
}

/// Build a descriptor stem from the inner query of a CTE body, used to
/// check whether a CTE alias is redundant with its content.
fn describe_body(body: &str, cte_names: &HashSet<String>) -> Option<String> {
    let flat = flatten_subqueries(body);
    let mut inner_signals = Vec::new();
    let mut inner_order = 0usize;
    extract_query_signals(&flat, cte_names, &mut inner_signals, &mut inner_order);
    build_stem(inner_signals)
}

// ── Query-level signal extraction ───────────────────────────────────────

fn extract_query_signals(
    flat_sql: &str,
    cte_names: &HashSet<String>,
    signals: &mut Vec<(usize, u8, String)>,
    order: &mut usize,
) {
    // FROM tables (P8).
    for cap in FROM_TABLE.captures_iter(flat_sql) {
        let name = unquote(&cap[1]);
        if !cte_names.contains(&name.to_ascii_lowercase()) {
            push_signal(signals, order, 8, &name);
        }
    }

    // JOINed tables (P4).
    for cap in JOIN_TABLE.captures_iter(flat_sql) {
        let name = unquote(&cap[1]);
        if !cte_names.contains(&name.to_ascii_lowercase()) {
            push_signal(signals, order, 4, &name);
        }
    }

    // GROUP BY columns (P7).
    if let Some(clause) = find_clause_text(flat_sql, &GROUP_BY_START, &GROUP_BY_END) {
        for col in extract_columns_from_clause(clause) {
            push_signal(signals, order, 7, &col);
        }
    }

    // ORDER BY columns (P6).
    if let Some(clause) = find_clause_text(flat_sql, &ORDER_BY_START, &ORDER_BY_END) {
        for col in extract_columns_from_clause(clause) {
            push_signal(signals, order, 6, &col);
        }
    }

    // WHERE columns (P5), capped at 2.
    let mut where_count = 0;
    for cap in WHERE_COLUMN.captures_iter(flat_sql) {
        if where_count >= 2 {
            break;
        }
        push_signal(signals, order, 5, &cap[1]);
        where_count += 1;
    }
}

/// Extract the text of a clause (e.g. GROUP BY columns, ORDER BY columns)
/// between `start_re` and the nearest following `end_re` or semicolon.
fn find_clause_text<'a>(
    sql: &'a str,
    start_re: &Regex,
    end_re: &Regex,
) -> Option<&'a str> {
    let mat = start_re.find(sql)?;
    let start = mat.end();
    let end = end_re
        .find(&sql[start..])
        .map(|m| start + m.start())
        .unwrap_or_else(|| {
            sql[start..]
                .find(';')
                .map(|p| start + p)
                .unwrap_or(sql.len())
        });
    Some(&sql[start..end])
}

/// Extract individual column names from a comma-separated GROUP BY or
/// ORDER BY clause. Handles `alias.column`, strips ASC/DESC, and skips
/// numeric positional references.
fn extract_columns_from_clause(clause: &str) -> Vec<String> {
    let mut cols = Vec::new();
    for part in clause.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Skip pure numeric positional references.
        if trimmed
            .chars()
            .all(|c| c.is_ascii_digit() || c.is_ascii_whitespace())
        {
            continue;
        }
        // Take the first whitespace-delimited token (strips ASC/DESC/NULLS).
        let token = match trimmed.split_whitespace().next() {
            Some(t) => t,
            None => continue,
        };
        // Take the last part after '.' (handle alias.column).
        let col = token.rsplit('.').next().unwrap_or("");
        if !col.is_empty() && col.chars().all(|c| c.is_alphanumeric() || c == '_') {
            cols.push(col.to_string());
        }
    }
    cols
}

// ── CTE name classification ─────────────────────────────────────────────

fn is_generic_cte_name(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    if lower.len() == 1 {
        return true;
    }
    for prefix in ["step", "cte", "tmp", "temp", "subq", "q"] {
        if let Some(rest) = lower.strip_prefix(prefix) {
            if !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_digit()) {
                return true;
            }
        }
    }
    false
}

fn alias_is_redundant(alias: &str, descriptor: &str) -> bool {
    let Some(alias_slug) = slugify(alias) else {
        return false;
    };
    let Some(descriptor_slug) = slugify(descriptor) else {
        return false;
    };
    let descriptor_words: HashSet<&str> = descriptor_slug.split('-').collect();
    alias_slug
        .split('-')
        .all(|word| descriptor_words.contains(word))
}

// ── Signal infrastructure ───────────────────────────────────────────────

fn push_signal(
    signals: &mut Vec<(usize, u8, String)>,
    order: &mut usize,
    pri: u8,
    name: &str,
) {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return;
    }
    signals.push((*order, pri, trimmed.to_string()));
    *order += 1;
}

fn build_stem(mut signals: Vec<(usize, u8, String)>) -> Option<String> {
    if signals.is_empty() {
        return None;
    }

    // Dedup: keep the highest-priority (or earliest) instance of each token.
    let mut best_by_key: HashMap<String, (usize, u8, String)> = HashMap::new();
    for (ord, pri, token) in signals.drain(..) {
        let key = token.to_ascii_lowercase();
        match best_by_key.get(&key) {
            Some((best_ord, best_pri, _))
                if *best_pri > pri || (*best_pri == pri && *best_ord <= ord) => {}
            _ => {
                best_by_key.insert(key, (ord, pri, token));
            }
        }
    }

    let mut ranked: Vec<(usize, u8, String)> = best_by_key.into_values().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let mut used_words: Vec<String> = Vec::new();
    let mut parts: Vec<String> = Vec::new();

    for (_, _, token) in &ranked {
        if let Some(slug) = slugify(token) {
            let fresh: Vec<&str> = slug
                .split('-')
                .filter(|w| {
                    if used_words.iter().any(|uw| uw == w) {
                        return false;
                    }
                    let dominated = used_words.iter().any(|existing| {
                        let min_len = w.len().min(existing.len());
                        min_len >= 4
                            && (existing.starts_with(*w)
                                || w.starts_with(existing.as_str()))
                    });
                    if dominated {
                        return false;
                    }
                    used_words.push(w.to_string());
                    true
                })
                .collect();
            if !fresh.is_empty() {
                parts.push(fresh.join("-"));
            }
        }
        if parts.len() >= 2 {
            break;
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("-"))
    }
}

#[cfg(test)]
mod tests {
    use crate::suggest_stem;

    // ── SQL: Basic queries ─────────────────────────────────────────────────

    #[test]
    fn sql_simple_select_star() {
        let stem = suggest_stem("SELECT * FROM users;", "sql").unwrap();
        assert_eq!(stem, "users");
    }

    #[test]
    fn sql_select_specific_columns() {
        let stem = suggest_stem("SELECT id, name, email FROM users;", "sql").unwrap();
        assert_eq!(stem, "users");
    }

    #[test]
    fn sql_where_condition() {
        let stem = suggest_stem("SELECT * FROM orders WHERE status = 'completed';", "sql").unwrap();
        assert_eq!(stem, "orders-status");
    }

    #[test]
    fn sql_order_by() {
        let stem = suggest_stem("SELECT * FROM products ORDER BY price DESC;", "sql").unwrap();
        assert_eq!(stem, "products-price");
    }

    #[test]
    fn sql_limit() {
        let stem = suggest_stem("SELECT * FROM logs LIMIT 10;", "sql").unwrap();
        assert_eq!(stem, "logs");
    }

    // ── SQL: Joins & aggregation ────────────────────────────────────────────

    #[test]
    fn sql_join() {
        let sql = r#"
                SELECT u.name, o.order_id, o.amount
                FROM users u
                JOIN orders o ON u.id = o.user_id;
            "#;
        let stem = suggest_stem(sql, "sql").unwrap();
        assert_eq!(stem, "users-orders");
    }

    #[test]
    fn sql_aggregation_group_by() {
        let sql = r#"
                SELECT status, COUNT(*) AS total_orders
                FROM orders
                GROUP BY status;
            "#;
        let stem = suggest_stem(sql, "sql").unwrap();
        assert_eq!(stem, "orders-status");
    }

    #[test]
    fn sql_having_clause() {
        let sql = r#"
                SELECT user_id, SUM(amount) AS total_spent
                FROM orders
                GROUP BY user_id
                HAVING SUM(amount) > 1000;
            "#;
        let stem = suggest_stem(sql, "sql").unwrap();
        assert_eq!(stem, "orders-user-id");
    }

    #[test]
    fn sql_subquery() {
        let sql = r#"
                SELECT name
                FROM users
                WHERE id IN (
                    SELECT user_id
                    FROM orders
                    WHERE amount > 500
                );
            "#;
        let stem = suggest_stem(sql, "sql").unwrap();
        assert_eq!(stem, "users-id");
    }

    #[test]
    fn sql_window_function() {
        let sql = r#"
                SELECT
                    user_id,
                    order_id,
                    amount,
                    RANK() OVER (PARTITION BY user_id ORDER BY amount DESC) AS rank_within_user
                FROM orders;
            "#;
        let stem = suggest_stem(sql, "sql").unwrap();
        assert_eq!(stem, "orders");
    }

    // ── SQL: CTEs (simple to complex) ───────────────────────────────────────

    #[test]
    fn sql_cte_single() {
        let sql = r#"
                WITH monthly_sales AS (
                    SELECT
                        DATE_TRUNC('month', created_at) AS month,
                        SUM(amount) AS total_sales
                    FROM orders
                    GROUP BY month
                )
                SELECT *
                FROM monthly_sales
                ORDER BY month;
            "#;
        let stem = suggest_stem(sql, "sql").unwrap();
        assert_eq!(stem, "monthly-sales");
    }

    #[test]
    fn sql_multiple_joins() {
        let sql = r#"
                SELECT
                    u.name,
                    p.product_name,
                    o.amount,
                    c.category_name
                FROM orders o
                JOIN users u ON o.user_id = u.id
                JOIN products p ON o.product_id = p.id
                JOIN categories c ON p.category_id = c.id;
            "#;
        let stem = suggest_stem(sql, "sql").unwrap();
        assert_eq!(stem, "orders-users");
    }

    #[test]
    fn sql_complex_cte_churn_risk() {
        let sql = r#"
                WITH user_order_summary AS (
                    SELECT
                        u.id AS user_id, u.name,
                        COUNT(o.id) AS total_orders,
                        SUM(o.amount) AS total_spent,
                        MAX(o.created_at) AS last_order_date
                    FROM users u
                    LEFT JOIN orders o ON u.id = o.user_id
                    GROUP BY u.id, u.name
                ),
                ranked_users AS (
                    SELECT *,
                           NTILE(4) OVER (ORDER BY total_spent DESC) AS spending_quartile
                    FROM user_order_summary
                ),
                churn_risk AS (
                    SELECT *,
                           CASE
                               WHEN last_order_date < NOW() - INTERVAL '90 days' THEN 'HIGH'
                               WHEN last_order_date < NOW() - INTERVAL '30 days' THEN 'MEDIUM'
                               ELSE 'LOW'
                           END AS churn_risk_level
                    FROM ranked_users
                )
                SELECT *
                FROM churn_risk
                WHERE spending_quartile = 1
                ORDER BY total_spent DESC;
            "#;
        let stem = suggest_stem(sql, "sql").unwrap();
        // First CTE + last CTE, no duplicates.
        assert_eq!(stem, "user-order-summary-churn-risk");
    }

    #[test]
    fn sql_massive_cte_pipeline() {
        let sql = r#"
                WITH base_orders AS (
                    SELECT o.id, o.user_id, o.product_id, o.amount, o.created_at,
                           u.country, u.signup_date, p.category_id, p.price
                    FROM orders o
                    JOIN users u ON o.user_id = u.id
                    JOIN products p ON o.product_id = p.id
                ),
                enriched_orders AS (
                    SELECT *,
                        DATE_TRUNC('month', created_at) AS order_month,
                        CASE WHEN amount > 1000 THEN 'high_value'
                             WHEN amount > 500 THEN 'medium_value'
                             ELSE 'low_value' END AS order_segment
                    FROM base_orders
                ),
                category_stats AS (
                    SELECT category_id, COUNT(*) AS total_orders,
                           SUM(amount) AS total_revenue, AVG(amount) AS avg_order_value
                    FROM enriched_orders
                    GROUP BY category_id
                ),
                user_lifetime AS (
                    SELECT user_id, MIN(created_at) AS first_order,
                           MAX(created_at) AS last_order, SUM(amount) AS lifetime_value,
                           COUNT(*) AS order_count
                    FROM enriched_orders
                    GROUP BY user_id
                ),
                final_dataset AS (
                    SELECT eo.*, cs.total_revenue, cs.avg_order_value,
                           ul.lifetime_value, ul.order_count,
                           CASE WHEN ul.lifetime_value > 5000 THEN 'VIP'
                                WHEN ul.lifetime_value > 2000 THEN 'LOYAL'
                                ELSE 'REGULAR' END AS user_tier
                    FROM enriched_orders eo
                    LEFT JOIN category_stats cs ON eo.category_id = cs.category_id
                    LEFT JOIN user_lifetime ul ON eo.user_id = ul.user_id
                )
                SELECT user_id, country, order_month, user_tier,
                       COUNT(*) AS total_orders, SUM(amount) AS revenue, AVG(amount) AS avg_order
                FROM final_dataset
                GROUP BY user_id, country, order_month, user_tier
                ORDER BY revenue DESC
                LIMIT 100;
            "#;
        let stem = suggest_stem(sql, "sql").unwrap();
        // First CTE "base_orders" + last CTE "final_dataset".
        assert_eq!(stem, "base-orders-final-dataset");
    }

    #[test]
    fn sql_generic_cte_aliases_fall_back_to_inner_tables() {
        let sql = r#"
                WITH a AS (
                    SELECT u.id uid, u.name,
                           SUM(o.amount) amt, COUNT(*) cnt
                    FROM users u
                    JOIN orders o ON u.id = o.user_id
                    WHERE o.created_at > NOW() - INTERVAL '1 year'
                    GROUP BY u.id, u.name
                ),
                b AS (
                    SELECT uid,
                           CASE
                               WHEN amt > 10000 THEN 'VIP'
                               WHEN amt > 5000 THEN 'LOYAL'
                               ELSE 'REGULAR'
                           END tier
                    FROM a
                ),
                c AS (
                    SELECT o.user_id,
                           DATE_TRUNC('month', o.created_at) m,
                           SUM(o.amount) m_amt
                    FROM orders o
                    GROUP BY o.user_id, m
                )
                SELECT c.user_id, b.tier, c.m, SUM(c.m_amt) total
                FROM c
                JOIN b ON c.user_id = b.uid
                GROUP BY c.user_id, b.tier, c.m
                HAVING SUM(c.m_amt) > 100
                ORDER BY total DESC;
            "#;
        let stem = suggest_stem(sql, "sql").unwrap();
        assert_eq!(stem, "users-orders");
    }

    #[test]
    fn sql_step_ctes_use_inner_signal_instead_of_step_names() {
        let sql = r#"
                WITH events AS (
                    SELECT
                        user_id,
                        event_name,
                        event_time,
                        DATE_TRUNC('day', event_time) AS event_day
                    FROM user_events
                ),
                step1 AS (
                    SELECT DISTINCT user_id, event_day
                    FROM events
                    WHERE event_name = 'app_open'
                ),
                step2 AS (
                    SELECT DISTINCT e.user_id, e.event_day
                    FROM events e
                    JOIN step1 s1
                        ON e.user_id = s1.user_id
                       AND e.event_day = s1.event_day
                    WHERE e.event_name = 'view_product'
                ),
                step3 AS (
                    SELECT DISTINCT e.user_id, e.event_day
                    FROM events e
                    JOIN step2 s2
                        ON e.user_id = s2.user_id
                       AND e.event_day = s2.event_day
                    WHERE e.event_name = 'purchase'
                )
                SELECT
                    s1.event_day,
                    COUNT(DISTINCT s1.user_id) AS step1_users,
                    COUNT(DISTINCT s2.user_id) AS step2_users,
                    COUNT(DISTINCT s3.user_id) AS step3_users,
                    ROUND(
                        COUNT(DISTINCT s3.user_id) * 100.0 / NULLIF(COUNT(DISTINCT s1.user_id), 0),
                        2
                    ) AS conversion_rate
                FROM step1 s1
                LEFT JOIN step2 s2 ON s1.user_id = s2.user_id AND s1.event_day = s2.event_day
                LEFT JOIN step3 s3 ON s1.user_id = s3.user_id AND s1.event_day = s3.event_day
                GROUP BY s1.event_day
                ORDER BY s1.event_day;
            "#;
        let stem = suggest_stem(sql, "sql").unwrap();
        assert_eq!(stem, "user-events-day");
    }

    // ── SQL: Edge cases ─────────────────────────────────────────────────────

    #[test]
    fn sql_weird_formatting() {
        let stem = suggest_stem("select*from users where name like '%test%';", "sql").unwrap();
        assert_eq!(stem, "users-name");
    }

    #[test]
    fn sql_reserved_keywords_quoted() {
        let stem = suggest_stem(r#"SELECT "order", "group" FROM "transaction";"#, "sql").unwrap();
        assert_eq!(stem, "transaction");
    }

    #[test]
    fn sql_json_postgres_operators() {
        let sql = r#"
                SELECT
                    data->>'name' AS name,
                    data->'items' AS items
                FROM events;
            "#;
        let stem = suggest_stem(sql, "sql").unwrap();
        assert_eq!(stem, "events");
    }

    #[test]
    fn sql_case_sensitive_table() {
        let stem = suggest_stem(r#"SELECT * FROM "Users";"#, "sql").unwrap();
        assert_eq!(stem, "users");
    }

    // ── SQL: DDL & DML ──────────────────────────────────────────────────────

    #[test]
    fn sql_create_table() {
        let sql = "CREATE TABLE users (id INT, name VARCHAR(100));";
        let stem = suggest_stem(sql, "sql").unwrap();
        assert_eq!(stem, "users");
    }

    #[test]
    fn sql_create_view() {
        let sql = "CREATE OR REPLACE VIEW active_users AS SELECT * FROM users WHERE active = 1;";
        let stem = suggest_stem(sql, "sql").unwrap();
        assert!(
            stem.contains("active") || stem.contains("users"),
            "got: {stem}"
        );
    }

    #[test]
    fn sql_insert() {
        let sql = "INSERT INTO orders (user_id, total) VALUES (1, 99.99);";
        let stem = suggest_stem(sql, "sql").unwrap();
        assert_eq!(stem, "orders");
    }

    #[test]
    fn sql_update() {
        let sql = "UPDATE products SET price = 19.99 WHERE id = 42;";
        let stem = suggest_stem(sql, "sql").unwrap();
        assert_eq!(stem, "products-id");
    }

    // ── SQL: The original CTE dedup test ────────────────────────────────────

    #[test]
    fn sql_cte_no_duplicate_words() {
        // CTE "JobHistorySummary" + FROM "employees"; slug must not repeat words.
        let sql = r#"
                WITH JobHistorySummary AS (
                    SELECT employee_id, COUNT(*) AS total_jobs FROM job_history GROUP BY employee_id
                )
                SELECT e.employee_id, e.first_name
                FROM employees e
                JOIN jobs j ON e.job_id = j.job_id
                LEFT JOIN JobHistorySummary jhs ON e.employee_id = jhs.employee_id
                ORDER BY total_jobs DESC;
            "#;
        let stem = suggest_stem(sql, "sql").unwrap();
        assert!(
            stem.contains("job") || stem.contains("history"),
            "got: {stem}"
        );
        assert!(stem.contains("employee"), "got: {stem}");
        // No word should appear twice.
        let words: Vec<&str> = stem.split('-').collect();
        let unique: std::collections::HashSet<&str> = words.iter().copied().collect();
        assert_eq!(words.len(), unique.len(), "duplicate word in slug: {stem}");
    }

    #[test]
    fn sql_select_from_where_active() {
        let sql = "SELECT id, name FROM products WHERE active = 1;";
        let stem = suggest_stem(sql, "sql").unwrap();
        assert!(stem.contains("products"), "got: {stem}");
        assert!(stem.contains("active"), "got: {stem}");
    }

    // ── SQL: Verify no word ever duplicates ─────────────────────────────────

    /// Helper: asserts no word appears twice in a hyphenated slug.
    fn assert_no_dup_words(stem: &str) {
        let words: Vec<&str> = stem.split('-').collect();
        let unique: std::collections::HashSet<&str> = words.iter().copied().collect();
        assert_eq!(words.len(), unique.len(), "duplicate word in slug: {stem}");
    }

    #[test]
    fn sql_all_stems_have_no_duplicate_words() {
        let queries = vec![
            "SELECT * FROM users;",
            "SELECT * FROM orders WHERE status = 'completed';",
            "SELECT * FROM products ORDER BY price DESC;",
            "SELECT status, COUNT(*) AS total FROM orders GROUP BY status;",
            "SELECT u.name FROM users u JOIN orders o ON u.id = o.user_id;",
            "INSERT INTO orders (user_id, total) VALUES (1, 99.99);",
            "CREATE TABLE users (id INT, name VARCHAR(100));",
        ];
        for sql in queries {
            if let Some(stem) = suggest_stem(sql, "sql") {
                assert_no_dup_words(&stem);
            }
        }
    }
}
