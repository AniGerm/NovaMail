//! Push local flag / archive / delete actions to the IMAP server.

use futures::TryStreamExt;
use novamail_crypto::SecretStore;
use novamail_db::Database;
use uuid::Uuid;

use crate::credentials::ensure_fresh_credentials;
use crate::imap_client::LiveImap;
use crate::{MailError, MailResult};

#[derive(Debug, Clone)]
pub struct ImapLocator {
    pub account_id: Uuid,
    pub mailbox_name: String,
    pub uid: u32,
}

pub async fn set_flags_remote(
    db: &Database,
    secrets: &SecretStore,
    message_id: Uuid,
    unread: Option<bool>,
    starred: Option<bool>,
) -> MailResult<()> {
    let Some(locator) = load_locator(db, message_id)? else {
        return Ok(());
    };
    let account = db.get_account(locator.account_id)?;
    let credentials = ensure_fresh_credentials(&account, secrets).await?;
    let mut imap = LiveImap::connect(&account, &credentials).await?;
    imap.select(&locator.mailbox_name).await?;
    let uid = locator.uid.to_string();
    if let Some(unread) = unread {
        let query = if unread {
            "-FLAGS (\\Seen)"
        } else {
            "+FLAGS (\\Seen)"
        };
        imap.uid_store(&uid, query).await?;
    }
    if let Some(starred) = starred {
        let query = if starred {
            "+FLAGS (\\Flagged)"
        } else {
            "-FLAGS (\\Flagged)"
        };
        imap.uid_store(&uid, query).await?;
    }
    let _ = imap.logout().await;
    Ok(())
}

pub async fn archive_remote(
    db: &Database,
    secrets: &SecretStore,
    message_id: Uuid,
) -> MailResult<()> {
    let Some(locator) = load_locator(db, message_id)? else {
        return Ok(());
    };
    let account = db.get_account(locator.account_id)?;
    let credentials = ensure_fresh_credentials(&account, secrets).await?;
    let mut imap = LiveImap::connect(&account, &credentials).await?;
    imap.select(&locator.mailbox_name).await?;
    let uid = locator.uid.to_string();

    if let Some(archive) = db.find_mailbox_by_role(locator.account_id, "archive")? {
        if archive.name != locator.mailbox_name {
            match imap.uid_move(&uid, &archive.name).await {
                Ok(()) => {
                    let _ = imap.logout().await;
                    return Ok(());
                }
                Err(err) => {
                    tracing::warn!(error = %err, "UID MOVE failed; falling back to \\Seen");
                }
            }
        }
    }

    // Fallback: mark seen on server; local archive flag still hides it in inbox.
    imap.uid_store(&uid, "+FLAGS (\\Seen)").await?;
    let _ = imap.logout().await;
    Ok(())
}

pub async fn delete_remote(
    db: &Database,
    secrets: &SecretStore,
    message_id: Uuid,
) -> MailResult<()> {
    let Some(locator) = load_locator(db, message_id)? else {
        return Ok(());
    };
    let account = db.get_account(locator.account_id)?;
    let credentials = ensure_fresh_credentials(&account, secrets).await?;
    let mut imap = LiveImap::connect(&account, &credentials).await?;
    imap.select(&locator.mailbox_name).await?;
    let uid = locator.uid.to_string();

    if let Some(trash) = db
        .find_mailbox_by_role(locator.account_id, "trash")?
        .or(db.find_mailbox_by_role(locator.account_id, "junk")?)
    {
        if trash.name != locator.mailbox_name {
            if imap.uid_move(&uid, &trash.name).await.is_ok() {
                let _ = imap.logout().await;
                return Ok(());
            }
        }
    }

    imap.uid_store(&uid, "+FLAGS (\\Deleted)").await?;
    imap.uid_expunge(&uid).await?;
    let _ = imap.logout().await;
    Ok(())
}

fn load_locator(db: &Database, message_id: Uuid) -> MailResult<Option<ImapLocator>> {
    let detail = db.get_message(message_id)?;
    let Some(uid) = db.get_message_uid(message_id)? else {
        return Ok(None);
    };
    let mailbox = db.get_mailbox(detail.summary.mailbox_id)?;
    Ok(Some(ImapLocator {
        account_id: detail.summary.account_id,
        mailbox_name: mailbox.name,
        uid,
    }))
}

/// Drain a uid_store / uid_expunge stream so the command completes.
pub(crate) async fn drain_fetches<S, T, E>(stream: S) -> MailResult<()>
where
    S: futures::Stream<Item = Result<T, E>>,
    E: std::fmt::Display,
{
    futures::pin_mut!(stream);
    while stream
        .try_next()
        .await
        .map_err(|e| MailError::Imap(e.to_string()))?
        .is_some()
    {}
    Ok(())
}
