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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventDto {
    pub id: Uuid,
    pub calendar_account_id: Option<Uuid>,
    pub ical_uid: Option<String>,
    pub title: String,
    pub starts_at: i64,
    pub ends_at: Option<i64>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub all_day: bool,
    pub source_message_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpsertCalendarEventRequest {
    pub id: Option<Uuid>,
    pub calendar_account_id: Option<Uuid>,
    pub title: String,
    pub starts_at: i64,
    pub ends_at: Option<i64>,
    pub location: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub all_day: bool,
    pub source_message_id: Option<Uuid>,
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
