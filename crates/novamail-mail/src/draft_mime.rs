//! Build RFC822 bytes for IMAP APPEND of drafts.

use lettre::message::header::ContentType;
use lettre::message::{Mailbox, Message, MultiPart, SinglePart};
use novamail_db::AccountRecord;
use novamail_ipc::{AddressDto, SaveDraftRequest};

use crate::{MailError, MailResult};

pub fn build_draft_rfc822(
    account: &AccountRecord,
    request: &SaveDraftRequest,
    rfc_message_id: &str,
) -> MailResult<Vec<u8>> {
    let from = Mailbox::new(
        Some(account.name.clone()),
        account
            .email
            .parse()
            .map_err(|e| MailError::Smtp(format!("invalid from address: {e}")))?,
    );

    let subject = if request.subject.trim().is_empty() {
        "(no subject)".to_string()
    } else {
        request.subject.trim().to_string()
    };

    let mut builder = Message::builder()
        .from(from)
        .subject(subject)
        .message_id(Some(rfc_message_id.to_string()));

    for addr in &request.to {
        builder = builder.to(to_mailbox(addr)?);
    }
    for addr in &request.cc {
        builder = builder.cc(to_mailbox(addr)?);
    }
    if let Some(in_reply_to) = &request.in_reply_to {
        builder = builder.in_reply_to(in_reply_to.clone());
    }
    if !request.references.is_empty() {
        builder = builder.references(request.references.join(" "));
    }

    let body = if let Some(html) = &request.body_html {
        if html.trim().is_empty() {
            MultiPart::mixed().singlepart(SinglePart::plain(request.body_text.clone()))
        } else {
            MultiPart::alternative()
                .singlepart(SinglePart::plain(request.body_text.clone()))
                .singlepart(SinglePart::html(html.clone()))
        }
    } else {
        MultiPart::mixed().singlepart(SinglePart::plain(request.body_text.clone()))
    };

    // Force a Content-Type so lettre accepts multipart as body.
    let _ = ContentType::TEXT_PLAIN;
    let email = builder
        .multipart(body)
        .map_err(|e| MailError::Smtp(e.to_string()))?;
    Ok(email.formatted())
}

fn to_mailbox(addr: &AddressDto) -> MailResult<Mailbox> {
    Ok(Mailbox::new(
        addr.name.clone(),
        addr.email
            .parse()
            .map_err(|e| MailError::Smtp(format!("invalid address {}: {e}", addr.email)))?,
    ))
}
