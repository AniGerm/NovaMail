//! HTML sanitization for untrusted message bodies.
//!
//! ADR 0002 / 0004: never hand raw HTML from IMAP to the UI renderer.
//! Configured for typical HTML email (tables, inline styles, remote/data images).

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
    "display",
    "float",
    "clear",
    "overflow",
    "visibility",
    "opacity",
    "list-style",
    "list-style-type",
    "table-layout",
];

pub fn sanitize_html(html: &str) -> String {
    let mut builder = Builder::default();
    builder
        .link_rel(Some("noopener noreferrer"))
        // Keep relative URLs so newsletter layouts are not stripped to bare text.
        .url_relative(UrlRelative::PassThrough)
        .add_url_schemes(["data", "cid", "http", "https", "mailto"])
        .add_tags(["center", "font"])
        .add_generic_attributes([
            "style",
            "class",
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
    builder.clean(html).to_string()
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
        let html = r#"<table width="600" style="background-color:#f5f5f5"><tr><td style="padding:12px;color:#333"><img src="data:image/png;base64,aaa=" width="120" alt="logo"><p>Hi</p></td></tr></table>"#;
        let clean = sanitize_html(html);
        assert!(clean.contains("<table"));
        assert!(clean.contains("data:image/png;base64,aaa="));
        assert!(clean.contains("Hi"));
        assert!(clean.contains("padding"));
    }

    #[test]
    fn rewrites_cid_to_data_url() {
        let html = r#"<img src="cid:pic@mail">"#;
        let map = vec![(
            "pic@mail".into(),
            "image/png".into(),
            vec![0u8, 1, 2],
        )];
        let rewritten = rewrite_cid_urls(html, &map);
        assert!(rewritten.contains("data:image/png;base64,"));
        assert!(!rewritten.contains("cid:"));
    }
}
