use std::path::{Path, PathBuf};

use novamail_crypto::{AccountCredentials, SecretStore};
use novamail_db::models::{
    AttachmentRecord, MailboxRecord, MessageRecord, ThreadRecord, FLAG_SEEN, FLAG_STARRED,
};
use novamail_db::Database;
use novamail_ipc::SyncProgressEvent;
use uuid::Uuid;

use crate::imap_client::LiveImap;
use crate::parse::parse_rfc822;
use crate::{MailError, MailResult};

#[derive(Debug, Clone)]
pub struct SyncReport {
    pub account_id: Uuid,
    pub mailboxes_synced: u32,
    pub messages_fetched: u32,
}

pub struct SyncEngine {
    db: Database,
    secrets: SecretStore,
    blobs_dir: PathBuf,
}

impl SyncEngine {
    pub fn new(db: Database, secrets: SecretStore, blobs_dir: impl AsRef<Path>) -> Self {
        Self {
            db,
            secrets,
            blobs_dir: blobs_dir.as_ref().to_path_buf(),
        }
    }

    pub async fn sync_account<F>(
        &self,
        account_id: Uuid,
        mut on_progress: F,
    ) -> MailResult<SyncReport>
    where
        F: FnMut(SyncProgressEvent) + Send,
    {
        let account = self.db.get_account(account_id)?;
        let credentials = self.secrets.load_credentials(account_id)?;

        let mut imap = LiveImap::connect(&account, &credentials).await?;
        let mailboxes = imap.list_mailboxes().await?;
        let mut mailboxes_synced = 0u32;
        let mut messages_fetched = 0u32;

        for (name, role) in mailboxes {
            let should_sync = role.as_deref() == Some("inbox")
                || role.as_deref() == Some("sent")
                || name.eq_ignore_ascii_case("INBOX");
            if !should_sync {
                continue;
            }

            on_progress(SyncProgressEvent {
                account_id,
                mailbox_name: name.clone(),
                fetched: 0,
                total_estimate: None,
                done: false,
                error: None,
            });

            match self
                .sync_mailbox(&mut imap, account_id, &name, role.as_deref(), &mut on_progress)
                .await
            {
                Ok(fetched) => {
                    messages_fetched += fetched;
                    mailboxes_synced += 1;
                    on_progress(SyncProgressEvent {
                        account_id,
                        mailbox_name: name,
                        fetched,
                        total_estimate: Some(fetched),
                        done: true,
                        error: None,
                    });
                }
                Err(err) => {
                    on_progress(SyncProgressEvent {
                        account_id,
                        mailbox_name: name,
                        fetched: 0,
                        total_estimate: None,
                        done: true,
                        error: Some(err.to_string()),
                    });
                }
            }
        }

        let _ = imap.logout().await;
        Ok(SyncReport {
            account_id,
            mailboxes_synced,
            messages_fetched,
        })
    }

    async fn sync_mailbox<F>(
        &self,
        imap: &mut LiveImap,
        account_id: Uuid,
        name: &str,
        role: Option<&str>,
        on_progress: &mut F,
    ) -> MailResult<u32>
    where
        F: FnMut(SyncProgressEvent) + Send,
    {
        let (exists, uidvalidity, uidnext) = imap.select(name).await?;
        let mailbox = if let Some(existing) = self.db.find_mailbox_by_name(account_id, name)? {
            let mut updated = existing;
            updated.role = role.map(|r| r.to_string()).or(updated.role);
            updated.uidvalidity = Some(uidvalidity as i64);
            updated.uidnext = Some(uidnext as i64);
            updated.total_count = exists;
            self.db.upsert_mailbox(&updated)?;
            updated
        } else {
            let record = MailboxRecord {
                id: Uuid::new_v4(),
                account_id,
                name: name.to_string(),
                role: role.map(|r| r.to_string()),
                uidvalidity: Some(uidvalidity as i64),
                uidnext: Some(uidnext as i64),
                unread_count: 0,
                total_count: exists,
            };
            self.db.upsert_mailbox(&record)?;
            record
        };

        let window = 100u32;
        let from_uid = uidnext.saturating_sub(window).max(1);
        let fetched_msgs = imap.fetch_uid_range(from_uid, None).await?;
        let mut count = 0u32;

        for fetched in fetched_msgs {
            let parsed = parse_rfc822(&fetched.raw, fetched.flags_seen, fetched.flags_flagged)?;
            let thread_id = self.resolve_thread(account_id, &parsed)?;

            let mut flags = 0i64;
            if parsed.seen {
                flags |= FLAG_SEEN;
            }
            if parsed.starred {
                flags |= FLAG_STARRED;
            }

            let local_id = Uuid::new_v4();
            let message = MessageRecord {
                id: local_id,
                account_id,
                mailbox_id: mailbox.id,
                thread_id,
                uid: Some(fetched.uid),
                message_id: parsed.message_id.clone(),
                in_reply_to: parsed.in_reply_to.clone(),
                references: parsed.references.clone(),
                subject: parsed.subject.clone(),
                from: parsed.from.clone(),
                to: parsed.to.clone(),
                cc: parsed.cc.clone(),
                date: parsed.date,
                flags,
                snippet: parsed.snippet.clone(),
                body_text: parsed.body_text.clone(),
                body_html: parsed.body_html.clone(),
                has_attachments: parsed.has_attachments,
                raw_path: None,
            };
            self.db.insert_message(&message)?;
            let message_id = self
                .db
                .find_message_id_by_uid(mailbox.id, fetched.uid)?
                .unwrap_or(local_id);

            if !parsed.attachments.is_empty() {
                std::fs::create_dir_all(&self.blobs_dir)?;
                let mut records = Vec::new();
                for attachment in parsed.attachments {
                    let id = Uuid::new_v4();
                    let safe_name = sanitize_filename(&attachment.filename);
                    let path = self.blobs_dir.join(format!("{id}_{safe_name}"));
                    std::fs::write(&path, &attachment.data)?;
                    records.push(AttachmentRecord {
                        id,
                        message_id,
                        filename: attachment.filename,
                        mime: attachment.mime,
                        size: attachment.data.len() as u64,
                        path: path.to_string_lossy().to_string(),
                    });
                }
                self.db.replace_attachments(message_id, &records)?;
            }

            count += 1;
            if count % 10 == 0 {
                on_progress(SyncProgressEvent {
                    account_id,
                    mailbox_name: name.to_string(),
                    fetched: count,
                    total_estimate: Some(exists),
                    done: false,
                    error: None,
                });
            }
        }

        self.db.refresh_mailbox_counts(mailbox.id)?;
        Ok(count)
    }

    fn resolve_thread(
        &self,
        account_id: Uuid,
        parsed: &crate::parse::ParsedMail,
    ) -> MailResult<Uuid> {
        if let Some(in_reply_to) = &parsed.in_reply_to {
            if let Some(existing) = self.db.find_message_by_message_id(account_id, in_reply_to)? {
                let detail = self.db.get_message(existing)?;
                return Ok(detail.summary.thread_id);
            }
        }
        for reference in parsed.references.iter().rev() {
            if let Some(existing) = self.db.find_message_by_message_id(account_id, reference)? {
                let detail = self.db.get_message(existing)?;
                return Ok(detail.summary.thread_id);
            }
        }

        let thread_id = Uuid::new_v4();
        self.db.upsert_thread(&ThreadRecord {
            id: thread_id,
            account_id,
            subject: parsed.subject.clone(),
            last_message_at: parsed.date,
            message_count: 1,
            unread_count: if parsed.seen { 0 } else { 1 },
            participants: {
                let mut p = vec![parsed.from.clone()];
                p.extend(parsed.to.iter().cloned());
                p
            },
            snippet: parsed.snippet.clone(),
        })?;
        Ok(thread_id)
    }

    pub async fn test_connection(
        account: &novamail_db::AccountRecord,
        credentials: &AccountCredentials,
    ) -> MailResult<()> {
        let imap = LiveImap::connect(account, credentials).await?;
        imap.logout().await?;
        Ok(())
    }
}

fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "file".into()
    } else {
        cleaned.chars().take(120).collect()
    }
}

impl From<MailError> for String {
    fn from(value: MailError) -> Self {
        value.to_string()
    }
}
