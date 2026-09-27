use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CalendarAccountDto {
    pub id: Uuid,
    pub name: String,
    pub caldav_url: String,
    pub username: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpsertCalendarAccountRequest {
    pub id: Option<Uuid>,
    pub name: String,
    pub caldav_url: String,
    pub username: String,
    /// Stored in OS keyring when provided.
    #[serde(default)]
    pub password: Option<String>,
    /// When set, create/update collections for this account after save.
    #[serde(default)]
    pub collections: Vec<CalDavCollectionDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CalendarCollectionDto {
    pub id: Uuid,
    pub calendar_account_id: Option<Uuid>,
    pub href: Option<String>,
    pub display_name: String,
    pub color: String,
    pub is_visible: bool,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpsertCalendarCollectionRequest {
    pub id: Option<Uuid>,
    pub calendar_account_id: Option<Uuid>,
    pub href: Option<String>,
    pub display_name: String,
    pub color: String,
    #[serde(default = "default_true")]
    pub is_visible: bool,
    #[serde(default)]
    pub is_default: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CalendarReminderDto {
    /// Minutes before start (0 = at start).
    pub minutes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CalendarAttendeeDto {
    pub email: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub partstat: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventDto {
    pub id: Uuid,
    pub calendar_account_id: Option<Uuid>,
    pub collection_id: Option<Uuid>,
    pub ical_uid: Option<String>,
    pub title: String,
    pub starts_at: i64,
    pub ends_at: Option<i64>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub all_day: bool,
    pub source_message_id: Option<Uuid>,
    #[serde(default)]
    pub reminders: Vec<CalendarReminderDto>,
    #[serde(default)]
    pub status: String,
    pub organizer: Option<String>,
    #[serde(default)]
    pub attendees: Vec<CalendarAttendeeDto>,
    /// Collection color for UI chips (joined when listing).
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpsertCalendarEventRequest {
    pub id: Option<Uuid>,
    pub calendar_account_id: Option<Uuid>,
    pub collection_id: Option<Uuid>,
    pub title: String,
    pub starts_at: i64,
    pub ends_at: Option<i64>,
    pub location: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub all_day: bool,
    pub source_message_id: Option<Uuid>,
    #[serde(default)]
    pub reminders: Vec<CalendarReminderDto>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CalendarTaskDto {
    pub id: Uuid,
    pub calendar_account_id: Option<Uuid>,
    pub ical_uid: Option<String>,
    pub title: String,
    pub due_at: Option<i64>,
    pub completed: bool,
    pub notes: String,
    pub source_message_id: Option<Uuid>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpsertCalendarTaskRequest {
    pub id: Option<Uuid>,
    pub calendar_account_id: Option<Uuid>,
    pub title: String,
    pub due_at: Option<i64>,
    #[serde(default)]
    pub completed: bool,
    #[serde(default)]
    pub notes: String,
    pub source_message_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ListCalendarRangeRequest {
    pub from: i64,
    pub to: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CalDavCollectionDto {
    pub href: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverCalDavRequest {
    pub caldav_url: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CalendarInvitationDto {
    pub id: Uuid,
    pub message_id: Option<Uuid>,
    pub ical_uid: String,
    pub title: String,
    pub starts_at: i64,
    pub ends_at: Option<i64>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub organizer: Option<String>,
    pub partstat: String,
    pub received_at: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum InvitationResponse {
    Accept,
    Decline,
    Tentative,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RespondInvitationRequest {
    pub id: Uuid,
    pub response: InvitationResponse,
}
