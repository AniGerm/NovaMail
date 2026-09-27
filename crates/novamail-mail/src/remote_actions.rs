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

/// Hard-purge a message from IMAP while keeping the local SQLite/blob copy.
///
/// Safety: refuses when the local body (and attachment files) are incomplete.
/// Does **not** move to Trash — that would still consume server quota.
pub async fn offload_message_remote(
    db: &Database,
    secrets: &SecretStore,
    blobs_dir: &std::path::Path,
    message_id: Uuid,
) -> MailResult<u64> {
    let detail = db.get_message(message_id)?;
    if detail.summary.local_only {
        return Ok(0);
    }

    let Some(locator) = load_locator(db, message_id)? else {
        return Err(MailError::Other(
            "cannot offload: message has no IMAP UID".into(),
        ));
    };

    // Ensure a complete local copy before touching the server.
    if !db.message_has_complete_local_copy(message_id)? {
        ensure_full_local_copy(db, secrets, blobs_dir, message_id, &locator).await?;
        if !db.message_has_complete_local_copy(message_id)? {
            return Err(MailError::Other(
                "refusing IMAP purge: local copy incomplete".into(),
            ));
        }
    }

    let size = db
        .message_size_bytes(message_id)?
        .unwrap_or_else(|| {
            let body = detail.body_text.as_deref().unwrap_or("").len()
                + detail.body_html.as_deref().unwrap_or("").len();
            body as i64
        })
        .max(0) as u64;

    let account = db.get_account(locator.account_id)?;
    let credentials = ensure_fresh_credentials(&account, secrets).await?;
    let mut imap = LiveImap::connect(&account, &credentials).await?;
    imap.select(&locator.mailbox_name).await?;
    let uid = locator.uid.to_string();
    imap.uid_store(&uid, "+FLAGS (\\Deleted)").await?;
    imap.uid_expunge(&uid).await?;
    let _ = imap.logout().await;

    let now = chrono::Utc::now().timestamp();
    db.mark_message_local_only(message_id, Some(size as i64), now)?;
    Ok(size)
}

async fn ensure_full_local_copy(
    db: &Database,
    secrets: &SecretStore,
    blobs_dir: &std::path::Path,
    message_id: Uuid,
    locator: &ImapLocator,
) -> MailResult<()> {
    let account = db.get_account(locator.account_id)?;
    let credentials = ensure_fresh_credentials(&account, secrets).await?;
    let mut imap = LiveImap::connect(&account, &credentials).await?;
    imap.select(&locator.mailbox_name).await?;
    let Some(fetched) = imap.fetch_uid(locator.uid).await? else {
        let _ = imap.logout().await;
        return Err(MailError::Other(format!(
            "UID {} missing on server; cannot complete local copy",
            locator.uid
        )));
    };
    let parsed = crate::parse::parse_rfc822(&fetched.raw, fetched.flags_seen, fetched.flags_flagged)?;
    // Re-insert preserves local_only when set; here we refresh body/size.
    let existing = db.get_message(message_id)?;
    let record = novamail_db::models::MessageRecord {
        id: message_id,
        account_id: existing.summary.account_id,
        mailbox_id: existing.summary.mailbox_id,
        thread_id: existing.summary.thread_id,
        uid: Some(fetched.uid),
        message_id: parsed.message_id.or(existing.message_id),
        in_reply_to: parsed.in_reply_to.or(existing.in_reply_to),
        references: if parsed.references.is_empty() {
            existing.references
        } else {
            parsed.references
        },
        subject: parsed.subject,
        from: parsed.from,
        to: parsed.to,
        cc: parsed.cc,
        date: existing.summary.date,
        flags: {
            let mut flags = 0i64;
            if !existing.summary.unread {
                flags |= novamail_db::models::FLAG_SEEN;
            }
            if existing.summary.starred {
                flags |= novamail_db::models::FLAG_STARRED;
            }
            flags
        },
        snippet: parsed.snippet,
        body_text: parsed.body_text,
        body_html: parsed.body_html,
        has_attachments: parsed.has_attachments,
        raw_path: None,
        local_only: false,
        offline_at: None,
        size_bytes: Some(fetched.raw.len() as i64),
    };
    db.insert_message(&record)?;
    if !parsed.attachments.is_empty() {
        std::fs::create_dir_all(blobs_dir)?;
        let mut records = Vec::new();
        for attachment in parsed.attachments {
            let id = Uuid::new_v4();
            let safe_name: String = attachment
                .filename
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .take(120)
                .collect();
            let safe_name = if safe_name.is_empty() {
                "file".into()
            } else {
                safe_name
            };
            let path = blobs_dir.join(format!("{id}_{safe_name}"));
            std::fs::write(&path, &attachment.data)?;
            records.push(novamail_db::models::AttachmentRecord {
                id,
                message_id,
                filename: attachment.filename,
                mime: attachment.mime,
                size: attachment.data.len() as u64,
                path: path.to_string_lossy().to_string(),
            });
        }
        db.replace_attachments(message_id, &records)?;
    }
    let _ = imap.logout().await;
    Ok(())
}

/// Probe IMAP QUOTA for an account; falls back to a local size estimate.
pub async fn probe_account_quota(
    db: &Database,
    secrets: &SecretStore,
    account_id: Uuid,
) -> MailResult<novamail_ipc::AccountQuotaDto> {
    use novamail_ipc::{AccountQuotaDto, QuotaSource};

    let estimated = db.estimate_account_storage_bytes(account_id)?;
    let account = db.get_account(account_id)?;
    if account.imap_host.trim().is_empty() {
        return Ok(AccountQuotaDto {
            account_id,
            used_bytes: estimated,
            limit_bytes: None,
            percent: None,
            source: QuotaSource::Estimate,
        });
    }

    let credentials = ensure_fresh_credentials(&account, secrets).await?;
    let mut imap = LiveImap::connect(&account, &credentials).await?;
    let server = imap.storage_quota("INBOX").await?;
    let _ = imap.logout().await;

    if let Some((used, limit)) = server {
        let percent = limit.map(|lim| {
            if lim == 0 {
                0.0
            } else {
                (used as f64 / lim as f64 * 100.0) as f32
            }
        });
        return Ok(AccountQuotaDto {
            account_id,
            used_bytes: used,
            limit_bytes: limit,
            percent,
            source: QuotaSource::Server,
        });
    }

    Ok(AccountQuotaDto {
        account_id,
        used_bytes: estimated,
        limit_bytes: None,
        percent: None,
        source: QuotaSource::Estimate,
    })
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
