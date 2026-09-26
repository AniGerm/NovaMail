use rusqlite::Connection;

use crate::DbResult;

const MIGRATIONS: &[&str] = &[
    // v1 — core mail schema + FTS5
    r#"
    PRAGMA foreign_keys = ON;

    CREATE TABLE IF NOT EXISTS schema_migrations (
      version INTEGER PRIMARY KEY NOT NULL,
      applied_at INTEGER NOT NULL
    );

    CREATE TABLE IF NOT EXISTS accounts (
      id TEXT PRIMARY KEY NOT NULL,
      name TEXT NOT NULL,
      email TEXT NOT NULL UNIQUE,
      provider TEXT NOT NULL,
      auth_type TEXT NOT NULL,
      imap_host TEXT NOT NULL,
      imap_port INTEGER NOT NULL,
      imap_tls INTEGER NOT NULL DEFAULT 1,
      smtp_host TEXT NOT NULL,
      smtp_port INTEGER NOT NULL,
      smtp_tls INTEGER NOT NULL DEFAULT 1,
      created_at INTEGER NOT NULL
    );

    CREATE TABLE IF NOT EXISTS mailboxes (
      id TEXT PRIMARY KEY NOT NULL,
      account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
      name TEXT NOT NULL,
      role TEXT,
      uidvalidity INTEGER,
      uidnext INTEGER,
      unread_count INTEGER NOT NULL DEFAULT 0,
      total_count INTEGER NOT NULL DEFAULT 0,
      UNIQUE(account_id, name)
    );

    CREATE TABLE IF NOT EXISTS threads (
      id TEXT PRIMARY KEY NOT NULL,
      account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
      subject TEXT NOT NULL,
      last_message_at INTEGER NOT NULL,
      message_count INTEGER NOT NULL DEFAULT 0,
      unread_count INTEGER NOT NULL DEFAULT 0,
      participants_json TEXT NOT NULL DEFAULT '[]',
      snippet TEXT NOT NULL DEFAULT ''
    );

    CREATE TABLE IF NOT EXISTS messages (
      id TEXT PRIMARY KEY NOT NULL,
      account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
      mailbox_id TEXT NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
      thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
      uid INTEGER,
      message_id TEXT,
      in_reply_to TEXT,
      references_json TEXT NOT NULL DEFAULT '[]',
      subject TEXT NOT NULL,
      from_json TEXT NOT NULL,
      to_json TEXT NOT NULL DEFAULT '[]',
      cc_json TEXT NOT NULL DEFAULT '[]',
      date INTEGER NOT NULL,
      flags INTEGER NOT NULL DEFAULT 0,
      snippet TEXT NOT NULL DEFAULT '',
      body_text TEXT,
      body_html TEXT,
      has_attachments INTEGER NOT NULL DEFAULT 0,
      raw_path TEXT,
      UNIQUE(mailbox_id, uid)
    );

    CREATE INDEX IF NOT EXISTS idx_messages_mailbox_date
      ON messages(mailbox_id, date DESC);
    CREATE INDEX IF NOT EXISTS idx_messages_thread_date
      ON messages(thread_id, date ASC);
    CREATE INDEX IF NOT EXISTS idx_messages_account_date
      ON messages(account_id, date DESC);
    CREATE INDEX IF NOT EXISTS idx_messages_message_id
      ON messages(message_id);
    CREATE INDEX IF NOT EXISTS idx_mailboxes_role
      ON mailboxes(account_id, role);

    CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
      subject,
      snippet,
      body_text,
      content='messages',
      content_rowid='rowid'
    );

    CREATE TRIGGER IF NOT EXISTS messages_ai AFTER INSERT ON messages BEGIN
      INSERT INTO messages_fts(rowid, subject, snippet, body_text)
      VALUES (new.rowid, new.subject, new.snippet, coalesce(new.body_text, ''));
    END;

    CREATE TRIGGER IF NOT EXISTS messages_ad AFTER DELETE ON messages BEGIN
      INSERT INTO messages_fts(messages_fts, rowid, subject, snippet, body_text)
      VALUES ('delete', old.rowid, old.subject, old.snippet, coalesce(old.body_text, ''));
    END;

    CREATE TRIGGER IF NOT EXISTS messages_au AFTER UPDATE ON messages BEGIN
      INSERT INTO messages_fts(messages_fts, rowid, subject, snippet, body_text)
      VALUES ('delete', old.rowid, old.subject, old.snippet, coalesce(old.body_text, ''));
      INSERT INTO messages_fts(rowid, subject, snippet, body_text)
      VALUES (new.rowid, new.subject, new.snippet, coalesce(new.body_text, ''));
    END;
    "#,
];

pub fn migrate(conn: &Connection) -> DbResult<()> {
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS schema_migrations (
          version INTEGER PRIMARY KEY NOT NULL,
          applied_at INTEGER NOT NULL
        );
        "#,
    )?;

    let current: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    for (idx, sql) in MIGRATIONS.iter().enumerate() {
        let version = (idx + 1) as i64;
        if version <= current {
            continue;
        }
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT INTO schema_migrations(version, applied_at) VALUES (?1, ?2)",
            rusqlite::params![version, chrono::Utc::now().timestamp()],
        )?;
        tx.commit()?;
        tracing::info!(version, "applied database migration");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn migrates_fresh_database() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
