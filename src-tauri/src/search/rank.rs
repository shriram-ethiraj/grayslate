use std::cmp::Ordering;

use super::{
    query::ParsedSearchQuery,
    types::{
        get_line_excerpt, split_text_by_terms, FileSearchCandidate, MatchedLine,
        SearchResultRecord,
    },
};

const BM25_K1: f32 = 1.2;
const BM25_B: f32 = 0.75;

pub struct RankContext<'a> {
    pub query: &'a ParsedSearchQuery,
    pub average_document_length: f32,
    pub document_count: usize,
}

pub fn resolve_average_document_length(
    cached_value: Option<f32>,
    lengths: impl Iterator<Item = f32>,
) -> f32 {
    let observed = lengths.filter(|length| *length > 0.0).collect::<Vec<_>>();
    if !observed.is_empty() {
        return observed.iter().sum::<f32>() / observed.len() as f32;
    }

    cached_value.unwrap_or(1.0)
}

pub fn rank_candidate(
    candidate: &mut FileSearchCandidate,
    context: &RankContext<'_>,
) -> Option<SearchResultRecord> {
    let filename_score = score_filename(candidate, context.query);
    let content_score = score_content(candidate, context);
    if filename_score <= 0.0 && content_score <= 0.0 {
        return None;
    }

    let freshness_score = score_freshness(candidate.last_modified_at);
    let usage_score = score_usage(
        candidate.last_opened_at,
        candidate.last_saved_at,
        candidate.last_seen_at,
    );
    let pinned_boost = if candidate.pinned { 0.35 } else { 0.0 };
    let final_score = filename_score * 1.6
        + content_score * 1.0
        + freshness_score * 0.15
        + usage_score * 0.1
        + pinned_boost;

    Some(SearchResultRecord {
        path: candidate.path.clone(),
        file_name: candidate.file_name.clone(),
        extension: candidate.extension.clone(),
        source: candidate.source.clone(),
        exists_on_disk: candidate.exists_on_disk,
        size_bytes: candidate.size_bytes,
        last_opened_at: candidate.last_opened_at,
        last_saved_at: candidate.last_saved_at,
        last_seen_at: candidate.last_seen_at,
        last_modified_at: candidate.last_modified_at,
        pinned: candidate.pinned,
        matched_lines: candidate
            .content
            .previews
            .iter()
            .filter_map(|preview| {
                preview.line_number.map(|ln| {
                    let excerpt = get_line_excerpt(&preview.line_text, &context.query.terms);
                    MatchedLine {
                        line_number: ln,
                        fragments: split_text_by_terms(&excerpt, &context.query.terms),
                    }
                })
            })
            .collect(),
        match_count: candidate.content.total_hits,
        filename_fragments: split_text_by_terms(
            &candidate.file_name,
            &context.query.terms,
        ),
        filename_score,
        content_score,
        freshness_score,
        usage_score,
        final_score,
    })
}

/// Sorts results by score descending. The frontend applies the user's
/// chosen sort_mode as a secondary tiebreaker, so backend ordering only
/// needs to be stable by score.
pub fn sort_search_results(results: &mut [SearchResultRecord]) {
    results.sort_by(|left, right| {
        right
            .final_score
            .partial_cmp(&left.final_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                left.file_name
                    .to_lowercase()
                    .cmp(&right.file_name.to_lowercase())
            })
            .then_with(|| left.path.to_lowercase().cmp(&right.path.to_lowercase()))
    });
}

fn score_filename(candidate: &FileSearchCandidate, query: &ParsedSearchQuery) -> f32 {
    let normalized_name = candidate.file_name.to_lowercase();
    let normalized_path = candidate.path.to_lowercase();
    let mut score = 0.0;

    if normalized_name == query.normalized {
        score += 8.0;
    }

    let stem = candidate
        .file_name
        .split('.')
        .next()
        .unwrap_or(&candidate.file_name)
        .to_lowercase();
    if stem == query.normalized {
        score += 7.0;
    }

    if normalized_name.starts_with(&query.normalized) {
        score += 5.0;
    }

    if let Some(index) = normalized_name.find(&query.normalized) {
        score += 4.0 + (1.0 / (index as f32 + 1.0));
    }

    for term in &query.terms {
        if let Some(index) = normalized_name.find(term) {
            score += 2.4 + (0.5 / (index as f32 + 1.0));
        } else if let Some(index) = normalized_path.find(term) {
            score += 1.4 + (0.25 / (index as f32 + 1.0));
        }
    }

    if query
        .terms
        .iter()
        .all(|term| normalized_name.contains(term))
    {
        score += 1.5;
    }

    score
}

fn score_content(candidate: &FileSearchCandidate, context: &RankContext<'_>) -> f32 {
    if context.document_count == 0 {
        return 0.0;
    }

    context
        .query
        .terms
        .iter()
        .map(|term| {
            let tf = *candidate.content.term_frequencies.get(term).unwrap_or(&0) as f32;
            if tf <= 0.0 {
                return 0.0;
            }

            let df = candidate
                .content
                .document_frequencies
                .get(term)
                .copied()
                .unwrap_or(1) as f32;
            let idf = ((context.document_count as f32 - df + 0.5) / (df + 0.5) + 1.0).ln();
            let normalization = tf
                + BM25_K1
                    * (1.0 - BM25_B
                        + BM25_B
                            * (candidate.document_length
                                / context.average_document_length.max(1.0)));

            idf * ((tf * (BM25_K1 + 1.0)) / normalization.max(1.0))
        })
        .sum::<f32>()
}

fn score_freshness(last_modified_at: Option<i64>) -> f32 {
    let Some(last_modified_at) = last_modified_at else {
        return 0.0;
    };

    let age_ms = (crate::search::types::current_time_ms() - last_modified_at).max(0) as f32;
    let age_days = age_ms / 86_400_000.0;
    1.0 / (1.0 + (age_days / 7.0))
}

fn score_usage(
    last_opened_at: Option<i64>,
    last_saved_at: Option<i64>,
    last_seen_at: Option<i64>,
) -> f32 {
    let recency = [last_opened_at, last_saved_at, last_seen_at]
        .into_iter()
        .flatten()
        .max();
    let Some(recency) = recency else {
        return 0.0;
    };

    let age_ms = (crate::search::types::current_time_ms() - recency).max(0) as f32;
    let age_days = age_ms / 86_400_000.0;
    1.0 / (1.0 + age_days)
}
