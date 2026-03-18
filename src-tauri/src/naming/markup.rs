/// XML / HTML: root element name + up to two attribute values.
pub(super) fn extract_xml_html(content: &str) -> Option<String> {
    // Find the first tag: <tagname attr="val" ...>
    // Skip XML declaration and DOCTYPE.
    let tag_re = regex::Regex::new(r"<([a-zA-Z][a-zA-Z0-9\-:_]*)").ok()?;
    let attr_re =
        regex::Regex::new(r#"(?:id|name|class|title)\s*=\s*["']([^"']{1,40})["']"#).ok()?;

    let trimmed = content.trim();
    let mut tokens: Vec<String> = Vec::new();

    for cap in tag_re.captures_iter(trimmed).take(8) {
        let name = &cap[1];
        // Skip XML declaration and meta tags.
        if name.eq_ignore_ascii_case("?xml")
            || name.eq_ignore_ascii_case("!DOCTYPE")
            || name.eq_ignore_ascii_case("meta")
            || name.eq_ignore_ascii_case("link")
            || name.eq_ignore_ascii_case("script")
            || name.eq_ignore_ascii_case("style")
        {
            continue;
        }
        tokens.push(name.to_string());
        break;
    }

    // Add meaningful attribute values.
    for cap in attr_re.captures_iter(trimmed).take(2) {
        let val = cap[1].trim();
        if !val.is_empty() {
            tokens.push(val.to_string());
        }
    }

    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join("-"))
    }
}

/// Markdown: first `# heading` or frontmatter `title:` value.
pub(super) fn extract_markdown(content: &str) -> Option<String> {
    let mut in_frontmatter = false;
    let mut frontmatter_done = false;
    let mut first_line_was_dashes = false;

    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // YAML frontmatter block.
        if i == 0 && trimmed == "---" {
            in_frontmatter = true;
            first_line_was_dashes = true;
            continue;
        }
        if in_frontmatter {
            if trimmed == "---" || trimmed == "..." {
                in_frontmatter = false;
                frontmatter_done = true;
                continue;
            }
            // title: My Document
            if let Some(rest) = trimmed.strip_prefix("title:") {
                let title = rest.trim().trim_matches('"').trim_matches('\'').trim();
                if !title.is_empty() {
                    return Some(title.to_string());
                }
            }
            continue;
        }
        let _ = (frontmatter_done, first_line_was_dashes); // suppress warnings

        // ATX headings: # H1, ## H2, etc.
        if trimmed.starts_with('#') {
            let heading = trimmed.trim_start_matches('#').trim();
            if !heading.is_empty() {
                return Some(heading.to_string());
            }
        }
    }
    None
}
