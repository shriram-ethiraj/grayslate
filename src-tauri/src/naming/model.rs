/// Maximum bytes fed to any extractor.
pub const MAX_CONTENT_BYTES: usize = 5_000;

/// Maximum characters in the final slug (including any suffix).
pub const MAX_STEM_LEN: usize = 60;

/// Maximum symbol tokens combined into the stem.
/// Set to 1 so extractors pick the single best name, not a concatenation.
pub const MAX_TOKENS: usize = 1;

// ---------------------------------------------------------------------------
// Extracted-name metadata
// ---------------------------------------------------------------------------

/// Distinguishes generic content from email / prompt prose detections so the
/// pipeline can append a descriptive suffix to the final filename.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StemKind {
    Generic,
    Email,
    Prompt,
}

impl StemKind {
    /// The suffix string appended to the slug (empty for generic content).
    pub fn suffix(self) -> &'static str {
        match self {
            StemKind::Generic => "",
            StemKind::Email => "-email",
            StemKind::Prompt => "-prompt",
        }
    }
}

/// A named extraction result that carries both the raw stem and the prose
/// kind (email / prompt / generic).
#[derive(Debug, Clone)]
pub struct ExtractedName {
    pub stem: String,
    pub kind: StemKind,
}
