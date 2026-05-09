use std::cmp::Ordering;

use super::{
    query::ParsedSearchQuery,
    types::{
        find_whole_word, get_line_excerpt, split_text_by_terms, FileSearchCandidate, MatchedLine,
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

    let freshness_score = score_freshness(candidate.file_modified_disk_at);
    let usage_score = score_usage(candidate.file_modified_app_at);
    let final_score = filename_score * 1.6
        + content_score * 1.0
        + freshness_score * 0.15
        + usage_score * 0.1;

    Some(SearchResultRecord {
        path: candidate.path.clone(),
        file_name: candidate.file_name.clone(),
        extension: candidate.extension.clone(),
        source: candidate.source.clone(),
        size_bytes: candidate.size_bytes,
        file_modified_app_at: candidate.file_modified_app_at,
        file_modified_disk_at: candidate.file_modified_disk_at,
        matched_lines: candidate
            .content
            .previews
            .iter()
            .filter_map(|preview| {
                preview.line_number.map(|ln| {
                    let excerpt = get_line_excerpt(
                        &preview.line_text,
                        &context.query.terms,
                        &context.query.options,
                        context.query.is_glob,
                    );
                    MatchedLine {
                        line_number: ln,
                        fragments: split_text_by_terms(
                            &excerpt,
                            &context.query.terms,
                            &context.query.options,
                            context.query.is_glob,
                        ),
                    }
                })
            })
            .collect(),
        match_count: candidate.content.total_hits,
        filename_fragments: split_text_by_terms(
            &candidate.file_name,
            &context.query.terms,
            &context.query.options,
            context.query.is_glob,
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
    let cs = query.options.case_sensitive;

    // Auto-detected glob: score by glob pattern matching against filename/path.
    if query.is_glob {
        return score_filename_glob(candidate, query);
    }

    // Explicit regex toggle: score by regex matching against filename/path.
    if query.options.use_regex {
        return score_filename_regex(candidate, query);
    }

    let normalized_name = if cs {
        candidate.file_name.clone()
    } else {
        candidate.file_name.to_lowercase()
    };
    let normalized_path = if cs {
        candidate.path.clone()
    } else {
        candidate.path.to_lowercase()
    };
    let mut score = 0.0;
    let ww = query.options.whole_word;

    // Helper closures for whole-word–aware substring operations.
    let name_find = |needle: &str| -> Option<usize> {
        if ww { find_whole_word(&normalized_name, needle) } else { normalized_name.find(needle) }
    };
    let path_find = |needle: &str| -> Option<usize> {
        if ww { find_whole_word(&normalized_path, needle) } else { normalized_path.find(needle) }
    };

    if normalized_name == query.normalized {
        score += 8.0;
    }

    let stem = candidate
        .file_name
        .split('.')
        .next()
        .unwrap_or(&candidate.file_name);
    let normalized_stem = if cs {
        stem.to_string()
    } else {
        stem.to_lowercase()
    };
    if normalized_stem == query.normalized {
        score += 7.0;
    }

    // starts_with with whole-word awareness: must match at position 0 AND respect boundary.
    let starts_match = if ww {
        name_find(&query.normalized).map_or(false, |i| i == 0)
    } else {
        normalized_name.starts_with(&query.normalized)
    };
    if starts_match {
        score += 5.0;
    }

    if let Some(index) = name_find(&query.normalized) {
        score += 4.0 + (1.0 / (index as f32 + 1.0));
    }

    for term in &query.terms {
        if let Some(index) = name_find(term.as_str()) {
            score += 2.4 + (0.5 / (index as f32 + 1.0));
        } else if let Some(index) = path_find(term.as_str()) {
            score += 1.4 + (0.25 / (index as f32 + 1.0));
        }
    }

    if query
        .terms
        .iter()
        .all(|term| name_find(term.as_str()).is_some())
    {
        score += 1.5;
    }

    score
}

/// Glob-mode filename scoring using `globset`, which supports all standard
/// glob syntax: `*`, `**`, `?`, `[abc]` character classes, `{a,b}` brace
/// expansion.  Returns 0.0 for unparseable patterns (no error banner).
fn score_filename_glob(candidate: &FileSearchCandidate, query: &ParsedSearchQuery) -> f32 {
    let glob = match globset::GlobBuilder::new(&query.raw)
        .case_insensitive(!query.options.case_sensitive)
        .literal_separator(false)
        .build()
        .and_then(|g| {
            let mut builder = globset::GlobSetBuilder::new();
            builder.add(g);
            builder.build()
        }) {
        Ok(gs) => gs,
        Err(_) => return 0.0,
    };

    let mut score = 0.0;
    if glob.is_match(&candidate.file_name) {
        score += 4.0;
    }
    if glob.is_match(&candidate.path) {
        score += 2.0;
    }
    score
}

/// Regex-mode filename scoring: build a regex from the raw query and test it
/// against the filename and full path.
fn score_filename_regex(candidate: &FileSearchCandidate, query: &ParsedSearchQuery) -> f32 {
    let pattern = if query.options.whole_word {
        format!(r"\b(?:{})\b", &query.raw)
    } else {
        query.raw.clone()
    };

    let re = match regex::RegexBuilder::new(&pattern)
        .case_insensitive(!query.options.case_sensitive)
        .build()
    {
        Ok(r) => r,
        Err(_) => return 0.0,
    };

    let mut score = 0.0;
    if re.is_match(&candidate.file_name) {
        score += 4.0;
    }
    if re.is_match(&candidate.path) {
        score += 2.0;
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

fn score_freshness(file_modified_disk_at: Option<i64>) -> f32 {
    let Some(file_modified_disk_at) = file_modified_disk_at else {
        return 0.0;
    };

    let age_ms = (crate::search::types::current_time_ms() - file_modified_disk_at).max(0) as f32;
    let age_days = age_ms / 86_400_000.0;
    1.0 / (1.0 + (age_days / 7.0))
}

fn score_usage(file_modified_app_at: Option<i64>) -> f32 {
    let Some(recency) = file_modified_app_at else {
        return 0.0;
    };

    let age_ms = (crate::search::types::current_time_ms() - recency).max(0) as f32;
    let age_days = age_ms / 86_400_000.0;
    1.0 / (1.0 + age_days)
}
