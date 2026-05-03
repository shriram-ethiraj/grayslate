/// Neighbor disambiguation (Phase 3 of pipeline).
///
/// When multiple candidates survive family-gated scoring, this module
/// picks a winner using score gap, superset relationships, and
/// declared rival context.
use super::scoring::ScoredCandidate;

/// Known superset relationships: the superset language absorbs all
/// syntax of its base language. When both score above threshold and
/// the superset's score is ≥ 60% of the base, the superset wins.
const SUPERSET_PAIRS: &[(&str, &str)] = &[
    ("typescript", "javascript"),
    ("angular", "typescript"),
    ("angular", "javascript"),
    ("cpp", "c"),
    ("kotlin", "java"),
];

/// Disambiguate between the top candidates.
///
/// Returns the winning language name, or None if the candidates are
/// too close to call (abstention).
pub fn disambiguate(
    _content: &str,
    candidates: &[ScoredCandidate],
) -> Option<&'static str> {
    if candidates.is_empty() {
        if cfg!(debug_assertions) {
            eprintln!("[Lang Detect] [Phase 3] Disambiguation: No candidates to disambiguate");
        }
        return None;
    }

    if candidates.len() == 1 {
        if cfg!(debug_assertions) {
            eprintln!("[Lang Detect] [Phase 3] Disambiguation: Only 1 candidate \"{}\" — picking it", candidates[0].name);
        }
        return Some(candidates[0].name);
    }

    let first = &candidates[0];
    let second = &candidates[1];

    if cfg!(debug_assertions) {
        eprintln!(
            "[Lang Detect]   [Phase 3] Top: \"{}\" (score={}) vs Runner-up: \"{}\" (score={}) | Gap = {}",
            first.name, first.total_score,
            second.name, second.total_score,
            first.total_score - second.total_score,
        );
    }

    // Superset check FIRST — must run before the score-gap shortcut.
    if let Some(winner) = resolve_superset(first, second) {
        if cfg!(debug_assertions) {
            eprintln!("[Lang Detect]   [Phase 3] \"{}\" is a superset of the other language — \"{}\" wins ✓", winner, winner);
        }
        return Some(winner);
    }

    // If the first candidate has a significant score lead, no need to disambiguate
    if first.total_score > second.total_score + 3 {
        if cfg!(debug_assertions) {
            eprintln!("[Lang Detect]   [Phase 3] Large score gap (>3 points) — \"{}\" wins ✓", first.name);
        }
        return Some(first.name);
    }

    // Fall back to score-based winner
    if first.total_score > second.total_score {
        if cfg!(debug_assertions) {
            eprintln!("[Lang Detect]   [Phase 3] Narrow score lead — \"{}\" wins ✓", first.name);
        }
        return Some(first.name);
    }

    // Truly tied — abstain
    if cfg!(debug_assertions) {
        eprintln!("[Lang Detect]   [Phase 3] Scores tied — cannot decide, abstaining");
    }
    None
}

/// If the two candidates form a superset/base pair, return the superset
/// when it has anchor evidence (superset-specific syntax). Since a
/// superset language shares ALL syntax with its base, the base will
/// always score well on superset content — so anchor_score > 0 is
/// sufficient evidence to prefer the superset.
fn resolve_superset(
    first: &ScoredCandidate,
    second: &ScoredCandidate,
) -> Option<&'static str> {
    for &(superset, base) in SUPERSET_PAIRS {
        if first.name == superset && second.name == base {
            if first.anchor_score > 0 {
                return Some(first.name);
            }
        }
        if first.name == base && second.name == superset {
            if second.anchor_score > 0 {
                return Some(second.name);
            }
        }
    }
    None
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
            keyword_score: 0,
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
                keyword_score: 0,
                total_score: 13,
            },
            ScoredCandidate {
                name: "ruby",
                anchor_score: 4,
                hint_score: 2,
                keyword_score: 0,
                total_score: 6,
            },
        ];
        assert_eq!(disambiguate("", &candidates), Some("python"));
    }
}
