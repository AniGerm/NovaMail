use std::path::{Path, PathBuf};
use std::sync::Arc;

use novamail_ipc::{
    AccountDto, AddressDto, AttachmentDto, AuthType, CalendarAccountDto, CalendarAttendeeDto,
    CalendarCollectionDto, CalendarEventDto, CalendarInvitationDto, CalendarReminderDto,
    CalendarTaskDto, ContactAddress, ContactCustomField, ContactDto, LabelDto, ListMessagesRequest,
    ListThreadsResponse, MailProvider, MailboxDto, MessageDetailDto, MessageSortBy,
    MessageSummaryDto, OutboundQueueItemDto, OutboundStatus, PgpKeyDto, RecipientSuggestion,
    RuleDto, SignatureDto, SnoozedMessageDto, SortDirection, ThreadListItemDto,
};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::migrations;
use crate::models::{
    AccountRecord, AttachmentRecord, ContactRecord, LabelRecord, MailboxRecord, MessageRecord,
    RuleRecord, SignatureRecord, ThreadRecord, FLAG_ARCHIVED, FLAG_SEEN, FLAG_STARRED,
};
use crate::{DbError, DbResult};

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
    path: PathBuf,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> DbResult<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        migrations::migrate(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            path,
        })
    }

    pub fn open_in_memory() -> DbResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        migrations::migrate(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            path: PathBuf::from(":memory:"),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn insert_account(&self, account: &AccountRecord) -> DbResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            r#"
            INSERT INTO accounts (
              id, name, label, email, provider, auth_type,
              imap_host, imap_port, imap_tls,
              smtp_host, smtp_port, smtp_tls, created_at
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
            "#,
            params![
                account.id.to_string(),
                account.name,
                account.label,
                account.email,
                provider_to_str(&account.provider),
                auth_to_str(&account.auth_type),
                account.imap_host,
                account.imap_port as i64,
                account.imap_tls as i64,
                account.smtp_host,
                account.smtp_port as i64,
                account.smtp_tls as i64,
                account.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_accounts(&self) -> DbResult<Vec<AccountDto>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, label, email, provider, auth_type,
                   imap_host, imap_port, imap_tls,
                   smtp_host, smtp_port, smtp_tls, created_at
            FROM accounts
            ORDER BY created_at ASC
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(AccountDto {
                id: parse_uuid(row.get::<_, String>(0)?)?,
                name: row.get(1)?,
                label: row.get(2)?,
                email: row.get(3)?,
                provider: parse_provider(&row.get::<_, String>(4)?),
                auth_type: parse_auth(&row.get::<_, String>(5)?),
                imap_host: row.get(6)?,
                imap_port: row.get::<_, i64>(7)? as u16,
                imap_tls: row.get::<_, i64>(8)? != 0,
                smtp_host: row.get(9)?,
                smtp_port: row.get::<_, i64>(10)? as u16,
                smtp_tls: row.get::<_, i64>(11)? != 0,
                created_at: row.get(12)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| DbError::Sqlite(e))?);
        }
        Ok(out)
    }

    pub fn get_account(&self, id: Uuid) -> DbResult<AccountRecord> {
        let conn = self.conn.lock();
        conn.query_row(
            r#"
            SELECT id, name, label, email, provider, auth_type,
                   imap_host, imap_port, imap_tls,
                   smtp_host, smtp_port, smtp_tls, created_at
            FROM accounts WHERE id = ?1
            "#,
            params![id.to_string()],
            |row| {
                Ok(AccountRecord {
                    id: parse_uuid(row.get::<_, String>(0)?)?,
                    name: row.get(1)?,
                    label: row.get(2)?,
                    email: row.get(3)?,
                    provider: parse_provider(&row.get::<_, String>(4)?),
                    auth_type: parse_auth(&row.get::<_, String>(5)?),
                    imap_host: row.get(6)?,
                    imap_port: row.get::<_, i64>(7)? as u16,
                    imap_tls: row.get::<_, i64>(8)? != 0,
                    smtp_host: row.get(9)?,
                    smtp_port: row.get::<_, i64>(10)? as u16,
                    smtp_tls: row.get::<_, i64>(11)? != 0,
                    created_at: row.get(12)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| DbError::NotFound(format!("account {id}")))
    }

    pub fn delete_account(&self, id: Uuid) -> DbResult<()> {
        let conn = self.conn.lock();
        let changed = conn.execute("DELETE FROM accounts WHERE id = ?1", params![id.to_string()])?;
        if changed == 0 {
            return Err(DbError::NotFound(format!("account {id}")));
        }
        Ok(())
    }

    pub fn find_account_by_email(&self, email: &str) -> DbResult<Option<AccountRecord>> {
        let conn = self.conn.lock();
        conn.query_row(
            r#"
            SELECT id, name, label, email, provider, auth_type,
                   imap_host, imap_port, imap_tls,
                   smtp_host, smtp_port, smtp_tls, created_at
            FROM accounts WHERE lower(email) = lower(?1)
            "#,
            params![email],
            |row| {
                Ok(AccountRecord {
                    id: parse_uuid(row.get::<_, String>(0)?)?,
                    name: row.get(1)?,
                    label: row.get(2)?,
                    email: row.get(3)?,
                    provider: parse_provider(&row.get::<_, String>(4)?),
                    auth_type: parse_auth(&row.get::<_, String>(5)?),
                    imap_host: row.get(6)?,
                    imap_port: row.get::<_, i64>(7)? as u16,
                    imap_tls: row.get::<_, i64>(8)? != 0,
                    smtp_host: row.get(9)?,
                    smtp_port: row.get::<_, i64>(10)? as u16,
                    smtp_tls: row.get::<_, i64>(11)? != 0,
                    created_at: row.get(12)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn update_account(&self, account: &AccountRecord) -> DbResult<()> {
        let conn = self.conn.lock();
        let changed = conn.execute(
            r#"
            UPDATE accounts SET
              name = ?2,
              label = ?3,
              email = ?4,
              provider = ?5,
              auth_type = ?6,
              imap_host = ?7,
              imap_port = ?8,
              imap_tls = ?9,
              smtp_host = ?10,
              smtp_port = ?11,
              smtp_tls = ?12
            WHERE id = ?1
            "#,
            params![
                account.id.to_string(),
                account.name,
                account.label,
                account.email,
                provider_to_str(&account.provider),
                auth_to_str(&account.auth_type),
                account.imap_host,
                account.imap_port as i64,
                account.imap_tls as i64,
                account.smtp_host,
                account.smtp_port as i64,
                account.smtp_tls as i64,
            ],
        )?;
        if changed == 0 {
            return Err(DbError::NotFound(format!("account {}", account.id)));
        }
        Ok(())
    }

    pub fn upsert_mailbox(&self, mailbox: &MailboxRecord) -> DbResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            r#"
            INSERT INTO mailboxes (
              id, account_id, name, role, uidvalidity, uidnext, unread_count, total_count
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
            ON CONFLICT(account_id, name) DO UPDATE SET
              role = excluded.role,
              uidvalidity = excluded.uidvalidity,
              uidnext = excluded.uidnext,
              unread_count = excluded.unread_count,
              total_count = excluded.total_count
            "#,
            params![
                mailbox.id.to_string(),
                mailbox.account_id.to_string(),
                mailbox.name,
                mailbox.role,
                mailbox.uidvalidity,
                mailbox.uidnext,
                mailbox.unread_count as i64,
                mailbox.total_count as i64,
            ],
        )?;
        Ok(())
    }

    pub fn find_mailbox_by_name(
        &self,
        account_id: Uuid,
        name: &str,
    ) -> DbResult<Option<MailboxRecord>> {
        let conn = self.conn.lock();
        conn.query_row(
            r#"
            SELECT id, account_id, name, role, uidvalidity, uidnext, unread_count, total_count
            FROM mailboxes WHERE account_id = ?1 AND name = ?2
            "#,
            params![account_id.to_string(), name],
            map_mailbox_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn get_mailbox(&self, mailbox_id: Uuid) -> DbResult<MailboxRecord> {
        let conn = self.conn.lock();
        conn.query_row(
            r#"
            SELECT id, account_id, name, role, uidvalidity, uidnext, unread_count, total_count
            FROM mailboxes WHERE id = ?1
            "#,
            params![mailbox_id.to_string()],
            map_mailbox_row,
        )
        .optional()?
        .ok_or_else(|| DbError::NotFound(format!("mailbox {mailbox_id}")))
    }

    /// Ensure a local mailbox exists (e.g. Drafts) and return it.
    ///
    /// When a mailbox already exists for `role` but under a different name,
    /// prefers the provided `name` (typically the IMAP server folder name).
    pub fn ensure_mailbox(
        &self,
        account_id: Uuid,
        name: &str,
        role: &str,
    ) -> DbResult<MailboxRecord> {
        if let Some(existing) = self.find_mailbox_by_name(account_id, name)? {
            if existing.role.as_deref() != Some(role) {
                let conn = self.conn.lock();
                conn.execute(
                    "UPDATE mailboxes SET role = ?2 WHERE id = ?1",
                    params![existing.id.to_string(), role],
                )?;
                drop(conn);
                return self.get_mailbox(existing.id);
            }
            return Ok(existing);
        }
        if let Some(existing) = self.find_mailbox_by_role(account_id, role)? {
            if existing.name != name {
                let conn = self.conn.lock();
                conn.execute(
                    "UPDATE mailboxes SET name = ?2 WHERE id = ?1",
                    params![existing.id.to_string(), name],
                )?;
                drop(conn);
                return self.get_mailbox(existing.id);
            }
            return Ok(existing);
        }
        let record = MailboxRecord {
            id: Uuid::new_v4(),
            account_id,
            name: name.to_string(),
            role: Some(role.to_string()),
            uidvalidity: None,
            uidnext: None,
            unread_count: 0,
            total_count: 0,
        };
        self.upsert_mailbox(&record)?;
        Ok(record)
    }

    pub fn set_message_mailbox(&self, message_id: Uuid, mailbox_id: Uuid) -> DbResult<()> {
        let conn = self.conn.lock();
        let changed = conn.execute(
            "UPDATE messages SET mailbox_id = ?2 WHERE id = ?1",
            params![message_id.to_string(), mailbox_id.to_string()],
        )?;
        if changed == 0 {
            return Err(DbError::NotFound(format!("message {message_id}")));
        }
        Ok(())
    }

    pub fn update_message_draft(
        &self,
        message_id: Uuid,
        subject: &str,
        to: &[AddressDto],
        cc: &[AddressDto],
        body_text: &str,
        body_html: Option<&str>,
        date: i64,
        snippet: &str,
    ) -> DbResult<()> {
        let conn = self.conn.lock();
        let to_json = serde_json::to_string(to)?;
        let cc_json = serde_json::to_string(cc)?;
        let changed = conn.execute(
            r#"
            UPDATE messages SET
              subject = ?2,
              to_json = ?3,
              cc_json = ?4,
              body_text = ?5,
              body_html = ?6,
              date = ?7,
              snippet = ?8
            WHERE id = ?1
            "#,
            params![
                message_id.to_string(),
                subject,
                to_json,
                cc_json,
                body_text,
                body_html,
                date,
                snippet,
            ],
        )?;
        if changed == 0 {
            return Err(DbError::NotFound(format!("message {message_id}")));
        }
        Ok(())
    }

    pub fn find_mailbox_by_role(
        &self,
        account_id: Uuid,
        role: &str,
    ) -> DbResult<Option<MailboxRecord>> {
        let conn = self.conn.lock();
        conn.query_row(
            r#"
            SELECT id, account_id, name, role, uidvalidity, uidnext, unread_count, total_count
            FROM mailboxes
            WHERE account_id = ?1 AND lower(coalesce(role, '')) = lower(?2)
            LIMIT 1
            "#,
            params![account_id.to_string(), role],
            map_mailbox_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn set_message_uid(&self, message_id: Uuid, uid: Option<u32>) -> DbResult<()> {
        let conn = self.conn.lock();
        let changed = conn.execute(
            "UPDATE messages SET uid = ?2 WHERE id = ?1",
            params![message_id.to_string(), uid.map(|u| u as i64)],
        )?;
        if changed == 0 {
            return Err(DbError::NotFound(format!("message {message_id}")));
        }
        Ok(())
    }

    pub fn set_message_rfc_id(&self, message_id: Uuid, rfc_message_id: &str) -> DbResult<()> {
        let conn = self.conn.lock();
        let changed = conn.execute(
            "UPDATE messages SET message_id = ?2 WHERE id = ?1",
            params![message_id.to_string(), rfc_message_id],
        )?;
        if changed == 0 {
            return Err(DbError::NotFound(format!("message {message_id}")));
        }
        Ok(())
    }

    pub fn get_message_uid(&self, message_id: Uuid) -> DbResult<Option<u32>> {
        let conn = self.conn.lock();
        let uid: Option<i64> = conn
            .query_row(
                "SELECT uid FROM messages WHERE id = ?1",
                params![message_id.to_string()],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| DbError::NotFound(format!("message {message_id}")))?;
        Ok(uid.map(|u| u as u32))
    }

    pub fn list_mailboxes(&self, account_id: Option<Uuid>) -> DbResult<Vec<MailboxDto>> {
        let conn = self.conn.lock();
        let mut out = Vec::new();
        if let Some(account_id) = account_id {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, account_id, name, role, unread_count, total_count
                FROM mailboxes WHERE account_id = ?1
                ORDER BY name ASC
                "#,
            )?;
            let rows = stmt.query_map(params![account_id.to_string()], map_mailbox_dto)?;
            for row in rows {
                out.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, account_id, name, role, unread_count, total_count
                FROM mailboxes
                ORDER BY name ASC
                "#,
            )?;
            let rows = stmt.query_map([], map_mailbox_dto)?;
            for row in rows {
                out.push(row?);
            }
        }
        Ok(out)
    }

    pub fn inbox_mailboxes(&self) -> DbResult<Vec<MailboxRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, account_id, name, role, uidvalidity, uidnext, unread_count, total_count
            FROM mailboxes
            WHERE role = 'inbox' OR lower(name) = 'inbox' OR name = 'INBOX'
            "#,
        )?;
        let rows = stmt.query_map([], map_mailbox_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn upsert_thread(&self, thread: &ThreadRecord) -> DbResult<()> {
        let conn = self.conn.lock();
        let participants = serde_json::to_string(&thread.participants)?;
        conn.execute(
            r#"
            INSERT INTO threads (
              id, account_id, subject, last_message_at, message_count,
              unread_count, participants_json, snippet
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
            ON CONFLICT(id) DO UPDATE SET
              subject = excluded.subject,
              last_message_at = excluded.last_message_at,
              message_count = excluded.message_count,
              unread_count = excluded.unread_count,
              participants_json = excluded.participants_json,
              snippet = excluded.snippet
            "#,
            params![
                thread.id.to_string(),
                thread.account_id.to_string(),
                thread.subject,
                thread.last_message_at,
                thread.message_count as i64,
                thread.unread_count as i64,
                participants,
                thread.snippet,
            ],
        )?;
        Ok(())
    }

    pub fn insert_message(&self, message: &MessageRecord) -> DbResult<()> {
        let conn = self.conn.lock();
        let from = serde_json::to_string(&message.from)?;
        let to = serde_json::to_string(&message.to)?;
        let cc = serde_json::to_string(&message.cc)?;
        let references = serde_json::to_string(&message.references)?;
        conn.execute(
            r#"
            INSERT INTO messages (
              id, account_id, mailbox_id, thread_id, uid, message_id, in_reply_to,
              references_json, subject, from_json, to_json, cc_json, date, flags,
              snippet, body_text, body_html, has_attachments, raw_path,
              local_only, offline_at, size_bytes
            ) VALUES (
              ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22
            )
            ON CONFLICT(mailbox_id, uid) DO UPDATE SET
              flags = excluded.flags,
              subject = excluded.subject,
              snippet = excluded.snippet,
              body_text = COALESCE(excluded.body_text, messages.body_text),
              body_html = COALESCE(excluded.body_html, messages.body_html),
              size_bytes = COALESCE(excluded.size_bytes, messages.size_bytes),
              local_only = CASE
                WHEN messages.local_only != 0 THEN messages.local_only
                ELSE excluded.local_only
              END
            "#,
            params![
                message.id.to_string(),
                message.account_id.to_string(),
                message.mailbox_id.to_string(),
                message.thread_id.to_string(),
                message.uid.map(|u| u as i64),
                message.message_id,
                message.in_reply_to,
                references,
                message.subject,
                from,
                to,
                cc,
                message.date,
                message.flags,
                message.snippet,
                message.body_text,
                message.body_html,
                message.has_attachments as i64,
                message.raw_path,
                message.local_only as i64,
                message.offline_at,
                message.size_bytes,
            ],
        )?;
        drop(conn);
        let mut addrs = vec![message.from.clone()];
        addrs.extend(message.to.iter().cloned());
        addrs.extend(message.cc.iter().cloned());
        self.remember_recipients(&addrs, message.date)?;
        Ok(())
    }

    pub fn remember_recipients(&self, addresses: &[AddressDto], seen_at: i64) -> DbResult<()> {
        let conn = self.conn.lock();
        for addr in addresses {
            let email = addr.email.trim().to_ascii_lowercase();
            if email.is_empty() || !email.contains('@') {
                continue;
            }
            let name = addr
                .name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("");
            conn.execute(
                r#"
                INSERT INTO known_recipients (email, name, last_seen, seen_count)
                VALUES (?1, ?2, ?3, 1)
                ON CONFLICT(email) DO UPDATE SET
                  name = CASE
                    WHEN excluded.name != '' THEN excluded.name
                    ELSE known_recipients.name
                  END,
                  last_seen = MAX(known_recipients.last_seen, excluded.last_seen),
                  seen_count = known_recipients.seen_count + 1
                "#,
                params![email, name, seen_at],
            )?;
        }
        Ok(())
    }

    /// Build/refresh the recipient index from recent message headers (idempotent).
    pub fn backfill_known_recipients(&self, limit: usize) -> DbResult<usize> {
        let rows: Vec<(String, String, String, i64)> = {
            let conn = self.conn.lock();
            let mut stmt = conn.prepare(
                r#"
                SELECT from_json, to_json, cc_json, date
                FROM messages
                ORDER BY date DESC
                LIMIT ?1
                "#,
            )?;
            let mapped = stmt.query_map(params![limit as i64], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?;
            mapped.collect::<Result<Vec<_>, _>>()?
        };
        let mut count = 0usize;
        for (from_json, to_json, cc_json, date) in rows {
            let mut addrs = Vec::new();
            if let Ok(from) = serde_json::from_str::<AddressDto>(&from_json) {
                addrs.push(from);
            }
            if let Ok(to) = serde_json::from_str::<Vec<AddressDto>>(&to_json) {
                addrs.extend(to);
            }
            if let Ok(cc) = serde_json::from_str::<Vec<AddressDto>>(&cc_json) {
                addrs.extend(cc);
            }
            count += addrs.len();
            self.remember_recipients(&addrs, date)?;
        }
        Ok(count)
    }

    pub fn suggest_recipients(
        &self,
        query: &str,
        limit: usize,
    ) -> DbResult<Vec<RecipientSuggestion>> {
        let needle = query.trim();
        if needle.is_empty() {
            return Ok(Vec::new());
        }
        let known_count: i64 = {
            let conn = self.conn.lock();
            conn.query_row("SELECT COUNT(*) FROM known_recipients", [], |r| r.get(0))?
        };
        if known_count == 0 {
            let _ = self.backfill_known_recipients(3_000);
        }

        let contacts = self.list_contacts(Some(needle))?;
        let mut out: Vec<(u32, RecipientSuggestion)> = Vec::new();
        let mut seen_emails = std::collections::HashSet::new();

        for contact in contacts.into_iter().take(limit.saturating_mul(2)) {
            for email in &contact.emails {
                let email_l = email.trim().to_ascii_lowercase();
                if email_l.is_empty() || !seen_emails.insert(email_l.clone()) {
                    continue;
                }
                let score = contact_fuzzy_score(&contact, needle).unwrap_or(0) + 50;
                out.push((
                    score,
                    RecipientSuggestion {
                        email: email_l,
                        name: Some(contact.display_name.clone()).filter(|s| !s.is_empty()),
                        source: "contact".into(),
                        in_contacts: true,
                        contact_id: Some(contact.id),
                    },
                ));
            }
        }

        let history: Vec<(String, String, i64, i64)> = {
            let conn = self.conn.lock();
            let mut stmt = conn.prepare(
                r#"
                SELECT email, name, last_seen, seen_count
                FROM known_recipients
                ORDER BY seen_count DESC, last_seen DESC
                LIMIT 2000
                "#,
            )?;
            let mapped = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?;
            mapped.collect::<Result<Vec<_>, _>>()?
        };

        let contact_emails: std::collections::HashSet<String> = {
            let all = self.list_contacts(None)?;
            all.into_iter()
                .flat_map(|c| c.emails.into_iter().map(|e| e.to_ascii_lowercase()))
                .collect()
        };

        for (email, name, _last_seen, seen_count) in history {
            if seen_emails.contains(&email) {
                continue;
            }
            let mut score = 0u32;
            let mut hit = false;
            if let Some(s) = fuzzy_match_score(&email, needle) {
                hit = true;
                score = score.max(s);
            }
            if let Some(s) = fuzzy_match_score(&name, needle) {
                hit = true;
                score = score.max(s + 20);
            }
            if !hit {
                continue;
            }
            score = score.saturating_add((seen_count as u32).min(40));
            let in_contacts = contact_emails.contains(&email);
            seen_emails.insert(email.clone());
            out.push((
                score,
                RecipientSuggestion {
                    email,
                    name: Some(name).filter(|s| !s.is_empty()),
                    source: if in_contacts {
                        "contact".into()
                    } else {
                        "history".into()
                    },
                    in_contacts,
                    contact_id: None,
                },
            ));
        }

        out.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.email.cmp(&b.1.email)));
        Ok(out.into_iter().take(limit).map(|(_, s)| s).collect())
    }

    pub fn get_message(&self, id: Uuid) -> DbResult<MessageDetailDto> {
        let mut detail = {
            let conn = self.conn.lock();
            conn.query_row(
                r#"
                SELECT
                  m.id, m.account_id, m.mailbox_id, m.thread_id, m.subject,
                  m.from_json, m.to_json, m.date, m.snippet, m.flags, m.has_attachments,
                  a.email, m.body_text, m.body_html, m.message_id, m.in_reply_to, m.references_json,
                  coalesce(m.local_only, 0)
                FROM messages m
                JOIN accounts a ON a.id = m.account_id
                WHERE m.id = ?1
                "#,
                params![id.to_string()],
                |row| {
                    let flags: i64 = row.get(9)?;
                    let from: AddressDto = serde_json::from_str(&row.get::<_, String>(5)?)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                    let to: Vec<AddressDto> = serde_json::from_str(&row.get::<_, String>(6)?)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                    let references: Vec<String> = serde_json::from_str(&row.get::<_, String>(16)?)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                    Ok(MessageDetailDto {
                        summary: MessageSummaryDto {
                            id: parse_uuid(row.get::<_, String>(0)?)?,
                            account_id: parse_uuid(row.get::<_, String>(1)?)?,
                            mailbox_id: parse_uuid(row.get::<_, String>(2)?)?,
                            thread_id: parse_uuid(row.get::<_, String>(3)?)?,
                            subject: row.get(4)?,
                            from,
                            to,
                            date: row.get(7)?,
                            snippet: row.get(8)?,
                            unread: flags & FLAG_SEEN == 0,
                            starred: flags & FLAG_STARRED != 0,
                            has_attachments: row.get::<_, i64>(10)? != 0,
                            account_email: row.get(11)?,
                            local_only: row.get::<_, i64>(17)? != 0,
                            snoozed_until: None,
                        },
                        body_text: row.get(12)?,
                        body_html: row.get(13)?,
                        message_id: row.get(14)?,
                        in_reply_to: row.get(15)?,
                        references,
                        attachments: Vec::new(),
                    })
                },
            )
            .optional()?
            .ok_or_else(|| DbError::NotFound(format!("message {id}")))?
        };
        detail.summary.snoozed_until = self.snooze_wake_at(detail.summary.id)?;
        detail.attachments = self.list_attachments(detail.summary.id)?;
        Ok(detail)
    }

    pub fn list_messages(
        &self,
        req: &ListMessagesRequest,
    ) -> DbResult<(Vec<MessageSummaryDto>, u32)> {
        let conn = self.conn.lock();
        let limit = req.limit.max(1).min(500) as i64;
        let offset = req.offset as i64;
        let (where_sql, bind_ids) = message_filters_sql(req);
        let order_sql = message_order_sql(&req.sort_by, &req.sort_dir);

        let count_sql = format!(
            r#"
            SELECT COUNT(*)
            FROM messages m
            JOIN mailboxes mb ON mb.id = m.mailbox_id
            {where_sql}
            "#
        );

        let total: u32 = if bind_ids.is_empty() {
            conn.query_row(&count_sql, [], |row| row.get::<_, i64>(0))
                .map(|v| v as u32)?
        } else {
            conn.query_row(&count_sql, params![bind_ids[0]], |row| row.get::<_, i64>(0))
                .map(|v| v as u32)?
        };

        let list_sql = if bind_ids.is_empty() {
            format!(
                r#"
                SELECT
                  m.id, m.account_id, m.mailbox_id, m.thread_id, m.subject,
                  m.from_json, m.to_json, m.date, m.snippet, m.flags, m.has_attachments,
                  a.email, coalesce(m.local_only, 0)
                FROM messages m
                JOIN mailboxes mb ON mb.id = m.mailbox_id
                JOIN accounts a ON a.id = m.account_id
                {where_sql}
                ORDER BY {order_sql}
                LIMIT ?1 OFFSET ?2
                "#
            )
        } else {
            format!(
                r#"
                SELECT
                  m.id, m.account_id, m.mailbox_id, m.thread_id, m.subject,
                  m.from_json, m.to_json, m.date, m.snippet, m.flags, m.has_attachments,
                  a.email, coalesce(m.local_only, 0)
                FROM messages m
                JOIN mailboxes mb ON mb.id = m.mailbox_id
                JOIN accounts a ON a.id = m.account_id
                {where_sql}
                ORDER BY {order_sql}
                LIMIT ?2 OFFSET ?3
                "#
            )
        };

        let mut stmt = conn.prepare(&list_sql)?;
        let rows = if bind_ids.is_empty() {
            stmt.query_map(params![limit, offset], map_message_summary)?
        } else {
            stmt.query_map(params![bind_ids[0], limit, offset], map_message_summary)?
        };

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        drop(stmt);
        drop(conn);
        if req.snoozed_only {
            self.hydrate_snoozed_until(&mut messages)?;
        }
        Ok((messages, total))
    }

    pub fn list_threads(&self, req: &ListMessagesRequest) -> DbResult<ListThreadsResponse> {
        let conn = self.conn.lock();
        let limit = req.limit.max(1).min(500) as i64;
        let offset = req.offset as i64;
        let (where_sql, bind_ids) = message_filters_sql(req);
        let dir = match req.sort_dir {
            SortDirection::Asc => "ASC",
            SortDirection::Desc => "DESC",
        };
        let order_sql = match req.sort_by {
            MessageSortBy::Subject => format!("subject COLLATE NOCASE {dir}"),
            MessageSortBy::From => format!("latest_from_json COLLATE NOCASE {dir}"),
            MessageSortBy::Attachments => {
                format!("has_attachments {dir}, last_message_at DESC")
            }
            MessageSortBy::Date => format!("last_message_at {dir}"),
        };

        let count_sql = format!(
            r#"
            SELECT COUNT(DISTINCT m.thread_id)
            FROM messages m
            JOIN mailboxes mb ON mb.id = m.mailbox_id
            {where_sql}
            "#
        );
        let total: u32 = if bind_ids.is_empty() {
            conn.query_row(&count_sql, [], |row| row.get::<_, i64>(0))
                .map(|v| v as u32)?
        } else {
            conn.query_row(&count_sql, params![bind_ids[0]], |row| row.get::<_, i64>(0))
                .map(|v| v as u32)?
        };

        let list_sql = if bind_ids.is_empty() {
            format!(
                r#"
                SELECT
                  m.thread_id,
                  m.account_id,
                  MAX(m.date) AS last_message_at,
                  COUNT(*) AS message_count,
                  SUM(CASE WHEN (m.flags & {FLAG_SEEN}) = 0 THEN 1 ELSE 0 END) AS unread_count,
                  MAX(m.has_attachments) AS has_attachments,
                  (
                    SELECT m2.subject FROM messages m2
                    WHERE m2.thread_id = m.thread_id
                    ORDER BY m2.date DESC LIMIT 1
                  ) AS subject,
                  (
                    SELECT m2.snippet FROM messages m2
                    WHERE m2.thread_id = m.thread_id
                    ORDER BY m2.date DESC LIMIT 1
                  ) AS snippet,
                  (
                    SELECT m2.from_json FROM messages m2
                    WHERE m2.thread_id = m.thread_id
                    ORDER BY m2.date DESC LIMIT 1
                  ) AS latest_from_json,
                  (
                    SELECT a2.email FROM messages m2
                    JOIN accounts a2 ON a2.id = m2.account_id
                    WHERE m2.thread_id = m.thread_id
                    ORDER BY m2.date DESC LIMIT 1
                  ) AS account_email,
                  COALESCE((
                    SELECT t.participants_json FROM threads t WHERE t.id = m.thread_id
                  ), '[]') AS participants_json
                FROM messages m
                JOIN mailboxes mb ON mb.id = m.mailbox_id
                {where_sql}
                GROUP BY m.thread_id, m.account_id
                ORDER BY {order_sql}
                LIMIT ?1 OFFSET ?2
                "#
            )
        } else {
            format!(
                r#"
                SELECT
                  m.thread_id,
                  m.account_id,
                  MAX(m.date) AS last_message_at,
                  COUNT(*) AS message_count,
                  SUM(CASE WHEN (m.flags & {FLAG_SEEN}) = 0 THEN 1 ELSE 0 END) AS unread_count,
                  MAX(m.has_attachments) AS has_attachments,
                  (
                    SELECT m2.subject FROM messages m2
                    WHERE m2.thread_id = m.thread_id
                    ORDER BY m2.date DESC LIMIT 1
                  ) AS subject,
                  (
                    SELECT m2.snippet FROM messages m2
                    WHERE m2.thread_id = m.thread_id
                    ORDER BY m2.date DESC LIMIT 1
                  ) AS snippet,
                  (
                    SELECT m2.from_json FROM messages m2
                    WHERE m2.thread_id = m.thread_id
                    ORDER BY m2.date DESC LIMIT 1
                  ) AS latest_from_json,
                  (
                    SELECT a2.email FROM messages m2
                    JOIN accounts a2 ON a2.id = m2.account_id
                    WHERE m2.thread_id = m.thread_id
                    ORDER BY m2.date DESC LIMIT 1
                  ) AS account_email,
                  COALESCE((
                    SELECT t.participants_json FROM threads t WHERE t.id = m.thread_id
                  ), '[]') AS participants_json
                FROM messages m
                JOIN mailboxes mb ON mb.id = m.mailbox_id
                {where_sql}
                GROUP BY m.thread_id, m.account_id
                ORDER BY {order_sql}
                LIMIT ?2 OFFSET ?3
                "#
            )
        };

        let mut stmt = conn.prepare(&list_sql)?;
        let rows = if bind_ids.is_empty() {
            stmt.query_map(params![limit, offset], map_thread_list_item)?
        } else {
            stmt.query_map(params![bind_ids[0], limit, offset], map_thread_list_item)?
        };
        let mut threads = Vec::new();
        for row in rows {
            threads.push(row?);
        }
        Ok(ListThreadsResponse { threads, total })
    }

    pub fn list_messages_by_thread(&self, thread_id: Uuid) -> DbResult<Vec<MessageSummaryDto>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT
              m.id, m.account_id, m.mailbox_id, m.thread_id, m.subject,
              m.from_json, m.to_json, m.date, m.snippet, m.flags, m.has_attachments,
              a.email, coalesce(m.local_only, 0)
            FROM messages m
            JOIN accounts a ON a.id = m.account_id
            WHERE m.thread_id = ?1
            ORDER BY m.date ASC
            "#,
        )?;
        let rows = stmt.query_map(params![thread_id.to_string()], map_message_summary)?;
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    pub fn set_flags(
        &self,
        message_id: Uuid,
        unread: Option<bool>,
        starred: Option<bool>,
    ) -> DbResult<()> {
        self.apply_flag_updates(message_id, unread, starred, None)
    }

    pub fn archive_message(&self, message_id: Uuid) -> DbResult<()> {
        self.apply_flag_updates(message_id, Some(false), None, Some(true))
    }

    fn apply_flag_updates(
        &self,
        message_id: Uuid,
        unread: Option<bool>,
        starred: Option<bool>,
        archived: Option<bool>,
    ) -> DbResult<()> {
        let conn = self.conn.lock();
        let (flags, mailbox_id): (i64, String) = conn
            .query_row(
                "SELECT flags, mailbox_id FROM messages WHERE id = ?1",
                params![message_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| DbError::NotFound(format!("message {message_id}")))?;

        let mut flags = flags;
        if let Some(unread) = unread {
            if unread {
                flags &= !FLAG_SEEN;
            } else {
                flags |= FLAG_SEEN;
            }
        }
        if let Some(starred) = starred {
            if starred {
                flags |= FLAG_STARRED;
            } else {
                flags &= !FLAG_STARRED;
            }
        }
        if let Some(archived) = archived {
            if archived {
                flags |= FLAG_ARCHIVED | FLAG_SEEN;
            } else {
                flags &= !FLAG_ARCHIVED;
            }
        }

        conn.execute(
            "UPDATE messages SET flags = ?1 WHERE id = ?2",
            params![flags, message_id.to_string()],
        )?;
        // Keep sidebar badge counts in sync without waiting for IMAP sync.
        conn.execute(
            r#"
            UPDATE mailboxes SET
              total_count = (SELECT COUNT(*) FROM messages WHERE mailbox_id = ?1),
              unread_count = (
                SELECT COUNT(*) FROM messages
                WHERE mailbox_id = ?1 AND (flags & 1) = 0
              )
            WHERE id = ?1
            "#,
            params![mailbox_id],
        )?;
        Ok(())
    }

    pub fn find_message_by_message_id(
        &self,
        account_id: Uuid,
        message_id: &str,
    ) -> DbResult<Option<Uuid>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id FROM messages WHERE account_id = ?1 AND message_id = ?2 LIMIT 1",
            params![account_id.to_string(), message_id],
            |row| {
                let id: String = row.get(0)?;
                Ok(parse_uuid(id)?)
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn refresh_mailbox_counts(&self, mailbox_id: Uuid) -> DbResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            r#"
            UPDATE mailboxes SET
              total_count = (SELECT COUNT(*) FROM messages WHERE mailbox_id = ?1),
              unread_count = (
                SELECT COUNT(*) FROM messages
                WHERE mailbox_id = ?1 AND (flags & 1) = 0
              )
            WHERE id = ?1
            "#,
            params![mailbox_id.to_string()],
        )?;
        Ok(())
    }

    pub fn find_message_id_by_uid(&self, mailbox_id: Uuid, uid: u32) -> DbResult<Option<Uuid>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id FROM messages WHERE mailbox_id = ?1 AND uid = ?2",
            params![mailbox_id.to_string(), uid as i64],
            |row| Ok(parse_uuid(row.get::<_, String>(0)?)?),
        )
        .optional()
        .map_err(Into::into)
    }

    /// Highest IMAP UID stored for a mailbox (ignores local-only rows without UID).
    pub fn max_message_uid(&self, mailbox_id: Uuid) -> DbResult<Option<u32>> {
        let conn = self.conn.lock();
        let max: Option<i64> = conn.query_row(
            "SELECT MAX(uid) FROM messages WHERE mailbox_id = ?1 AND uid IS NOT NULL",
            params![mailbox_id.to_string()],
            |row| row.get(0),
        )?;
        Ok(max.map(|u| u as u32))
    }

    /// Sum of unread counts across inbox-role mailboxes (tray / taskbar badge).
    pub fn total_inbox_unread(&self) -> DbResult<u32> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            r#"
            SELECT COALESCE(SUM(unread_count), 0)
            FROM mailboxes
            WHERE role = 'inbox' OR lower(name) = 'inbox' OR name = 'INBOX'
            "#,
            [],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as u32)
    }

    /// Apply IMAP FLAGS for a known UID without rewriting the message body.
    ///
    /// When `allow_unseen` is false, a remote missing `\Seen` never clears a
    /// local read mark (avoids Yahoo/slow STORE races flipping mail unread).
    pub fn set_flags_by_uid(
        &self,
        mailbox_id: Uuid,
        uid: u32,
        seen: bool,
        starred: bool,
        allow_unseen: bool,
    ) -> DbResult<bool> {
        let conn = self.conn.lock();
        let Some((id, flags)): Option<(String, i64)> = conn
            .query_row(
                "SELECT id, flags FROM messages WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id.to_string(), uid as i64],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
        else {
            return Ok(false);
        };
        let mut next = flags;
        if seen {
            next |= FLAG_SEEN;
        } else if allow_unseen {
            next &= !FLAG_SEEN;
        }
        if starred {
            next |= FLAG_STARRED;
        } else {
            next &= !FLAG_STARRED;
        }
        if next == flags {
            return Ok(false);
        }
        conn.execute(
            "UPDATE messages SET flags = ?1 WHERE id = ?2",
            params![next, id],
        )?;
        Ok(true)
    }

    pub fn replace_attachments(
        &self,
        message_id: Uuid,
        attachments: &[AttachmentRecord],
    ) -> DbResult<()> {
        // Preserve stable IDs when filename / content-id still match so the UI
        // does not hold stale attachment UUIDs across refetch/sync.
        let existing = self.list_attachments(message_id)?;
        let mut stable: Vec<AttachmentRecord> = Vec::with_capacity(attachments.len());
        for attachment in attachments {
            let reuse = existing.iter().find(|old| {
                (attachment.content_id.is_some()
                    && old.content_id == attachment.content_id)
                    || old.filename == attachment.filename
            });
            let mut row = attachment.clone();
            if let Some(old) = reuse {
                row.id = old.id;
                // Drop previous blob if path changed.
                if old.path != row.path {
                    let _ = std::fs::remove_file(&old.path);
                }
            }
            stable.push(row);
        }
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM attachments WHERE message_id = ?1",
            params![message_id.to_string()],
        )?;
        for attachment in &stable {
            conn.execute(
                r#"
                INSERT INTO attachments (id, message_id, filename, mime, size, path, content_id)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
                params![
                    attachment.id.to_string(),
                    message_id.to_string(),
                    attachment.filename,
                    attachment.mime,
                    attachment.size as i64,
                    attachment.path,
                    attachment.content_id,
                ],
            )?;
        }
        Ok(())
    }

    /// All message IDs matching the list filters (for select-all across the mailbox).
    pub fn list_message_ids(&self, req: &ListMessagesRequest) -> DbResult<Vec<Uuid>> {
        let conn = self.conn.lock();
        let (where_sql, bind_ids) = message_filters_sql(req);
        let order_sql = message_order_sql(&req.sort_by, &req.sort_dir);
        let sql = format!(
            r#"
            SELECT m.id
            FROM messages m
            JOIN mailboxes mb ON mb.id = m.mailbox_id
            {where_sql}
            ORDER BY {order_sql}
            LIMIT 20000
            "#
        );
        let mut stmt = conn.prepare(&sql)?;
        let mut out = Vec::new();
        if bind_ids.is_empty() {
            let rows = stmt.query_map([], |row| Ok(parse_uuid(row.get::<_, String>(0)?)?))?;
            for row in rows {
                out.push(row?);
            }
        } else {
            let rows = stmt.query_map(params![bind_ids[0]], |row| {
                Ok(parse_uuid(row.get::<_, String>(0)?)?)
            })?;
            for row in rows {
                out.push(row?);
            }
        }
        Ok(out)
    }

    pub fn list_attachments(&self, message_id: Uuid) -> DbResult<Vec<AttachmentDto>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, message_id, filename, mime, size, path, content_id FROM attachments WHERE message_id = ?1 ORDER BY filename",
        )?;
        let rows = stmt.query_map(params![message_id.to_string()], |row| {
            Ok(AttachmentDto {
                id: parse_uuid(row.get::<_, String>(0)?)?,
                message_id: parse_uuid(row.get::<_, String>(1)?)?,
                filename: row.get(2)?,
                mime: row.get(3)?,
                size: row.get::<_, i64>(4)? as u64,
                path: row.get(5)?,
                content_id: row.get(6)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_attachment(&self, attachment_id: Uuid) -> DbResult<AttachmentDto> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, message_id, filename, mime, size, path, content_id FROM attachments WHERE id = ?1",
            params![attachment_id.to_string()],
            |row| {
                Ok(AttachmentDto {
                    id: parse_uuid(row.get::<_, String>(0)?)?,
                    message_id: parse_uuid(row.get::<_, String>(1)?)?,
                    filename: row.get(2)?,
                    mime: row.get(3)?,
                    size: row.get::<_, i64>(4)? as u64,
                    path: row.get(5)?,
                    content_id: row.get(6)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| DbError::NotFound(format!("attachment {attachment_id}")))
    }

    pub fn delete_message(&self, message_id: Uuid) -> DbResult<()> {
        let conn = self.conn.lock();
        let changed =
            conn.execute("DELETE FROM messages WHERE id = ?1", params![message_id.to_string()])?;
        if changed == 0 {
            return Err(DbError::NotFound(format!("message {message_id}")));
        }
        Ok(())
    }

    /// Mark a message as local-only after a successful IMAP purge (keeps body/blobs).
    pub fn mark_message_local_only(
        &self,
        message_id: Uuid,
        size_bytes: Option<i64>,
        offline_at: i64,
    ) -> DbResult<()> {
        let conn = self.conn.lock();
        let changed = conn.execute(
            r#"
            UPDATE messages
            SET local_only = 1,
                offline_at = ?2,
                uid = NULL,
                size_bytes = COALESCE(?3, size_bytes)
            WHERE id = ?1
            "#,
            params![message_id.to_string(), offline_at, size_bytes],
        )?;
        if changed == 0 {
            return Err(DbError::NotFound(format!("message {message_id}")));
        }
        Ok(())
    }

    pub fn set_message_size_bytes(&self, message_id: Uuid, size_bytes: i64) -> DbResult<()> {
        let conn = self.conn.lock();
        let changed = conn.execute(
            "UPDATE messages SET size_bytes = ?2 WHERE id = ?1",
            params![message_id.to_string(), size_bytes],
        )?;
        if changed == 0 {
            return Err(DbError::NotFound(format!("message {message_id}")));
        }
        Ok(())
    }

    /// Whether the message has a readable body and (if flagged) attachment files on disk.
    pub fn message_has_complete_local_copy(&self, message_id: Uuid) -> DbResult<bool> {
        let detail = self.get_message(message_id)?;
        let has_body = detail
            .body_text
            .as_deref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
            || detail
                .body_html
                .as_deref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
        if !has_body {
            return Ok(false);
        }
        if detail.summary.has_attachments {
            let attachments = self.list_attachments(message_id)?;
            if attachments.is_empty() {
                return Ok(false);
            }
            for att in &attachments {
                if !std::path::Path::new(&att.path).is_file() {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    /// Oldest non-local-only messages eligible for IMAP offload.
    pub fn list_offload_candidates(
        &self,
        account_id: Uuid,
        older_than: i64,
        skip_starred: bool,
        limit: u32,
    ) -> DbResult<Vec<Uuid>> {
        let conn = self.conn.lock();
        let limit = limit.max(1).min(500) as i64;
        let starred_sql = if skip_starred {
            format!("AND (m.flags & {FLAG_STARRED}) = 0")
        } else {
            String::new()
        };
        let sql = format!(
            r#"
            SELECT m.id
            FROM messages m
            JOIN mailboxes mb ON mb.id = m.mailbox_id
            WHERE m.account_id = ?1
              AND coalesce(m.local_only, 0) = 0
              AND m.uid IS NOT NULL
              AND m.date < ?2
              AND lower(coalesce(mb.role, '')) NOT IN ('drafts', 'trash')
              AND lower(mb.name) NOT IN ('drafts', 'entwürfe', 'entwuerfe', 'trash', 'deleted', 'deleted items')
              {starred_sql}
            ORDER BY
              CASE WHEN (m.flags & {FLAG_STARRED}) != 0 THEN 1 ELSE 0 END ASC,
              m.date ASC
            LIMIT ?3
            "#
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            params![account_id.to_string(), older_than, limit],
            |row| {
                let id: String = row.get(0)?;
                Ok(Uuid::parse_str(&id).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(e))
                })?)
            },
        )?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn estimate_account_storage_bytes(&self, account_id: Uuid) -> DbResult<u64> {
        let conn = self.conn.lock();
        let msg_bytes: i64 = conn.query_row(
            r#"
            SELECT COALESCE(SUM(
              COALESCE(
                size_bytes,
                length(COALESCE(body_text, '')) + length(COALESCE(body_html, '')) + 512
              )
            ), 0)
            FROM messages
            WHERE account_id = ?1 AND coalesce(local_only, 0) = 0
            "#,
            params![account_id.to_string()],
            |row| row.get(0),
        )?;
        let att_bytes: i64 = conn.query_row(
            r#"
            SELECT COALESCE(SUM(a.size), 0)
            FROM attachments a
            JOIN messages m ON m.id = a.message_id
            WHERE m.account_id = ?1 AND coalesce(m.local_only, 0) = 0
            "#,
            params![account_id.to_string()],
            |row| row.get(0),
        )?;
        Ok((msg_bytes.max(0) + att_bytes.max(0)) as u64)
    }

    pub fn count_local_only_messages(&self, account_id: Option<Uuid>) -> DbResult<u32> {
        let conn = self.conn.lock();
        let total: i64 = if let Some(id) = account_id {
            conn.query_row(
                "SELECT COUNT(*) FROM messages WHERE coalesce(local_only, 0) != 0 AND account_id = ?1",
                params![id.to_string()],
                |row| row.get(0),
            )?
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM messages WHERE coalesce(local_only, 0) != 0",
                [],
                |row| row.get(0),
            )?
        };
        Ok(total as u32)
    }

    pub fn message_size_bytes(&self, message_id: Uuid) -> DbResult<Option<i64>> {
        let conn = self.conn.lock();
        let size: Option<i64> = conn.query_row(
            "SELECT size_bytes FROM messages WHERE id = ?1",
            params![message_id.to_string()],
            |row| row.get(0),
        )?;
        Ok(size)
    }

    pub fn snooze_wake_at(&self, message_id: Uuid) -> DbResult<Option<i64>> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().timestamp();
        let wake: Option<i64> = conn
            .query_row(
                "SELECT wake_at FROM snoozed_messages WHERE message_id = ?1 AND wake_at > ?2",
                params![message_id.to_string(), now],
                |row| row.get(0),
            )
            .optional()?;
        Ok(wake)
    }

    pub fn snooze_message(
        &self,
        message_id: Uuid,
        account_id: Uuid,
        wake_at: i64,
        previous_mailbox_id: Option<Uuid>,
    ) -> DbResult<()> {
        let now = chrono::Utc::now().timestamp();
        let conn = self.conn.lock();
        conn.execute(
            r#"
            INSERT INTO snoozed_messages (message_id, account_id, wake_at, previous_mailbox_id, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(message_id) DO UPDATE SET
              wake_at = excluded.wake_at,
              previous_mailbox_id = excluded.previous_mailbox_id
            "#,
            params![
                message_id.to_string(),
                account_id.to_string(),
                wake_at,
                previous_mailbox_id.map(|id| id.to_string()),
                now,
            ],
        )?;
        Ok(())
    }

    pub fn unsnooze_message(&self, message_id: Uuid) -> DbResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM snoozed_messages WHERE message_id = ?1",
            params![message_id.to_string()],
        )?;
        Ok(())
    }

    pub fn list_due_snoozes(&self, now: i64) -> DbResult<Vec<Uuid>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT message_id FROM snoozed_messages WHERE wake_at <= ?1 ORDER BY wake_at ASC",
        )?;
        let rows = stmt.query_map(params![now], |row| {
            let id: String = row.get(0)?;
            Ok(Uuid::parse_str(&id).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(e))
            })?)
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn count_active_snoozes(&self) -> DbResult<u32> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().timestamp();
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM snoozed_messages WHERE wake_at > ?1",
            params![now],
            |row| row.get(0),
        )?;
        Ok(n as u32)
    }

    pub fn list_snoozed_messages(&self, limit: u32) -> DbResult<Vec<SnoozedMessageDto>> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().timestamp();
        let limit = limit.max(1).min(500) as i64;
        let mut stmt = conn.prepare(
            r#"
            SELECT s.message_id, s.account_id, s.wake_at, m.subject, m.from_json, a.email
            FROM snoozed_messages s
            JOIN messages m ON m.id = s.message_id
            JOIN accounts a ON a.id = s.account_id
            WHERE s.wake_at > ?1
            ORDER BY s.wake_at ASC
            LIMIT ?2
            "#,
        )?;
        let rows = stmt.query_map(params![now, limit], |row| {
            let from: AddressDto = serde_json::from_str(&row.get::<_, String>(4)?)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            Ok(SnoozedMessageDto {
                message_id: parse_uuid(row.get::<_, String>(0)?)?,
                account_id: parse_uuid(row.get::<_, String>(1)?)?,
                wake_at: row.get(2)?,
                subject: row.get(3)?,
                from_email: from.email,
                account_email: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn hydrate_snoozed_until(
        &self,
        messages: &mut [MessageSummaryDto],
    ) -> DbResult<()> {
        for message in messages.iter_mut() {
            message.snoozed_until = self.snooze_wake_at(message.id)?;
        }
        Ok(())
    }

    pub fn enqueue_outbound(
        &self,
        id: Uuid,
        account_id: Uuid,
        payload_json: &str,
        send_at: i64,
    ) -> DbResult<()> {
        let now = chrono::Utc::now().timestamp();
        let conn = self.conn.lock();
        conn.execute(
            r#"
            INSERT INTO outbound_queue
              (id, account_id, payload_json, send_at, status, last_error, created_at, sent_at)
            VALUES (?1, ?2, ?3, ?4, 'pending', NULL, ?5, NULL)
            "#,
            params![
                id.to_string(),
                account_id.to_string(),
                payload_json,
                send_at,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn list_due_outbound(&self, now: i64, limit: u32) -> DbResult<Vec<(Uuid, Uuid, String)>> {
        let conn = self.conn.lock();
        let limit = limit.max(1).min(50) as i64;
        let mut stmt = conn.prepare(
            r#"
            SELECT id, account_id, payload_json
            FROM outbound_queue
            WHERE status IN ('pending', 'failed') AND send_at <= ?1
            ORDER BY send_at ASC
            LIMIT ?2
            "#,
        )?;
        let rows = stmt.query_map(params![now, limit], |row| {
            Ok((
                parse_uuid(row.get::<_, String>(0)?)?,
                parse_uuid(row.get::<_, String>(1)?)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn set_outbound_status(
        &self,
        id: Uuid,
        status: OutboundStatus,
        last_error: Option<&str>,
        sent_at: Option<i64>,
    ) -> DbResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            r#"
            UPDATE outbound_queue
            SET status = ?2, last_error = ?3, sent_at = COALESCE(?4, sent_at)
            WHERE id = ?1
            "#,
            params![
                id.to_string(),
                status.as_str(),
                last_error,
                sent_at,
            ],
        )?;
        Ok(())
    }

    pub fn cancel_outbound(&self, id: Uuid) -> DbResult<()> {
        self.set_outbound_status(id, OutboundStatus::Cancelled, None, None)
    }

    pub fn count_pending_outbound(&self) -> DbResult<u32> {
        let conn = self.conn.lock();
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM outbound_queue WHERE status IN ('pending', 'failed')",
            [],
            |row| row.get(0),
        )?;
        Ok(n as u32)
    }

    pub fn list_outbound_queue(&self, limit: u32) -> DbResult<Vec<OutboundQueueItemDto>> {
        let conn = self.conn.lock();
        let limit = limit.max(1).min(500) as i64;
        let mut stmt = conn.prepare(
            r#"
            SELECT o.id, o.account_id, a.email, o.payload_json, o.send_at, o.status,
                   o.last_error, o.created_at
            FROM outbound_queue o
            JOIN accounts a ON a.id = o.account_id
            WHERE o.status IN ('pending', 'failed', 'sending')
            ORDER BY o.send_at ASC
            LIMIT ?1
            "#,
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            let payload: String = row.get(3)?;
            let (subject, to_summary) = parse_outbound_preview(&payload);
            Ok(OutboundQueueItemDto {
                id: parse_uuid(row.get::<_, String>(0)?)?,
                account_id: parse_uuid(row.get::<_, String>(1)?)?,
                account_email: row.get(2)?,
                subject,
                to_summary,
                send_at: row.get(4)?,
                status: OutboundStatus::parse(&row.get::<_, String>(5)?),
                last_error: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Message IDs in mailboxes with the given role whose `date` is older than `cutoff` (unix secs).
    pub fn list_message_ids_in_role_older_than(
        &self,
        role: &str,
        cutoff: i64,
    ) -> DbResult<Vec<Uuid>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT m.id
            FROM messages m
            JOIN mailboxes mb ON mb.id = m.mailbox_id
            WHERE lower(coalesce(mb.role, '')) = lower(?1)
              AND m.date < ?2
            "#,
        )?;
        let rows = stmt.query_map(params![role, cutoff], |row| {
            let id: String = row.get(0)?;
            Ok(Uuid::parse_str(&id).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(e))
            })?)
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn find_label_by_name(
        &self,
        account_id: Uuid,
        name: &str,
    ) -> DbResult<Option<LabelDto>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, account_id, name, color FROM labels
            WHERE account_id = ?1 AND lower(name) = lower(?2)
            LIMIT 1
            "#,
        )?;
        let mut rows = stmt.query(params![account_id.to_string(), name])?;
        if let Some(row) = rows.next()? {
            Ok(Some(LabelDto {
                id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(e))
                })?,
                account_id: Uuid::parse_str(&row.get::<_, String>(1)?).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(e))
                })?,
                name: row.get(2)?,
                color: row.get(3)?,
            }))
        } else {
            Ok(None)
        }
    }


    pub fn upsert_contact(&self, contact: &ContactRecord) -> DbResult<()> {
        let conn = self.conn.lock();
        let emails = serde_json::to_string(&contact.emails)?;
        let profile = encode_contact_profile(contact);
        conn.execute(
            r#"
            INSERT INTO contacts (id, display_name, emails_json, notes, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(id) DO UPDATE SET
              display_name = excluded.display_name,
              emails_json = excluded.emails_json,
              notes = excluded.notes,
              updated_at = excluded.updated_at
            "#,
            params![
                contact.id.to_string(),
                contact.display_name,
                emails,
                profile,
                contact.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_contacts(&self, query: Option<&str>) -> DbResult<Vec<ContactDto>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, display_name, emails_json, notes, updated_at FROM contacts ORDER BY display_name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], map_contact_row)?;
        let mut scored: Vec<(u32, ContactDto)> = Vec::new();
        for row in rows {
            let contact = row?;
            if let Some(q) = query {
                if let Some(score) = contact_fuzzy_score(&contact, q) {
                    scored.push((score, contact));
                }
            } else {
                scored.push((0, contact));
            }
        }
        if query.is_some() {
            scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.display_name.cmp(&b.1.display_name)));
        }
        Ok(scored.into_iter().map(|(_, c)| c).collect())
    }

    pub fn delete_contact(&self, contact_id: Uuid) -> DbResult<()> {
        let conn = self.conn.lock();
        let changed =
            conn.execute("DELETE FROM contacts WHERE id = ?1", params![contact_id.to_string()])?;
        if changed == 0 {
            return Err(DbError::NotFound(format!("contact {contact_id}")));
        }
        Ok(())
    }

    pub fn get_contact(&self, contact_id: Uuid) -> DbResult<ContactDto> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, display_name, emails_json, notes, updated_at FROM contacts WHERE id = ?1",
            params![contact_id.to_string()],
            map_contact_row,
        )
        .optional()?
        .ok_or_else(|| DbError::NotFound(format!("contact {contact_id}")))
    }

    pub fn find_contact_by_ldap_dn(&self, ldap_dn: &str) -> DbResult<Option<ContactDto>> {
        let contacts = self.list_contacts(None)?;
        Ok(contacts
            .into_iter()
            .find(|c| c.ldap_dn.as_deref() == Some(ldap_dn)))
    }

    pub fn upsert_label(&self, label: &LabelRecord) -> DbResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            r#"
            INSERT INTO labels (id, account_id, name, color)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(id) DO UPDATE SET
              name = excluded.name,
              color = excluded.color
            "#,
            params![
                label.id.to_string(),
                label.account_id.to_string(),
                label.name,
                label.color,
            ],
        )?;
        Ok(())
    }

    pub fn list_labels(&self, account_id: Option<Uuid>) -> DbResult<Vec<LabelDto>> {
        let conn = self.conn.lock();
        let mut out = Vec::new();
        if let Some(account_id) = account_id {
            let mut stmt = conn.prepare(
                "SELECT id, account_id, name, color FROM labels WHERE account_id = ?1 ORDER BY name",
            )?;
            let rows = stmt.query_map(params![account_id.to_string()], map_label_dto)?;
            for row in rows {
                out.push(row?);
            }
        } else {
            let mut stmt =
                conn.prepare("SELECT id, account_id, name, color FROM labels ORDER BY name")?;
            let rows = stmt.query_map([], map_label_dto)?;
            for row in rows {
                out.push(row?);
            }
        }
        Ok(out)
    }

    pub fn delete_label(&self, label_id: Uuid) -> DbResult<()> {
        let conn = self.conn.lock();
        let changed =
            conn.execute("DELETE FROM labels WHERE id = ?1", params![label_id.to_string()])?;
        if changed == 0 {
            return Err(DbError::NotFound(format!("label {label_id}")));
        }
        Ok(())
    }

    pub fn set_message_labels(&self, message_id: Uuid, label_ids: &[Uuid]) -> DbResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM message_labels WHERE message_id = ?1",
            params![message_id.to_string()],
        )?;
        for label_id in label_ids {
            conn.execute(
                "INSERT OR IGNORE INTO message_labels (message_id, label_id) VALUES (?1, ?2)",
                params![message_id.to_string(), label_id.to_string()],
            )?;
        }
        Ok(())
    }

    pub fn list_message_labels(&self, message_id: Uuid) -> DbResult<Vec<LabelDto>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT l.id, l.account_id, l.name, l.color
            FROM labels l
            INNER JOIN message_labels ml ON ml.label_id = l.id
            WHERE ml.message_id = ?1
            ORDER BY l.name
            "#,
        )?;
        let rows = stmt.query_map(params![message_id.to_string()], map_label_dto)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn upsert_rule(&self, rule: &RuleRecord) -> DbResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            r#"
            INSERT INTO rules (id, account_id, name, predicate_json, action_json, enabled)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(id) DO UPDATE SET
              account_id = excluded.account_id,
              name = excluded.name,
              predicate_json = excluded.predicate_json,
              action_json = excluded.action_json,
              enabled = excluded.enabled
            "#,
            params![
                rule.id.to_string(),
                rule.account_id.map(|id| id.to_string()),
                rule.name,
                rule.predicate_json,
                rule.action_json,
                rule.enabled as i64,
            ],
        )?;
        Ok(())
    }

    pub fn list_rules(&self) -> DbResult<Vec<RuleDto>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, account_id, name, predicate_json, action_json, enabled FROM rules ORDER BY name",
        )?;
        let rows = stmt.query_map([], |row| {
            let account_id = row
                .get::<_, Option<String>>(1)?
                .map(|s| parse_uuid(s))
                .transpose()?;
            Ok(RuleDto {
                id: parse_uuid(row.get::<_, String>(0)?)?,
                account_id,
                name: row.get(2)?,
                predicate_json: row.get(3)?,
                action_json: row.get(4)?,
                enabled: row.get::<_, i64>(5)? != 0,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn delete_rule(&self, rule_id: Uuid) -> DbResult<()> {
        let conn = self.conn.lock();
        let changed =
            conn.execute("DELETE FROM rules WHERE id = ?1", params![rule_id.to_string()])?;
        if changed == 0 {
            return Err(DbError::NotFound(format!("rule {rule_id}")));
        }
        Ok(())
    }

    pub fn upsert_signature(&self, signature: &SignatureRecord) -> DbResult<()> {
        let conn = self.conn.lock();
        if signature.is_default {
            if let Some(account_id) = signature.account_id {
                conn.execute(
                    "UPDATE signatures SET is_default = 0 WHERE account_id = ?1",
                    params![account_id.to_string()],
                )?;
            } else {
                conn.execute(
                    "UPDATE signatures SET is_default = 0 WHERE account_id IS NULL",
                    [],
                )?;
            }
        }
        conn.execute(
            r#"
            INSERT INTO signatures (id, account_id, name, body_text, is_default)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(id) DO UPDATE SET
              account_id = excluded.account_id,
              name = excluded.name,
              body_text = excluded.body_text,
              is_default = excluded.is_default
            "#,
            params![
                signature.id.to_string(),
                signature.account_id.map(|id| id.to_string()),
                signature.name,
                signature.body_text,
                signature.is_default as i64,
            ],
        )?;
        Ok(())
    }

    pub fn list_signatures(&self, account_id: Option<Uuid>) -> DbResult<Vec<SignatureDto>> {
        let conn = self.conn.lock();
        let mut out = Vec::new();
        if let Some(account_id) = account_id {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, account_id, name, body_text, is_default FROM signatures
                WHERE account_id = ?1 OR account_id IS NULL
                ORDER BY is_default DESC, name
                "#,
            )?;
            let rows = stmt.query_map(params![account_id.to_string()], map_signature_dto)?;
            for row in rows {
                out.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, account_id, name, body_text, is_default FROM signatures ORDER BY name",
            )?;
            let rows = stmt.query_map([], map_signature_dto)?;
            for row in rows {
                out.push(row?);
            }
        }
        Ok(out)
    }

    pub fn delete_signature(&self, signature_id: Uuid) -> DbResult<()> {
        let conn = self.conn.lock();
        let changed = conn.execute(
            "DELETE FROM signatures WHERE id = ?1",
            params![signature_id.to_string()],
        )?;
        if changed == 0 {
            return Err(DbError::NotFound(format!("signature {signature_id}")));
        }
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> DbResult<Option<String>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> DbResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            r#"
            INSERT INTO settings (key, value) VALUES (?1, ?2)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            "#,
            params![key, value],
        )?;
        Ok(())
    }

    pub fn upsert_ai_insight(
        &self,
        message_id: Uuid,
        kind: &str,
        payload_json: &str,
    ) -> DbResult<()> {
        let conn = self.conn.lock();
        let id = Uuid::new_v4();
        let now = chrono::Utc::now().timestamp();
        // Keep one row per (message, kind).
        conn.execute(
            "DELETE FROM ai_insights WHERE message_id = ?1 AND kind = ?2",
            params![message_id.to_string(), kind],
        )?;
        conn.execute(
            r#"
            INSERT INTO ai_insights (id, message_id, kind, payload_json, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![
                id.to_string(),
                message_id.to_string(),
                kind,
                payload_json,
                now
            ],
        )?;
        Ok(())
    }

    pub fn get_ai_insight(&self, message_id: Uuid, kind: &str) -> DbResult<Option<String>> {
        let conn = self.conn.lock();
        conn.query_row(
            r#"
            SELECT payload_json FROM ai_insights
            WHERE message_id = ?1 AND kind = ?2
            ORDER BY created_at DESC LIMIT 1
            "#,
            params![message_id.to_string(), kind],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn has_ai_insight(&self, message_id: Uuid, kind: &str) -> DbResult<bool> {
        Ok(self.get_ai_insight(message_id, kind)?.is_some())
    }

    // —— OpenPGP keyring ——

    pub fn upsert_pgp_key(
        &self,
        fingerprint: &str,
        user_ids: &[String],
        has_secret: bool,
        armored_public: &str,
        armored_secret: Option<&str>,
    ) -> DbResult<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().timestamp();
        let user_ids_json = serde_json::to_string(user_ids).unwrap_or_else(|_| "[]".into());
        conn.execute(
            r#"
            INSERT INTO pgp_keys (fingerprint, user_ids_json, has_secret, armored_public, armored_secret, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(fingerprint) DO UPDATE SET
              user_ids_json = excluded.user_ids_json,
              has_secret = CASE WHEN excluded.has_secret = 1 THEN 1 ELSE pgp_keys.has_secret END,
              armored_public = excluded.armored_public,
              armored_secret = COALESCE(excluded.armored_secret, pgp_keys.armored_secret)
            "#,
            params![
                fingerprint,
                user_ids_json,
                if has_secret { 1 } else { 0 },
                armored_public,
                armored_secret,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn list_pgp_keys(&self) -> DbResult<Vec<PgpKeyDto>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT fingerprint, user_ids_json, has_secret, created_at FROM pgp_keys ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let user_ids: Vec<String> =
                serde_json::from_str(&row.get::<_, String>(1)?).unwrap_or_default();
            Ok(PgpKeyDto {
                fingerprint: row.get(0)?,
                user_ids,
                has_secret: row.get::<_, i64>(2)? != 0,
                created_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn delete_pgp_key(&self, fingerprint: &str) -> DbResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM pgp_keys WHERE fingerprint = ?1",
            params![fingerprint],
        )?;
        Ok(())
    }

    pub fn get_pgp_public(&self, fingerprint: &str) -> DbResult<Option<String>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT armored_public FROM pgp_keys WHERE fingerprint = ?1",
            params![fingerprint],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn get_pgp_secret(&self, fingerprint: &str) -> DbResult<Option<String>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT armored_secret FROM pgp_keys WHERE fingerprint = ?1 AND has_secret = 1",
            params![fingerprint],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn find_pgp_secret_for_email(&self, email: &str) -> DbResult<Option<(String, String)>> {
        let email = email.trim().to_ascii_lowercase();
        let keys = self.list_pgp_keys()?;
        for key in keys {
            if !key.has_secret {
                continue;
            }
            let matched = key.user_ids.iter().any(|uid| {
                uid.to_ascii_lowercase().contains(&email)
            });
            if matched {
                if let Some(secret) = self.get_pgp_secret(&key.fingerprint)? {
                    return Ok(Some((key.fingerprint, secret)));
                }
            }
        }
        Ok(None)
    }

    pub fn find_pgp_public_for_email(&self, email: &str) -> DbResult<Option<String>> {
        let email = email.trim().to_ascii_lowercase();
        let keys = self.list_pgp_keys()?;
        for key in keys {
            let matched = key.user_ids.iter().any(|uid| {
                uid.to_ascii_lowercase().contains(&email)
            });
            if matched {
                return self.get_pgp_public(&key.fingerprint);
            }
        }
        Ok(None)
    }

    pub fn list_pgp_secrets(&self) -> DbResult<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt =
            conn.prepare("SELECT armored_secret FROM pgp_keys WHERE has_secret = 1 AND armored_secret IS NOT NULL")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_pgp_publics(&self) -> DbResult<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT armored_public FROM pgp_keys")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    // —— Calendar ——

    pub fn upsert_calendar_account(
        &self,
        id: Uuid,
        name: &str,
        caldav_url: &str,
        username: &str,
    ) -> DbResult<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            r#"
            INSERT INTO calendar_accounts (id, name, caldav_url, username, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(id) DO UPDATE SET
              name = excluded.name,
              caldav_url = excluded.caldav_url,
              username = excluded.username
            "#,
            params![id.to_string(), name, caldav_url, username, now],
        )?;
        Ok(())
    }

    pub fn list_calendar_accounts(&self) -> DbResult<Vec<CalendarAccountDto>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, name, caldav_url, username, created_at FROM calendar_accounts ORDER BY name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(CalendarAccountDto {
                id: parse_uuid(row.get::<_, String>(0)?)?,
                name: row.get(1)?,
                caldav_url: row.get(2)?,
                username: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn delete_calendar_account(&self, id: Uuid) -> DbResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM calendar_accounts WHERE id = ?1",
            params![id.to_string()],
        )?;
        Ok(())
    }

    pub fn ensure_local_default_collection(&self) -> DbResult<CalendarCollectionDto> {
        if let Some(existing) = self.list_calendar_collections()?.into_iter().find(|c| c.is_default)
        {
            return Ok(existing);
        }
        if let Some(any) = self.list_calendar_collections()?.into_iter().next() {
            self.set_default_calendar_collection(any.id)?;
            return Ok(self
                .list_calendar_collections()?
                .into_iter()
                .find(|c| c.id == any.id)
                .unwrap_or(any));
        }
        let id = Uuid::new_v4();
        self.upsert_calendar_collection(
            id,
            None,
            None,
            "Personal",
            "#1e3a5f",
            true,
            true,
        )?;
        self.list_calendar_collections()?
            .into_iter()
            .find(|c| c.id == id)
            .ok_or_else(|| DbError::Invalid("failed to create local calendar".into()))
    }

    pub fn upsert_calendar_collection(
        &self,
        id: Uuid,
        calendar_account_id: Option<Uuid>,
        href: Option<&str>,
        display_name: &str,
        color: &str,
        is_visible: bool,
        is_default: bool,
    ) -> DbResult<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().timestamp();
        if is_default {
            conn.execute("UPDATE calendar_collections SET is_default = 0", [])?;
        }
        conn.execute(
            r#"
            INSERT INTO calendar_collections (
              id, calendar_account_id, href, display_name, color, is_visible, is_default, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(id) DO UPDATE SET
              calendar_account_id = excluded.calendar_account_id,
              href = excluded.href,
              display_name = excluded.display_name,
              color = excluded.color,
              is_visible = excluded.is_visible,
              is_default = excluded.is_default
            "#,
            params![
                id.to_string(),
                calendar_account_id.map(|u| u.to_string()),
                href,
                display_name,
                color,
                if is_visible { 1 } else { 0 },
                if is_default { 1 } else { 0 },
                now,
            ],
        )?;
        Ok(())
    }

    pub fn set_default_calendar_collection(&self, id: Uuid) -> DbResult<()> {
        let conn = self.conn.lock();
        conn.execute("UPDATE calendar_collections SET is_default = 0", [])?;
        conn.execute(
            "UPDATE calendar_collections SET is_default = 1 WHERE id = ?1",
            params![id.to_string()],
        )?;
        Ok(())
    }

    /// Delete a calendar collection and its local events.
    /// Refuses to delete the last remaining calendar.
    pub fn delete_calendar_collection(&self, id: Uuid) -> DbResult<()> {
        let collections = self.list_calendar_collections()?;
        let Some(target) = collections.iter().find(|c| c.id == id) else {
            return Err(DbError::NotFound(format!("calendar collection {id}")));
        };
        if collections.len() <= 1 {
            return Err(DbError::Invalid(
                "cannot delete the last calendar".into(),
            ));
        }
        let was_default = target.is_default;
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM calendar_events WHERE collection_id = ?1",
            params![id.to_string()],
        )?;
        let changed = conn.execute(
            "DELETE FROM calendar_collections WHERE id = ?1",
            params![id.to_string()],
        )?;
        if changed == 0 {
            return Err(DbError::NotFound(format!("calendar collection {id}")));
        }
        drop(conn);
        if was_default {
            if let Some(next) = self
                .list_calendar_collections()?
                .into_iter()
                .next()
            {
                self.set_default_calendar_collection(next.id)?;
            }
        }
        Ok(())
    }

    pub fn list_calendar_collections(&self) -> DbResult<Vec<CalendarCollectionDto>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, calendar_account_id, href, display_name, color, is_visible, is_default
            FROM calendar_collections
            ORDER BY is_default DESC, display_name ASC
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(CalendarCollectionDto {
                id: parse_uuid(row.get::<_, String>(0)?)?,
                calendar_account_id: row
                    .get::<_, Option<String>>(1)?
                    .map(parse_uuid)
                    .transpose()?,
                href: row.get(2)?,
                display_name: row.get(3)?,
                color: row.get(4)?,
                is_visible: row.get::<_, i64>(5)? != 0,
                is_default: row.get::<_, i64>(6)? != 0,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_calendar_collection(&self, id: Uuid) -> DbResult<Option<CalendarCollectionDto>> {
        Ok(self
            .list_calendar_collections()?
            .into_iter()
            .find(|c| c.id == id))
    }

    pub fn find_collection_by_href(
        &self,
        account_id: Uuid,
        href: &str,
    ) -> DbResult<Option<CalendarCollectionDto>> {
        Ok(self
            .list_calendar_collections()?
            .into_iter()
            .find(|c| c.calendar_account_id == Some(account_id) && c.href.as_deref() == Some(href)))
    }

    pub fn upsert_calendar_event(
        &self,
        id: Uuid,
        calendar_account_id: Option<Uuid>,
        collection_id: Option<Uuid>,
        ical_uid: Option<&str>,
        title: &str,
        starts_at: i64,
        ends_at: Option<i64>,
        location: Option<&str>,
        description: Option<&str>,
        all_day: bool,
        source_message_id: Option<Uuid>,
        reminders_json: &str,
        status: &str,
        organizer: Option<&str>,
        attendees_json: &str,
        etag: Option<&str>,
        href: Option<&str>,
    ) -> DbResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            r#"
            INSERT INTO calendar_events (
              id, account_id, ical_uid, title, starts_at, ends_at, location, description,
              calendar_account_id, all_day, source_message_id, collection_id, reminders_json,
              status, organizer, attendees_json, etag, href
            ) VALUES (?1, NULL, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
            ON CONFLICT(id) DO UPDATE SET
              ical_uid = excluded.ical_uid,
              title = excluded.title,
              starts_at = excluded.starts_at,
              ends_at = excluded.ends_at,
              location = excluded.location,
              description = excluded.description,
              calendar_account_id = excluded.calendar_account_id,
              all_day = excluded.all_day,
              source_message_id = excluded.source_message_id,
              collection_id = excluded.collection_id,
              reminders_json = excluded.reminders_json,
              status = excluded.status,
              organizer = excluded.organizer,
              attendees_json = excluded.attendees_json,
              etag = excluded.etag,
              href = excluded.href
            "#,
            params![
                id.to_string(),
                ical_uid,
                title,
                starts_at,
                ends_at,
                location,
                description,
                calendar_account_id.map(|u| u.to_string()),
                if all_day { 1 } else { 0 },
                source_message_id.map(|u| u.to_string()),
                collection_id.map(|u| u.to_string()),
                reminders_json,
                status,
                organizer,
                attendees_json,
                etag,
                href,
            ],
        )?;
        Ok(())
    }

    pub fn list_calendar_events(&self, from: i64, to: i64) -> DbResult<Vec<CalendarEventDto>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT e.id, e.calendar_account_id, e.collection_id, e.ical_uid, e.title, e.starts_at,
                   e.ends_at, e.location, e.description, e.all_day, e.source_message_id,
                   e.reminders_json, e.status, e.organizer, e.attendees_json, c.color,
                   e.etag, e.href
            FROM calendar_events e
            LEFT JOIN calendar_collections c ON c.id = e.collection_id
            WHERE e.starts_at <= ?2 AND (e.ends_at IS NULL OR e.ends_at >= ?1)
              AND (e.collection_id IS NULL OR c.is_visible = 1 OR c.id IS NULL)
            ORDER BY e.starts_at ASC
            "#,
        )?;
        let rows = stmt.query_map(params![from, to], |row| {
            let reminders_raw: String = row.get(11)?;
            let attendees_raw: String = row.get(14)?;
            Ok(CalendarEventDto {
                id: parse_uuid(row.get::<_, String>(0)?)?,
                calendar_account_id: row
                    .get::<_, Option<String>>(1)?
                    .map(parse_uuid)
                    .transpose()?,
                collection_id: row
                    .get::<_, Option<String>>(2)?
                    .map(parse_uuid)
                    .transpose()?,
                ical_uid: row.get(3)?,
                title: row.get(4)?,
                starts_at: row.get(5)?,
                ends_at: row.get(6)?,
                location: row.get(7)?,
                description: row.get(8)?,
                all_day: row.get::<_, i64>(9)? != 0,
                source_message_id: row
                    .get::<_, Option<String>>(10)?
                    .map(parse_uuid)
                    .transpose()?,
                reminders: parse_reminders_json(&reminders_raw),
                status: row.get(12)?,
                organizer: row.get(13)?,
                attendees: parse_attendees_json(&attendees_raw),
                color: row.get(15)?,
                etag: row.get(16)?,
                href: row.get(17)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_calendar_event(&self, id: Uuid) -> DbResult<Option<CalendarEventDto>> {
        Ok(self
            .list_calendar_events(0, i64::MAX)?
            .into_iter()
            .find(|e| e.id == id))
    }

    pub fn list_due_calendar_reminders(&self, now: i64) -> DbResult<Vec<CalendarEventDto>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT e.id, e.calendar_account_id, e.collection_id, e.ical_uid, e.title, e.starts_at,
                   e.ends_at, e.location, e.description, e.all_day, e.source_message_id,
                   e.reminders_json, e.status, e.organizer, e.attendees_json, c.color,
                   e.etag, e.href
            FROM calendar_events e
            LEFT JOIN calendar_collections c ON c.id = e.collection_id
            WHERE e.reminder_fired_at IS NULL
              AND e.starts_at >= ?1 - 86400
              AND e.starts_at <= ?1 + 86400
            "#,
        )?;
        let rows = stmt.query_map(params![now], |row| {
            let reminders_raw: String = row.get(11)?;
            let attendees_raw: String = row.get(14)?;
            Ok(CalendarEventDto {
                id: parse_uuid(row.get::<_, String>(0)?)?,
                calendar_account_id: row
                    .get::<_, Option<String>>(1)?
                    .map(parse_uuid)
                    .transpose()?,
                collection_id: row
                    .get::<_, Option<String>>(2)?
                    .map(parse_uuid)
                    .transpose()?,
                ical_uid: row.get(3)?,
                title: row.get(4)?,
                starts_at: row.get(5)?,
                ends_at: row.get(6)?,
                location: row.get(7)?,
                description: row.get(8)?,
                all_day: row.get::<_, i64>(9)? != 0,
                source_message_id: row
                    .get::<_, Option<String>>(10)?
                    .map(parse_uuid)
                    .transpose()?,
                reminders: parse_reminders_json(&reminders_raw),
                status: row.get(12)?,
                organizer: row.get(13)?,
                attendees: parse_attendees_json(&attendees_raw),
                color: row.get(15)?,
                etag: row.get(16)?,
                href: row.get(17)?,
            })
        })?;
        let events = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(events
            .into_iter()
            .filter(|e| {
                e.reminders.iter().any(|r| {
                    let fire_at = e.starts_at - r.minutes * 60;
                    fire_at <= now && e.starts_at + 3600 >= now
                })
            })
            .collect())
    }

    pub fn mark_calendar_reminder_fired(&self, id: Uuid, at: i64) -> DbResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE calendar_events SET reminder_fired_at = ?2 WHERE id = ?1",
            params![id.to_string(), at],
        )?;
        Ok(())
    }

    pub fn delete_calendar_events_by_ical_uid(&self, ical_uid: &str) -> DbResult<u32> {
        let conn = self.conn.lock();
        let changed = conn.execute(
            "DELETE FROM calendar_events WHERE ical_uid = ?1",
            params![ical_uid],
        )?;
        Ok(changed as u32)
    }

    pub fn delete_calendar_event(&self, id: Uuid) -> DbResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM calendar_events WHERE id = ?1",
            params![id.to_string()],
        )?;
        Ok(())
    }

    pub fn upsert_calendar_task(
        &self,
        id: Uuid,
        calendar_account_id: Option<Uuid>,
        ical_uid: Option<&str>,
        title: &str,
        due_at: Option<i64>,
        completed: bool,
        notes: &str,
        source_message_id: Option<Uuid>,
    ) -> DbResult<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            r#"
            INSERT INTO calendar_tasks (
              id, calendar_account_id, ical_uid, title, due_at, completed, notes,
              source_message_id, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
            ON CONFLICT(id) DO UPDATE SET
              calendar_account_id = excluded.calendar_account_id,
              ical_uid = excluded.ical_uid,
              title = excluded.title,
              due_at = excluded.due_at,
              completed = excluded.completed,
              notes = excluded.notes,
              source_message_id = excluded.source_message_id,
              updated_at = excluded.updated_at
            "#,
            params![
                id.to_string(),
                calendar_account_id.map(|u| u.to_string()),
                ical_uid,
                title,
                due_at,
                if completed { 1 } else { 0 },
                notes,
                source_message_id.map(|u| u.to_string()),
                now,
            ],
        )?;
        Ok(())
    }

    pub fn list_calendar_tasks(&self, include_completed: bool) -> DbResult<Vec<CalendarTaskDto>> {
        let conn = self.conn.lock();
        let sql = if include_completed {
            "SELECT id, calendar_account_id, ical_uid, title, due_at, completed, notes, source_message_id, created_at, updated_at FROM calendar_tasks ORDER BY completed ASC, due_at IS NULL, due_at ASC"
        } else {
            "SELECT id, calendar_account_id, ical_uid, title, due_at, completed, notes, source_message_id, created_at, updated_at FROM calendar_tasks WHERE completed = 0 ORDER BY due_at IS NULL, due_at ASC"
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(CalendarTaskDto {
                id: parse_uuid(row.get::<_, String>(0)?)?,
                calendar_account_id: row
                    .get::<_, Option<String>>(1)?
                    .map(parse_uuid)
                    .transpose()?,
                ical_uid: row.get(2)?,
                title: row.get(3)?,
                due_at: row.get(4)?,
                completed: row.get::<_, i64>(5)? != 0,
                notes: row.get(6)?,
                source_message_id: row
                    .get::<_, Option<String>>(7)?
                    .map(parse_uuid)
                    .transpose()?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn delete_calendar_task(&self, id: Uuid) -> DbResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM calendar_tasks WHERE id = ?1",
            params![id.to_string()],
        )?;
        Ok(())
    }

    pub fn upsert_calendar_invitation(
        &self,
        id: Uuid,
        message_id: Option<Uuid>,
        ical_uid: &str,
        title: &str,
        starts_at: i64,
        ends_at: Option<i64>,
        location: Option<&str>,
        description: Option<&str>,
        organizer: Option<&str>,
        partstat: &str,
        payload_ics: &str,
        received_at: i64,
    ) -> DbResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            r#"
            INSERT INTO calendar_invitations (
              id, message_id, ical_uid, title, starts_at, ends_at, location, description,
              organizer, partstat, payload_ics, received_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ON CONFLICT(id) DO UPDATE SET
              title = excluded.title,
              starts_at = excluded.starts_at,
              ends_at = excluded.ends_at,
              location = excluded.location,
              description = excluded.description,
              organizer = excluded.organizer,
              partstat = CASE
                WHEN calendar_invitations.partstat = 'needs-action' THEN excluded.partstat
                ELSE calendar_invitations.partstat
              END,
              payload_ics = excluded.payload_ics
            "#,
            params![
                id.to_string(),
                message_id.map(|u| u.to_string()),
                ical_uid,
                title,
                starts_at,
                ends_at,
                location,
                description,
                organizer,
                partstat,
                payload_ics,
                received_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_calendar_invitations(
        &self,
        pending_only: bool,
    ) -> DbResult<Vec<CalendarInvitationDto>> {
        let conn = self.conn.lock();
        let sql = if pending_only {
            "SELECT id, message_id, ical_uid, title, starts_at, ends_at, location, description, organizer, partstat, received_at FROM calendar_invitations WHERE partstat = 'needs-action' ORDER BY starts_at ASC"
        } else {
            "SELECT id, message_id, ical_uid, title, starts_at, ends_at, location, description, organizer, partstat, received_at FROM calendar_invitations ORDER BY received_at DESC"
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(CalendarInvitationDto {
                id: parse_uuid(row.get::<_, String>(0)?)?,
                message_id: row
                    .get::<_, Option<String>>(1)?
                    .map(parse_uuid)
                    .transpose()?,
                ical_uid: row.get(2)?,
                title: row.get(3)?,
                starts_at: row.get(4)?,
                ends_at: row.get(5)?,
                location: row.get(6)?,
                description: row.get(7)?,
                organizer: row.get(8)?,
                partstat: row.get(9)?,
                received_at: row.get(10)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_calendar_invitation(
        &self,
        id: Uuid,
    ) -> DbResult<Option<(CalendarInvitationDto, String)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, message_id, ical_uid, title, starts_at, ends_at, location, description, organizer, partstat, received_at, payload_ics FROM calendar_invitations WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id.to_string()])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        Ok(Some((
            CalendarInvitationDto {
                id: parse_uuid(row.get::<_, String>(0)?)?,
                message_id: row
                    .get::<_, Option<String>>(1)?
                    .map(parse_uuid)
                    .transpose()?,
                ical_uid: row.get(2)?,
                title: row.get(3)?,
                starts_at: row.get(4)?,
                ends_at: row.get(5)?,
                location: row.get(6)?,
                description: row.get(7)?,
                organizer: row.get(8)?,
                partstat: row.get(9)?,
                received_at: row.get(10)?,
            },
            row.get(11)?,
        )))
    }

    pub fn set_invitation_partstat(&self, id: Uuid, partstat: &str) -> DbResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE calendar_invitations SET partstat = ?2 WHERE id = ?1",
            params![id.to_string(), partstat],
        )?;
        Ok(())
    }

    /// Force invitation partstat to cancelled (iMIP METHOD:CANCEL).
    pub fn cancel_calendar_invitation_by_uid(
        &self,
        ical_uid: &str,
        message_id: Option<Uuid>,
        title: &str,
        starts_at: i64,
        ends_at: Option<i64>,
        location: Option<&str>,
        description: Option<&str>,
        organizer: Option<&str>,
        payload_ics: &str,
        received_at: i64,
    ) -> DbResult<()> {
        let id = Uuid::new_v5(&Uuid::NAMESPACE_URL, ical_uid.as_bytes());
        let conn = self.conn.lock();
        conn.execute(
            r#"
            INSERT INTO calendar_invitations (
              id, message_id, ical_uid, title, starts_at, ends_at, location, description,
              organizer, partstat, payload_ics, received_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'cancelled', ?10, ?11)
            ON CONFLICT(id) DO UPDATE SET
              title = excluded.title,
              starts_at = excluded.starts_at,
              ends_at = excluded.ends_at,
              location = excluded.location,
              description = excluded.description,
              organizer = excluded.organizer,
              partstat = 'cancelled',
              payload_ics = excluded.payload_ics,
              message_id = COALESCE(excluded.message_id, calendar_invitations.message_id)
            "#,
            params![
                id.to_string(),
                message_id.map(|u| u.to_string()),
                ical_uid,
                title,
                starts_at,
                ends_at,
                location,
                description,
                organizer,
                payload_ics,
                received_at,
            ],
        )?;
        drop(conn);
        let _ = self.delete_calendar_events_by_ical_uid(ical_uid)?;
        Ok(())
    }

    /// Scan message bodies and `.ics` / text/calendar attachments for iMIP REQUEST/CANCEL.
    pub fn scan_messages_for_invites(&self, limit: i64) -> DbResult<u32> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT DISTINCT m.id, m.body_text, m.date
            FROM messages m
            LEFT JOIN attachments a ON a.message_id = m.id
            WHERE (
              (
                m.body_text LIKE '%BEGIN:VCALENDAR%'
                AND (
                  m.body_text LIKE '%METHOD:REQUEST%'
                  OR m.body_text LIKE '%METHOD:CANCEL%'
                )
              )
              OR lower(a.filename) LIKE '%.ics'
              OR lower(a.mime) LIKE '%text/calendar%'
              OR lower(a.mime) LIKE '%application/ics%'
            )
            ORDER BY m.date DESC
            LIMIT ?1
            "#,
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok((
                parse_uuid(row.get::<_, String>(0)?)?,
                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                row.get::<_, i64>(2)?,
            ))
        })?;
        let candidates: Vec<_> = rows.collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        drop(conn);
        let mut count = 0u32;
        for (message_id, body, date) in candidates {
            let mut payloads = Vec::new();
            if body.to_ascii_uppercase().contains("BEGIN:VCALENDAR") {
                payloads.push(body);
            }
            for att in self.list_attachments(message_id)? {
                let name = att.filename.to_ascii_lowercase();
                let mime = att.mime.to_ascii_lowercase();
                let is_ics = name.ends_with(".ics")
                    || mime.contains("text/calendar")
                    || mime.contains("application/ics");
                if !is_ics {
                    continue;
                }
                if let Ok(text) = std::fs::read_to_string(&att.path) {
                    if text.to_ascii_uppercase().contains("BEGIN:VCALENDAR") {
                        payloads.push(text);
                    }
                }
            }
            for payload_src in payloads {
                for invite in crate_parse_imip_messages(&payload_src) {
                    match invite.method {
                        ImipMethod::Request => {
                            let id = Uuid::new_v5(&Uuid::NAMESPACE_URL, invite.uid.as_bytes());
                            self.upsert_calendar_invitation(
                                id,
                                Some(message_id),
                                &invite.uid,
                                &invite.title,
                                invite.starts_at,
                                invite.ends_at,
                                invite.location.as_deref(),
                                invite.description.as_deref(),
                                invite.organizer.as_deref(),
                                "needs-action",
                                &invite.payload,
                                date,
                            )?;
                            count += 1;
                        }
                        ImipMethod::Cancel => {
                            self.cancel_calendar_invitation_by_uid(
                                &invite.uid,
                                Some(message_id),
                                &invite.title,
                                invite.starts_at,
                                invite.ends_at,
                                invite.location.as_deref(),
                                invite.description.as_deref(),
                                invite.organizer.as_deref(),
                                &invite.payload,
                                date,
                            )?;
                            count += 1;
                        }
                    }
                }
            }
        }
        Ok(count)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImipMethod {
    Request,
    Cancel,
}

struct ParsedImip {
    method: ImipMethod,
    uid: String,
    title: String,
    starts_at: i64,
    ends_at: Option<i64>,
    location: Option<String>,
    description: Option<String>,
    organizer: Option<String>,
    payload: String,
}

fn crate_parse_imip_messages(body: &str) -> Vec<ParsedImip> {
    let mut out = Vec::new();
    let upper_all = body.to_ascii_uppercase();
    let mut search = 0;
    while let Some(rel) = upper_all[search..].find("BEGIN:VCALENDAR") {
        let start = search + rel;
        let Some(end_rel) = upper_all[start..].find("END:VCALENDAR") else {
            break;
        };
        let end = start + end_rel + "END:VCALENDAR".len();
        let payload = body[start..end].to_string();
        if let Some(parsed) = crate_parse_imip_invite(&payload) {
            out.push(parsed);
        }
        search = end;
    }
    out
}

fn crate_parse_imip_invite(payload: &str) -> Option<ParsedImip> {
    let upper = payload.to_ascii_uppercase();
    let method = if upper.contains("METHOD:CANCEL") {
        ImipMethod::Cancel
    } else if upper.contains("METHOD:REQUEST") {
        ImipMethod::Request
    } else {
        return None;
    };
    let vevent_start = upper.find("BEGIN:VEVENT")?;
    let vevent_end = upper.find("END:VEVENT")?;
    let block = &payload[vevent_start..vevent_end];
    let mut uid = String::new();
    let mut title = match method {
        ImipMethod::Cancel => "(cancelled)".to_string(),
        ImipMethod::Request => "(invitation)".to_string(),
    };
    let mut starts_at = 0i64;
    let mut ends_at = None;
    let mut location = None;
    let mut description = None;
    let mut organizer = None;
    for line in block.replace("\r\n ", "").replace("\n ", "").lines() {
        let line = line.trim();
        let Some((key_part, value)) = line.split_once(':') else {
            continue;
        };
        let key = key_part
            .split(';')
            .next()
            .unwrap_or(key_part)
            .to_ascii_uppercase();
        match key.as_str() {
            "UID" => uid = value.to_string(),
            "SUMMARY" => title = value.to_string(),
            "DTSTART" => {
                starts_at = parse_simple_ical_ts(value);
            }
            "DTEND" => {
                let t = parse_simple_ical_ts(value);
                if t > 0 {
                    ends_at = Some(t);
                }
            }
            "LOCATION" => location = Some(value.to_string()),
            "DESCRIPTION" => description = Some(value.to_string()),
            "ORGANIZER" => {
                organizer = Some(
                    value
                        .trim_start_matches("mailto:")
                        .trim_start_matches("MAILTO:")
                        .to_string(),
                );
            }
            _ => {}
        }
    }
    if uid.is_empty() {
        return None;
    }
    // CANCEL may omit DTSTART; keep a placeholder so we can still store the invite row.
    if starts_at == 0 && method == ImipMethod::Request {
        return None;
    }
    Some(ParsedImip {
        method,
        uid,
        title,
        starts_at,
        ends_at,
        location,
        description,
        organizer,
        payload: payload.to_string(),
    })
}

fn parse_simple_ical_ts(raw: &str) -> i64 {
    let compact: String = raw.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if compact.len() == 8 {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(&compact, "%Y%m%d") {
            return date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
        }
    }
    if compact.len() >= 15 {
        let slice = if compact.ends_with('Z') {
            &compact[..15]
        } else {
            &compact[..15]
        };
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(slice, "%Y%m%dT%H%M%S") {
            return dt.and_utc().timestamp();
        }
    }
    0
}

fn parse_reminders_json(raw: &str) -> Vec<CalendarReminderDto> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn parse_attendees_json(raw: &str) -> Vec<CalendarAttendeeDto> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn map_label_dto(row: &rusqlite::Row<'_>) -> rusqlite::Result<LabelDto> {
    Ok(LabelDto {
        id: parse_uuid(row.get::<_, String>(0)?)?,
        account_id: parse_uuid(row.get::<_, String>(1)?)?,
        name: row.get(2)?,
        color: row.get(3)?,
    })
}

fn map_signature_dto(row: &rusqlite::Row<'_>) -> rusqlite::Result<SignatureDto> {
    let account_id = row
        .get::<_, Option<String>>(1)?
        .map(parse_uuid)
        .transpose()?;
    Ok(SignatureDto {
        id: parse_uuid(row.get::<_, String>(0)?)?,
        account_id,
        name: row.get(2)?,
        body_text: row.get(3)?,
        is_default: row.get::<_, i64>(4)? != 0,
    })
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContactProfileJson {
    #[serde(default)]
    phones: Vec<String>,
    #[serde(default)]
    faxes: Vec<String>,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    given_name: String,
    #[serde(default)]
    family_name: String,
    #[serde(default)]
    organization: String,
    #[serde(default)]
    job_title: String,
    #[serde(default)]
    addresses: Vec<ContactAddress>,
    #[serde(default)]
    custom_fields: Vec<ContactCustomField>,
    #[serde(default)]
    photo_base64: Option<String>,
    #[serde(default)]
    ldap_dn: Option<String>,
}

fn encode_contact_profile(contact: &ContactRecord) -> String {
    serde_json::to_string(&ContactProfileJson {
        phones: contact.phones.clone(),
        faxes: contact.faxes.clone(),
        notes: contact.notes.clone(),
        given_name: contact.given_name.clone(),
        family_name: contact.family_name.clone(),
        organization: contact.organization.clone(),
        job_title: contact.job_title.clone(),
        addresses: contact.addresses.clone(),
        custom_fields: contact.custom_fields.clone(),
        photo_base64: contact.photo_base64.clone(),
        ldap_dn: contact.ldap_dn.clone(),
    })
    .unwrap_or_else(|_| {
        serde_json::json!({
            "phones": contact.phones,
            "notes": contact.notes,
        })
        .to_string()
    })
}

fn decode_contact_profile(raw: &str) -> ContactProfileJson {
    if let Ok(value) = serde_json::from_str::<ContactProfileJson>(raw) {
        value
    } else {
        ContactProfileJson {
            notes: raw.to_string(),
            ..Default::default()
        }
    }
}

fn map_contact_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ContactDto> {
    let emails: Vec<String> =
        serde_json::from_str(&row.get::<_, String>(2)?).unwrap_or_default();
    let profile = decode_contact_profile(&row.get::<_, String>(3)?);
    Ok(ContactDto {
        id: parse_uuid(row.get::<_, String>(0)?)?,
        display_name: row.get(1)?,
        given_name: profile.given_name,
        family_name: profile.family_name,
        emails,
        phones: profile.phones,
        faxes: profile.faxes,
        organization: profile.organization,
        job_title: profile.job_title,
        addresses: profile.addresses,
        custom_fields: profile.custom_fields,
        photo_base64: profile.photo_base64,
        ldap_dn: profile.ldap_dn,
        notes: profile.notes,
        updated_at: row.get(4)?,
    })
}

fn contact_fuzzy_score(contact: &ContactDto, query: &str) -> Option<u32> {
    let needle = query.trim();
    if needle.is_empty() {
        return Some(0);
    }
    let mut best = 0u32;
    let mut hit = false;
    let fields = std::iter::once(contact.display_name.as_str())
        .chain(std::iter::once(contact.given_name.as_str()))
        .chain(std::iter::once(contact.family_name.as_str()))
        .chain(std::iter::once(contact.organization.as_str()))
        .chain(std::iter::once(contact.job_title.as_str()))
        .chain(std::iter::once(contact.notes.as_str()))
        .chain(contact.emails.iter().map(String::as_str))
        .chain(contact.phones.iter().map(String::as_str))
        .chain(contact.faxes.iter().map(String::as_str))
        .chain(contact.custom_fields.iter().flat_map(|f| [f.label.as_str(), f.value.as_str()]))
        .chain(contact.addresses.iter().flat_map(|a| {
            [
                a.label.as_str(),
                a.street.as_str(),
                a.city.as_str(),
                a.region.as_str(),
                a.postal_code.as_str(),
                a.country.as_str(),
            ]
        }));
    for field in fields {
        if let Some(score) = fuzzy_match_score(field, needle) {
            hit = true;
            best = best.max(score);
        }
    }
    hit.then_some(best)
}

/// Lightweight fuzzy score: exact substring > prefix > subsequence.
fn fuzzy_match_score(haystack: &str, needle: &str) -> Option<u32> {
    let h = haystack.to_ascii_lowercase();
    let n = needle.to_ascii_lowercase();
    if n.is_empty() {
        return Some(0);
    }
    if let Some(idx) = h.find(&n) {
        let bonus = if idx == 0 { 200 } else { 0 };
        return Some(800 + bonus - (idx as u32).min(100));
    }
    let mut hi = h.chars();
    let mut matched = 0u32;
    for nc in n.chars() {
        loop {
            match hi.next() {
                Some(hc) if hc == nc => {
                    matched += 1;
                    break;
                }
                Some(_) => continue,
                None => return None,
            }
        }
    }
    Some(200 + matched * 10)
}

fn map_mailbox_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MailboxRecord> {
    Ok(MailboxRecord {
        id: parse_uuid(row.get::<_, String>(0)?)?,
        account_id: parse_uuid(row.get::<_, String>(1)?)?,
        name: row.get(2)?,
        role: row.get(3)?,
        uidvalidity: row.get(4)?,
        uidnext: row.get(5)?,
        unread_count: row.get::<_, i64>(6)? as u32,
        total_count: row.get::<_, i64>(7)? as u32,
    })
}

fn map_mailbox_dto(row: &rusqlite::Row<'_>) -> rusqlite::Result<MailboxDto> {
    Ok(MailboxDto {
        id: parse_uuid(row.get::<_, String>(0)?)?,
        account_id: parse_uuid(row.get::<_, String>(1)?)?,
        name: row.get(2)?,
        role: row.get(3)?,
        unread_count: row.get::<_, i64>(4)? as u32,
        total_count: row.get::<_, i64>(5)? as u32,
    })
}

fn map_message_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<MessageSummaryDto> {
    let flags: i64 = row.get(9)?;
    let from: AddressDto = serde_json::from_str(&row.get::<_, String>(5)?)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    let to: Vec<AddressDto> = serde_json::from_str(&row.get::<_, String>(6)?)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    Ok(MessageSummaryDto {
        id: parse_uuid(row.get::<_, String>(0)?)?,
        account_id: parse_uuid(row.get::<_, String>(1)?)?,
        mailbox_id: parse_uuid(row.get::<_, String>(2)?)?,
        thread_id: parse_uuid(row.get::<_, String>(3)?)?,
        subject: row.get(4)?,
        from,
        to,
        date: row.get(7)?,
        snippet: row.get(8)?,
        unread: flags & FLAG_SEEN == 0,
        starred: flags & FLAG_STARRED != 0,
        has_attachments: row.get::<_, i64>(10)? != 0,
        account_email: row.get(11)?,
        local_only: row.get::<_, i64>(12).unwrap_or(0) != 0,
        snoozed_until: None,
    })
}

fn map_thread_list_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<ThreadListItemDto> {
    let latest_from: AddressDto = serde_json::from_str(&row.get::<_, String>(8)?)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    let participants: Vec<AddressDto> = serde_json::from_str(&row.get::<_, String>(10)?)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    Ok(ThreadListItemDto {
        id: parse_uuid(row.get::<_, String>(0)?)?,
        account_id: parse_uuid(row.get::<_, String>(1)?)?,
        last_message_at: row.get(2)?,
        message_count: row.get::<_, i64>(3)? as u32,
        unread_count: row.get::<_, i64>(4)? as u32,
        has_attachments: row.get::<_, i64>(5)? != 0,
        subject: row.get(6)?,
        snippet: row.get(7)?,
        latest_from,
        account_email: row.get(9)?,
        participants,
    })
}

/// Build WHERE clause and at most one UUID bind (`?1`) for list/thread queries.
fn message_filters_sql(req: &ListMessagesRequest) -> (String, Vec<String>) {
    let mut where_parts = Vec::new();
    let mut bind_ids: Vec<String> = Vec::new();

    // Offline mailbox virtual folder: all local-only messages (optionally per account).
    if req.local_only {
        where_parts.push("coalesce(m.local_only, 0) != 0".into());
        if let Some(account_id) = req.account_id {
            where_parts.push("m.account_id = ?1".into());
            bind_ids.push(account_id.to_string());
        }
        if req.unread_only {
            where_parts.push(format!("(m.flags & {FLAG_SEEN}) = 0"));
        }
        if req.starred_only {
            where_parts.push(format!("(m.flags & {FLAG_STARRED}) != 0"));
        }
        if req.has_attachments {
            where_parts.push("m.has_attachments != 0".into());
        }
        if let Some(query) = &req.query {
            if !query.trim().is_empty() {
                where_parts.push(format!(
                    "m.rowid IN (SELECT rowid FROM messages_fts WHERE messages_fts MATCH '{}')",
                    escape_fts(query)
                ));
            }
        }
        let where_sql = format!("WHERE {}", where_parts.join(" AND "));
        return (where_sql, bind_ids);
    }

    // Snoozed virtual folder: currently sleeping messages.
    if req.snoozed_only {
        let now = chrono::Utc::now().timestamp();
        where_parts.push(format!(
            "EXISTS (SELECT 1 FROM snoozed_messages s WHERE s.message_id = m.id AND s.wake_at > {now})"
        ));
        if let Some(account_id) = req.account_id {
            where_parts.push("m.account_id = ?1".into());
            bind_ids.push(account_id.to_string());
        }
        if req.unread_only {
            where_parts.push(format!("(m.flags & {FLAG_SEEN}) = 0"));
        }
        if req.starred_only {
            where_parts.push(format!("(m.flags & {FLAG_STARRED}) != 0"));
        }
        if req.has_attachments {
            where_parts.push("m.has_attachments != 0".into());
        }
        if let Some(query) = &req.query {
            if !query.trim().is_empty() {
                where_parts.push(format!(
                    "m.rowid IN (SELECT rowid FROM messages_fts WHERE messages_fts MATCH '{}')",
                    escape_fts(query)
                ));
            }
        }
        let where_sql = format!("WHERE {}", where_parts.join(" AND "));
        return (where_sql, bind_ids);
    }

    let drafts_role = req
        .mailbox_role
        .as_deref()
        .map(|r| r.eq_ignore_ascii_case("drafts"))
        .unwrap_or(false);
    let junk_role = req
        .mailbox_role
        .as_deref()
        .map(|r| r.eq_ignore_ascii_case("junk") || r.eq_ignore_ascii_case("spam"))
        .unwrap_or(false);

    if drafts_role {
        where_parts.push(
            "(lower(coalesce(mb.role, '')) = 'drafts' OR lower(mb.name) IN ('drafts', 'entwürfe', 'entwuerfe'))"
                .into(),
        );
        if let Some(account_id) = req.account_id {
            where_parts.push("m.account_id = ?1".into());
            bind_ids.push(account_id.to_string());
        }
    } else if junk_role {
        where_parts.push(
            "(lower(coalesce(mb.role, '')) = 'junk' OR lower(mb.name) IN ('junk', 'spam', 'junk e-mail', 'junk email'))"
                .into(),
        );
        if let Some(account_id) = req.account_id {
            where_parts.push("m.account_id = ?1".into());
            bind_ids.push(account_id.to_string());
        }
    } else if req.unified {
        where_parts.push(
            "(mb.role = 'inbox' OR lower(mb.name) = 'inbox' OR mb.name = 'INBOX')".into(),
        );
        where_parts.push(format!("(m.flags & {FLAG_ARCHIVED}) = 0"));
        // Keep drafts and spam out of the unified inbox.
        where_parts.push(
            "(lower(coalesce(mb.role, '')) != 'drafts' AND lower(mb.name) NOT IN ('drafts', 'entwürfe', 'entwuerfe'))"
                .into(),
        );
        where_parts.push(
            "(lower(coalesce(mb.role, '')) != 'junk' AND lower(mb.name) NOT IN ('junk', 'spam', 'junk e-mail', 'junk email'))"
                .into(),
        );
        if let Some(mailbox_id) = req.mailbox_id {
            where_parts.push("m.mailbox_id = ?1".into());
            bind_ids.push(mailbox_id.to_string());
        } else if let Some(account_id) = req.account_id {
            where_parts.push("m.account_id = ?1".into());
            bind_ids.push(account_id.to_string());
        }
    } else if let Some(mailbox_id) = req.mailbox_id {
        where_parts.push("m.mailbox_id = ?1".into());
        bind_ids.push(mailbox_id.to_string());
    } else if let Some(account_id) = req.account_id {
        where_parts.push("m.account_id = ?1".into());
        bind_ids.push(account_id.to_string());
    }

    // Hide actively snoozed mail from normal inbox/mailbox views.
    {
        let now = chrono::Utc::now().timestamp();
        where_parts.push(format!(
            "NOT EXISTS (SELECT 1 FROM snoozed_messages s WHERE s.message_id = m.id AND s.wake_at > {now})"
        ));
    }

    if req.unread_only {
        where_parts.push(format!("(m.flags & {FLAG_SEEN}) = 0"));
    }
    if req.starred_only {
        where_parts.push(format!("(m.flags & {FLAG_STARRED}) != 0"));
    }
    if req.has_attachments {
        where_parts.push("m.has_attachments != 0".into());
    }

    if let Some(query) = &req.query {
        if !query.trim().is_empty() {
            where_parts.push(format!(
                "m.rowid IN (SELECT rowid FROM messages_fts WHERE messages_fts MATCH '{}')",
                escape_fts(query)
            ));
        }
    }

    let where_sql = if where_parts.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_parts.join(" AND "))
    };
    (where_sql, bind_ids)
}

fn message_order_sql(sort_by: &MessageSortBy, sort_dir: &SortDirection) -> String {
    let dir = match sort_dir {
        SortDirection::Asc => "ASC",
        SortDirection::Desc => "DESC",
    };
    match sort_by {
        MessageSortBy::Subject => format!("m.subject COLLATE NOCASE {dir}"),
        MessageSortBy::From => format!("m.from_json COLLATE NOCASE {dir}"),
        MessageSortBy::Attachments => format!("m.has_attachments {dir}, m.date DESC"),
        MessageSortBy::Date => format!("m.date {dir}"),
    }
}

fn parse_uuid(value: String) -> rusqlite::Result<Uuid> {
    Uuid::parse_str(&value).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
}

fn provider_to_str(provider: &MailProvider) -> &'static str {
    match provider {
        MailProvider::Generic => "generic",
        MailProvider::Gmail => "gmail",
        MailProvider::Microsoft365 => "microsoft365",
        MailProvider::Yahoo => "yahoo",
        MailProvider::ProtonBridge => "protonBridge",
        MailProvider::Icloud => "icloud",
    }
}

fn parse_provider(value: &str) -> MailProvider {
    match value {
        "gmail" => MailProvider::Gmail,
        "microsoft365" => MailProvider::Microsoft365,
        "yahoo" => MailProvider::Yahoo,
        "protonBridge" => MailProvider::ProtonBridge,
        "icloud" => MailProvider::Icloud,
        _ => MailProvider::Generic,
    }
}

fn auth_to_str(auth: &AuthType) -> &'static str {
    match auth {
        AuthType::Password => "password",
        AuthType::OAuth2 => "oauth2",
    }
}

fn parse_auth(value: &str) -> AuthType {
    match value {
        "oauth2" => AuthType::OAuth2,
        _ => AuthType::Password,
    }
}

fn escape_fts(query: &str) -> String {
    let cleaned = query.replace('"', " ").replace('\'', " ");
    let tokens: Vec<_> = cleaned
        .split_whitespace()
        .map(|t| format!("\"{t}\"*"))
        .collect();
    if tokens.is_empty() {
        "\"\"".into()
    } else {
        tokens.join(" ")
    }
}

fn parse_outbound_preview(payload_json: &str) -> (String, String) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload_json) else {
        return ("(queued)".into(), String::new());
    };
    let subject = value
        .get("subject")
        .and_then(|v| v.as_str())
        .unwrap_or("(no subject)")
        .to_string();
    let to_summary = value
        .get("to")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.get("email").and_then(|e| e.as_str()))
                .take(3)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    (subject, to_summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{FLAG_SEEN, MailboxRecord, MessageRecord, ThreadRecord};
    use novamail_ipc::{AddressDto, AuthType, MailProvider};

    fn sample_account() -> AccountRecord {
        AccountRecord {
            id: Uuid::new_v4(),
            name: "Work".into(),
            label: String::new(),
            email: "user@example.com".into(),
            provider: MailProvider::Generic,
            auth_type: AuthType::Password,
            imap_host: "imap.example.com".into(),
            imap_port: 993,
            imap_tls: true,
            smtp_host: "smtp.example.com".into(),
            smtp_port: 465,
            smtp_tls: true,
            created_at: 1_700_000_000,
        }
    }

    #[test]
    fn unified_inbox_lists_across_accounts() {
        let db = Database::open_in_memory().unwrap();
        let a1 = sample_account();
        let mut a2 = sample_account();
        a2.id = Uuid::new_v4();
        a2.email = "other@example.com".into();
        db.insert_account(&a1).unwrap();
        db.insert_account(&a2).unwrap();

        let mb1 = MailboxRecord {
            id: Uuid::new_v4(),
            account_id: a1.id,
            name: "INBOX".into(),
            role: Some("inbox".into()),
            uidvalidity: Some(1),
            uidnext: Some(2),
            unread_count: 0,
            total_count: 0,
        };
        let mb2 = MailboxRecord {
            id: Uuid::new_v4(),
            account_id: a2.id,
            name: "INBOX".into(),
            role: Some("inbox".into()),
            uidvalidity: Some(1),
            uidnext: Some(2),
            unread_count: 0,
            total_count: 0,
        };
        db.upsert_mailbox(&mb1).unwrap();
        db.upsert_mailbox(&mb2).unwrap();

        for (account, mailbox, subject, date) in [
            (&a1, &mb1, "Hello A", 100_i64),
            (&a2, &mb2, "Hello B", 200_i64),
        ] {
            let thread_id = Uuid::new_v4();
            db.upsert_thread(&ThreadRecord {
                id: thread_id,
                account_id: account.id,
                subject: subject.into(),
                last_message_at: date,
                message_count: 1,
                unread_count: 1,
                participants: vec![AddressDto {
                    name: None,
                    email: "sender@example.com".into(),
                }],
                snippet: subject.into(),
            })
            .unwrap();
            db.insert_message(&MessageRecord {
                id: Uuid::new_v4(),
                account_id: account.id,
                mailbox_id: mailbox.id,
                thread_id,
                uid: Some(1),
                message_id: Some(format!("<{subject}@example.com>")),
                in_reply_to: None,
                references: vec![],
                subject: subject.into(),
                from: AddressDto {
                    name: Some("Sender".into()),
                    email: "sender@example.com".into(),
                },
                to: vec![AddressDto {
                    name: None,
                    email: account.email.clone(),
                }],
                cc: vec![],
                date,
                flags: 0,
                snippet: subject.into(),
                body_text: Some(subject.into()),
                body_html: None,
                has_attachments: false,
                raw_path: None,
                local_only: false,
                offline_at: None,
                size_bytes: Some(subject.len() as i64),
            })
            .unwrap();
        }

        let (messages, total) = db
            .list_messages(&ListMessagesRequest {
                mailbox_id: None,
                account_id: None,
                unified: true,
                mailbox_role: None,
                local_only: false,
                snoozed_only: false,
                limit: 50,
                offset: 0,
                query: None,
                unread_only: false,
                starred_only: false,
                has_attachments: false,
                sort_by: MessageSortBy::Date,
                sort_dir: SortDirection::Desc,
            })
            .unwrap();

        assert_eq!(total, 2);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].subject, "Hello B");
        assert!(messages[0].unread);

        let threads = db
            .list_threads(&ListMessagesRequest {
                mailbox_id: None,
                account_id: None,
                unified: true,
                mailbox_role: None,
                local_only: false,
                snoozed_only: false,
                limit: 50,
                offset: 0,
                query: None,
                unread_only: false,
                starred_only: false,
                has_attachments: false,
                sort_by: MessageSortBy::Date,
                sort_dir: SortDirection::Desc,
            })
            .unwrap();
        assert_eq!(threads.total, 2);
        assert_eq!(threads.threads.len(), 2);

        db.set_flags(messages[0].id, Some(false), Some(true))
            .unwrap();
        let detail = db.get_message(messages[0].id).unwrap();
        assert!(!detail.summary.unread);
        assert!(detail.summary.starred);
        let _ = FLAG_SEEN;
    }

    #[test]
    fn local_only_offload_and_filter() {
        let db = Database::open_in_memory().unwrap();
        let account = sample_account();
        db.insert_account(&account).unwrap();
        let mailbox = MailboxRecord {
            id: Uuid::new_v4(),
            account_id: account.id,
            name: "INBOX".into(),
            role: Some("inbox".into()),
            uidvalidity: Some(1),
            uidnext: Some(2),
            unread_count: 0,
            total_count: 1,
        };
        db.upsert_mailbox(&mailbox).unwrap();
        let thread_id = Uuid::new_v4();
        db.upsert_thread(&ThreadRecord {
            id: thread_id,
            account_id: account.id,
            subject: "Old".into(),
            last_message_at: 100,
            message_count: 1,
            unread_count: 0,
            participants: vec![AddressDto {
                name: None,
                email: "a@example.com".into(),
            }],
            snippet: "Old".into(),
        })
        .unwrap();
        let message_id = Uuid::new_v4();
        db.insert_message(&MessageRecord {
            id: message_id,
            account_id: account.id,
            mailbox_id: mailbox.id,
            thread_id,
            uid: Some(7),
            message_id: Some("<old@example.com>".into()),
            in_reply_to: None,
            references: vec![],
            subject: "Old".into(),
            from: AddressDto {
                name: None,
                email: "a@example.com".into(),
            },
            to: vec![],
            cc: vec![],
            date: 100,
            flags: FLAG_SEEN,
            snippet: "Old".into(),
            body_text: Some("body".into()),
            body_html: None,
            has_attachments: false,
            raw_path: None,
            local_only: false,
            offline_at: None,
            size_bytes: Some(128),
        })
        .unwrap();

        assert!(db.message_has_complete_local_copy(message_id).unwrap());
        db.mark_message_local_only(message_id, Some(128), 200)
            .unwrap();
        assert!(db.get_message_uid(message_id).unwrap().is_none());
        assert!(db.get_message(message_id).unwrap().summary.local_only);
        assert_eq!(db.count_local_only_messages(Some(account.id)).unwrap(), 1);

        let (offline, total) = db
            .list_messages(&ListMessagesRequest {
                mailbox_id: None,
                account_id: None,
                unified: false,
                mailbox_role: None,
                local_only: true,
                snoozed_only: false,
                limit: 50,
                offset: 0,
                query: None,
                unread_only: false,
                starred_only: false,
                has_attachments: false,
                sort_by: MessageSortBy::Date,
                sort_dir: SortDirection::Desc,
            })
            .unwrap();
        assert_eq!(total, 1);
        assert_eq!(offline[0].id, message_id);

        let candidates = db
            .list_offload_candidates(account.id, 10_000, true, 10)
            .unwrap();
        assert!(candidates.is_empty());
    }

    #[test]
    fn snooze_hides_from_inbox_until_wake() {
        let db = Database::open_in_memory().unwrap();
        let account = sample_account();
        db.insert_account(&account).unwrap();
        let mailbox = MailboxRecord {
            id: Uuid::new_v4(),
            account_id: account.id,
            name: "INBOX".into(),
            role: Some("inbox".into()),
            uidvalidity: Some(1),
            uidnext: Some(2),
            unread_count: 0,
            total_count: 1,
        };
        db.upsert_mailbox(&mailbox).unwrap();
        let thread_id = Uuid::new_v4();
        db.upsert_thread(&ThreadRecord {
            id: thread_id,
            account_id: account.id,
            subject: "Snooze me".into(),
            last_message_at: 100,
            message_count: 1,
            unread_count: 1,
            participants: vec![AddressDto {
                name: None,
                email: "a@example.com".into(),
            }],
            snippet: "Snooze me".into(),
        })
        .unwrap();
        let message_id = Uuid::new_v4();
        db.insert_message(&MessageRecord {
            id: message_id,
            account_id: account.id,
            mailbox_id: mailbox.id,
            thread_id,
            uid: Some(1),
            message_id: Some("<snooze@example.com>".into()),
            in_reply_to: None,
            references: vec![],
            subject: "Snooze me".into(),
            from: AddressDto {
                name: None,
                email: "a@example.com".into(),
            },
            to: vec![],
            cc: vec![],
            date: 100,
            flags: 0,
            snippet: "Snooze me".into(),
            body_text: Some("hi".into()),
            body_html: None,
            has_attachments: false,
            raw_path: None,
            local_only: false,
            offline_at: None,
            size_bytes: Some(2),
        })
        .unwrap();

        let future = chrono::Utc::now().timestamp() + 3_600;
        db.snooze_message(message_id, account.id, future, Some(mailbox.id))
            .unwrap();
        assert_eq!(db.count_active_snoozes().unwrap(), 1);

        let (_, inbox_total) = db
            .list_messages(&ListMessagesRequest {
                mailbox_id: None,
                account_id: None,
                unified: true,
                mailbox_role: None,
                local_only: false,
                snoozed_only: false,
                limit: 50,
                offset: 0,
                query: None,
                unread_only: false,
                starred_only: false,
                has_attachments: false,
                sort_by: MessageSortBy::Date,
                sort_dir: SortDirection::Desc,
            })
            .unwrap();
        assert_eq!(inbox_total, 0);

        let (snoozed, snoozed_total) = db
            .list_messages(&ListMessagesRequest {
                mailbox_id: None,
                account_id: None,
                unified: false,
                mailbox_role: None,
                local_only: false,
                snoozed_only: true,
                limit: 50,
                offset: 0,
                query: None,
                unread_only: false,
                starred_only: false,
                has_attachments: false,
                sort_by: MessageSortBy::Date,
                sort_dir: SortDirection::Desc,
            })
            .unwrap();
        assert_eq!(snoozed_total, 1);
        assert_eq!(snoozed[0].snoozed_until, Some(future));

        let due = db.list_due_snoozes(future + 1).unwrap();
        assert_eq!(due, vec![message_id]);
        db.unsnooze_message(message_id).unwrap();
        assert_eq!(db.count_active_snoozes().unwrap(), 0);
    }

    #[test]
    fn flags_catchup_does_not_demote_local_seen() {
        let db = Database::open_in_memory().unwrap();
        let account = sample_account();
        db.insert_account(&account).unwrap();
        let mailbox = MailboxRecord {
            id: Uuid::new_v4(),
            account_id: account.id,
            name: "INBOX".into(),
            role: Some("inbox".into()),
            uidvalidity: Some(1),
            uidnext: Some(2),
            unread_count: 0,
            total_count: 0,
        };
        db.upsert_mailbox(&mailbox).unwrap();
        let thread_id = Uuid::new_v4();
        db.upsert_thread(&ThreadRecord {
            id: thread_id,
            account_id: account.id,
            subject: "Seen".into(),
            last_message_at: 100,
            message_count: 1,
            unread_count: 0,
            participants: vec![AddressDto {
                name: None,
                email: "a@example.com".into(),
            }],
            snippet: "Seen".into(),
        })
        .unwrap();
        let message_id = Uuid::new_v4();
        db.insert_message(&MessageRecord {
            id: message_id,
            account_id: account.id,
            mailbox_id: mailbox.id,
            thread_id,
            uid: Some(42),
            message_id: Some("<seen@example.com>".into()),
            in_reply_to: None,
            references: vec![],
            subject: "Seen".into(),
            from: AddressDto {
                name: None,
                email: "a@example.com".into(),
            },
            to: vec![AddressDto {
                name: None,
                email: account.email.clone(),
            }],
            cc: vec![],
            date: 100,
            flags: FLAG_SEEN,
            snippet: "Seen".into(),
            body_text: Some("Seen".into()),
            body_html: None,
            has_attachments: false,
            raw_path: None,
            local_only: false,
            offline_at: None,
            size_bytes: None,
        })
        .unwrap();

        // Remote still missing \Seen — must not clear local read mark.
        let changed = db
            .set_flags_by_uid(mailbox.id, 42, false, false, false)
            .unwrap();
        assert!(!changed);
        assert!(!db.get_message(message_id).unwrap().summary.unread);

        // Explicit allow_unseen can demote.
        let changed = db
            .set_flags_by_uid(mailbox.id, 42, false, false, true)
            .unwrap();
        assert!(changed);
        assert!(db.get_message(message_id).unwrap().summary.unread);
    }

    #[test]
    fn list_message_ids_returns_all_matching() {
        let db = Database::open_in_memory().unwrap();
        let account = sample_account();
        db.insert_account(&account).unwrap();
        let mailbox = MailboxRecord {
            id: Uuid::new_v4(),
            account_id: account.id,
            name: "INBOX".into(),
            role: Some("inbox".into()),
            uidvalidity: Some(1),
            uidnext: Some(10),
            unread_count: 0,
            total_count: 0,
        };
        db.upsert_mailbox(&mailbox).unwrap();
        let mut ids = Vec::new();
        for i in 0..5 {
            let thread_id = Uuid::new_v4();
            db.upsert_thread(&ThreadRecord {
                id: thread_id,
                account_id: account.id,
                subject: format!("M{i}"),
                last_message_at: 100 + i,
                message_count: 1,
                unread_count: 1,
                participants: vec![AddressDto {
                    name: None,
                    email: "a@example.com".into(),
                }],
                snippet: format!("M{i}"),
            })
            .unwrap();
            let message_id = Uuid::new_v4();
            ids.push(message_id);
            db.insert_message(&MessageRecord {
                id: message_id,
                account_id: account.id,
                mailbox_id: mailbox.id,
                thread_id,
                uid: Some(i as u32 + 1),
                message_id: Some(format!("<m{i}@example.com>")),
                in_reply_to: None,
                references: vec![],
                subject: format!("M{i}"),
                from: AddressDto {
                    name: None,
                    email: "a@example.com".into(),
                },
                to: vec![AddressDto {
                    name: None,
                    email: account.email.clone(),
                }],
                cc: vec![],
                date: 100 + i,
                flags: 0,
                snippet: format!("M{i}"),
                body_text: Some(format!("M{i}")),
                body_html: None,
                has_attachments: false,
                raw_path: None,
                local_only: false,
                offline_at: None,
                size_bytes: None,
            })
            .unwrap();
        }
        let listed = db
            .list_message_ids(&ListMessagesRequest {
                mailbox_id: Some(mailbox.id),
                account_id: None,
                unified: false,
                mailbox_role: None,
                local_only: false,
                snoozed_only: false,
                limit: 2,
                offset: 0,
                query: None,
                unread_only: false,
                starred_only: false,
                has_attachments: false,
                sort_by: MessageSortBy::Date,
                sort_dir: SortDirection::Asc,
            })
            .unwrap();
        assert_eq!(listed.len(), 5);
        assert_eq!(listed, ids);
    }

    #[test]
    fn replace_attachments_preserves_stable_ids() {
        use crate::models::AttachmentRecord;
        let db = Database::open_in_memory().unwrap();
        let account = sample_account();
        db.insert_account(&account).unwrap();
        let mailbox = MailboxRecord {
            id: Uuid::new_v4(),
            account_id: account.id,
            name: "INBOX".into(),
            role: Some("inbox".into()),
            uidvalidity: Some(1),
            uidnext: Some(2),
            unread_count: 0,
            total_count: 0,
        };
        db.upsert_mailbox(&mailbox).unwrap();
        let thread_id = Uuid::new_v4();
        db.upsert_thread(&ThreadRecord {
            id: thread_id,
            account_id: account.id,
            subject: "Att".into(),
            last_message_at: 100,
            message_count: 1,
            unread_count: 0,
            participants: vec![AddressDto {
                name: None,
                email: "a@example.com".into(),
            }],
            snippet: "Att".into(),
        })
        .unwrap();
        let message_id = Uuid::new_v4();
        db.insert_message(&MessageRecord {
            id: message_id,
            account_id: account.id,
            mailbox_id: mailbox.id,
            thread_id,
            uid: Some(1),
            message_id: Some("<att@example.com>".into()),
            in_reply_to: None,
            references: vec![],
            subject: "Att".into(),
            from: AddressDto {
                name: None,
                email: "a@example.com".into(),
            },
            to: vec![AddressDto {
                name: None,
                email: account.email.clone(),
            }],
            cc: vec![],
            date: 100,
            flags: FLAG_SEEN,
            snippet: "Att".into(),
            body_text: Some("Att".into()),
            body_html: None,
            has_attachments: true,
            raw_path: None,
            local_only: false,
            offline_at: None,
            size_bytes: None,
        })
        .unwrap();
        let att_id = Uuid::new_v4();
        let dir = tempfile::tempdir().unwrap();
        let path1 = dir.path().join("a.pdf");
        std::fs::write(&path1, b"pdf1").unwrap();
        db.replace_attachments(
            message_id,
            &[AttachmentRecord {
                id: att_id,
                message_id,
                filename: "a.pdf".into(),
                mime: "application/pdf".into(),
                size: 4,
                path: path1.to_string_lossy().into_owned(),
                content_id: None,
            }],
        )
        .unwrap();
        let path2 = dir.path().join("a2.pdf");
        std::fs::write(&path2, b"pdf2").unwrap();
        let new_id = Uuid::new_v4();
        db.replace_attachments(
            message_id,
            &[AttachmentRecord {
                id: new_id,
                message_id,
                filename: "a.pdf".into(),
                mime: "application/pdf".into(),
                size: 4,
                path: path2.to_string_lossy().into_owned(),
                content_id: None,
            }],
        )
        .unwrap();
        let rows = db.list_attachments(message_id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, att_id);
        assert_eq!(rows[0].path, path2.to_string_lossy());
    }
}
