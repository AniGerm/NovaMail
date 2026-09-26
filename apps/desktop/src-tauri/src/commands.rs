use novamail_ipc::{
    AccountDto, AddAccountOAuthRequest, AddAccountPasswordRequest, AppError, AttachmentDto,
    CardDavServerStatus, ContactDto, ContactsBookSettings, ExportBackupRequest,
    ExportBackupResponse, ImportBackupRequest, ImportBackupResult, LabelDto, LdapSearchRequest,
    LdapSyncRequest, LdapSyncResult, LdapSyncSettings, ListMessagesRequest, ListMessagesResponse,
    ListThreadsResponse, MailProvider, MailboxDto, MessageAiInsights, MessageDetailDto,
    MessageSummaryDto, OAuthExchangeRequest, OAuthExchangeResponse, ProviderPreset, RuleDto,
    SearchRequest, SearchResponse, SendMessageRequest, SetFlagsRequest, SetMessageLabelsRequest,
    SignatureDto, SuggestReplyMessageRequest, SuggestReplyMessageResponse, SummarizeMessageRequest,
    SummarizeMessageResponse, SyncProgressEvent, SyncRequest, SyncResult, UpsertContactRequest,
    UpsertLabelRequest, UpsertRuleRequest, UpsertSignatureRequest,
};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::state::DesktopState;

fn map_err(err: novamail_core::CoreError) -> AppError {
    err.into()
}

#[tauri::command]
pub fn provider_presets(state: State<'_, DesktopState>) -> Result<Vec<ProviderPreset>, AppError> {
    Ok(state.app.provider_presets())
}

#[tauri::command]
pub fn accounts_list(state: State<'_, DesktopState>) -> Result<Vec<AccountDto>, AppError> {
    state.app.list_accounts().map_err(map_err)
}

#[tauri::command]
pub async fn accounts_add_password(
    state: State<'_, DesktopState>,
    request: AddAccountPasswordRequest,
) -> Result<AccountDto, AppError> {
    state
        .app
        .add_account_password(request)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn accounts_add_oauth(
    state: State<'_, DesktopState>,
    request: AddAccountOAuthRequest,
) -> Result<AccountDto, AppError> {
    state.app.add_account_oauth(request).await.map_err(map_err)
}

#[tauri::command]
pub fn accounts_remove(
    state: State<'_, DesktopState>,
    account_id: Uuid,
) -> Result<(), AppError> {
    state.app.remove_account(account_id).map_err(map_err)
}

#[tauri::command]
pub fn mailboxes_list(
    state: State<'_, DesktopState>,
    account_id: Option<Uuid>,
) -> Result<Vec<MailboxDto>, AppError> {
    state.app.list_mailboxes(account_id).map_err(map_err)
}

#[tauri::command]
pub fn messages_list(
    state: State<'_, DesktopState>,
    request: ListMessagesRequest,
) -> Result<ListMessagesResponse, AppError> {
    state.app.list_messages(request).map_err(map_err)
}

#[tauri::command]
pub fn threads_list(
    state: State<'_, DesktopState>,
    request: ListMessagesRequest,
) -> Result<ListThreadsResponse, AppError> {
    state.app.list_threads(request).map_err(map_err)
}

#[tauri::command]
pub fn messages_list_by_thread(
    state: State<'_, DesktopState>,
    thread_id: Uuid,
) -> Result<Vec<MessageSummaryDto>, AppError> {
    state
        .app
        .list_messages_by_thread(thread_id)
        .map_err(map_err)
}

#[tauri::command]
pub fn messages_get(
    state: State<'_, DesktopState>,
    message_id: Uuid,
) -> Result<MessageDetailDto, AppError> {
    state.app.get_message(message_id).map_err(map_err)
}

#[tauri::command]
pub async fn messages_set_flags(
    state: State<'_, DesktopState>,
    request: SetFlagsRequest,
) -> Result<(), AppError> {
    state.app.set_flags(request).await.map_err(map_err)
}

#[tauri::command]
pub fn messages_search(
    state: State<'_, DesktopState>,
    request: SearchRequest,
) -> Result<SearchResponse, AppError> {
    state.app.search(request).map_err(map_err)
}

#[tauri::command]
pub async fn messages_send(
    state: State<'_, DesktopState>,
    request: SendMessageRequest,
) -> Result<(), AppError> {
    state.app.send_message(request).await.map_err(map_err)
}

#[tauri::command]
pub async fn mail_sync(
    app: AppHandle,
    state: State<'_, DesktopState>,
    request: SyncRequest,
) -> Result<Vec<SyncResult>, AppError> {
    let handle = app.clone();
    state
        .app
        .sync(request, move |event: SyncProgressEvent| {
            let _ = handle.emit("sync://progress", event);
        })
        .await
        .map_err(map_err)
}

#[tauri::command]
pub fn oauth_authorize_url(
    state: State<'_, DesktopState>,
    provider: MailProvider,
) -> Result<String, AppError> {
    state.app.oauth_authorize_url(provider).map_err(map_err)
}

#[tauri::command]
pub async fn oauth_exchange_code(
    state: State<'_, DesktopState>,
    request: OAuthExchangeRequest,
) -> Result<OAuthExchangeResponse, AppError> {
    state
        .app
        .oauth_exchange_code(request)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn ai_summarize_message(
    state: State<'_, DesktopState>,
    request: SummarizeMessageRequest,
) -> Result<SummarizeMessageResponse, AppError> {
    state
        .app
        .summarize_message(request)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn ai_suggest_reply(
    state: State<'_, DesktopState>,
    request: SuggestReplyMessageRequest,
) -> Result<SuggestReplyMessageResponse, AppError> {
    state
        .app
        .suggest_reply_message(request)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn oauth_wait_callback(
    state: State<'_, DesktopState>,
    timeout_secs: Option<u64>,
) -> Result<novamail_mail::OAuthCallbackResult, AppError> {
    state
        .app
        .oauth_wait_callback(timeout_secs.unwrap_or(180))
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn messages_archive(
    state: State<'_, DesktopState>,
    message_id: Uuid,
) -> Result<(), AppError> {
    state.app.archive_message(message_id).await.map_err(map_err)
}

#[tauri::command]
pub async fn messages_delete(
    state: State<'_, DesktopState>,
    message_id: Uuid,
) -> Result<(), AppError> {
    state.app.delete_message(message_id).await.map_err(map_err)
}

#[tauri::command]
pub fn messages_forward_draft(
    state: State<'_, DesktopState>,
    message_id: Uuid,
) -> Result<SendMessageRequest, AppError> {
    state.app.build_forward_draft(message_id).map_err(map_err)
}

#[tauri::command]
pub fn attachments_list(
    state: State<'_, DesktopState>,
    message_id: Uuid,
) -> Result<Vec<AttachmentDto>, AppError> {
    state.app.list_attachments(message_id).map_err(map_err)
}

#[tauri::command]
pub fn attachments_open_path(
    state: State<'_, DesktopState>,
    attachment_id: Uuid,
) -> Result<String, AppError> {
    state
        .app
        .open_attachment_path(attachment_id)
        .map_err(map_err)
}

#[tauri::command]
pub fn contacts_list(
    state: State<'_, DesktopState>,
    query: Option<String>,
) -> Result<Vec<ContactDto>, AppError> {
    state.app.list_contacts(query).map_err(map_err)
}

#[tauri::command]
pub fn contacts_upsert(
    state: State<'_, DesktopState>,
    request: UpsertContactRequest,
) -> Result<ContactDto, AppError> {
    state.app.upsert_contact(request).map_err(map_err)
}

#[tauri::command]
pub fn contacts_delete(
    state: State<'_, DesktopState>,
    contact_id: Uuid,
) -> Result<(), AppError> {
    state.app.delete_contact(contact_id).map_err(map_err)
}

#[tauri::command]
pub async fn carddav_start(
    state: State<'_, DesktopState>,
) -> Result<CardDavServerStatus, AppError> {
    state.app.start_carddav().await.map_err(map_err)
}

#[tauri::command]
pub fn carddav_stop(state: State<'_, DesktopState>) -> Result<CardDavServerStatus, AppError> {
    state.app.stop_carddav().map_err(map_err)
}

#[tauri::command]
pub fn carddav_status(state: State<'_, DesktopState>) -> Result<CardDavServerStatus, AppError> {
    state.app.carddav_status().map_err(map_err)
}

#[tauri::command]
pub fn ai_message_insights(
    state: State<'_, DesktopState>,
    message_id: Uuid,
) -> Result<MessageAiInsights, AppError> {
    state
        .app
        .get_message_ai_insights(message_id)
        .map_err(map_err)
}

#[tauri::command]
pub async fn ldap_search(
    state: State<'_, DesktopState>,
    request: LdapSearchRequest,
) -> Result<Vec<ContactDto>, AppError> {
    state.app.ldap_search(request).await.map_err(map_err)
}

#[tauri::command]
pub fn ldap_get_settings(
    state: State<'_, DesktopState>,
) -> Result<LdapSyncSettings, AppError> {
    state.app.ldap_get_settings().map_err(map_err)
}

#[tauri::command]
pub fn contacts_book_settings(
    state: State<'_, DesktopState>,
) -> Result<ContactsBookSettings, AppError> {
    state.app.contacts_book_settings().map_err(map_err)
}

#[tauri::command]
pub fn contacts_set_book_settings(
    state: State<'_, DesktopState>,
    settings: ContactsBookSettings,
) -> Result<ContactsBookSettings, AppError> {
    state
        .app
        .set_contacts_book_settings(settings)
        .map_err(map_err)
}

#[tauri::command]
pub async fn ldap_sync(
    state: State<'_, DesktopState>,
    request: LdapSyncRequest,
) -> Result<LdapSyncResult, AppError> {
    state.app.ldap_sync(request).await.map_err(map_err)
}

#[tauri::command]
pub fn labels_list(
    state: State<'_, DesktopState>,
    account_id: Option<Uuid>,
) -> Result<Vec<LabelDto>, AppError> {
    state.app.list_labels(account_id).map_err(map_err)
}

#[tauri::command]
pub fn labels_upsert(
    state: State<'_, DesktopState>,
    request: UpsertLabelRequest,
) -> Result<LabelDto, AppError> {
    state.app.upsert_label(request).map_err(map_err)
}

#[tauri::command]
pub fn labels_delete(state: State<'_, DesktopState>, label_id: Uuid) -> Result<(), AppError> {
    state.app.delete_label(label_id).map_err(map_err)
}

#[tauri::command]
pub fn messages_set_labels(
    state: State<'_, DesktopState>,
    request: SetMessageLabelsRequest,
) -> Result<(), AppError> {
    state.app.set_message_labels(request).map_err(map_err)
}

#[tauri::command]
pub fn messages_list_labels(
    state: State<'_, DesktopState>,
    message_id: Uuid,
) -> Result<Vec<LabelDto>, AppError> {
    state.app.list_message_labels(message_id).map_err(map_err)
}

#[tauri::command]
pub fn rules_list(state: State<'_, DesktopState>) -> Result<Vec<RuleDto>, AppError> {
    state.app.list_rules().map_err(map_err)
}

#[tauri::command]
pub fn rules_upsert(
    state: State<'_, DesktopState>,
    request: UpsertRuleRequest,
) -> Result<RuleDto, AppError> {
    state.app.upsert_rule(request).map_err(map_err)
}

#[tauri::command]
pub fn rules_delete(state: State<'_, DesktopState>, rule_id: Uuid) -> Result<(), AppError> {
    state.app.delete_rule(rule_id).map_err(map_err)
}

#[tauri::command]
pub fn signatures_list(
    state: State<'_, DesktopState>,
    account_id: Option<Uuid>,
) -> Result<Vec<SignatureDto>, AppError> {
    state.app.list_signatures(account_id).map_err(map_err)
}

#[tauri::command]
pub fn signatures_upsert(
    state: State<'_, DesktopState>,
    request: UpsertSignatureRequest,
) -> Result<SignatureDto, AppError> {
    state.app.upsert_signature(request).map_err(map_err)
}

#[tauri::command]
pub fn signatures_delete(
    state: State<'_, DesktopState>,
    signature_id: Uuid,
) -> Result<(), AppError> {
    state.app.delete_signature(signature_id).map_err(map_err)
}

#[tauri::command]
pub async fn pop3_test(
    state: State<'_, DesktopState>,
    host: String,
    port: u16,
    use_tls: bool,
    user: String,
    password: String,
) -> Result<u32, AppError> {
    state
        .app
        .pop3_test(host, port, use_tls, user, password)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub fn backup_export(
    state: State<'_, DesktopState>,
    request: ExportBackupRequest,
) -> Result<ExportBackupResponse, AppError> {
    state.app.export_backup(request).map_err(map_err)
}

#[tauri::command]
pub fn backup_import(
    state: State<'_, DesktopState>,
    request: ImportBackupRequest,
) -> Result<ImportBackupResult, AppError> {
    state.app.import_backup(request).map_err(map_err)
}
