/// Search modifier flags that travel with every sidebar search request.
#[derive(Clone, Debug, Default)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub use_regex: bool,
}

#[derive(Clone, Debug)]
pub struct ParsedSearchQuery {
    pub raw: String,
    pub normalized: String,
    pub terms: Vec<String>,
    pub options: SearchOptions,
    /// Auto-detected: true when the raw query contains glob wildcards (`*`, `?`)
    /// and regex mode is off.  Glob queries match filenames/paths only — content
    /// search is skipped.
    pub is_glob: bool,
}

/// Returns `true` when `query` looks like a glob pattern.
/// Detects `*`, `?`, `[...]` character classes, and `{a,b}` brace expansion.
fn looks_like_glob(query: &str) -> bool {
    query.contains('*')
        || query.contains('?')
        || (query.contains('[') && query.contains(']'))
        || (query.contains('{') && query.contains('}'))
}

/// Parses a raw search string into a `ParsedSearchQuery`.
///
/// **Glob auto-detection** (when regex mode is off): if the trimmed query
/// contains `*` or `?` it is treated as a glob pattern — the full query
/// becomes a single term and content search is skipped by the caller.
///
/// In **regex mode** the entire trimmed query becomes a single term so it is
/// handed to the regex engine as-is (no whitespace splitting).
///
/// In **literal mode** the query is split by whitespace into independent
/// search terms (existing behaviour).
///
/// Case normalisation is controlled by `options.case_sensitive`: when off
/// (the default) both `normalized` and `terms` are lowercased.
pub fn parse_query(raw: &str, options: SearchOptions) -> Result<ParsedSearchQuery, String> {
    let trimmed = raw.trim();
    let is_glob = !options.use_regex && looks_like_glob(trimmed);

    if is_glob {
        // Glob mode: single-term, no whitespace splitting.
        // Keep the *original* casing — the glob engine handles case via MatchOptions.
        let normalized = if options.case_sensitive {
            trimmed.to_string()
        } else {
            trimmed.to_lowercase()
        };
        let terms = if trimmed.is_empty() {
            Vec::new()
        } else {
            vec![trimmed.to_string()]
        };
        Ok(ParsedSearchQuery {
            raw: trimmed.to_string(),
            normalized,
            terms,
            options,
            is_glob,
        })
    } else if options.use_regex {
        // Regex mode: single-term, no whitespace splitting.
        let normalized = if options.case_sensitive {
            trimmed.to_string()
        } else {
            trimmed.to_lowercase()
        };
        let terms = if trimmed.is_empty() {
            Vec::new()
        } else {
            // Keep the *original* casing in the term — the regex engine will
            // handle case insensitivity via its own flag.
            vec![trimmed.to_string()]
        };
        Ok(ParsedSearchQuery {
            raw: trimmed.to_string(),
            normalized,
            terms,
            options,
            is_glob,
        })
    } else {
        // Literal mode: split by whitespace.
        let normalized = if options.case_sensitive {
            trimmed.to_string()
        } else {
            trimmed.to_lowercase()
        };
        let source = if options.case_sensitive {
            trimmed
        } else {
            &normalized
        };
        let terms = source
            .split_whitespace()
            .map(str::trim)
            .filter(|term| !term.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();

        Ok(ParsedSearchQuery {
            raw: trimmed.to_string(),
            normalized,
            terms,
            options,
            is_glob,
        })
    }
}
