use novamail_ipc::{ContactAddress, ContactCustomField, ContactDto};
use uuid::Uuid;

/// Serialize a contact to a vCard 3.0 document (RFC 6352 / fax MFP friendly).
pub fn contact_to_vcard(contact: &ContactDto) -> String {
    let mut lines = vec![
        "BEGIN:VCARD".to_string(),
        "VERSION:3.0".to_string(),
        format!("UID:{}", contact.id),
        format!("FN:{}", escape_text(&contact.display_name)),
        format!("N:;{};;;", escape_text(&contact.display_name)),
    ];
    if !contact.organization.trim().is_empty() {
        lines.push(format!("ORG:{}", escape_text(&contact.organization)));
    }
    if !contact.job_title.trim().is_empty() {
        lines.push(format!("TITLE:{}", escape_text(&contact.job_title)));
    }
    for email in &contact.emails {
        lines.push(format!("EMAIL;TYPE=INTERNET:{}", escape_text(email)));
    }
    for phone in &contact.phones {
        lines.push(format!("TEL;TYPE=VOICE:{}", escape_text(phone)));
    }
    for fax in &contact.faxes {
        lines.push(format!("TEL;TYPE=FAX:{}", escape_text(fax)));
    }
    for address in &contact.addresses {
        // ADR: PO Box; Extended; Street; City; Region; Postal; Country
        lines.push(format!(
            "ADR;TYPE={}:;;{};{};{};{};{}",
            escape_text(if address.label.is_empty() {
                "HOME"
            } else {
                &address.label
            }),
            escape_text(&address.street),
            escape_text(&address.city),
            escape_text(&address.region),
            escape_text(&address.postal_code),
            escape_text(&address.country),
        ));
    }
    for field in &contact.custom_fields {
        if field.label.trim().is_empty() && field.value.trim().is_empty() {
            continue;
        }
        lines.push(format!(
            "X-NOVA-CUSTOM;LABEL={}:{}",
            escape_text(&field.label),
            escape_text(&field.value)
        ));
    }
    if let Some(dn) = &contact.ldap_dn {
        if !dn.trim().is_empty() {
            lines.push(format!("X-LDAP-DN:{}", escape_text(dn)));
        }
    }
    if let Some(photo) = &contact.photo_base64 {
        if !photo.trim().is_empty() {
            lines.push(format!("PHOTO;ENCODING=b;TYPE=JPEG:{photo}"));
        }
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
    let mut faxes = Vec::new();
    let mut addresses = Vec::new();
    let mut custom_fields = Vec::new();
    let mut organization = String::new();
    let mut job_title = String::new();
    let mut photo_base64 = None;
    let mut ldap_dn = None;
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
        } else if upper.starts_with("ORG:") {
            organization = unescape_text(&line[4..]);
        } else if upper.starts_with("TITLE:") {
            job_title = unescape_text(&line[6..]);
        } else if upper.starts_with("EMAIL") {
            if let Some((_, value)) = split_prop(&line) {
                emails.push(unescape_text(value));
            }
        } else if upper.starts_with("TEL") {
            if let Some((name, value)) = split_prop(&line) {
                let name_upper = name.to_ascii_uppercase();
                if name_upper.contains("FAX") {
                    faxes.push(unescape_text(value));
                } else {
                    phones.push(unescape_text(value));
                }
            }
        } else if upper.starts_with("ADR") {
            if let Some((name, value)) = split_prop(&line) {
                let parts: Vec<String> = value.split(';').map(unescape_text).collect();
                let label = name
                    .split(';')
                    .find_map(|p| {
                        let p = p.trim();
                        p.strip_prefix("TYPE=")
                            .or_else(|| p.strip_prefix("type="))
                            .map(|v| v.to_string())
                    })
                    .unwrap_or_else(|| "HOME".into());
                addresses.push(ContactAddress {
                    label,
                    street: parts.get(2).cloned().unwrap_or_default(),
                    city: parts.get(3).cloned().unwrap_or_default(),
                    region: parts.get(4).cloned().unwrap_or_default(),
                    postal_code: parts.get(5).cloned().unwrap_or_default(),
                    country: parts.get(6).cloned().unwrap_or_default(),
                });
            }
        } else if upper.starts_with("X-NOVA-CUSTOM") {
            if let Some((name, value)) = split_prop(&line) {
                let label = name
                    .split(';')
                    .find_map(|p| {
                        p.strip_prefix("LABEL=")
                            .or_else(|| p.strip_prefix("label="))
                            .map(|v| unescape_text(v))
                    })
                    .unwrap_or_default();
                custom_fields.push(ContactCustomField {
                    label,
                    value: unescape_text(value),
                });
            }
        } else if upper.starts_with("X-LDAP-DN:") {
            ldap_dn = Some(unescape_text(&line["X-LDAP-DN:".len()..]));
        } else if upper.starts_with("PHOTO") {
            if let Some((_, value)) = split_prop(&line) {
                photo_base64 = Some(value.replace(['\r', '\n', ' '], ""));
            }
        } else if upper.starts_with("NOTE:") {
            notes = unescape_text(&line[5..]);
        } else if upper.starts_with("REV:") {
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
        faxes,
        organization,
        job_title,
        addresses,
        custom_fields,
        photo_base64,
        ldap_dn,
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
            faxes: vec!["+499999".into()],
            organization: "Nova".into(),
            job_title: "MFP".into(),
            addresses: vec![ContactAddress {
                label: "WORK".into(),
                street: "Main 1".into(),
                city: "Berlin".into(),
                region: "".into(),
                postal_code: "10115".into(),
                country: "DE".into(),
            }],
            custom_fields: vec![ContactCustomField {
                label: "Raum".into(),
                value: "2.14".into(),
            }],
            photo_base64: None,
            ldap_dn: Some("cn=fax,ou=people,dc=example,dc=com".into()),
            notes: "MFP".into(),
            updated_at: 1_700_000_000,
        };
        let vcard = contact_to_vcard(&contact);
        let parsed = vcard_to_contact(&vcard, None).unwrap();
        assert_eq!(parsed.display_name, "Fax Room");
        assert_eq!(parsed.emails, vec!["fax@example.com"]);
        assert_eq!(parsed.phones, vec!["+491234"]);
        assert_eq!(parsed.faxes, vec!["+499999"]);
        assert_eq!(parsed.organization, "Nova");
        assert_eq!(parsed.addresses[0].city, "Berlin");
        assert_eq!(parsed.custom_fields[0].value, "2.14");
        assert_eq!(
            parsed.ldap_dn.as_deref(),
            Some("cn=fax,ou=people,dc=example,dc=com")
        );
    }
}
