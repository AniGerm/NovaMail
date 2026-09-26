use std::path::{Path, PathBuf};
use std::sync::Arc;

use novamail_ipc::{
    AccountDto, AddressDto, AuthType, ListMessagesRequest, MailProvider, MailboxDto,
    MessageDetailDto, MessageSummaryDto,
};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::migrations;
use crate::models::{
    AccountRecord, AttachmentRecord, ContactRecord, LabelRecord, MailboxRecord, MessageRecord,
    RuleRecord, SignatureRecord, ThreadRecord, FLAG_ARCHIVED, FLAG_SEEN, FLAG_STARRED,
};
use novamail_ipc::{AttachmentDto, ContactDto, LabelDto, RuleDto, SignatureDto};
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
              id, name, email, provider, auth_type,
              imap_host, imap_port, imap_tls,
              smtp_host, smtp_port, smtp_tls, created_at
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
            "#,
            params![
                account.id.to_string(),
                account.name,
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
            SELECT id, name, email, provider, auth_type,
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
                email: row.get(2)?,
                provider: parse_provider(&row.get::<_, String>(3)?),
                auth_type: parse_auth(&row.get::<_, String>(4)?),
                imap_host: row.get(5)?,
                imap_port: row.get::<_, i64>(6)? as u16,
                imap_tls: row.get::<_, i64>(7)? != 0,
                smtp_host: row.get(8)?,
                smtp_port: row.get::<_, i64>(9)? as u16,
                smtp_tls: row.get::<_, i64>(10)? != 0,
                created_at: row.get(11)?,
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
            SELECT id, name, email, provider, auth_type,
                   imap_host, imap_port, imap_tls,
                   smtp_host, smtp_port, smtp_tls, created_at
            FROM accounts WHERE id = ?1
            "#,
            params![id.to_string()],
            |row| {
                Ok(AccountRecord {
                    id: parse_uuid(row.get::<_, String>(0)?)?,
                    name: row.get(1)?,
                    email: row.get(2)?,
                    provider: parse_provider(&row.get::<_, String>(3)?),
                    auth_type: parse_auth(&row.get::<_, String>(4)?),
                    imap_host: row.get(5)?,
                    imap_port: row.get::<_, i64>(6)? as u16,
                    imap_tls: row.get::<_, i64>(7)? != 0,
                    smtp_host: row.get(8)?,
                    smtp_port: row.get::<_, i64>(9)? as u16,
                    smtp_tls: row.get::<_, i64>(10)? != 0,
                    created_at: row.get(11)?,
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
              snippet, body_text, body_html, has_attachments, raw_path
            ) VALUES (
              ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19
            )
            ON CONFLICT(mailbox_id, uid) DO UPDATE SET
              flags = excluded.flags,
              subject = excluded.subject,
              snippet = excluded.snippet,
              body_text = COALESCE(excluded.body_text, messages.body_text),
              body_html = COALESCE(excluded.body_html, messages.body_html)
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
            ],
        )?;
        Ok(())
    }

    pub fn get_message(&self, id: Uuid) -> DbResult<MessageDetailDto> {
        let mut detail = {
            let conn = self.conn.lock();
            conn.query_row(
                r#"
                SELECT
                  m.id, m.account_id, m.mailbox_id, m.thread_id, m.subject,
                  m.from_json, m.to_json, m.date, m.snippet, m.flags, m.has_attachments,
                  a.email, m.body_text, m.body_html, m.message_id, m.in_reply_to, m.references_json
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

        let mut where_parts = Vec::new();
        let mut bind_ids: Vec<String> = Vec::new();

        if req.unified {
            where_parts.push(
                "(mb.role = 'inbox' OR lower(mb.name) = 'inbox' OR mb.name = 'INBOX')".into(),
            );
            where_parts.push(format!("(m.flags & {FLAG_ARCHIVED}) = 0"));
        } else if let Some(mailbox_id) = req.mailbox_id {
            where_parts.push("m.mailbox_id = ?1".into());
            bind_ids.push(mailbox_id.to_string());
        } else if let Some(account_id) = req.account_id {
            where_parts.push("m.account_id = ?1".into());
            bind_ids.push(account_id.to_string());
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
                  a.email
                FROM messages m
                JOIN mailboxes mb ON mb.id = m.mailbox_id
                JOIN accounts a ON a.id = m.account_id
                {where_sql}
                ORDER BY m.date DESC
                LIMIT ?1 OFFSET ?2
                "#
            )
        } else {
            format!(
                r#"
                SELECT
                  m.id, m.account_id, m.mailbox_id, m.thread_id, m.subject,
                  m.from_json, m.to_json, m.date, m.snippet, m.flags, m.has_attachments,
                  a.email
                FROM messages m
                JOIN mailboxes mb ON mb.id = m.mailbox_id
                JOIN accounts a ON a.id = m.account_id
                {where_sql}
                ORDER BY m.date DESC
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
        Ok((messages, total))
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
        let flags: i64 = conn
            .query_row(
                "SELECT flags FROM messages WHERE id = ?1",
                params![message_id.to_string()],
                |row| row.get(0),
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

    pub fn replace_attachments(
        &self,
        message_id: Uuid,
        attachments: &[AttachmentRecord],
    ) -> DbResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM attachments WHERE message_id = ?1",
            params![message_id.to_string()],
        )?;
        for attachment in attachments {
            conn.execute(
                r#"
                INSERT INTO attachments (id, message_id, filename, mime, size, path)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
                params![
                    attachment.id.to_string(),
                    message_id.to_string(),
                    attachment.filename,
                    attachment.mime,
                    attachment.size as i64,
                    attachment.path,
                ],
            )?;
        }
        Ok(())
    }

    pub fn list_attachments(&self, message_id: Uuid) -> DbResult<Vec<AttachmentDto>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, message_id, filename, mime, size, path FROM attachments WHERE message_id = ?1 ORDER BY filename",
        )?;
        let rows = stmt.query_map(params![message_id.to_string()], |row| {
            Ok(AttachmentDto {
                id: parse_uuid(row.get::<_, String>(0)?)?,
                message_id: parse_uuid(row.get::<_, String>(1)?)?,
                filename: row.get(2)?,
                mime: row.get(3)?,
                size: row.get::<_, i64>(4)? as u64,
                path: row.get(5)?,
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
            "SELECT id, message_id, filename, mime, size, path FROM attachments WHERE id = ?1",
            params![attachment_id.to_string()],
            |row| {
                Ok(AttachmentDto {
                    id: parse_uuid(row.get::<_, String>(0)?)?,
                    message_id: parse_uuid(row.get::<_, String>(1)?)?,
                    filename: row.get(2)?,
                    mime: row.get(3)?,
                    size: row.get::<_, i64>(4)? as u64,
                    path: row.get(5)?,
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

    pub fn upsert_contact(&self, contact: &ContactRecord) -> DbResult<()> {
        let conn = self.conn.lock();
        let emails = serde_json::to_string(&contact.emails)?;
        // phones stored inside notes_json extension until schema phones column exists —
        // migration v2 contacts table has emails_json/notes only. Store phones in notes prefix.
        let notes = serde_json::json!({
            "phones": contact.phones,
            "notes": contact.notes,
        })
        .to_string();
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
                notes,
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
        let rows = stmt.query_map([], |row| {
            let emails: Vec<String> = serde_json::from_str(&row.get::<_, String>(2)?)
                .unwrap_or_default();
            let notes_raw: String = row.get(3)?;
            let (phones, notes) = parse_contact_notes(&notes_raw);
            Ok(ContactDto {
                id: parse_uuid(row.get::<_, String>(0)?)?,
                display_name: row.get(1)?,
                emails,
                phones,
                notes,
                updated_at: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            let contact = row?;
            if let Some(q) = query {
                let q = q.to_ascii_lowercase();
                let hit = contact.display_name.to_ascii_lowercase().contains(&q)
                    || contact.emails.iter().any(|e| e.to_ascii_lowercase().contains(&q))
                    || contact.phones.iter().any(|p| p.contains(&q));
                if !hit {
                    continue;
                }
            }
            out.push(contact);
        }
        Ok(out)
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
            |row| {
                let emails: Vec<String> = serde_json::from_str(&row.get::<_, String>(2)?)
                    .unwrap_or_default();
                let notes_raw: String = row.get(3)?;
                let (phones, notes) = parse_contact_notes(&notes_raw);
                Ok(ContactDto {
                    id: parse_uuid(row.get::<_, String>(0)?)?,
                    display_name: row.get(1)?,
                    emails,
                    phones,
                    notes,
                    updated_at: row.get(4)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| DbError::NotFound(format!("contact {contact_id}")))
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

fn parse_contact_notes(raw: &str) -> (Vec<String>, String) {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
        let phones = value
            .get("phones")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let notes = value
            .get("notes")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        (phones, notes)
    } else {
        (Vec::new(), raw.to_string())
    }
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
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{FLAG_SEEN, MessageRecord, ThreadRecord};
    use novamail_ipc::{AddressDto, AuthType, MailProvider};

    fn sample_account() -> AccountRecord {
        AccountRecord {
            id: Uuid::new_v4(),
            name: "Work".into(),
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
            })
            .unwrap();
        }

        let (messages, total) = db
            .list_messages(&ListMessagesRequest {
                mailbox_id: None,
                account_id: None,
                unified: true,
                limit: 50,
                offset: 0,
                query: None,
            })
            .unwrap();

        assert_eq!(total, 2);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].subject, "Hello B");
        assert!(messages[0].unread);

        db.set_flags(messages[0].id, Some(false), Some(true))
            .unwrap();
        let detail = db.get_message(messages[0].id).unwrap();
        assert!(!detail.summary.unread);
        assert!(detail.summary.starred);
        let _ = FLAG_SEEN;
    }
}
