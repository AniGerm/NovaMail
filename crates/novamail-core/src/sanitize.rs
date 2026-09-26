//! HTML sanitization for untrusted message bodies.
//!
//! ADR 0002 / 0004: never hand raw HTML from IMAP to the UI renderer.

pub fn sanitize_html(html: &str) -> String {
    ammonia::Builder::default()
        .link_rel(Some("noopener noreferrer"))
        .url_relative(ammonia::UrlRelative::Deny)
        .clean(html)
        .to_string()
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
}
