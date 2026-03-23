/// Neighbor disambiguation (Phase 3 of new pipeline).
///
/// When multiple candidates survive family-gated scoring, this module
/// compares them head-to-head using declared rival relationships and
/// differentiator patterns. Tree-sitter validation is integrated here
/// for languages with available grammars.
use super::languages::COMPILED_FAMILY;
use super::scoring::ScoredCandidate;

/// Disambiguate between the top candidates using rival relationships.
///
/// Returns the winning language name, or None if the candidates are
/// too close to call (abstention).
pub fn disambiguate(
    content: &str,
    candidates: &[ScoredCandidate],
) -> Option<&'static str> {
    if candidates.is_empty() {
        return None;
    }

    if candidates.len() == 1 {
        return Some(candidates[0].name);
    }

    let first = &candidates[0];
    let second = &candidates[1];

    // If the first candidate has a significant score lead, no need to disambiguate
    if first.total_score > second.total_score + 3 {
        return Some(first.name);
    }

    // Check if they are declared rivals
    let first_lang = COMPILED_FAMILY.iter().find(|l| l.name == first.name);
    let second_lang = COMPILED_FAMILY.iter().find(|l| l.name == second.name);

    if let (Some(fl), Some(sl)) = (first_lang, second_lang) {
        let first_declares_rival = fl.rivals.contains(&second.name);
        let second_declares_rival = sl.rivals.contains(&first.name);

        if first_declares_rival || second_declares_rival {
            // Score differentiators for each
            let first_diff_score: i32 = fl
                .differentiators
                .iter()
                .filter(|p| p.regex.is_match(content))
                .map(|p| p.weight)
                .sum();

            let second_diff_score: i32 = sl
                .differentiators
                .iter()
                .filter(|p| p.regex.is_match(content))
                .map(|p| p.weight)
                .sum();

            // Clear winner from differentiators
            if first_diff_score > second_diff_score + 2 {
                return Some(first.name);
            }
            if second_diff_score > first_diff_score + 2 {
                return Some(second.name);
            }
        }
    }

    // Try tree-sitter validation as a tiebreaker
    let first_errors = tree_sitter_error_ratio(content, first.name);
    let second_errors = tree_sitter_error_ratio(content, second.name);

    if let (Some(fe), Some(se)) = (first_errors, second_errors) {
        if fe < se - 0.05 {
            return Some(first.name);
        }
        if se < fe - 0.05 {
            return Some(second.name);
        }
    }

    // Fall back to score-based winner if still tied
    if first.total_score > second.total_score {
        return Some(first.name);
    }

    // Truly tied — abstain
    None
}

/// Get tree-sitter error ratio for a language (0.0 = perfect parse, 1.0 = all errors).
/// Returns None if no tree-sitter grammar is available for this language.
fn tree_sitter_error_ratio(content: &str, language: &str) -> Option<f64> {
    let ts_lang = match language {
        "javascript" => Some(tree_sitter_javascript::LANGUAGE),
        "typescript" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT),
        "python" => Some(tree_sitter_python::LANGUAGE),
        "rust" => Some(tree_sitter_rust::LANGUAGE),
        "java" => Some(tree_sitter_java::LANGUAGE),
        "go" => Some(tree_sitter_go::LANGUAGE),
        "c" => Some(tree_sitter_c::LANGUAGE),
        "cpp" => Some(tree_sitter_cpp::LANGUAGE),
        _ => None,
    }?;

    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&ts_lang.into())
        .ok()?;

    let tree = parser.parse(content, None)?;
    let root = tree.root_node();
    let total = root.descendant_count();
    if total == 0 {
        return Some(0.0);
    }

    let errors = count_errors(root);
    Some(errors as f64 / total as f64)
}

/// Count ERROR and MISSING nodes in a tree-sitter tree.
fn count_errors(node: tree_sitter::Node) -> usize {
    let mut count = if node.is_error() || node.is_missing() {
        1
    } else {
        0
    };
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        count += count_errors(child);
    }
    count
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_candidate_wins() {
        let candidates = vec![ScoredCandidate {
            name: "python",
            anchor_score: 8,
            hint_score: 3,
            total_score: 11,
        }];
        assert_eq!(disambiguate("", &candidates), Some("python"));
    }

    #[test]
    fn empty_candidates_returns_none() {
        assert_eq!(disambiguate("", &[]), None);
    }

    #[test]
    fn clear_score_lead_wins() {
        let candidates = vec![
            ScoredCandidate {
                name: "python",
                anchor_score: 10,
                hint_score: 3,
                total_score: 13,
            },
            ScoredCandidate {
                name: "ruby",
                anchor_score: 4,
                hint_score: 2,
                total_score: 6,
            },
        ];
        assert_eq!(disambiguate("", &candidates), Some("python"));
    }
}
