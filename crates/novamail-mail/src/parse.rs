use mail_parser::MessageParser;
use novamail_ipc::AddressDto;

use crate::{MailError, MailResult};

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
    let body_html = message.body_html(0).map(|s| s.to_string());
    let snippet_source = body_text
        .clone()
        .or_else(|| body_html.clone().map(strip_tags))
        .unwrap_or_default();
    let snippet = snippet_source.chars().take(180).collect::<String>();

    let has_attachments = message.attachment_count() > 0;

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
        has_attachments,
        seen: flags_seen,
        starred: flags_flagged,
    })
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
}
