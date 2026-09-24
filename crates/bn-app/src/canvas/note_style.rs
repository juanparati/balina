//! Sticky-note styling and text shared by every note renderer: the canvas
//! widget ([`super::sticky`]), the context-menu swatches, the minimap, and
//! the SVG export. Pure functions, headless-testable.

/// macOS-Stickies-like palette shown in the note context menu.
pub const NOTE_PALETTE: [([u8; 3], &str); 6] = [
    ([255, 244, 165], "Yellow"),
    ([181, 220, 255], "Blue"),
    ([200, 244, 178], "Green"),
    ([255, 209, 220], "Pink"),
    ([223, 205, 255], "Purple"),
    ([230, 230, 230], "Gray"),
];

pub fn note_fill(c: [u8; 3]) -> String {
    format!("rgb({},{},{})", c[0], c[1], c[2])
}

/// The darker top drag bar: each channel × 0.86.
pub fn note_bar_color(c: [u8; 3]) -> String {
    let d = |v: u8| (v as f32 * 0.86) as u8;
    format!("rgb({},{},{})", d(c[0]), d(c[1]), d(c[2]))
}

/// Note text (CommonMark, no extensions) → HTML for the display body.
/// `to_html`'s defaults are safe (raw HTML and dangerous protocols stay
/// escaped), so the output can feed `dangerous_inner_html`. Links are
/// retargeted to a new window; the string rewrite is sound because
/// `<a href=` can only come from links the crate itself emitted.
pub fn note_html(md: &str) -> String {
    markdown::to_html(md)
        .replace("<a href=", "<a target=\"_blank\" rel=\"noopener noreferrer\" href=")
}

/// Markdown → plain text: block nodes on their own lines, all markers
/// dropped. Used where rendered HTML is impossible (the collapsed note's
/// first line, the SVG export).
pub fn note_plain_text(md: &str) -> String {
    use markdown::mdast::Node;

    fn is_block(n: &Node) -> bool {
        matches!(
            n,
            Node::Paragraph(_)
                | Node::Heading(_)
                | Node::List(_)
                | Node::ListItem(_)
                | Node::Blockquote(_)
                | Node::Code(_)
                | Node::ThematicBreak(_)
        )
    }

    fn walk(n: &Node, out: &mut String) {
        match n {
            Node::Text(t) => out.push_str(&t.value),
            Node::InlineCode(c) => out.push_str(&c.value),
            Node::Code(c) => out.push_str(&c.value),
            Node::Html(h) => out.push_str(&h.value),
            Node::Break(_) => out.push('\n'),
            _ => {}
        }
        if let Some(children) = n.children() {
            for c in children {
                walk(c, out);
                if is_block(c) && !out.ends_with('\n') {
                    out.push('\n');
                }
            }
        }
    }

    match markdown::to_mdast(md, &markdown::ParseOptions::default()) {
        Ok(root) => {
            let mut out = String::new();
            walk(&root, &mut out);
            out.truncate(out.trim_end_matches('\n').len());
            out
        }
        Err(_) => md.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_strips_markdown_markers() {
        assert_eq!(note_plain_text("# Title"), "Title");
        assert_eq!(note_plain_text("some *emphasis* and **bold**"), "some emphasis and bold");
        assert_eq!(note_plain_text("- one\n- two"), "one\ntwo");
        assert_eq!(note_plain_text("`code` here"), "code here");
        assert_eq!(note_plain_text("# H\n\npara\n\n```\nblock\n```"), "H\npara\nblock");
        assert_eq!(note_plain_text("[label](https://x.y)"), "label");
    }

    #[test]
    fn links_open_in_a_new_window() {
        let html = note_html("[x](https://example.com)");
        assert!(
            html.contains(
                "<a target=\"_blank\" rel=\"noopener noreferrer\" href=\"https://example.com\""
            ),
            "got: {html}"
        );
    }

    #[test]
    fn raw_html_stays_escaped() {
        let html = note_html("<script>alert(1)</script> and <a href=\"x\">y</a>");
        assert!(!html.contains("<script>"), "got: {html}");
        assert!(!html.contains("<a href="), "got: {html}");
    }
}
