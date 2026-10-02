//! HTML sanitization for untrusted message bodies.
//!
//! ADR 0002 / 0004: never hand raw HTML from IMAP to the UI renderer.
//! Configured for typical HTML email (tables, `<style>`, inline CSS, remote/data images).

use ammonia::{Builder, UrlRelative};

/// CSS properties commonly used by marketing / newsletter HTML.
const STYLE_PROPS: &[&str] = &[
    "color",
    "background",
    "background-color",
    "background-image",
    "background-size",
    "background-repeat",
    "background-position",
    "background-attachment",
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
    "flex-grow",
    "flex-shrink",
    "justify-content",
    "align-items",
    "align-self",
    "gap",
    "row-gap",
    "column-gap",
    "float",
    "clear",
    "overflow",
    "overflow-x",
    "overflow-y",
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
    // Preserve `<style>` blocks (newsletters rely on them) while still stripping scripts.
    let (styles, without_styles) = extract_style_blocks(html);

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
    if styles.is_empty() {
        cleaned_body
    } else {
        format!("{styles}{cleaned_body}")
    }
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
            // Unclosed style — drop the rest of the tag opener and continue.
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
    // Drop constructs that can pull scripts or navigate the top window.
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

/// Rewrite `cid:` image/link references to `data:` URLs using Content-ID → blob map.
pub fn rewrite_cid_urls(html: &str, cid_map: &[(String, String, Vec<u8>)]) -> String {
    if html.is_empty() || cid_map.is_empty() || !html.to_ascii_lowercase().contains("cid:") {
        return html.to_string();
    }

    let mut out = html.to_string();
    for (cid, mime, data) in cid_map {
        if data.is_empty() {
            continue;
        }
        let bare = cid.trim().trim_matches(|c| c == '<' || c == '>');
        if bare.is_empty() {
            continue;
        }
        let data_url = format!(
            "data:{};base64,{}",
            if mime.is_empty() {
                "application/octet-stream"
            } else {
                mime
            },
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data)
        );
        for candidate in [
            format!("cid:{bare}"),
            format!("cid:<{bare}>"),
            format!("CID:{bare}"),
        ] {
            if out.contains(&candidate) {
                out = out.replace(&candidate, &data_url);
            }
        }
    }
    out
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
    fn keeps_tables_styles_and_data_images() {
        let html = r#"<style>.x{color:#333;padding:12px}</style><table width="600" style="background-color:#f5f5f5"><tr><td class="x"><img src="data:image/png;base64,aaa=" width="120" alt="logo"><p>Hi</p></td></tr></table>"#;
        let clean = sanitize_html(html);
        assert!(clean.contains("<table"));
        assert!(clean.contains("data:image/png;base64,aaa="));
        assert!(clean.contains("Hi"));
        assert!(clean.contains("<style"));
        assert!(clean.contains(".x{color:#333;padding:12px}") || clean.contains("color:#333"));
    }

    #[test]
    fn rewrites_cid_to_data_url() {
        let html = r#"<img src="cid:pic@mail">"#;
        let map = vec![("pic@mail".into(), "image/png".into(), vec![0u8, 1, 2])];
        let rewritten = rewrite_cid_urls(html, &map);
        assert!(rewritten.contains("data:image/png;base64,"));
        assert!(!rewritten.contains("cid:"));
    }
}
