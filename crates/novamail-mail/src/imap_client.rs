use std::fmt::Debug;

use async_imap::types::Fetch;
use async_imap::Session;
use futures::TryStreamExt;
use novamail_crypto::{AccountCredentials, OAuthTokens};
use novamail_db::AccountRecord;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;

use crate::tls::connect_tls;
use crate::{MailError, MailResult};

pub struct ImapSession<T: AsyncRead + AsyncWrite + Unpin + Send + Debug> {
    session: Session<T>,
}

type TlsSession = Session<tokio_rustls::client::TlsStream<TcpStream>>;
type PlainSession = Session<TcpStream>;

pub enum LiveImap {
    Tls(ImapSession<tokio_rustls::client::TlsStream<TcpStream>>),
    Plain(ImapSession<TcpStream>),
}

impl LiveImap {
    pub async fn connect(
        account: &AccountRecord,
        credentials: &AccountCredentials,
    ) -> MailResult<Self> {
        if account.imap_tls {
            let session = connect_and_login_tls(account, credentials).await?;
            Ok(LiveImap::Tls(ImapSession { session }))
        } else {
            let session = connect_and_login_plain(account, credentials).await?;
            Ok(LiveImap::Plain(ImapSession { session }))
        }
    }

    pub async fn list_mailboxes(&mut self) -> MailResult<Vec<(String, Option<String>)>> {
        match self {
            LiveImap::Tls(s) => s.list_mailboxes().await,
            LiveImap::Plain(s) => s.list_mailboxes().await,
        }
    }

    pub async fn select(&mut self, mailbox: &str) -> MailResult<(u32, u32, u32)> {
        match self {
            LiveImap::Tls(s) => s.select(mailbox).await,
            LiveImap::Plain(s) => s.select(mailbox).await,
        }
    }

    pub async fn fetch_uid_range(
        &mut self,
        from_uid: u32,
        to_uid: Option<u32>,
    ) -> MailResult<Vec<FetchedMessage>> {
        match self {
            LiveImap::Tls(s) => s.fetch_uid_range(from_uid, to_uid).await,
            LiveImap::Plain(s) => s.fetch_uid_range(from_uid, to_uid).await,
        }
    }

    pub async fn uid_store(&mut self, uid: &str, query: &str) -> MailResult<()> {
        match self {
            LiveImap::Tls(s) => s.uid_store(uid, query).await,
            LiveImap::Plain(s) => s.uid_store(uid, query).await,
        }
    }

    pub async fn uid_move(&mut self, uid: &str, mailbox: &str) -> MailResult<()> {
        match self {
            LiveImap::Tls(s) => s.uid_move(uid, mailbox).await,
            LiveImap::Plain(s) => s.uid_move(uid, mailbox).await,
        }
    }

    pub async fn uid_expunge(&mut self, uid: &str) -> MailResult<()> {
        match self {
            LiveImap::Tls(s) => s.uid_expunge(uid).await,
            LiveImap::Plain(s) => s.uid_expunge(uid).await,
        }
    }

    pub async fn append(
        &mut self,
        mailbox: &str,
        flags: Option<&str>,
        content: &[u8],
    ) -> MailResult<()> {
        match self {
            LiveImap::Tls(s) => s.append(mailbox, flags, content).await,
            LiveImap::Plain(s) => s.append(mailbox, flags, content).await,
        }
    }

    pub async fn create_mailbox(&mut self, mailbox: &str) -> MailResult<()> {
        match self {
            LiveImap::Tls(s) => s.create_mailbox(mailbox).await,
            LiveImap::Plain(s) => s.create_mailbox(mailbox).await,
        }
    }

    pub async fn uid_search(&mut self, query: &str) -> MailResult<Vec<u32>> {
        match self {
            LiveImap::Tls(s) => s.uid_search(query).await,
            LiveImap::Plain(s) => s.uid_search(query).await,
        }
    }

    /// RFC 2087 STORAGE usage/limit in bytes when the server advertises QUOTA.
    pub async fn storage_quota(
        &mut self,
        mailbox: &str,
    ) -> MailResult<Option<(u64, Option<u64>)>> {
        match self {
            LiveImap::Tls(s) => s.storage_quota(mailbox).await,
            LiveImap::Plain(s) => s.storage_quota(mailbox).await,
        }
    }

    pub async fn fetch_uid(&mut self, uid: u32) -> MailResult<Option<FetchedMessage>> {
        match self {
            LiveImap::Tls(s) => s.fetch_uid(uid).await,
            LiveImap::Plain(s) => s.fetch_uid(uid).await,
        }
    }

    pub async fn logout(self) -> MailResult<()> {
        match self {
            LiveImap::Tls(s) => s.logout().await,
            LiveImap::Plain(s) => s.logout().await,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FetchedMessage {
    pub uid: u32,
    pub flags_seen: bool,
    pub flags_flagged: bool,
    pub raw: Vec<u8>,
}

impl<T: AsyncRead + AsyncWrite + Unpin + Send + Debug> ImapSession<T> {
    pub async fn list_mailboxes(&mut self) -> MailResult<Vec<(String, Option<String>)>> {
        let mut stream = self
            .session
            .list(Some(""), Some("*"))
            .await
            .map_err(|e| MailError::Imap(e.to_string()))?;
        let mut out = Vec::new();
        while let Some(name) = stream
            .try_next()
            .await
            .map_err(|e| MailError::Imap(e.to_string()))?
        {
            let role = infer_role(name.name());
            out.push((name.name().to_string(), role));
        }
        Ok(out)
    }

    /// Returns (exists, uidvalidity, uidnext)
    pub async fn select(&mut self, mailbox: &str) -> MailResult<(u32, u32, u32)> {
        let mailbox = self
            .session
            .select(mailbox)
            .await
            .map_err(|e| MailError::Imap(e.to_string()))?;
        Ok((
            mailbox.exists,
            mailbox.uid_validity.unwrap_or(0),
            mailbox.uid_next.unwrap_or(1),
        ))
    }

    pub async fn fetch_uid_range(
        &mut self,
        from_uid: u32,
        to_uid: Option<u32>,
    ) -> MailResult<Vec<FetchedMessage>> {
        let set = match to_uid {
            Some(to) if to >= from_uid => format!("{from_uid}:{to}"),
            _ => format!("{from_uid}:*"),
        };
        let mut stream = self
            .session
            .uid_fetch(&set, "(UID FLAGS BODY.PEEK[])")
            .await
            .map_err(|e| MailError::Imap(e.to_string()))?;

        let mut out = Vec::new();
        while let Some(fetch) = stream
            .try_next()
            .await
            .map_err(|e| MailError::Imap(e.to_string()))?
        {
            if let Some(msg) = map_fetch(fetch) {
                out.push(msg);
            }
        }
        Ok(out)
    }

    pub async fn uid_store(&mut self, uid: &str, query: &str) -> MailResult<()> {
        let stream = self
            .session
            .uid_store(uid, query)
            .await
            .map_err(|e| MailError::Imap(e.to_string()))?;
        crate::remote_actions::drain_fetches(stream).await
    }

    pub async fn uid_move(&mut self, uid: &str, mailbox: &str) -> MailResult<()> {
        self.session
            .uid_mv(uid, mailbox)
            .await
            .map_err(|e| MailError::Imap(e.to_string()))
    }

    pub async fn uid_expunge(&mut self, uid: &str) -> MailResult<()> {
        let stream = self
            .session
            .uid_expunge(uid)
            .await
            .map_err(|e| MailError::Imap(e.to_string()))?;
        crate::remote_actions::drain_fetches(stream).await
    }

    pub async fn append(
        &mut self,
        mailbox: &str,
        flags: Option<&str>,
        content: &[u8],
    ) -> MailResult<()> {
        self.session
            .append(mailbox, flags, None, content)
            .await
            .map_err(|e| MailError::Imap(e.to_string()))
    }

    pub async fn create_mailbox(&mut self, mailbox: &str) -> MailResult<()> {
        self.session
            .create(mailbox)
            .await
            .map_err(|e| MailError::Imap(e.to_string()))
    }

    pub async fn uid_search(&mut self, query: &str) -> MailResult<Vec<u32>> {
        let set = self
            .session
            .uid_search(query)
            .await
            .map_err(|e| MailError::Imap(e.to_string()))?;
        Ok(set.into_iter().collect())
    }

    pub async fn fetch_uid(&mut self, uid: u32) -> MailResult<Option<FetchedMessage>> {
        let mut msgs = self.fetch_uid_range(uid, Some(uid)).await?;
        Ok(msgs.pop())
    }

    /// Returns `(used_bytes, limit_bytes)` from GETQUOTAROOT STORAGE (KB → bytes).
    pub async fn storage_quota(
        &mut self,
        mailbox: &str,
    ) -> MailResult<Option<(u64, Option<u64>)>> {
        let (_roots, quotas) = match self.session.get_quota_root(mailbox).await {
            Ok(v) => v,
            Err(err) => {
                tracing::debug!(error = %err, mailbox, "GETQUOTAROOT unavailable");
                return Ok(None);
            }
        };
        for quota in quotas {
            for resource in quota.resources {
                if matches!(
                    resource.name,
                    async_imap::types::QuotaResourceName::Storage
                ) {
                    // RFC 2087: STORAGE units are 1024-octet blocks.
                    let used = resource.usage.saturating_mul(1024);
                    let limit = if resource.limit == 0 {
                        None
                    } else {
                        Some(resource.limit.saturating_mul(1024))
                    };
                    return Ok(Some((used, limit)));
                }
            }
        }
        Ok(None)
    }

    pub async fn logout(mut self) -> MailResult<()> {
        self.session
            .logout()
            .await
            .map_err(|e| MailError::Imap(e.to_string()))?;
        Ok(())
    }
}

async fn connect_and_login_tls(
    account: &AccountRecord,
    credentials: &AccountCredentials,
) -> MailResult<TlsSession> {
    let tls = connect_tls(&account.imap_host, account.imap_port).await?;
    let client = async_imap::Client::new(tls);
    login(client, account, credentials).await
}

async fn connect_and_login_plain(
    account: &AccountRecord,
    credentials: &AccountCredentials,
) -> MailResult<PlainSession> {
    let addr = format!("{}:{}", account.imap_host, account.imap_port);
    let stream = TcpStream::connect(&addr)
        .await
        .map_err(|e| MailError::Imap(format!("tcp connect {addr}: {e}")))?;
    let client = async_imap::Client::new(stream);
    login(client, account, credentials).await
}

async fn login<T: AsyncRead + AsyncWrite + Unpin + Send + Debug>(
    client: async_imap::Client<T>,
    account: &AccountRecord,
    credentials: &AccountCredentials,
) -> MailResult<Session<T>> {
    match credentials {
        AccountCredentials::Password { password } => client
            .login(&account.email, password)
            .await
            .map_err(|(e, _)| MailError::Auth(e.to_string())),
        AccountCredentials::OAuth2 { tokens } => {
            let auth = Xoauth2::new(&account.email, &tokens.access_token);
            client
                .authenticate("XOAUTH2", auth)
                .await
                .map_err(|(e, _)| MailError::Auth(e.to_string()))
        }
    }
}

struct Xoauth2 {
    user: String,
    token: String,
}

impl Xoauth2 {
    fn new(user: &str, token: &str) -> Self {
        Self {
            user: user.to_string(),
            token: token.to_string(),
        }
    }
}

impl async_imap::Authenticator for Xoauth2 {
    type Response = Vec<u8>;

    fn process(&mut self, _challenge: &[u8]) -> Self::Response {
        format!("user={}\x01auth=Bearer {}\x01\x01", self.user, self.token).into_bytes()
    }
}

fn map_fetch(fetch: Fetch) -> Option<FetchedMessage> {
    let uid = fetch.uid?;
    let flags = fetch.flags();
    let mut flags_seen = false;
    let mut flags_flagged = false;
    for flag in flags {
        match flag {
            async_imap::types::Flag::Seen => flags_seen = true,
            async_imap::types::Flag::Flagged => flags_flagged = true,
            _ => {}
        }
    }
    let raw = fetch.body()?.to_vec();
    Some(FetchedMessage {
        uid,
        flags_seen,
        flags_flagged,
        raw,
    })
}

fn infer_role(name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    if lower == "inbox" {
        Some("inbox".into())
    } else if lower.contains("sent") {
        Some("sent".into())
    } else if lower.contains("draft") || lower.contains("entwurf") {
        Some("drafts".into())
    } else if lower.contains("trash") || lower.contains("deleted") {
        Some("trash".into())
    } else if lower.contains("junk") || lower.contains("spam") {
        Some("junk".into())
    } else if lower.contains("archive") {
        Some("archive".into())
    } else {
        None
    }
}

/// Encode XOAUTH2 for SMTP / diagnostics (base64 form).
#[allow(dead_code)]
pub fn encode_xoauth2(user: &str, token: &OAuthTokens) -> String {
    use base64::Engine;
    let raw = format!(
        "user={}\x01auth=Bearer {}\x01\x01",
        user, token.access_token
    );
    base64::engine::general_purpose::STANDARD.encode(raw.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_inbox_role() {
        assert_eq!(infer_role("INBOX").as_deref(), Some("inbox"));
        assert_eq!(infer_role("[Gmail]/Sent Mail").as_deref(), Some("sent"));
    }
}
