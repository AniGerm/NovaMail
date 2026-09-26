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
    AccountRecord, MailboxRecord, MessageRecord, ThreadRecord, FLAG_ARCHIVED, FLAG_SEEN,
    FLAG_STARRED,
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
                })
            },
        )
        .optional()?
        .ok_or_else(|| DbError::NotFound(format!("message {id}")))
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
    }
}

fn parse_provider(value: &str) -> MailProvider {
    match value {
        "gmail" => MailProvider::Gmail,
        "microsoft365" => MailProvider::Microsoft365,
        "yahoo" => MailProvider::Yahoo,
        "protonBridge" => MailProvider::ProtonBridge,
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
