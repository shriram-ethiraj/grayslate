//! Markdown → safe preview HTML renderer.
//!
//! Converts markdown source text into sanitized HTML with `data-line`
//! attributes on block-level elements.  The frontend scroll-sync system
//! relies on these anchors to map editor source lines to preview DOM
//! positions.
//!
//! Architecture:
//! - `pulldown-cmark` parses the markdown (GFM tables, task-lists,
//!   strikethrough enabled).
//! - A custom event renderer produces HTML with `data-line` attributes
//!   on every block-level open tag.
//! - `ammonia` sanitizes the final output so inline HTML from the
//!   markdown source is safe.  `data-line` is explicitly allowed.
//! - Cancellation is checked periodically via an `AtomicBool` so
//!   rapid edits can abort superseded renders.

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::fmt::Write;
use std::sync::atomic::{AtomicBool, Ordering};

/// Check cancellation flag every N events to avoid hot-path overhead.
const CANCEL_CHECK_INTERVAL: u32 = 5_000;

// ───────────────────────────────────────────────────────────────────────
// Line mapping
// ───────────────────────────────────────────────────────────────────────

/// Build a lookup table of byte offsets where each source line begins.
fn build_line_starts(src: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    for (i, byte) in src.as_bytes().iter().enumerate() {
        if *byte == b'\n' {
            starts.push(i + 1);
        }
    }
    starts
}

/// Convert a byte offset to a 1-based line number using binary search.
fn offset_to_line(line_starts: &[usize], offset: usize) -> usize {
    // partition_point returns the count of elements where start <= offset,
    // which equals the 1-based line number (line_starts[0] is always 0).
    line_starts.partition_point(|&start| start <= offset).max(1)
}

// ───────────────────────────────────────────────────────────────────────
// HTML escaping helpers
// ───────────────────────────────────────────────────────────────────────

fn escape_html(src: &str, out: &mut String) {
    for ch in src.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
}

fn escape_href(url: &str, out: &mut String) {
    for ch in url.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '\'' => out.push_str("&#x27;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
}

// ───────────────────────────────────────────────────────────────────────
// Renderer
// ───────────────────────────────────────────────────────────────────────

struct Renderer {
    html: String,
    line_starts: Vec<usize>,
    /// Column alignments for the table currently being rendered.
    table_alignments: Vec<Alignment>,
    /// Column index within the current table row.
    table_cell_index: usize,
    /// Whether we are inside a `<thead>` section (for th vs td).
    in_table_head: bool,
    /// When true, only collect plain text for the image alt attribute.
    in_image: bool,
}

impl Renderer {
    fn new(src: &str) -> Self {
        Self {
            html: String::with_capacity(src.len() * 2),
            line_starts: build_line_starts(src),
            table_alignments: Vec::new(),
            table_cell_index: 0,
            in_table_head: false,
            in_image: false,
        }
    }

    fn line_at(&self, byte_offset: usize) -> usize {
        offset_to_line(&self.line_starts, byte_offset)
    }

    /// Append ` data-line="N"` to the output buffer.
    fn write_data_line(&mut self, byte_offset: usize) {
        let line = self.line_at(byte_offset);
        write!(self.html, " data-line=\"{}\"", line).unwrap();
    }

    /// Append alignment attribute for the current table cell.
    fn write_cell_align(&mut self) {
        if self.table_cell_index < self.table_alignments.len() {
            match self.table_alignments[self.table_cell_index] {
                Alignment::Left => self.html.push_str(" align=\"left\""),
                Alignment::Center => self.html.push_str(" align=\"center\""),
                Alignment::Right => self.html.push_str(" align=\"right\""),
                Alignment::None => {}
            }
        }
    }

    fn process_event(&mut self, event: Event<'_>, start: usize) {
        // Inside an image tag we only collect plain text for the alt
        // attribute — skip all HTML-producing events.
        if self.in_image {
            match event {
                Event::Text(ref text) | Event::Code(ref text) => {
                    escape_html(text, &mut self.html);
                }
                Event::SoftBreak | Event::HardBreak => {
                    self.html.push(' ');
                }
                Event::End(TagEnd::Image) => {
                    self.in_image = false;
                    self.html.push_str("\" />");
                }
                _ => {}
            }
            return;
        }

        match event {
            Event::Start(tag) => self.open_tag(tag, start),
            Event::End(tag_end) => self.close_tag(tag_end),
            Event::Text(text) => escape_html(&text, &mut self.html),
            Event::Code(code) => {
                self.html.push_str("<code>");
                escape_html(&code, &mut self.html);
                self.html.push_str("</code>");
            }
            Event::SoftBreak => self.html.push('\n'),
            Event::HardBreak => self.html.push_str("<br />\n"),
            Event::Rule => {
                self.html.push_str("<hr");
                self.write_data_line(start);
                self.html.push_str(" />\n");
            }
            // Raw HTML from the markdown source — passed through for
            // ammonia to sanitize.
            Event::Html(raw) | Event::InlineHtml(raw) => {
                self.html.push_str(&raw);
            }
            Event::TaskListMarker(checked) => {
                if checked {
                    self.html
                        .push_str("<input type=\"checkbox\" checked=\"\" disabled=\"\" /> ");
                } else {
                    self.html
                        .push_str("<input type=\"checkbox\" disabled=\"\" /> ");
                }
            }
            Event::FootnoteReference(_) => {}
            // Math: render as code spans / blocks to keep content visible
            // even without a dedicated math renderer.
            Event::InlineMath(math) => {
                self.html.push_str("<code>");
                escape_html(&math, &mut self.html);
                self.html.push_str("</code>");
            }
            Event::DisplayMath(math) => {
                self.html.push_str("<pre");
                self.write_data_line(start);
                self.html.push_str("><code>");
                escape_html(&math, &mut self.html);
                self.html.push_str("</code></pre>\n");
            }
        }
    }

    fn open_tag(&mut self, tag: Tag<'_>, start: usize) {
        match tag {
            Tag::Heading { level, .. } => {
                let n = heading_level_to_u8(level);
                write!(self.html, "<h{}", n).unwrap();
                self.write_data_line(start);
                self.html.push('>');
            }
            Tag::Paragraph => {
                self.html.push_str("<p");
                self.write_data_line(start);
                self.html.push('>');
            }
            Tag::CodeBlock(kind) => {
                self.html.push_str("<pre");
                self.write_data_line(start);
                self.html.push_str("><code");
                if let CodeBlockKind::Fenced(ref info) = kind {
                    let lang = info.split_whitespace().next().unwrap_or("");
                    if !lang.is_empty() {
                        self.html.push_str(" class=\"language-");
                        escape_html(lang, &mut self.html);
                        self.html.push('"');
                    }
                }
                self.html.push('>');
            }
            Tag::BlockQuote(_) => {
                self.html.push_str("<blockquote");
                self.write_data_line(start);
                self.html.push('>');
            }
            Tag::List(first_item) => match first_item {
                Some(start_num) => {
                    self.html.push_str("<ol");
                    self.write_data_line(start);
                    if start_num != 1 {
                        write!(self.html, " start=\"{}\"", start_num).unwrap();
                    }
                    self.html.push_str(">\n");
                }
                None => {
                    self.html.push_str("<ul");
                    self.write_data_line(start);
                    self.html.push_str(">\n");
                }
            },
            Tag::Item => {
                self.html.push_str("<li");
                self.write_data_line(start);
                self.html.push('>');
            }
            Tag::Table(alignments) => {
                self.table_alignments = alignments;
                self.html.push_str("<table");
                self.write_data_line(start);
                self.html.push_str(">\n");
            }
            Tag::TableHead => {
                self.in_table_head = true;
                self.table_cell_index = 0;
                self.html.push_str("<thead>\n<tr>");
            }
            Tag::TableRow => {
                self.table_cell_index = 0;
                self.html.push_str("<tr>");
            }
            Tag::TableCell => {
                let tag = if self.in_table_head { "th" } else { "td" };
                write!(self.html, "<{}", tag).unwrap();
                self.write_cell_align();
                self.html.push('>');
            }
            Tag::Emphasis => self.html.push_str("<em>"),
            Tag::Strong => self.html.push_str("<strong>"),
            Tag::Strikethrough => self.html.push_str("<del>"),
            Tag::Link { dest_url, title, .. } => {
                self.html.push_str("<a href=\"");
                escape_href(&dest_url, &mut self.html);
                self.html.push('"');
                if !title.is_empty() {
                    self.html.push_str(" title=\"");
                    escape_html(&title, &mut self.html);
                    self.html.push('"');
                }
                self.html.push('>');
            }
            Tag::Image { dest_url, title, .. } => {
                self.html.push_str("<img src=\"");
                escape_href(&dest_url, &mut self.html);
                self.html.push('"');
                if !title.is_empty() {
                    self.html.push_str(" title=\"");
                    escape_html(&title, &mut self.html);
                    self.html.push('"');
                }
                self.html.push_str(" alt=\"");
                self.in_image = true;
            }
            _ => {}
        }
    }

    fn close_tag(&mut self, tag_end: TagEnd) {
        match tag_end {
            TagEnd::Heading(level) => {
                let n = heading_level_to_u8(level);
                write!(self.html, "</h{}>\n", n).unwrap();
            }
            TagEnd::Paragraph => self.html.push_str("</p>\n"),
            TagEnd::CodeBlock => self.html.push_str("</code></pre>\n"),
            TagEnd::BlockQuote(_) => self.html.push_str("</blockquote>\n"),
            TagEnd::List(ordered) => {
                if ordered {
                    self.html.push_str("</ol>\n");
                } else {
                    self.html.push_str("</ul>\n");
                }
            }
            TagEnd::Item => self.html.push_str("</li>\n"),
            TagEnd::Table => {
                self.html.push_str("</tbody>\n</table>\n");
                self.table_alignments.clear();
            }
            TagEnd::TableHead => {
                self.in_table_head = false;
                self.html.push_str("</tr>\n</thead>\n<tbody>\n");
            }
            TagEnd::TableRow => self.html.push_str("</tr>\n"),
            TagEnd::TableCell => {
                let tag = if self.in_table_head { "th" } else { "td" };
                write!(self.html, "</{}>", tag).unwrap();
                self.table_cell_index += 1;
            }
            TagEnd::Emphasis => self.html.push_str("</em>"),
            TagEnd::Strong => self.html.push_str("</strong>"),
            TagEnd::Strikethrough => self.html.push_str("</del>"),
            TagEnd::Link => self.html.push_str("</a>"),
            TagEnd::Image => {
                // Image close is handled in process_event when in_image is set.
                // This arm fires only if the parser emits End(Image) without
                // a matching Start(Image) having set in_image — guard anyway.
                if self.in_image {
                    self.in_image = false;
                    self.html.push_str("\" />");
                }
            }
            _ => {}
        }
    }
}

fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

// ───────────────────────────────────────────────────────────────────────
// Sanitization
// ───────────────────────────────────────────────────────────────────────

/// Sanitize rendered HTML while preserving `data-line` anchors and
/// other attributes required by the preview surface.
fn sanitize_preview_html(html: &str) -> String {
    ammonia::Builder::new()
        .add_generic_attributes(&["data-line"])
        .add_tags(&["input"])
        .add_tag_attributes("input", &["type", "checked", "disabled"])
        .add_tag_attributes("code", &["class"])
        .add_tag_attributes("a", &["title"])
        .add_tag_attributes("img", &["title"])
        .add_tag_attributes("td", &["align"])
        .add_tag_attributes("th", &["align"])
        .clean(html)
        .to_string()
}

// ───────────────────────────────────────────────────────────────────────
// Public API
// ───────────────────────────────────────────────────────────────────────

/// Render markdown source to sanitized preview HTML with `data-line`
/// anchors on block-level elements.
///
/// The `cancelled` flag is checked periodically; if set, the function
/// returns `Err("Cancelled")` promptly.
pub(crate) fn render_markdown_to_html(
    src: &str,
    cancelled: &AtomicBool,
) -> Result<String, String> {
    if src.is_empty() {
        return Ok(String::new());
    }

    if cancelled.load(Ordering::Relaxed) {
        return Err("Cancelled".to_string());
    }

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(src, options);
    let mut renderer = Renderer::new(src);
    let mut event_count = 0u32;

    for (event, range) in parser.into_offset_iter() {
        event_count += 1;
        if event_count % CANCEL_CHECK_INTERVAL == 0 && cancelled.load(Ordering::Relaxed) {
            return Err("Cancelled".to_string());
        }
        renderer.process_event(event, range.start);
    }

    if cancelled.load(Ordering::Relaxed) {
        return Err("Cancelled".to_string());
    }

    Ok(sanitize_preview_html(&renderer.html))
}

// ───────────────────────────────────────────────────────────────────────
// Tests
// ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    fn no_cancel() -> AtomicBool {
        AtomicBool::new(false)
    }

    fn render(src: &str) -> String {
        render_markdown_to_html(src, &no_cancel()).unwrap()
    }

    #[test]
    fn empty_input_returns_empty() {
        assert_eq!(render(""), "");
    }

    #[test]
    fn heading_has_data_line() {
        let html = render("# Hello");
        assert!(html.contains("data-line=\"1\""), "html: {html}");
        assert!(html.contains("<h1"));
        assert!(html.contains("Hello"));
    }

    #[test]
    fn multiline_heading_lines() {
        let html = render("# First\n\n## Second\n\n### Third");
        assert!(html.contains("<h1 data-line=\"1\">First</h1>"), "html: {html}");
        assert!(html.contains("<h2 data-line=\"3\">Second</h2>"), "html: {html}");
        assert!(html.contains("<h3 data-line=\"5\">Third</h3>"), "html: {html}");
    }

    #[test]
    fn paragraph_has_data_line() {
        let html = render("Hello world");
        assert!(html.contains("<p data-line=\"1\">Hello world</p>"), "html: {html}");
    }

    #[test]
    fn code_block_has_data_line_and_lang() {
        let html = render("```rust\nfn main() {}\n```");
        assert!(html.contains("data-line="), "html: {html}");
        assert!(html.contains("language-rust"), "html: {html}");
        assert!(html.contains("fn main()"), "html: {html}");
    }

    #[test]
    fn code_block_escapes_html() {
        let html = render("```\n<script>alert('xss')</script>\n```");
        assert!(!html.contains("<script>"), "html: {html}");
    }

    #[test]
    fn unordered_list_has_data_line() {
        let html = render("- one\n- two\n- three");
        assert!(html.contains("<ul data-line="), "html: {html}");
        assert!(html.contains("<li data-line="), "html: {html}");
    }

    #[test]
    fn ordered_list_has_data_line() {
        let html = render("1. one\n2. two\n3. three");
        assert!(html.contains("<ol data-line="), "html: {html}");
        assert!(html.contains("<li data-line="), "html: {html}");
    }

    #[test]
    fn blockquote_has_data_line() {
        let html = render("> quoted text");
        assert!(html.contains("<blockquote data-line="), "html: {html}");
        assert!(html.contains("quoted text"), "html: {html}");
    }

    #[test]
    fn table_has_data_line_and_structure() {
        let html = render("| A | B |\n|---|---|\n| 1 | 2 |");
        assert!(html.contains("<table data-line="), "html: {html}");
        assert!(html.contains("<thead>"), "html: {html}");
        assert!(html.contains("<tbody>"), "html: {html}");
        assert!(html.contains("<th>"), "html: {html}");
        assert!(html.contains("<td>"), "html: {html}");
    }

    #[test]
    fn table_alignment() {
        let html = render("| Left | Center | Right |\n|:-----|:------:|------:|\n| a | b | c |");
        assert!(html.contains("align=\"left\""), "html: {html}");
        assert!(html.contains("align=\"center\""), "html: {html}");
        assert!(html.contains("align=\"right\""), "html: {html}");
    }

    #[test]
    fn task_list_checkboxes() {
        let html = render("- [x] done\n- [ ] todo");
        assert!(html.contains("checked"), "html: {html}");
        assert!(html.contains("disabled"), "html: {html}");
        assert!(html.contains("<input"), "html: {html}");
    }

    #[test]
    fn horizontal_rule_has_data_line() {
        let html = render("text\n\n---\n\nmore");
        assert!(html.contains("<hr"), "html: {html}");
        assert!(html.contains("data-line="), "html: {html}");
    }

    #[test]
    fn inline_formatting() {
        let html = render("**bold** *italic* ~~strike~~ `code`");
        assert!(html.contains("<strong>bold</strong>"), "html: {html}");
        assert!(html.contains("<em>italic</em>"), "html: {html}");
        assert!(html.contains("<del>strike</del>"), "html: {html}");
        assert!(html.contains("<code>code</code>"), "html: {html}");
    }

    #[test]
    fn link_rendering() {
        let html = render("[click](https://example.com \"tip\")");
        assert!(html.contains("href=\"https://example.com\""), "html: {html}");
        assert!(html.contains("title=\"tip\""), "html: {html}");
        assert!(html.contains(">click</a>"), "html: {html}");
    }

    #[test]
    fn image_rendering() {
        let html = render("![alt text](image.png \"title\")");
        assert!(html.contains("src=\"image.png\""), "html: {html}");
        assert!(html.contains("alt=\"alt text\""), "html: {html}");
    }

    #[test]
    fn inline_html_is_sanitized() {
        // Safe inline HTML should be preserved by ammonia
        let html = render("text <kbd>Ctrl</kbd> more");
        assert!(html.contains("<kbd>"), "safe html should be kept: {html}");

        // Dangerous inline HTML should be stripped
        let html = render("text <script>alert('xss')</script> more");
        assert!(!html.contains("<script>"), "script should be stripped: {html}");
    }

    #[test]
    fn cancellation_aborts_render() {
        let cancelled = AtomicBool::new(true);
        let result = render_markdown_to_html("# Hello", &cancelled);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Cancelled");
    }

    #[test]
    fn line_starts_basic() {
        let starts = build_line_starts("abc\ndef\nghi");
        assert_eq!(starts, vec![0, 4, 8]);
    }

    #[test]
    fn offset_to_line_mapping() {
        let starts = build_line_starts("abc\ndef\nghi");
        assert_eq!(offset_to_line(&starts, 0), 1); // 'a'
        assert_eq!(offset_to_line(&starts, 3), 1); // '\n'
        assert_eq!(offset_to_line(&starts, 4), 2); // 'd'
        assert_eq!(offset_to_line(&starts, 8), 3); // 'g'
    }
}
