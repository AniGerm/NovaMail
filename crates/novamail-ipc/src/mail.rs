use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MailboxDto {
    pub id: Uuid,
    pub account_id: Uuid,
    pub name: String,
    pub role: Option<String>,
    pub unread_count: u32,
    pub total_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AddressDto {
    pub name: Option<String>,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MessageSummaryDto {
    pub id: Uuid,
    pub account_id: Uuid,
    pub mailbox_id: Uuid,
    pub thread_id: Uuid,
    pub subject: String,
    pub from: AddressDto,
    pub to: Vec<AddressDto>,
    pub date: i64,
    pub snippet: String,
    pub unread: bool,
    pub starred: bool,
    pub has_attachments: bool,
    pub account_email: String,
    /// True when the message was offloaded from IMAP and exists only locally.
    #[serde(default)]
    pub local_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MessageDetailDto {
    pub summary: MessageSummaryDto,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    #[serde(default)]
    pub attachments: Vec<crate::AttachmentDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ThreadDto {
    pub id: Uuid,
    pub account_id: Uuid,
    pub subject: String,
    pub last_message_at: i64,
    pub message_count: u32,
    pub unread_count: u32,
    pub participants: Vec<AddressDto>,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum MessageSortBy {
    #[default]
    Date,
    Subject,
    From,
    Attachments,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum SortDirection {
    Asc,
    #[default]
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ListMessagesRequest {
    /// When set, restrict to one mailbox. When omitted with `unified = true`, query all inboxes.
    pub mailbox_id: Option<Uuid>,
    pub account_id: Option<Uuid>,
    pub unified: bool,
    /// When set (e.g. `"drafts"`), list messages in mailboxes with that role/name.
    #[serde(default)]
    pub mailbox_role: Option<String>,
    /// When true, only messages that were offloaded from IMAP (`local_only`).
    #[serde(default)]
    pub local_only: bool,
    pub limit: u32,
    pub offset: u32,
    pub query: Option<String>,
    #[serde(default)]
    pub unread_only: bool,
    #[serde(default)]
    pub starred_only: bool,
    #[serde(default)]
    pub has_attachments: bool,
    #[serde(default)]
    pub sort_by: MessageSortBy,
    #[serde(default)]
    pub sort_dir: SortDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ListMessagesResponse {
    pub messages: Vec<MessageSummaryDto>,
    pub total: u32,
}

/// One conversation row for the threaded inbox view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ThreadListItemDto {
    pub id: Uuid,
    pub account_id: Uuid,
    pub subject: String,
    pub snippet: String,
    pub last_message_at: i64,
    pub message_count: u32,
    pub unread_count: u32,
    pub has_attachments: bool,
    pub participants: Vec<AddressDto>,
    pub latest_from: AddressDto,
    pub account_email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ListThreadsResponse {
    pub threads: Vec<ThreadListItemDto>,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageRequest {
    pub account_id: Uuid,
    pub to: Vec<AddressDto>,
    pub cc: Vec<AddressDto>,
    pub bcc: Vec<AddressDto>,
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    #[serde(default)]
    pub attachments: Vec<crate::OutgoingAttachment>,
    /// When sending an edited draft, delete this local draft after success.
    #[serde(default)]
    pub draft_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SaveDraftRequest {
    /// Existing draft id when updating; omit to create a new draft.
    pub id: Option<Uuid>,
    pub account_id: Uuid,
    pub to: Vec<AddressDto>,
    #[serde(default)]
    pub cc: Vec<AddressDto>,
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub in_reply_to: Option<String>,
    #[serde(default)]
    pub references: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SetFlagsRequest {
    pub message_id: Uuid,
    pub unread: Option<bool>,
    pub starred: Option<bool>,
}
