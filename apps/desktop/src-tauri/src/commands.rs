use novamail_ipc::{
    AccountDto, AddAccountOAuthRequest, AddAccountPasswordRequest, AppError, ListMessagesRequest,
    ListMessagesResponse, MailProvider, MailboxDto, MessageDetailDto, OAuthExchangeRequest,
    OAuthExchangeResponse, ProviderPreset, SearchRequest, SearchResponse, SendMessageRequest,
    SetFlagsRequest, SuggestReplyMessageRequest, SuggestReplyMessageResponse,
    SummarizeMessageRequest, SummarizeMessageResponse, SyncProgressEvent, SyncRequest, SyncResult,
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
pub fn messages_get(
    state: State<'_, DesktopState>,
    message_id: Uuid,
) -> Result<MessageDetailDto, AppError> {
    state.app.get_message(message_id).map_err(map_err)
}

#[tauri::command]
pub fn messages_set_flags(
    state: State<'_, DesktopState>,
    request: SetFlagsRequest,
) -> Result<(), AppError> {
    state.app.set_flags(request).map_err(map_err)
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
pub fn messages_archive(
    state: State<'_, DesktopState>,
    message_id: Uuid,
) -> Result<(), AppError> {
    state.app.archive_message(message_id).map_err(map_err)
}

#[tauri::command]
pub fn messages_forward_draft(
    state: State<'_, DesktopState>,
    message_id: Uuid,
) -> Result<SendMessageRequest, AppError> {
    state.app.build_forward_draft(message_id).map_err(map_err)
}
