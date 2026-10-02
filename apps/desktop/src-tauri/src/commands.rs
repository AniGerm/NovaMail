use novamail_ipc::{
    AccountDto, AccountQuotaDto, AddAccountOAuthRequest, AddAccountPasswordRequest,
    UpdateAccountRequest,
    AiInstallOllamaRequest, AiInstallOllamaResponse, AiInstallProgressEvent, AiPullModelRequest,
    AiPullModelResponse, AiPullProgressEvent, AiRuntimeStatus, AiSettings, AppError, AttachmentDto,
    CalDavCollectionDto, CalendarAccountDto, CalendarCollectionDto, CalendarEventDto,
    CalendarInvitationDto, CalendarTaskDto, CardDavServerStatus, ContactsBookSettings,
    ContactsShareMode, ContactsShareStatus, ContactDto, DiscoverCalDavRequest, ExportBackupRequest,
    ExportBackupResponse, FolderPoliciesDto, ImportBackupRequest, ImportBackupResult,
    JobsTickReport, LabelDto, LdapSearchRequest, LdapSyncRequest, LdapSyncResult, LdapSyncSettings,
    ListCalendarRangeRequest, ListMessagesRequest, ListMessagesResponse, ListThreadsResponse,
    MailProvider, MailboxDto, MessageAiInsights, MessageDetailDto, MessageSummaryDto,
    OptimizeDraftRequest, OptimizeDraftResponse,
    MoveMessageRequest, OAuthExchangeRequest, OAuthExchangeResponse, OfflineMailboxAccountPolicy,
    OfflineMailboxMode, OfflineMailboxSettingsDto, OfflineOffloadReport, OfflinePromptEvent,
    OutboundQueueItemDto, PgpDecryptResult, PgpGenerateRequest, PgpImportRequest, PgpKeyDto,
    PgpVerifyResult, PlannedSummaryDto, ProviderPreset, RecipientSuggestion,
    RespondInvitationRequest, RuleDto, SaveDraftRequest, SearchRequest, SearchResponse,
    SendLaterRequest, SendMessageRequest, SetContactsShareModeRequest, SetFlagsRequest,
    SetMessageLabelsRequest, SignatureDto, SnoozeRequest, SnoozedMessageDto, SpamScoreDto,
    SpamSettingsDto, SpellDictionaryDto, SpellcheckStatus, SuggestRepliesMessageRequest,
    SuggestRepliesMessageResponse, SuggestReplyMessageRequest, SuggestReplyMessageResponse,
    SummarizeMessageRequest, SummarizeMessageResponse, SyncProgressEvent, SyncRequest, SyncResult,
    UpsertCalendarAccountRequest, UpsertCalendarCollectionRequest, UpsertCalendarEventRequest,
    UpsertCalendarTaskRequest, UpsertContactRequest, UpsertLabelRequest, UpsertRuleRequest,
    UpsertSignatureRequest, AppVersionInfo, UpdateActionResult, UpdateCheckResult,
};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::state::DesktopState;
use crate::updater;

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
pub async fn accounts_update(
    state: State<'_, DesktopState>,
    request: UpdateAccountRequest,
) -> Result<AccountDto, AppError> {
    state.app.update_account(request).await.map_err(map_err)
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
pub async fn messages_save_draft(
    state: State<'_, DesktopState>,
    request: SaveDraftRequest,
) -> Result<MessageDetailDto, AppError> {
    state.app.save_draft(request).await.map_err(map_err)
}

#[tauri::command]
pub async fn mail_sync(
    app: AppHandle,
    state: State<'_, DesktopState>,
    request: SyncRequest,
) -> Result<Vec<SyncResult>, AppError> {
    let handle = app.clone();
    let handle_prompt = app.clone();
    state
        .app
        .sync(
            request,
            move |event: SyncProgressEvent| {
                let _ = handle.emit("sync://progress", event);
            },
            move |prompt: OfflinePromptEvent| {
                let _ = handle_prompt.emit("offline://prompt", prompt);
            },
        )
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
pub async fn ai_suggest_replies(
    state: State<'_, DesktopState>,
    request: SuggestRepliesMessageRequest,
) -> Result<SuggestRepliesMessageResponse, AppError> {
    state
        .app
        .suggest_replies_message(request)
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
pub fn recipients_suggest(
    state: State<'_, DesktopState>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<RecipientSuggestion>, AppError> {
    state
        .app
        .suggest_recipients(query, limit)
        .map_err(map_err)
}

#[tauri::command]
pub fn spellcheck_status(
    state: State<'_, DesktopState>,
) -> Result<SpellcheckStatus, AppError> {
    state.app.spellcheck_status().map_err(map_err)
}

#[tauri::command]
pub async fn spellcheck_install(
    state: State<'_, DesktopState>,
    code: String,
) -> Result<SpellDictionaryDto, AppError> {
    state.app.spellcheck_install(code).await.map_err(map_err)
}

#[tauri::command]
pub async fn spellcheck_ensure_for_locale(
    state: State<'_, DesktopState>,
    locale: String,
) -> Result<SpellDictionaryDto, AppError> {
    state
        .app
        .spellcheck_ensure_for_locale(locale)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub fn spellcheck_set_languages(
    app: AppHandle,
    languages: Vec<String>,
) -> Result<(), AppError> {
    #[cfg(target_os = "linux")]
    {
        use tauri::Manager;
        if let Some(window) = app.get_webview_window("main") {
            crate::set_webkit_spellcheck_languages(&window, &languages);
        }
    }
    let _ = app;
    let _ = languages;
    Ok(())
}

#[tauri::command]
pub async fn ai_optimize_draft(
    state: State<'_, DesktopState>,
    request: OptimizeDraftRequest,
) -> Result<OptimizeDraftResponse, AppError> {
    state.app.optimize_draft(request).await.map_err(map_err)
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
pub fn contacts_share_status(
    state: State<'_, DesktopState>,
) -> Result<ContactsShareStatus, AppError> {
    state.app.contacts_share_status().map_err(map_err)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn contacts_set_share_mode(
    state: State<'_, DesktopState>,
    mode: String,
    client_url: Option<String>,
    client_bind_dn: Option<String>,
    client_password: Option<String>,
    client_base_dn: Option<String>,
) -> Result<ContactsShareStatus, AppError> {
    let mode = match mode.trim().to_ascii_lowercase().as_str() {
        "server" => ContactsShareMode::Server,
        "client" => ContactsShareMode::Client,
        "local" => ContactsShareMode::Local,
        other => {
            return Err(AppError::new(
                "bad_request",
                format!("unknown share mode: {other}"),
            ))
        }
    };
    let app = state.app.clone();
    app.set_contacts_share_mode(SetContactsShareModeRequest {
        mode,
        client_url,
        client_bind_dn,
        client_password,
        client_base_dn,
    })
    .await
    .map_err(map_err)
}

#[tauri::command]
pub async fn contacts_client_sync(
    state: State<'_, DesktopState>,
) -> Result<LdapSyncResult, AppError> {
    state.app.contacts_client_sync().await.map_err(map_err)
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
pub fn ai_get_settings(state: State<'_, DesktopState>) -> Result<AiSettings, AppError> {
    state.app.ai_settings().map_err(map_err)
}

#[tauri::command]
pub fn ai_set_settings(
    state: State<'_, DesktopState>,
    settings: AiSettings,
) -> Result<AiSettings, AppError> {
    state.app.set_ai_settings(settings).map_err(map_err)
}

#[tauri::command]
pub async fn ai_runtime_status(
    state: State<'_, DesktopState>,
) -> Result<AiRuntimeStatus, AppError> {
    state.app.ai_runtime_status().await.map_err(map_err)
}

#[tauri::command]
pub async fn ai_pull_model(
    app: AppHandle,
    state: State<'_, DesktopState>,
    request: AiPullModelRequest,
) -> Result<AiPullModelResponse, AppError> {
    state
        .app
        .ai_pull_model(request, |event: AiPullProgressEvent| {
            let _ = app.emit("ai://pull-progress", &event);
        })
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn ai_install_ollama(
    app: AppHandle,
    state: State<'_, DesktopState>,
    request: AiInstallOllamaRequest,
) -> Result<AiInstallOllamaResponse, AppError> {
    state
        .app
        .ai_install_ollama(request, |event: AiInstallProgressEvent| {
            let _ = app.emit("ai://install-progress", &event);
        })
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn ai_start_ollama(
    state: State<'_, DesktopState>,
) -> Result<AiInstallOllamaResponse, AppError> {
    state.app.ai_start_ollama().await.map_err(map_err)
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
pub async fn messages_move(
    state: State<'_, DesktopState>,
    request: MoveMessageRequest,
) -> Result<(), AppError> {
    state.app.move_message(request).await.map_err(map_err)
}

#[tauri::command]
pub async fn messages_mark_spam(
    state: State<'_, DesktopState>,
    message_id: Uuid,
) -> Result<(), AppError> {
    state.app.mark_spam(message_id).await.map_err(map_err)
}

#[tauri::command]
pub async fn messages_mark_not_spam(
    state: State<'_, DesktopState>,
    message_id: Uuid,
) -> Result<(), AppError> {
    state.app.mark_not_spam(message_id).await.map_err(map_err)
}

#[tauri::command]
pub fn spam_get_settings(state: State<'_, DesktopState>) -> Result<SpamSettingsDto, AppError> {
    state.app.spam_settings().map_err(map_err)
}

#[tauri::command]
pub fn spam_set_settings(
    state: State<'_, DesktopState>,
    settings: SpamSettingsDto,
) -> Result<SpamSettingsDto, AppError> {
    state.app.set_spam_settings(settings).map_err(map_err)
}

#[tauri::command]
pub fn spam_score_message(
    state: State<'_, DesktopState>,
    message_id: Uuid,
) -> Result<SpamScoreDto, AppError> {
    state.app.score_spam(message_id).map_err(map_err)
}

#[tauri::command]
pub fn folder_policies_get(
    state: State<'_, DesktopState>,
) -> Result<FolderPoliciesDto, AppError> {
    state.app.folder_policies().map_err(map_err)
}

#[tauri::command]
pub fn folder_policies_set(
    state: State<'_, DesktopState>,
    policies: FolderPoliciesDto,
) -> Result<FolderPoliciesDto, AppError> {
    state.app.set_folder_policies(policies).map_err(map_err)
}

#[tauri::command]
pub fn folder_policies_apply(state: State<'_, DesktopState>) -> Result<u32, AppError> {
    state.app.apply_folder_retention().map_err(map_err)
}

#[tauri::command]
pub fn offline_mailbox_get(
    state: State<'_, DesktopState>,
) -> Result<OfflineMailboxSettingsDto, AppError> {
    state.app.offline_mailbox_settings().map_err(map_err)
}

#[tauri::command]
pub fn offline_mailbox_set(
    state: State<'_, DesktopState>,
    settings: OfflineMailboxSettingsDto,
) -> Result<OfflineMailboxSettingsDto, AppError> {
    state
        .app
        .set_offline_mailbox_settings(settings)
        .map_err(map_err)
}

#[tauri::command]
pub fn offline_mailbox_set_policy(
    state: State<'_, DesktopState>,
    policy: OfflineMailboxAccountPolicy,
) -> Result<OfflineMailboxAccountPolicy, AppError> {
    state.app.set_offline_account_policy(policy).map_err(map_err)
}

#[tauri::command]
pub async fn offline_mailbox_quota(
    state: State<'_, DesktopState>,
    account_id: Uuid,
) -> Result<AccountQuotaDto, AppError> {
    state.app.account_quota(account_id).await.map_err(map_err)
}

#[tauri::command]
pub fn offline_mailbox_local_count(
    state: State<'_, DesktopState>,
    account_id: Option<Uuid>,
) -> Result<u32, AppError> {
    state.app.local_only_count(account_id).map_err(map_err)
}

#[tauri::command]
pub async fn offline_mailbox_run(
    state: State<'_, DesktopState>,
    account_id: Uuid,
    force: bool,
) -> Result<OfflineOffloadReport, AppError> {
    state
        .app
        .run_offline_offload(account_id, force)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub fn offline_mailbox_dismiss_prompt(
    state: State<'_, DesktopState>,
    account_id: Uuid,
) -> Result<(), AppError> {
    state.app.dismiss_offline_prompt(account_id).map_err(map_err)
}

#[tauri::command]
pub fn offline_mailbox_enable_from_prompt(
    state: State<'_, DesktopState>,
    account_id: Uuid,
    mode: OfflineMailboxMode,
) -> Result<OfflineMailboxAccountPolicy, AppError> {
    state
        .app
        .enable_offline_from_prompt(account_id, mode)
        .map_err(map_err)
}

#[tauri::command]
pub fn messages_snooze(
    state: State<'_, DesktopState>,
    request: SnoozeRequest,
) -> Result<i64, AppError> {
    state.app.snooze_message(request).map_err(map_err)
}

#[tauri::command]
pub fn messages_unsnooze(
    state: State<'_, DesktopState>,
    message_id: Uuid,
) -> Result<(), AppError> {
    state.app.unsnooze_message(message_id).map_err(map_err)
}

#[tauri::command]
pub fn messages_list_snoozed(
    state: State<'_, DesktopState>,
    limit: Option<u32>,
) -> Result<Vec<SnoozedMessageDto>, AppError> {
    state
        .app
        .list_snoozed(limit.unwrap_or(200))
        .map_err(map_err)
}

#[tauri::command]
pub fn messages_send_later(
    state: State<'_, DesktopState>,
    request: SendLaterRequest,
) -> Result<Uuid, AppError> {
    state.app.enqueue_send_later(request).map_err(map_err)
}

#[tauri::command]
pub fn outbound_list(
    state: State<'_, DesktopState>,
    limit: Option<u32>,
) -> Result<Vec<OutboundQueueItemDto>, AppError> {
    state
        .app
        .list_outbound_queue(limit.unwrap_or(200))
        .map_err(map_err)
}

#[tauri::command]
pub fn outbound_cancel(
    state: State<'_, DesktopState>,
    id: Uuid,
) -> Result<(), AppError> {
    state.app.cancel_outbound(id).map_err(map_err)
}

#[tauri::command]
pub fn planned_summary(
    state: State<'_, DesktopState>,
) -> Result<PlannedSummaryDto, AppError> {
    state.app.planned_summary().map_err(map_err)
}

#[tauri::command]
pub async fn jobs_tick(state: State<'_, DesktopState>) -> Result<JobsTickReport, AppError> {
    state.app.run_jobs_tick().await.map_err(map_err)
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

#[tauri::command]
pub fn pgp_list_keys(state: State<'_, DesktopState>) -> Result<Vec<PgpKeyDto>, AppError> {
    state.app.pgp_list_keys().map_err(map_err)
}

#[tauri::command]
pub fn pgp_generate(
    state: State<'_, DesktopState>,
    request: PgpGenerateRequest,
) -> Result<PgpKeyDto, AppError> {
    state.app.pgp_generate(request).map_err(map_err)
}

#[tauri::command]
pub fn pgp_import(
    state: State<'_, DesktopState>,
    request: PgpImportRequest,
) -> Result<PgpKeyDto, AppError> {
    state.app.pgp_import(request).map_err(map_err)
}

#[tauri::command]
pub fn pgp_delete(state: State<'_, DesktopState>, fingerprint: String) -> Result<(), AppError> {
    state.app.pgp_delete(&fingerprint).map_err(map_err)
}

#[tauri::command]
pub fn pgp_export_public(
    state: State<'_, DesktopState>,
    fingerprint: String,
) -> Result<String, AppError> {
    state.app.pgp_export_public(&fingerprint).map_err(map_err)
}

#[tauri::command]
pub fn pgp_decrypt_text(
    state: State<'_, DesktopState>,
    armored: String,
) -> Result<PgpDecryptResult, AppError> {
    state.app.pgp_decrypt_text(&armored).map_err(map_err)
}

#[tauri::command]
pub fn pgp_verify_text(
    state: State<'_, DesktopState>,
    armored: String,
) -> Result<PgpVerifyResult, AppError> {
    state.app.pgp_verify_text(&armored).map_err(map_err)
}

#[tauri::command]
pub fn pgp_inspect_message(
    state: State<'_, DesktopState>,
    message_id: Uuid,
) -> Result<Option<PgpDecryptResult>, AppError> {
    state.app.pgp_inspect_message(message_id).map_err(map_err)
}

#[tauri::command]
pub fn calendar_accounts_list(
    state: State<'_, DesktopState>,
) -> Result<Vec<CalendarAccountDto>, AppError> {
    state.app.list_calendar_accounts().map_err(map_err)
}

#[tauri::command]
pub fn calendar_accounts_upsert(
    state: State<'_, DesktopState>,
    request: UpsertCalendarAccountRequest,
) -> Result<CalendarAccountDto, AppError> {
    state.app.upsert_calendar_account(request).map_err(map_err)
}

#[tauri::command]
pub fn calendar_accounts_delete(
    state: State<'_, DesktopState>,
    id: Uuid,
) -> Result<(), AppError> {
    state.app.delete_calendar_account(id).map_err(map_err)
}

#[tauri::command]
pub async fn calendar_accounts_sync(
    state: State<'_, DesktopState>,
    id: Uuid,
) -> Result<(u32, u32), AppError> {
    state.app.sync_calendar_account(id).await.map_err(map_err)
}

#[tauri::command]
pub fn calendar_events_list(
    state: State<'_, DesktopState>,
    request: ListCalendarRangeRequest,
) -> Result<Vec<CalendarEventDto>, AppError> {
    state.app.list_calendar_events(request).map_err(map_err)
}

#[tauri::command]
pub async fn calendar_discover(
    state: State<'_, DesktopState>,
    request: DiscoverCalDavRequest,
) -> Result<Vec<CalDavCollectionDto>, AppError> {
    state.app.discover_caldav(request).await.map_err(map_err)
}

#[tauri::command]
pub async fn calendar_events_upsert(
    state: State<'_, DesktopState>,
    request: UpsertCalendarEventRequest,
) -> Result<CalendarEventDto, AppError> {
    state
        .app
        .upsert_calendar_event(request)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn calendar_events_delete(
    state: State<'_, DesktopState>,
    id: Uuid,
) -> Result<(), AppError> {
    state.app.delete_calendar_event(id).await.map_err(map_err)
}

#[tauri::command]
pub fn calendar_tasks_list(
    state: State<'_, DesktopState>,
    include_completed: Option<bool>,
) -> Result<Vec<CalendarTaskDto>, AppError> {
    state
        .app
        .list_calendar_tasks(include_completed.unwrap_or(true))
        .map_err(map_err)
}

#[tauri::command]
pub fn calendar_tasks_upsert(
    state: State<'_, DesktopState>,
    request: UpsertCalendarTaskRequest,
) -> Result<CalendarTaskDto, AppError> {
    state.app.upsert_calendar_task(request).map_err(map_err)
}

#[tauri::command]
pub fn calendar_tasks_delete(
    state: State<'_, DesktopState>,
    id: Uuid,
) -> Result<(), AppError> {
    state.app.delete_calendar_task(id).map_err(map_err)
}

#[tauri::command]
pub async fn calendar_event_from_message(
    state: State<'_, DesktopState>,
    message_id: Uuid,
    starts_at: i64,
    ends_at: Option<i64>,
    calendar_account_id: Option<Uuid>,
) -> Result<CalendarEventDto, AppError> {
    state
        .app
        .create_event_from_message(message_id, starts_at, ends_at, calendar_account_id)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub fn calendar_task_from_message(
    state: State<'_, DesktopState>,
    message_id: Uuid,
    due_at: Option<i64>,
) -> Result<CalendarTaskDto, AppError> {
    state
        .app
        .create_task_from_message(message_id, due_at)
        .map_err(map_err)
}

#[tauri::command]
pub fn calendar_collections_list(
    state: State<'_, DesktopState>,
) -> Result<Vec<CalendarCollectionDto>, AppError> {
    state.app.list_calendar_collections().map_err(map_err)
}

#[tauri::command]
pub fn calendar_collections_upsert(
    state: State<'_, DesktopState>,
    request: UpsertCalendarCollectionRequest,
) -> Result<CalendarCollectionDto, AppError> {
    state.app.upsert_calendar_collection(request).map_err(map_err)
}

#[tauri::command]
pub fn calendar_collections_set_default(
    state: State<'_, DesktopState>,
    id: Uuid,
) -> Result<CalendarCollectionDto, AppError> {
    state
        .app
        .set_default_calendar_collection(id)
        .map_err(map_err)
}

#[tauri::command]
pub fn calendar_invitations_list(
    state: State<'_, DesktopState>,
    pending_only: Option<bool>,
) -> Result<Vec<CalendarInvitationDto>, AppError> {
    state
        .app
        .list_calendar_invitations(pending_only.unwrap_or(true))
        .map_err(map_err)
}

#[tauri::command]
pub async fn calendar_invitations_respond(
    state: State<'_, DesktopState>,
    request: RespondInvitationRequest,
) -> Result<CalendarInvitationDto, AppError> {
    state
        .app
        .respond_calendar_invitation(request)
        .await
        .map_err(map_err)
}

#[tauri::command]
pub fn app_version() -> AppVersionInfo {
    updater::app_version_info()
}

#[tauri::command]
pub fn logs_path() -> String {
    dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("novamail")
        .join("logs")
        .join("novamail.log")
        .to_string_lossy()
        .into_owned()
}

#[tauri::command]
pub async fn updates_check(app: AppHandle) -> Result<UpdateCheckResult, AppError> {
    updater::check_for_updates(app).await
}

#[tauri::command]
pub async fn updates_download(app: AppHandle) -> Result<UpdateActionResult, AppError> {
    updater::download_update(app).await
}

#[tauri::command]
pub async fn updates_install(app: AppHandle) -> Result<UpdateActionResult, AppError> {
    updater::install_update(app).await
}
