//! Push local flag / archive / delete / draft actions to the IMAP server.

use futures::TryStreamExt;
use novamail_crypto::SecretStore;
use novamail_db::{AccountRecord, Database};
use novamail_ipc::SaveDraftRequest;
use uuid::Uuid;

use crate::credentials::ensure_fresh_credentials;
use crate::draft_mime::build_draft_rfc822;
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

/// Move a message into a target IMAP mailbox (by folder name), updating the local row.
pub async fn move_remote(
    db: &Database,
    secrets: &SecretStore,
    message_id: Uuid,
    target_mailbox_name: &str,
    target_role: Option<&str>,
) -> MailResult<()> {
    let Some(locator) = load_locator(db, message_id)? else {
        // Local-only message: just reassign mailbox when possible.
        if let Ok(detail) = db.get_message(message_id) {
            let mb = db.ensure_mailbox(
                detail.summary.account_id,
                target_mailbox_name,
                target_role.unwrap_or("archive"),
            )?;
            db.set_message_mailbox(message_id, mb.id)?;
            let _ = db.refresh_mailbox_counts(mb.id);
            let _ = db.refresh_mailbox_counts(detail.summary.mailbox_id);
        }
        return Ok(());
    };
    if locator.mailbox_name.eq_ignore_ascii_case(target_mailbox_name) {
        return Ok(());
    }

    let account = db.get_account(locator.account_id)?;
    let credentials = ensure_fresh_credentials(&account, secrets).await?;
    let mut imap = LiveImap::connect(&account, &credentials).await?;
    imap.select(&locator.mailbox_name).await?;
    let uid = locator.uid.to_string();

    // Ensure the destination exists on the server when missing.
    let listed = imap.list_mailboxes().await.unwrap_or_default();
    let dest_exists = listed
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case(target_mailbox_name));
    if !dest_exists {
        if let Err(err) = imap.create_mailbox(target_mailbox_name).await {
            tracing::warn!(error = %err, mailbox = %target_mailbox_name, "CREATE mailbox failed");
        }
    }

    imap.uid_move(&uid, target_mailbox_name).await?;
    let role = target_role.unwrap_or("archive");
    let mb = db.ensure_mailbox(locator.account_id, target_mailbox_name, role)?;
    db.set_message_mailbox(message_id, mb.id)?;
    // UID becomes invalid after MOVE; clear until next sync.
    let _ = db.set_message_uid(message_id, None);
    let _ = db.refresh_mailbox_counts(mb.id);
    if let Ok(Some(old)) = db.find_mailbox_by_name(locator.account_id, &locator.mailbox_name) {
        let _ = db.refresh_mailbox_counts(old.id);
    }
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

/// Append a draft to the account's IMAP Drafts folder (`\Draft \Seen`).
///
/// Returns the server UID when discoverable via Message-ID search.
pub async fn save_draft_remote(
    db: &Database,
    secrets: &SecretStore,
    account: &AccountRecord,
    request: &SaveDraftRequest,
    local_message_id: Uuid,
    rfc_message_id: &str,
) -> MailResult<Option<u32>> {
    let credentials = ensure_fresh_credentials(account, secrets).await?;
    let mut imap = LiveImap::connect(account, &credentials).await?;

    // Replace previous server copy when updating an already-synced draft.
    if let Some(existing_id) = request.id {
        if let Some(locator) = load_locator(db, existing_id)? {
            if let Err(err) = imap.select(&locator.mailbox_name).await {
                tracing::warn!(error = %err, "could not select old draft mailbox");
            } else {
                let uid = locator.uid.to_string();
                let _ = imap.uid_store(&uid, "+FLAGS (\\Deleted)").await;
                let _ = imap.uid_expunge(&uid).await;
            }
        }
    }

    let drafts_name = resolve_drafts_mailbox(&mut imap, db, account.id).await?;
    let raw = build_draft_rfc822(account, request, rfc_message_id)?;
    imap.append(&drafts_name, Some(r"(\Draft \Seen)"), &raw)
        .await?;

    // Best-effort: resolve the new UID so later edits/deletes hit the server copy.
    let mut new_uid = None;
    if imap.select(&drafts_name).await.is_ok() {
        let bare = rfc_message_id.trim_matches(['<', '>']);
        let query = format!("HEADER Message-ID {bare}");
        if let Ok(uids) = imap.uid_search(&query).await {
            new_uid = uids.into_iter().max();
        }
        if new_uid.is_none() {
            let query = format!("HEADER Message-ID <{bare}>");
            if let Ok(uids) = imap.uid_search(&query).await {
                new_uid = uids.into_iter().max();
            }
        }
    }

    // Keep local row in the IMAP Drafts mailbox with UID + Message-ID.
    if let Ok(mb) = db.ensure_mailbox(account.id, &drafts_name, "drafts") {
        let _ = db.set_message_mailbox(local_message_id, mb.id);
        let _ = db.set_message_uid(local_message_id, new_uid);
        let _ = db.set_message_rfc_id(local_message_id, rfc_message_id);
        let _ = db.refresh_mailbox_counts(mb.id);
    }

    let _ = imap.logout().await;
    Ok(new_uid)
}

async fn resolve_drafts_mailbox(
    imap: &mut LiveImap,
    db: &Database,
    account_id: Uuid,
) -> MailResult<String> {
    // Prefer the live IMAP LIST so we hit Entwürfe / INBOX.Drafts / etc.
    if let Ok(listed) = imap.list_mailboxes().await {
        for (name, role) in &listed {
            if role.as_deref() == Some("drafts") {
                let _ = db.ensure_mailbox(account_id, name, "drafts");
                return Ok(name.clone());
            }
        }
    }

    if let Some(existing) = db.find_mailbox_by_role(account_id, "drafts")? {
        return Ok(existing.name);
    }

    // Create a standard Drafts folder when the provider has none yet.
    if let Err(err) = imap.create_mailbox("Drafts").await {
        tracing::warn!(error = %err, "CREATE Drafts failed; trying APPEND anyway");
    }
    let _ = db.ensure_mailbox(account_id, "Drafts", "drafts");
    Ok("Drafts".into())
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
