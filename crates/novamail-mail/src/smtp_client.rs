use base64::Engine;
use lettre::message::header::ContentType;
use lettre::message::{Attachment, Body, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::{Credentials, Mechanism};
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use novamail_crypto::AccountCredentials;
use novamail_db::AccountRecord;
use novamail_ipc::{AddressDto, SendMessageRequest};

use crate::{MailError, MailResult};

pub struct SmtpClient;

impl SmtpClient {
    pub async fn send(
        account: &AccountRecord,
        credentials: &AccountCredentials,
        request: &SendMessageRequest,
    ) -> MailResult<()> {
        let from = Mailbox::new(
            Some(account.name.clone()),
            account
                .email
                .parse()
                .map_err(|e| MailError::Smtp(format!("invalid from address: {e}")))?,
        );

        let mut builder = Message::builder()
            .from(from)
            .subject(request.subject.clone());

        for addr in &request.to {
            builder = builder.to(to_mailbox(addr)?);
        }
        for addr in &request.cc {
            builder = builder.cc(to_mailbox(addr)?);
        }
        for addr in &request.bcc {
            builder = builder.bcc(to_mailbox(addr)?);
        }
        if let Some(in_reply_to) = &request.in_reply_to {
            builder = builder.in_reply_to(in_reply_to.clone());
        }
        if !request.references.is_empty() {
            builder = builder.references(request.references.join(" "));
        }

        let body_part = if let Some(html) = &request.body_html {
            MultiPart::alternative()
                .singlepart(SinglePart::plain(request.body_text.clone()))
                .singlepart(SinglePart::html(html.clone()))
        } else {
            MultiPart::mixed().singlepart(SinglePart::plain(request.body_text.clone()))
        };

        let mut mixed = MultiPart::mixed().multipart(body_part);
        for attachment in &request.attachments {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(attachment.data_base64.as_bytes())
                .map_err(|e| MailError::Smtp(format!("invalid attachment encoding: {e}")))?;
            let content_type = ContentType::parse(&attachment.mime)
                .unwrap_or_else(|_| ContentType::parse("application/octet-stream").unwrap());
            let body = Body::new(bytes);
            mixed = mixed.singlepart(
                Attachment::new(attachment.filename.clone()).body(body, content_type),
            );
        }

        let email = builder
            .multipart(mixed)
            .map_err(|e| MailError::Smtp(e.to_string()))?;

        let transport = build_transport(account, credentials)?;
        transport
            .send(email)
            .await
            .map_err(|e| MailError::Smtp(e.to_string()))?;
        Ok(())
    }
}

fn to_mailbox(addr: &AddressDto) -> MailResult<Mailbox> {
    Ok(Mailbox::new(
        addr.name.clone(),
        addr.email
            .parse()
            .map_err(|e| MailError::Smtp(format!("invalid address {}: {e}", addr.email)))?,
    ))
}

fn build_transport(
    account: &AccountRecord,
    credentials: &AccountCredentials,
) -> MailResult<AsyncSmtpTransport<Tokio1Executor>> {
    let mut builder = if account.smtp_tls && account.smtp_port == 465 {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&account.smtp_host)
            .map_err(|e| MailError::Smtp(e.to_string()))?
            .port(account.smtp_port)
            .tls(Tls::Wrapper(
                TlsParameters::new(account.smtp_host.clone())
                    .map_err(|e| MailError::Tls(e.to_string()))?,
            ))
    } else if account.smtp_tls {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&account.smtp_host)
            .map_err(|e| MailError::Smtp(e.to_string()))?
            .port(account.smtp_port)
            .tls(Tls::Required(
                TlsParameters::new(account.smtp_host.clone())
                    .map_err(|e| MailError::Tls(e.to_string()))?,
            ))
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&account.smtp_host)
            .port(account.smtp_port)
            .tls(Tls::None)
    };

    match credentials {
        AccountCredentials::Password { password } => {
            builder = builder.credentials(Credentials::new(
                account.email.clone(),
                password.clone(),
            ));
        }
        AccountCredentials::OAuth2 { tokens } => {
            builder = builder
                .authentication(vec![Mechanism::Xoauth2])
                .credentials(Credentials::new(
                    account.email.clone(),
                    tokens.access_token.clone(),
                ));
        }
    }

    Ok(builder.build())
}
