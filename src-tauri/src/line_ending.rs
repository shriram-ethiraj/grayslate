/**
 * line_ending.rs
 *
 * Cross-platform end-of-line (EOL) support.
 *
 * The editor keeps every document canonically LF-terminated in memory —
 * CodeMirror enforces this anyway, since `EditorState.create` splits on
 * `/\r\n?|\n/` and always rejoins with `\n`. The document's writable line
 * ending is therefore *metadata*, not part of the text, and conversion happens
 * only at the boundaries where canonical text leaves the app:
 *
 *   - disk writes  (`autosave_write_to_disk` / `atomic_create_to_disk`)
 *   - clipboard    (`commands::clipboard::write_text`)
 *
 * Detection runs once, over the bytes `read_file_content` already holds, so
 * opening a file costs no extra IO.
 */
use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// How many line breaks to inspect before deciding a file's dominant style.
///
/// Mature editors differ here — VS Code counts the entire buffer, Sublime
/// samples the head. We sample, because the read path supports files up to
/// 200 MB and a full scan buys nothing: a file whose first thousand breaks are
/// uniform is not going to be reclassified by break 900,000.
const MAX_EOL_SCAN_BREAKS: usize = 1_000;

/// A concrete line ending. This is per-document metadata, never part of the
/// document text itself.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Eol {
    #[default]
    Lf,
    Crlf,
}

impl Eol {
    /// The line ending convention used for new files on the host OS.
    pub const fn platform_default() -> Self {
        if cfg!(windows) {
            Eol::Crlf
        } else {
            Eol::Lf
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Eol::Lf => "lf",
            Eol::Crlf => "crlf",
        }
    }

    /// The byte sequence this EOL writes to disk.
    pub const fn sequence(self) -> &'static str {
        match self {
            Eol::Lf => "\n",
            Eol::Crlf => "\r\n",
        }
    }

    /// Parse a frontend-supplied value. Fail-closed: the webview is untrusted,
    /// so an unrecognized string is an error rather than a silent fallback.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "lf" => Ok(Eol::Lf),
            "crlf" => Ok(Eol::Crlf),
            other => Err(format!("Unsupported line ending: {other}")),
        }
    }
}

/// Detect the dominant line ending in `bytes`.
///
/// Scans at most [`MAX_EOL_SCAN_BREAKS`] breaks. The most frequent style wins;
/// an exact tie resolves to whichever style appeared first, which keeps the
/// result stable and matches what a reader would call the file's "real" style.
/// A file containing no line break at all yields `fallback` (the global
/// default) — there is nothing to detect.
///
/// Lone CR breaks count as LF-compatible single-byte breaks. This keeps legacy
/// files readable while restricting writable results to the modern LF/CRLF
/// pair; merely detecting the file never writes it.
///
/// Operates on bytes rather than `&str` because CR and LF are ASCII and cannot
/// occur inside a multi-byte UTF-8 sequence, so a raw byte scan is both correct
/// and free of any decoding cost.
pub fn detect_eol(bytes: &[u8], fallback: Eol) -> Eol {
    let mut crlf = 0_usize;
    let mut lf = 0_usize;
    let mut first: Option<Eol> = None;

    let mut index = 0;
    while index < bytes.len() && crlf + lf < MAX_EOL_SCAN_BREAKS {
        let style = match bytes[index] {
            b'\r' => {
                if bytes.get(index + 1) == Some(&b'\n') {
                    index += 1;
                    Eol::Crlf
                } else {
                    // Lone CR is readable because CodeMirror normalizes it to
                    // LF on input, but it is no longer a writable format.
                    // Classifying it as LF modernizes the file on its next
                    // actual write without touching it merely because it was
                    // opened.
                    Eol::Lf
                }
            }
            b'\n' => Eol::Lf,
            _ => {
                index += 1;
                continue;
            }
        };

        match style {
            Eol::Crlf => crlf += 1,
            Eol::Lf => lf += 1,
        }
        first.get_or_insert(style);
        index += 1;
    }

    let Some(first) = first else {
        return fallback;
    };

    // Dominant style wins; ties fall back to the first style encountered.
    let max = crlf.max(lf);
    let winner_count = |eol: Eol| match eol {
        Eol::Crlf => crlf,
        Eol::Lf => lf,
    };
    if winner_count(first) == max {
        return first;
    }
    if crlf == max {
        Eol::Crlf
    } else {
        Eol::Lf
    }
}

/// Convert canonical LF text to `eol`.
///
/// **Precondition:** `content` is canonically LF-terminated. Every caller
/// receives its text either straight from CodeMirror (which guarantees LF) or
/// from a Rust serializer that emits LF, so a naive `\n` substitution is exact
/// — there are no stray `\r` bytes to double-convert.
///
/// LF borrows rather than copies, which is both the default and the hot path.
/// Trailing-newline presence is preserved for free.
pub fn apply_eol(content: &str, eol: Eol) -> Cow<'_, str> {
    match eol {
        Eol::Lf => Cow::Borrowed(content),
        Eol::Crlf => Cow::Owned(content.replace('\n', "\r\n")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_pure_styles() {
        assert_eq!(detect_eol(b"a\nb\nc", Eol::Crlf), Eol::Lf);
        assert_eq!(detect_eol(b"a\r\nb\r\nc", Eol::Lf), Eol::Crlf);
        assert_eq!(detect_eol(b"a\rb\rc", Eol::Crlf), Eol::Lf);
    }

    #[test]
    fn empty_and_newline_free_input_uses_fallback() {
        assert_eq!(detect_eol(b"", Eol::Crlf), Eol::Crlf);
        assert_eq!(detect_eol(b"single line", Eol::Crlf), Eol::Crlf);
        assert_eq!(detect_eol(b"single line", Eol::Lf), Eol::Lf);
    }

    #[test]
    fn dominant_style_wins_in_mixed_input() {
        // 3 CRLF vs 1 LF.
        assert_eq!(detect_eol(b"a\r\nb\r\nc\r\nd\ne", Eol::Lf), Eol::Crlf);
        // 3 LF vs 1 CRLF.
        assert_eq!(detect_eol(b"a\nb\nc\nd\r\ne", Eol::Crlf), Eol::Lf);
    }

    #[test]
    fn exact_tie_resolves_to_first_style_encountered() {
        assert_eq!(detect_eol(b"a\r\nb\nc", Eol::Lf), Eol::Crlf);
        assert_eq!(detect_eol(b"a\nb\r\nc", Eol::Crlf), Eol::Lf);
        assert_eq!(detect_eol(b"a\rb\r\nc", Eol::Crlf), Eol::Lf);
    }

    #[test]
    fn lone_cr_is_not_confused_with_crlf() {
        // A CR followed by a non-LF byte is a readable single-byte break, not
        // half a CRLF. It maps to the modern LF output style.
        assert_eq!(detect_eol(b"a\rb\r\nc\r\nd", Eol::Lf), Eol::Crlf);
        // Trailing CR at end of input has no following byte to inspect.
        assert_eq!(detect_eol(b"a\r", Eol::Crlf), Eol::Lf);
    }

    #[test]
    fn scan_stops_at_the_break_cap() {
        // MAX_EOL_SCAN_BREAKS of CRLF, then a long LF tail that would win a
        // whole-file count. The cap means the head decides.
        let mut input = "a\r\n".repeat(MAX_EOL_SCAN_BREAKS);
        input.push_str(&"b\n".repeat(MAX_EOL_SCAN_BREAKS * 4));
        assert_eq!(detect_eol(input.as_bytes(), Eol::Lf), Eol::Crlf);
    }

    #[test]
    fn crlf_pair_straddling_the_cap_is_not_split() {
        // The final counted break sits exactly on the cap; its `\n` must be
        // consumed as part of the CRLF rather than counted as a separate LF.
        let input = "a\r\n".repeat(MAX_EOL_SCAN_BREAKS);
        assert_eq!(detect_eol(input.as_bytes(), Eol::Lf), Eol::Crlf);
    }

    #[test]
    fn detection_ignores_multibyte_utf8_content() {
        // CR/LF are ASCII and cannot appear inside a multi-byte sequence.
        assert_eq!(
            detect_eol("héllo\r\nwörld\r\n".as_bytes(), Eol::Lf),
            Eol::Crlf
        );
    }

    #[test]
    fn apply_eol_converts_every_break() {
        assert_eq!(apply_eol("a\nb\nc", Eol::Lf), "a\nb\nc");
        assert_eq!(apply_eol("a\nb\nc", Eol::Crlf), "a\r\nb\r\nc");
    }

    #[test]
    fn apply_eol_preserves_trailing_newline_presence() {
        assert_eq!(apply_eol("a\n", Eol::Crlf), "a\r\n");
        assert_eq!(apply_eol("a", Eol::Crlf), "a");
        assert_eq!(apply_eol("", Eol::Crlf), "");
    }

    #[test]
    fn apply_eol_borrows_for_lf() {
        assert!(matches!(apply_eol("a\nb", Eol::Lf), Cow::Borrowed(_)));
    }

    #[test]
    fn detect_round_trips_apply() {
        let canonical = "one\ntwo\nthree\n";
        for eol in [Eol::Lf, Eol::Crlf] {
            let written = apply_eol(canonical, eol);
            assert_eq!(detect_eol(written.as_bytes(), Eol::Lf), eol, "{eol:?}");
        }
    }

    #[test]
    fn platform_default_matches_the_host_convention() {
        let expected = if cfg!(windows) { Eol::Crlf } else { Eol::Lf };
        assert_eq!(Eol::platform_default(), expected);
    }

    #[test]
    fn parsing_rejects_unknown_values() {
        assert_eq!(Eol::parse("crlf"), Ok(Eol::Crlf));
        assert!(Eol::parse("cr").is_err());
        assert!(Eol::parse("system").is_err());
        assert!(Eol::parse("").is_err());
    }

    #[test]
    fn eol_serializes_as_lowercase_for_the_frontend() {
        assert_eq!(serde_json::to_string(&Eol::Crlf).unwrap(), "\"crlf\"");
        assert_eq!(serde_json::to_string(&Eol::Lf).unwrap(), "\"lf\"");
    }
}
