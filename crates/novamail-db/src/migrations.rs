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
    // v2 — attachments, labels, rules, contacts, calendar, ai, plugins
    r#"
    CREATE TABLE IF NOT EXISTS attachments (
      id TEXT PRIMARY KEY NOT NULL,
      message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
      filename TEXT NOT NULL,
      mime TEXT NOT NULL,
      size INTEGER NOT NULL DEFAULT 0,
      path TEXT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_attachments_message ON attachments(message_id);

    CREATE TABLE IF NOT EXISTS labels (
      id TEXT PRIMARY KEY NOT NULL,
      account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
      name TEXT NOT NULL,
      color TEXT NOT NULL DEFAULT '#0B6E4F',
      UNIQUE(account_id, name)
    );

    CREATE TABLE IF NOT EXISTS message_labels (
      message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
      label_id TEXT NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
      PRIMARY KEY(message_id, label_id)
    );

    CREATE TABLE IF NOT EXISTS rules (
      id TEXT PRIMARY KEY NOT NULL,
      account_id TEXT REFERENCES accounts(id) ON DELETE CASCADE,
      name TEXT NOT NULL,
      predicate_json TEXT NOT NULL,
      action_json TEXT NOT NULL,
      enabled INTEGER NOT NULL DEFAULT 1
    );

    CREATE TABLE IF NOT EXISTS contacts (
      id TEXT PRIMARY KEY NOT NULL,
      display_name TEXT NOT NULL,
      emails_json TEXT NOT NULL DEFAULT '[]',
      notes TEXT NOT NULL DEFAULT '',
      updated_at INTEGER NOT NULL
    );

    CREATE TABLE IF NOT EXISTS calendar_events (
      id TEXT PRIMARY KEY NOT NULL,
      account_id TEXT REFERENCES accounts(id) ON DELETE SET NULL,
      ical_uid TEXT,
      title TEXT NOT NULL,
      starts_at INTEGER NOT NULL,
      ends_at INTEGER,
      location TEXT,
      description TEXT
    );

    CREATE TABLE IF NOT EXISTS ai_insights (
      id TEXT PRIMARY KEY NOT NULL,
      message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
      kind TEXT NOT NULL,
      payload_json TEXT NOT NULL,
      created_at INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_ai_insights_message ON ai_insights(message_id);

    CREATE TABLE IF NOT EXISTS plugins (
      id TEXT PRIMARY KEY NOT NULL,
      name TEXT NOT NULL,
      version TEXT NOT NULL,
      enabled INTEGER NOT NULL DEFAULT 0,
      manifest_json TEXT NOT NULL
    );
    "#,
    // v3 — signatures + settings key/value
    r#"
    CREATE TABLE IF NOT EXISTS signatures (
      id TEXT PRIMARY KEY NOT NULL,
      account_id TEXT REFERENCES accounts(id) ON DELETE CASCADE,
      name TEXT NOT NULL,
      body_text TEXT NOT NULL DEFAULT '',
      is_default INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE IF NOT EXISTS settings (
      key TEXT PRIMARY KEY NOT NULL,
      value TEXT NOT NULL
    );
    "#,
    // v4 — background recipient index for composer autocomplete
    r#"
    CREATE TABLE IF NOT EXISTS known_recipients (
      email TEXT PRIMARY KEY NOT NULL,
      name TEXT NOT NULL DEFAULT '',
      last_seen INTEGER NOT NULL,
      seen_count INTEGER NOT NULL DEFAULT 1
    );

    CREATE INDEX IF NOT EXISTS idx_known_recipients_last_seen
      ON known_recipients(last_seen DESC);
    "#,
    // v5 — intelligent offline mailbox (quota offload)
    r#"
    ALTER TABLE messages ADD COLUMN local_only INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE messages ADD COLUMN offline_at INTEGER;
    ALTER TABLE messages ADD COLUMN size_bytes INTEGER;

    CREATE INDEX IF NOT EXISTS idx_messages_local_only
      ON messages(account_id, local_only, date ASC);
    CREATE INDEX IF NOT EXISTS idx_messages_offline_at
      ON messages(offline_at);
    "#,
    // v6 — snooze + send later
    r#"
    CREATE TABLE IF NOT EXISTS snoozed_messages (
      message_id TEXT PRIMARY KEY NOT NULL,
      account_id TEXT NOT NULL,
      wake_at INTEGER NOT NULL,
      previous_mailbox_id TEXT,
      created_at INTEGER NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_snoozed_wake
      ON snoozed_messages(wake_at);

    CREATE TABLE IF NOT EXISTS outbound_queue (
      id TEXT PRIMARY KEY NOT NULL,
      account_id TEXT NOT NULL,
      payload_json TEXT NOT NULL,
      send_at INTEGER NOT NULL,
      status TEXT NOT NULL,
      last_error TEXT,
      created_at INTEGER NOT NULL,
      sent_at INTEGER
    );

    CREATE INDEX IF NOT EXISTS idx_outbound_due
      ON outbound_queue(status, send_at);
    "#,
    // v7 — OpenPGP keyring
    r#"
    CREATE TABLE IF NOT EXISTS pgp_keys (
      fingerprint TEXT PRIMARY KEY NOT NULL,
      user_ids_json TEXT NOT NULL DEFAULT '[]',
      has_secret INTEGER NOT NULL DEFAULT 0,
      armored_public TEXT NOT NULL,
      armored_secret TEXT,
      created_at INTEGER NOT NULL
    );
    "#,
    // v8 — calendar accounts, tasks, richer events
    r#"
    CREATE TABLE IF NOT EXISTS calendar_accounts (
      id TEXT PRIMARY KEY NOT NULL,
      name TEXT NOT NULL,
      caldav_url TEXT NOT NULL,
      username TEXT NOT NULL DEFAULT '',
      created_at INTEGER NOT NULL
    );

    CREATE TABLE IF NOT EXISTS calendar_tasks (
      id TEXT PRIMARY KEY NOT NULL,
      calendar_account_id TEXT REFERENCES calendar_accounts(id) ON DELETE SET NULL,
      ical_uid TEXT,
      title TEXT NOT NULL,
      due_at INTEGER,
      completed INTEGER NOT NULL DEFAULT 0,
      notes TEXT NOT NULL DEFAULT '',
      source_message_id TEXT,
      created_at INTEGER NOT NULL,
      updated_at INTEGER NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_calendar_tasks_due
      ON calendar_tasks(due_at);

    ALTER TABLE calendar_events ADD COLUMN calendar_account_id TEXT;
    ALTER TABLE calendar_events ADD COLUMN all_day INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE calendar_events ADD COLUMN source_message_id TEXT;
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
        assert_eq!(count, 8);
        let attachments: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='attachments'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(attachments, 1);
    }
}
