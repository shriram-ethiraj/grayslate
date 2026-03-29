use super::model::{ExtractedName, StemKind, MAX_CONTENT_BYTES, MAX_STEM_LEN};

// ---------------------------------------------------------------------------
// Bounded input helper
// ---------------------------------------------------------------------------

pub(super) fn bound(content: &str) -> &str {
    // Slice on a char boundary.
    if content.len() <= MAX_CONTENT_BYTES {
        content
    } else {
        let mut end = MAX_CONTENT_BYTES;
        while !content.is_char_boundary(end) {
            end -= 1;
        }
        &content[..end]
    }
}

// ---------------------------------------------------------------------------
// Sanitization + slug
// ---------------------------------------------------------------------------

/// Converts a raw extracted stem into a safe, lowercase hyphenated filename
/// component. Returns `None` if the result would be empty.
///
/// The optional `budget` caps the slug portion *before* any suffix is
/// appended.  Pass `MAX_STEM_LEN` (or `None`) for the default cap.
pub fn slugify(raw: &str) -> Option<String> {
    slugify_with_budget(raw, MAX_STEM_LEN)
}

/// Core slugification with an explicit character budget.
fn slugify_with_budget(raw: &str, budget: usize) -> Option<String> {
    if raw.trim().is_empty() {
        return None;
    }

    // Insert hyphens at camelCase / PascalCase boundaries before lowercasing.
    let mut split = String::with_capacity(raw.len() + 8);
    let chars: Vec<char> = raw.chars().collect();
    for (i, &ch) in chars.iter().enumerate() {
        if i > 0 && ch.is_uppercase() && chars[i - 1].is_lowercase() {
            split.push('-');
        }
        split.push(ch);
    }

    // Strip invalid filesystem chars and replace separators with hyphens.
    let slug: String = split
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '-',
            '_' | ' ' | '\t' | '\n' | '\r' | '.' => '-',
            // Punctuation that should never appear in a filename stem.
            ',' | ';' | '(' | ')' | '{' | '}' | '[' | ']' | '#' | '@' => '-',
            _ => c,
        })
        .collect::<String>()
        .to_lowercase();

    // Collapse consecutive hyphens and trim.
    let re_collapse = regex::Regex::new(r"-{2,}").ok()?;
    let slug = re_collapse.replace_all(&slug, "-");
    let slug = slug.trim_matches('-');

    // Remove leading digits-only segments that look noisy (optional quality filter).
    let slug = slug.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-');
    let slug = slug.trim_matches('-');

    // Deduplicate words: remove any word that already appeared earlier in the
    // slug. This handles both adjacent repeats ("sanity-sanity-test" →
    // "sanity-test") and non-adjacent repeats ("github-com-xyz-github-com" →
    // "github-com-xyz").
    let slug = {
        let parts: Vec<&str> = slug.split('-').collect();
        let mut seen = std::collections::HashSet::new();
        let deduped: Vec<&str> = parts
            .into_iter()
            .filter(|w| !w.is_empty() && seen.insert(*w))
            .collect();
        deduped.join("-")
    };
    let slug = slug.as_str();

    if slug.is_empty() {
        return None;
    }

    // Cap length at a word boundary.
    let capped = truncate_at_word_boundary(slug, budget);

    if capped.is_empty() {
        None
    } else {
        Some(capped.to_string())
    }
}

/// Truncate a hyphenated slug to at most `max_len` characters, preferring
/// a break at the last `-` so words are not chopped mid-way.
fn truncate_at_word_boundary(slug: &str, max_len: usize) -> &str {
    if slug.len() <= max_len {
        return slug;
    }
    let end = &slug[..max_len];
    match end.rfind('-') {
        Some(pos) if pos > 10 => &end[..pos],
        _ => end,
    }
}

// ---------------------------------------------------------------------------
// Suffix-aware finalizer
// ---------------------------------------------------------------------------

/// Slugify a raw stem and append the appropriate suffix (`-email`,
/// `-prompt`, or nothing) while keeping the total length within
/// `MAX_STEM_LEN`.
pub(super) fn finalize(raw: Option<String>, kind: StemKind) -> Option<String> {
    let raw = raw?;
    let suffix = kind.suffix();
    // Reserve room for the suffix so it is never truncated.
    let budget = MAX_STEM_LEN.saturating_sub(suffix.len());
    let slug = slugify_with_budget(&raw, budget)?;
    if suffix.is_empty() {
        Some(slug)
    } else {
        Some(format!("{slug}{suffix}"))
    }
}

/// Finalize an `ExtractedName` (stem + kind) into the final slug.
pub(super) fn finalize_extracted(en: Option<ExtractedName>) -> Option<String> {
    let en = en?;
    finalize(Some(en.stem), en.kind)
}

/// Human-readable fallback: `slate-19-mar-2026-0530`.
pub fn fallback_stem() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_readable_timestamp(secs)
}

fn format_readable_timestamp(unix_secs: u64) -> String {
    // Simple manual UTC decomposition — no external crate needed.
    let mut s = unix_secs;
    s /= 60;
    let mins = s % 60;
    s /= 60;
    let hours = s % 24;
    s /= 24;

    // Days since Unix epoch → year/month/day.
    let (year, month, day) = days_to_ymd(s as u32);

    const MONTHS: [&str; 12] = [
        "jan", "feb", "mar", "apr", "may", "jun",
        "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    let mon = MONTHS[(month as usize).saturating_sub(1).min(11)];

    // Format: slate-DD-mon-YYYY-HHMM  e.g. slate-19-mar-2026-0530
    format!("slate-{:02}-{}-{:04}-{:02}{:02}", day, mon, year, hours, mins)
}

fn days_to_ymd(mut days: u32) -> (u32, u32, u32) {
    // Gregorian calendar decomposition.
    let mut year = 1970u32;
    loop {
        let leap = is_leap(year);
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = is_leap(year);
    let month_days = [
        31u32,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u32;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_adjacent_words() {
        let result = slugify("sanity-sanity-test").unwrap();
        assert_eq!(result, "sanity-test");
    }

    #[test]
    fn dedup_non_adjacent_words() {
        let result = slugify("github-com-xyz-github-com").unwrap();
        assert_eq!(result, "github-com-xyz");
    }

    #[test]
    fn dedup_multiple_repeats() {
        let result = slugify("fix-modifying-modifying-users-bugfixes").unwrap();
        assert_eq!(result, "fix-modifying-users-bugfixes");
    }

    #[test]
    fn dedup_callback_repeat() {
        let result = slugify("yaml-output-default-callback-callback-plugin").unwrap();
        assert_eq!(result, "yaml-output-default-callback-plugin");
    }

    #[test]
    fn no_dedup_when_unique() {
        let result = slugify("authentication-token-parser").unwrap();
        assert_eq!(result, "authentication-token-parser");
    }

    #[test]
    fn camel_case_split_and_dedup() {
        let result = slugify("SanitySanityTest").unwrap();
        assert_eq!(result, "sanity-test");
    }

    #[test]
    fn empty_returns_none() {
        assert!(slugify("").is_none());
        assert!(slugify("   ").is_none());
    }

    #[test]
    fn truncation_at_word_boundary() {
        let long = "a]b-".repeat(20); // 80 chars
        let result = slugify(&long).unwrap();
        assert!(result.len() <= MAX_STEM_LEN, "len={}", result.len());
    }

    #[test]
    fn leading_digits_stripped() {
        let result = slugify("123-my-module").unwrap();
        assert_eq!(result, "my-module");
    }
}
