use novamail_ipc::ContactDto;
use uuid::Uuid;

/// Serialize a contact to a minimal vCard 3.0 document (RFC 6352 / fax MFP friendly).
pub fn contact_to_vcard(contact: &ContactDto) -> String {
    let mut lines = vec![
        "BEGIN:VCARD".to_string(),
        "VERSION:3.0".to_string(),
        format!("UID:{}", contact.id),
        format!("FN:{}", escape_text(&contact.display_name)),
        format!("N:;{};;;", escape_text(&contact.display_name)),
    ];
    for email in &contact.emails {
        lines.push(format!("EMAIL;TYPE=INTERNET:{}", escape_text(email)));
    }
    for phone in &contact.phones {
        lines.push(format!("TEL;TYPE=VOICE:{}", escape_text(phone)));
    }
    if !contact.notes.trim().is_empty() {
        lines.push(format!("NOTE:{}", escape_text(&contact.notes)));
    }
    lines.push(format!("REV:{}", format_rev(contact.updated_at)));
    lines.push("END:VCARD".to_string());
    lines.join("\r\n") + "\r\n"
}

/// Parse a simple vCard 3.0 into a contact. Unknown properties are ignored.
pub fn vcard_to_contact(raw: &str, fallback_id: Option<Uuid>) -> Option<ContactDto> {
    let mut uid = fallback_id;
    let mut display_name = String::new();
    let mut emails = Vec::new();
    let mut phones = Vec::new();
    let mut notes = String::new();
    let mut updated_at = chrono::Utc::now().timestamp();

    for line in unfold(raw) {
        let upper = line.to_ascii_uppercase();
        if upper.starts_with("UID:") {
            if let Ok(id) = Uuid::parse_str(line[4..].trim()) {
                uid = Some(id);
            }
        } else if upper.starts_with("FN:") {
            display_name = unescape_text(&line[3..]);
        } else if upper.starts_with("EMAIL") {
            if let Some((_, value)) = split_prop(&line) {
                emails.push(unescape_text(value));
            }
        } else if upper.starts_with("TEL") {
            if let Some((_, value)) = split_prop(&line) {
                phones.push(unescape_text(value));
            }
        } else if upper.starts_with("NOTE:") {
            notes = unescape_text(&line[5..]);
        } else if upper.starts_with("REV:") {
            // Accept YYYYMMDDTHHMMSSZ loosely; keep now on parse failure.
            if let Ok(ts) = parse_rev(line[4..].trim()) {
                updated_at = ts;
            }
        }
    }

    if display_name.is_empty() && emails.is_empty() {
        return None;
    }
    if display_name.is_empty() {
        display_name = emails
            .first()
            .cloned()
            .unwrap_or_else(|| "Unnamed".into());
    }

    Some(ContactDto {
        id: uid.unwrap_or_else(Uuid::new_v4),
        display_name,
        emails,
        phones,
        notes,
        updated_at,
    })
}

fn unfold(raw: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in raw.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(last) = out.last_mut() {
                last.push_str(line.trim_start());
                continue;
            }
        }
        out.push(line.trim_end_matches('\r').to_string());
    }
    out
}

fn split_prop(line: &str) -> Option<(&str, &str)> {
    let idx = line.find(':')?;
    Some((&line[..idx], &line[idx + 1..]))
}

fn escape_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(',', "\\,")
        .replace(';', "\\;")
        .replace('\n', "\\n")
}

fn unescape_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') | Some('N') => out.push('\n'),
                Some(',') => out.push(','),
                Some(';') => out.push(';'),
                Some('\\') => out.push('\\'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn format_rev(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.format("%Y%m%dT%H%M%SZ").to_string())
        .unwrap_or_else(|| "19700101T000000Z".into())
}

fn parse_rev(value: &str) -> Result<i64, ()> {
    chrono::NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%SZ")
        .map(|dt| dt.and_utc().timestamp())
        .map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_vcard() {
        let contact = ContactDto {
            id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            display_name: "Fax Room".into(),
            emails: vec!["fax@example.com".into()],
            phones: vec!["+491234".into()],
            notes: "MFP".into(),
            updated_at: 1_700_000_000,
        };
        let vcard = contact_to_vcard(&contact);
        let parsed = vcard_to_contact(&vcard, None).unwrap();
        assert_eq!(parsed.display_name, "Fax Room");
        assert_eq!(parsed.emails, vec!["fax@example.com"]);
        assert_eq!(parsed.phones, vec!["+491234"]);
    }
}
