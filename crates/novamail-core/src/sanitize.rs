//! HTML sanitization for untrusted message bodies.
//!
//! ADR 0002 / 0004: never hand raw HTML from IMAP to the UI renderer.
//! Configured for typical HTML email (tables, `<style>`, inline CSS, remote/data images).

use ammonia::{Builder, UrlRelative};

/// Soft cap for HTML delivered to the UI (IPC + WebKit). Larger bodies OOM/crash.
pub const MAX_HTML_CHARS: usize = 800_000;
/// Inline `data:` images larger than this are replaced with a placeholder.
const MAX_DATA_URI_CHARS: usize = 32_768;

/// CSS properties commonly used by marketing / newsletter HTML.
const STYLE_PROPS: &[&str] = &[
    "color",
    "background",
    "background-color",
    "background-image",
    "background-size",
    "background-repeat",
    "background-position",
    "width",
    "height",
    "max-width",
    "min-width",
    "max-height",
    "min-height",
    "margin",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "padding",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "border",
    "border-top",
    "border-right",
    "border-bottom",
    "border-left",
    "border-color",
    "border-width",
    "border-style",
    "border-radius",
    "border-collapse",
    "border-spacing",
    "font",
    "font-family",
    "font-size",
    "font-weight",
    "font-style",
    "line-height",
    "letter-spacing",
    "text-align",
    "text-decoration",
    "text-transform",
    "vertical-align",
    "white-space",
    "word-break",
    "overflow-wrap",
    "display",
    "flex",
    "flex-direction",
    "flex-wrap",
    "justify-content",
    "align-items",
    "gap",
    "float",
    "clear",
    "overflow",
    "visibility",
    "opacity",
    "position",
    "top",
    "right",
    "bottom",
    "left",
    "z-index",
    "box-sizing",
    "list-style",
    "list-style-type",
    "table-layout",
    "object-fit",
];

pub fn sanitize_html(html: &str) -> String {
    // Drop giant base64 blobs first — they are the main open-message crash source.
    let html = strip_oversized_data_uris(html);
    let (styles, without_styles) = extract_style_blocks(&html);

    let mut builder = Builder::default();
    builder
        .link_rel(Some("noopener noreferrer"))
        .url_relative(UrlRelative::PassThrough)
        .add_url_schemes(["data", "cid", "http", "https", "mailto", "asset"])
        .add_tags(["center", "font"])
        .add_generic_attributes([
            "style",
            "class",
            "id",
            "align",
            "valign",
            "bgcolor",
            "background",
            "width",
            "height",
            "border",
            "cellpadding",
            "cellspacing",
            "color",
            "face",
            "size",
            "role",
            "dir",
        ])
        .add_tag_attributes("table", &["width", "height", "bgcolor", "background", "align"])
        .add_tag_attributes(
            "td",
            &[
                "width",
                "height",
                "bgcolor",
                "background",
                "valign",
                "align",
            ],
        )
        .add_tag_attributes(
            "th",
            &[
                "width",
                "height",
                "bgcolor",
                "background",
                "valign",
                "align",
            ],
        )
        .add_tag_attributes("tr", &["bgcolor", "align", "valign"])
        .add_tag_attributes(
            "img",
            &["align", "alt", "height", "src", "width", "border", "style"],
        )
        .add_tag_attributes("font", &["color", "face", "size"])
        .filter_style_properties(STYLE_PROPS.iter().copied().collect());

    let cleaned_body = builder.clean(&without_styles).to_string();
    let combined = if styles.is_empty() {
        cleaned_body
    } else {
        // Cap style blocks too — huge CSS sheets have crashed WebKit.
        let styles = if styles.len() > 100_000 {
            String::new()
        } else {
            styles
        };
        format!("{styles}{cleaned_body}")
    };
    cap_html(&combined, MAX_HTML_CHARS)
}

/// Replace oversized `data:` URIs (esp. inline images) with a tiny placeholder.
pub fn strip_oversized_data_uris(html: &str) -> String {
    let bytes = html.as_bytes();
    let lower = html.to_ascii_lowercase();
    let lower_bytes = lower.as_bytes();
    let mut out = String::with_capacity(html.len().min(MAX_HTML_CHARS));
    let mut i = 0;
    while i < bytes.len() {
        if lower_bytes.get(i..).is_some_and(|s| s.starts_with(b"data:")) {
            let start = i;
            // Scan until quote, whitespace, or `>` / `)`.
            let mut end = i + 5;
            while end < bytes.len() {
                let c = bytes[end];
                if c == b'"' || c == b'\'' || c == b' ' || c == b'\n' || c == b'\r' || c == b'\t'
                    || c == b'>' || c == b')'
                {
                    break;
                }
                end += 1;
                // Hard stop so pathological strings don't hang.
                if end - start > MAX_DATA_URI_CHARS * 4 {
                    break;
                }
            }
            let uri_len = end - start;
            if uri_len > MAX_DATA_URI_CHARS {
                out.push_str("data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7");
            } else {
                out.push_str(&html[start..end]);
            }
            i = end;
            continue;
        }
        out.push(html[i..].chars().next().unwrap_or('?'));
        i += html[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }
    out
}

fn cap_html(html: &str, max: usize) -> String {
    if html.len() <= max {
        return html.to_string();
    }
    let cut = html[..max].rfind('>').unwrap_or(max);
    let end = if cut > max / 2 { cut + 1 } else { max };
    format!(
        "{}<p style=\"margin-top:1em;color:#666\">[… HTML gekürzt …]</p>",
        &html[..end]
    )
}

/// Pull `<style>…</style>` blocks out so ammonia does not delete their CSS content.
fn extract_style_blocks(html: &str) -> (String, String) {
    let lower = html.to_ascii_lowercase();
    let mut styles = String::new();
    let mut body = String::with_capacity(html.len());
    let mut rest = html;
    let mut rest_lower = lower.as_str();

    while let Some(start) = rest_lower.find("<style") {
        body.push_str(&rest[..start]);
        let after_start = &rest[start..];
        let after_start_lower = &rest_lower[start..];
        let Some(tag_end_rel) = after_start_lower.find('>') else {
            break;
        };
        let content_start = tag_end_rel + 1;
        let search = &after_start_lower[content_start..];
        let Some(end_rel) = search.find("</style") else {
            rest = &after_start[content_start..];
            rest_lower = &after_start_lower[content_start..];
            continue;
        };
        let close_abs = content_start + end_rel;
        let css = &after_start[content_start..close_abs];
        let safe_css = scrub_style_css(css);
        if !safe_css.trim().is_empty() {
            styles.push_str("<style type=\"text/css\">");
            styles.push_str(&safe_css);
            styles.push_str("</style>");
        }
        let after_close = &after_start_lower[close_abs..];
        let close_end = after_close
            .find('>')
            .map(|i| close_abs + i + 1)
            .unwrap_or(after_start.len());
        rest = &after_start[close_end..];
        rest_lower = &after_start_lower[close_end..];
    }
    body.push_str(rest);
    (styles, body)
}

fn scrub_style_css(css: &str) -> String {
    css.lines()
        .filter(|line| {
            let t = line.trim().to_ascii_lowercase();
            !t.contains("expression(")
                && !t.contains("javascript:")
                && !t.contains("-moz-binding")
                && !t.starts_with("@import")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_scripts_and_event_handlers() {
        let dirty = r#"<p onclick="alert(1)">Hello<script>evil()</script><a href="javascript:alert(1)">x</a></p>"#;
        let clean = sanitize_html(dirty);
        assert!(clean.contains("Hello"));
        assert!(!clean.to_ascii_lowercase().contains("script"));
        assert!(!clean.contains("onclick"));
        assert!(!clean.contains("javascript:"));
    }

    #[test]
    fn keeps_tables_and_small_data_images() {
        let html = r#"<table width="600"><tr><td><img src="data:image/png;base64,aaa=" alt="logo"><p>Hi</p></td></tr></table>"#;
        let clean = sanitize_html(html);
        assert!(clean.contains("<table"));
        assert!(clean.contains("Hi"));
    }

    #[test]
    fn strips_huge_data_uris() {
        let huge = format!("data:image/png;base64,{}", "A".repeat(80_000));
        let html = format!(r#"<img src="{huge}">"#);
        let out = strip_oversized_data_uris(&html);
        assert!(!out.contains(&"A".repeat(1000)));
        assert!(out.contains("data:image/gif;base64,"));
    }

    #[test]
    fn caps_total_html_size() {
        let html = format!("<p>{}</p>", "x".repeat(MAX_HTML_CHARS + 10_000));
        let clean = sanitize_html(&html);
        assert!(clean.len() < MAX_HTML_CHARS + 200);
    }
}
