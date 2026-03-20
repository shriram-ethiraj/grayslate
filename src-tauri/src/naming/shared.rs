use super::model::{MAX_CONTENT_BYTES, MAX_STEM_LEN};

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
pub fn slugify(raw: &str) -> Option<String> {
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

    if slug.is_empty() {
        return None;
    }

    // Cap length at a word boundary.
    let capped = if slug.len() <= MAX_STEM_LEN {
        slug.to_string()
    } else {
        let end = &slug[..MAX_STEM_LEN];
        // Roll back to last hyphen to avoid cutting mid-word.
        match end.rfind('-') {
            Some(pos) if pos > 10 => end[..pos].to_string(),
            _ => end.to_string(),
        }
    };

    if capped.is_empty() {
        None
    } else {
        Some(capped)
    }
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
