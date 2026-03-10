#[derive(Clone, Debug)]
pub struct ParsedSearchQuery {
    pub raw: String,
    pub normalized: String,
    pub terms: Vec<String>,
}

pub fn parse_query(raw: &str) -> Result<ParsedSearchQuery, String> {
    let normalized = raw.trim().to_lowercase();
    let terms = normalized
        .split_whitespace()
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    Ok(ParsedSearchQuery {
        raw: raw.trim().to_string(),
        normalized,
        terms,
    })
}