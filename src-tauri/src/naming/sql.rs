use super::{prose::extract_yake, shared::slugify};

/// SQL: AST-based extraction via sqlparser-rs.
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
    use sqlparser::ast::{
        Expr, GroupByExpr, OrderByKind, SetExpr, Statement, TableFactor, TableObject,
    };
    use sqlparser::dialect::GenericDialect;
    use sqlparser::parser::Parser;
    use std::collections::{HashMap, HashSet};

    /// Extract a column name from a bare or compound identifier expression.
    fn expr_col_name(expr: &Expr) -> Option<String> {
        use sqlparser::ast::Expr;
        match expr {
            Expr::Identifier(ident) => Some(ident.value.clone()),
            Expr::CompoundIdentifier(parts) => parts.last().map(|i| i.value.clone()),
            _ => None,
        }
    }

    /// Walk a WHERE expression tree to collect left-hand column names from
    /// comparison operators. Depth-limited to avoid runaway recursion.
    fn collect_where_columns(expr: &Expr, out: &mut Vec<String>, depth: u8) {
        use sqlparser::ast::{BinaryOperator, Expr};
        if depth > 4 || out.len() >= 3 {
            return;
        }
        match expr {
            Expr::BinaryOp { left, right, op } => match op {
                BinaryOperator::And | BinaryOperator::Or => {
                    collect_where_columns(left, out, depth + 1);
                    collect_where_columns(right, out, depth + 1);
                }
                BinaryOperator::Eq
                | BinaryOperator::NotEq
                | BinaryOperator::Lt
                | BinaryOperator::LtEq
                | BinaryOperator::Gt
                | BinaryOperator::GtEq => {
                    if let Some(name) = expr_col_name(left) {
                        out.push(name);
                    }
                }
                _ => {
                    // For other operators, still try the left side.
                    if let Some(name) = expr_col_name(left) {
                        out.push(name);
                    }
                }
            },
            Expr::Like { expr: e, .. } | Expr::ILike { expr: e, .. } => {
                if let Some(name) = expr_col_name(e) {
                    out.push(name);
                }
            }
            Expr::InSubquery { expr: e, .. } | Expr::InList { expr: e, .. } => {
                if let Some(name) = expr_col_name(e) {
                    out.push(name);
                }
            }
            Expr::Between { expr: e, .. } => {
                if let Some(name) = expr_col_name(e) {
                    out.push(name);
                }
            }
            Expr::Nested(inner) => collect_where_columns(inner, out, depth),
            _ => {}
        }
    }

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

    fn push_signal(signals: &mut Vec<(usize, u8, String)>, order: &mut usize, pri: u8, name: &str) {
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

        let mut best_by_key: HashMap<String, (usize, u8, String)> = HashMap::new();
        for (order, pri, token) in signals.drain(..) {
            let key = token.to_ascii_lowercase();
            match best_by_key.get(&key) {
                Some((best_order, best_pri, _))
                    if *best_pri > pri || (*best_pri == pri && *best_order <= order) => {}
                _ => {
                    best_by_key.insert(key, (order, pri, token));
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
                                && (existing.starts_with(*w) || w.starts_with(existing.as_str()))
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

    fn collect_query_signals(
        query: &sqlparser::ast::Query,
        signals: &mut Vec<(usize, u8, String)>,
        order: &mut usize,
        inherited_ctes: &HashSet<String>,
    ) {
        fn relation_name(table: &TableFactor) -> Option<String> {
            match table {
                TableFactor::Table { name, .. } => name
                    .0
                    .last()
                    .and_then(|p| p.as_ident())
                    .map(|i| i.value.clone()),
                _ => None,
            }
        }

        fn describe_query(
            query: &sqlparser::ast::Query,
            inherited_ctes: &HashSet<String>,
        ) -> Option<String> {
            let mut nested_signals: Vec<(usize, u8, String)> = Vec::new();
            let mut nested_order = 0usize;
            collect_query_signals(
                query,
                &mut nested_signals,
                &mut nested_order,
                inherited_ctes,
            );
            build_stem(nested_signals)
        }

        let mut scoped_ctes = inherited_ctes.clone();

        if let Some(with) = &query.with {
            for cte in &with.cte_tables {
                scoped_ctes.insert(cte.alias.name.value.to_ascii_lowercase());
            }

            for (index, cte) in with.cte_tables.iter().enumerate() {
                let alias = &cte.alias.name.value;
                let descriptor = describe_query(&cte.query, &scoped_ctes);
                let alias_is_generic = is_generic_cte_name(alias);
                let alias_is_noise = descriptor
                    .as_deref()
                    .is_some_and(|value| alias_is_redundant(alias, value));
                let priority = if index == 0 {
                    10
                } else if index + 1 == with.cte_tables.len() {
                    9
                } else {
                    0
                };
                let keep_alias = !alias_is_generic && !alias_is_noise && priority > 0;

                if keep_alias {
                    push_signal(signals, order, priority, alias);
                }

                if !keep_alias {
                    collect_query_signals(&cte.query, signals, order, &scoped_ctes);
                }
            }
        }

        if let SetExpr::Select(sel) = query.body.as_ref() {
            for from in &sel.from {
                if let Some(name) = relation_name(&from.relation) {
                    if !scoped_ctes.contains(&name.to_ascii_lowercase()) {
                        push_signal(signals, order, 8, &name);
                    }
                }

                for join in &from.joins {
                    if let Some(name) = relation_name(&join.relation) {
                        if !scoped_ctes.contains(&name.to_ascii_lowercase()) {
                            push_signal(signals, order, 4, &name);
                        }
                    }
                }
            }

            if let GroupByExpr::Expressions(exprs, _) = &sel.group_by {
                for gb_expr in exprs {
                    if let Some(name) = expr_col_name(gb_expr) {
                        push_signal(signals, order, 7, &name);
                    }
                }
            }

            if let Some(selection) = &sel.selection {
                let mut where_cols: Vec<String> = Vec::new();
                collect_where_columns(selection, &mut where_cols, 0);
                for col in where_cols.into_iter().take(2) {
                    push_signal(signals, order, 5, &col);
                }
            }
        }

        if let Some(order_by) = &query.order_by {
            if let OrderByKind::Expressions(exprs) = &order_by.kind {
                for ob in exprs {
                    if let Some(name) = expr_col_name(&ob.expr) {
                        push_signal(signals, order, 6, &name);
                    }
                }
            }
        }
    }

    let ast = match Parser::parse_sql(&GenericDialect {}, content) {
        Ok(a) => a,
        // Fall back to YAKE for unparseable SQL (dialect quirks, partial queries).
        Err(_) => return extract_yake(content),
    };

    let mut signals: Vec<(usize, u8, String)> = Vec::new();
    let mut order = 0usize;

    for stmt in &ast {
        match stmt {
            Statement::Query(q) => {
                collect_query_signals(q, &mut signals, &mut order, &HashSet::new());
            }

            Statement::CreateTable(ct) => {
                if let Some(v) = ct
                    .name
                    .0
                    .last()
                    .and_then(|p| p.as_ident())
                    .map(|i| &i.value)
                {
                    push_signal(&mut signals, &mut order, 10, v);
                }
            }

            Statement::CreateView { name, .. } => {
                if let Some(v) = name.0.last().and_then(|p| p.as_ident()).map(|i| &i.value) {
                    push_signal(&mut signals, &mut order, 10, v);
                }
            }

            Statement::Insert(insert) => {
                if let TableObject::TableName(name) = &insert.table {
                    if let Some(v) = name.0.last().and_then(|p| p.as_ident()).map(|i| &i.value) {
                        push_signal(&mut signals, &mut order, 8, v);
                    }
                }
            }

            Statement::Update {
                table, selection, ..
            } => {
                if let TableFactor::Table { name, .. } = &table.relation {
                    if let Some(v) = name.0.last().and_then(|p| p.as_ident()).map(|i| &i.value) {
                        push_signal(&mut signals, &mut order, 8, v);
                    }
                }
                if let Some(sel) = selection {
                    let mut where_cols: Vec<String> = Vec::new();
                    collect_where_columns(sel, &mut where_cols, 0);
                    for col in where_cols.into_iter().take(2) {
                        push_signal(&mut signals, &mut order, 5, &col);
                    }
                }
            }

            _ => {}
        }
    }

    if signals.is_empty() {
        return extract_yake(content);
    }

    build_stem(signals)
}

#[cfg(test)]
mod tests {
    use crate::naming::suggest_stem;

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
