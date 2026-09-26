use novamail_ipc::{AddressDto, AuthType, MailProvider};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const FLAG_SEEN: i64 = 1 << 0;
pub const FLAG_STARRED: i64 = 1 << 1;
pub const FLAG_ANSWERED: i64 = 1 << 2;
pub const FLAG_FLAGGED: i64 = 1 << 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountRecord {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub provider: MailProvider,
    pub auth_type: AuthType,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_tls: bool,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_tls: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailboxRecord {
    pub id: Uuid,
    pub account_id: Uuid,
    pub name: String,
    pub role: Option<String>,
    pub uidvalidity: Option<i64>,
    pub uidnext: Option<i64>,
    pub unread_count: u32,
    pub total_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadRecord {
    pub id: Uuid,
    pub account_id: Uuid,
    pub subject: String,
    pub last_message_at: i64,
    pub message_count: u32,
    pub unread_count: u32,
    pub participants: Vec<AddressDto>,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRecord {
    pub id: Uuid,
    pub account_id: Uuid,
    pub mailbox_id: Uuid,
    pub thread_id: Uuid,
    pub uid: Option<u32>,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    pub subject: String,
    pub from: AddressDto,
    pub to: Vec<AddressDto>,
    pub cc: Vec<AddressDto>,
    pub date: i64,
    pub flags: i64,
    pub snippet: String,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub has_attachments: bool,
    pub raw_path: Option<String>,
}

impl MessageRecord {
    pub fn unread(&self) -> bool {
        self.flags & FLAG_SEEN == 0
    }

    pub fn starred(&self) -> bool {
        self.flags & FLAG_STARRED != 0
    }
}
