use base64::Engine;
use mail_parser::{MessageParser, MimeHeaders, PartType};
use novamail_ipc::AddressDto;

use crate::{MailError, MailResult};

#[derive(Debug, Clone)]
pub struct ParsedAttachment {
    pub filename: String,
    pub mime: String,
    pub data: Vec<u8>,
    pub content_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedMail {
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    pub subject: String,
    pub from: AddressDto,
    pub to: Vec<AddressDto>,
    pub cc: Vec<AddressDto>,
    pub date: i64,
    pub snippet: String,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub has_attachments: bool,
    pub attachments: Vec<ParsedAttachment>,
    pub seen: bool,
    pub starred: bool,
}

pub fn parse_rfc822(raw: &[u8], flags_seen: bool, flags_flagged: bool) -> MailResult<ParsedMail> {
    let message = MessageParser::default()
        .parse(raw)
        .ok_or_else(|| MailError::Parse("unable to parse RFC822 message".into()))?;

    let subject = message.subject().unwrap_or("(no subject)").to_string();
    let from = message
        .from()
        .and_then(|list| list.first())
        .map(|addr| AddressDto {
            name: addr.name().map(|n| n.to_string()),
            email: addr.address().unwrap_or("unknown@invalid").to_string(),
        })
        .unwrap_or(AddressDto {
            name: None,
            email: "unknown@invalid".into(),
        });

    let to = message
        .to()
        .map(|list| {
            list.iter()
                .map(|addr| AddressDto {
                    name: addr.name().map(|n| n.to_string()),
                    email: addr.address().unwrap_or("unknown@invalid").to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    let cc = message
        .cc()
        .map(|list| {
            list.iter()
                .map(|addr| AddressDto {
                    name: addr.name().map(|n| n.to_string()),
                    email: addr.address().unwrap_or("unknown@invalid").to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    let date = message
        .date()
        .map(|d| d.to_timestamp())
        .unwrap_or_else(|| chrono::Utc::now().timestamp());

    let body_text = message.body_text(0).map(|s| s.to_string());
    let mut body_html = message.body_html(0).map(|s| s.to_string());

    let mut attachments = Vec::new();
    let mut cid_parts: Vec<(String, String, Vec<u8>)> = Vec::new();

    for (idx, part) in message.attachments().enumerate() {
        let filename = part
            .attachment_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("attachment-{idx}"));
        let mime = part
            .content_type()
            .map(|ct| {
                format!(
                    "{}/{}",
                    ct.c_type,
                    ct.c_subtype.as_deref().unwrap_or("octet-stream")
                )
            })
            .unwrap_or_else(|| "application/octet-stream".into());
        let data = match &part.body {
            PartType::Text(text) => text.as_bytes().to_vec(),
            PartType::Html(text) => text.as_bytes().to_vec(),
            PartType::Binary(bytes) | PartType::InlineBinary(bytes) => bytes.to_vec(),
            PartType::Message(nested) => nested.raw_message().to_vec(),
            PartType::Multipart(_) => continue,
        };
        if data.is_empty() {
            continue;
        }
        let content_id = part.content_id().map(|cid| normalize_cid(cid));
        if let Some(cid) = content_id.clone() {
            cid_parts.push((cid, mime.clone(), data.clone()));
        }
        attachments.push(ParsedAttachment {
            filename,
            mime,
            data,
            content_id,
        });
    }

    // Also collect Content-ID parts that mail-parser did not classify as attachments
    // (some clients put related images only in multipart/related).
    for part in message.parts.iter() {
        let Some(cid) = part.content_id().map(normalize_cid) else {
            continue;
        };
        if cid_parts.iter().any(|(existing, _, _)| existing == &cid) {
            continue;
        }
        let mime = part
            .content_type()
            .map(|ct| {
                format!(
                    "{}/{}",
                    ct.c_type,
                    ct.c_subtype.as_deref().unwrap_or("octet-stream")
                )
            })
            .unwrap_or_else(|| "application/octet-stream".into());
        let data = match &part.body {
            PartType::Binary(bytes) | PartType::InlineBinary(bytes) => bytes.to_vec(),
            PartType::Text(text) if mime.starts_with("image/") => text.as_bytes().to_vec(),
            _ => continue,
        };
        if data.is_empty() {
            continue;
        }
        let filename = part
            .attachment_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("inline-{}.bin", attachments.len()));
        cid_parts.push((cid.clone(), mime.clone(), data.clone()));
        attachments.push(ParsedAttachment {
            filename,
            mime,
            data,
            content_id: Some(cid),
        });
    }

    if let Some(html) = body_html.as_mut() {
        *html = rewrite_cid_to_data(html, &cid_parts);
    }

    let snippet_source = body_text
        .clone()
        .or_else(|| body_html.clone().map(strip_tags))
        .unwrap_or_default();
    let snippet = snippet_source.chars().take(180).collect::<String>();

    let message_id = message.message_id().map(|s| s.to_string());
    let in_reply_to = message.in_reply_to().as_text().map(|s| s.to_string());
    let references = message
        .references()
        .as_text_list()
        .map(|list| list.into_iter().map(|s| s.to_string()).collect())
        .unwrap_or_default();

    Ok(ParsedMail {
        message_id,
        in_reply_to,
        references,
        subject,
        from,
        to,
        cc,
        date,
        snippet,
        body_text,
        body_html,
        has_attachments: !attachments.is_empty(),
        attachments,
        seen: flags_seen,
        starred: flags_flagged,
    })
}

fn normalize_cid(cid: &str) -> String {
    cid.trim()
        .trim_matches(|c| c == '<' || c == '>')
        .to_string()
}

fn rewrite_cid_to_data(html: &str, cid_parts: &[(String, String, Vec<u8>)]) -> String {
    if cid_parts.is_empty() || !html.to_ascii_lowercase().contains("cid:") {
        return html.to_string();
    }
    let mut out = html.to_string();
    for (cid, mime, data) in cid_parts {
        if data.is_empty() || cid.is_empty() {
            continue;
        }
        let data_url = format!(
            "data:{};base64,{}",
            mime,
            base64::engine::general_purpose::STANDARD.encode(data)
        );
        for candidate in [format!("cid:{cid}"), format!("cid:<{cid}>")] {
            if out.contains(&candidate) {
                out = out.replace(&candidate, &data_url);
            }
        }
    }
    out
}

fn strip_tags(html: String) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_message() {
        let raw = b"From: Alice <alice@example.com>\r\n\
To: Bob <bob@example.com>\r\n\
Subject: Hello\r\n\
Message-ID: <abc@example.com>\r\n\
Date: Tue, 1 Jan 2024 12:00:00 +0000\r\n\
\r\n\
Hi Bob!\r\n";
        let parsed = parse_rfc822(raw, false, false).unwrap();
        assert_eq!(parsed.subject, "Hello");
        assert_eq!(parsed.from.email, "alice@example.com");
        assert!(parsed.body_text.unwrap().contains("Hi Bob"));
        assert!(!parsed.seen);
    }

    #[test]
    fn rewrites_inline_cid_images_in_html() {
        let raw = b"From: Alice <alice@example.com>\r\n\
To: Bob <bob@example.com>\r\n\
Subject: Pic\r\n\
MIME-Version: 1.0\r\n\
Content-Type: multipart/related; boundary=\"b1\"\r\n\
\r\n\
--b1\r\n\
Content-Type: text/html; charset=utf-8\r\n\
\r\n\
<html><body><img src=\"cid:img1@x\"><p>Hello</p></body></html>\r\n\
--b1\r\n\
Content-Type: image/png\r\n\
Content-ID: <img1@x>\r\n\
Content-Transfer-Encoding: base64\r\n\
Content-Disposition: inline; filename=\"dot.png\"\r\n\
\r\n\
iVBORw0KGgo=\r\n\
--b1--\r\n";
        let parsed = parse_rfc822(raw, true, false).unwrap();
        let html = parsed.body_html.expect("html body");
        assert!(html.contains("data:image/png;base64,"));
        assert!(!html.to_ascii_lowercase().contains("cid:"));
        assert!(html.contains("Hello"));
    }
}
