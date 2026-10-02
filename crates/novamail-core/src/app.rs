use std::sync::Arc;

use novamail_ai::{
    allow_model_pick, default_ollama_model, default_ollama_url, detect_nvidia_gpu,
    ensure_ollama_running, install_ollama_system, install_ollama_user, list_ollama_models,
    normalize_model_ref, probe_ollama, pull_ollama_model, recommended_model, AiProvider,
    ExtractEventsRequest, NullAiProvider, OllamaProvider, PrioritizeRequest, SuggestReplyRequest,
    SummarizeRequest, DEFAULT_MODEL,
};
use parking_lot::RwLock;
use novamail_contacts::{
    search_ldap, CardDavServer, LdapServer, DEFAULT_BASE_DN, DEFAULT_BIND_DN,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use novamail_crypto::{
    decrypt_backup_payload, decrypt_message, encrypt_backup_payload, encrypt_message, generate_key,
    import_armored, looks_like_pgp, sign_message, verify_message, AccountCredentials,
    EncryptedBackupFile, OAuthTokens, SecretStore,
};
use novamail_db::{AccountRecord, ContactRecord, Database, LabelRecord, RuleRecord, SignatureRecord};
use novamail_ipc::{
    AccountDto, AccountQuotaDto, AddAccountOAuthRequest, AddAccountPasswordRequest, AddressDto,
    UpdateAccountRequest,
    AiInstallOllamaRequest, AiInstallOllamaResponse, AiInstallProgressEvent, AiPullModelRequest,
    AiPullModelResponse, AiPullProgressEvent, AiRuntimeStatus, AiSettings, AttachmentDto,
    BackupAccount, BackupAccountCredentials, BackupContact, BackupPayload, CalDavCollectionDto,
    CalendarAccountDto, CalendarCollectionDto, CalendarEventDto, CalendarInvitationDto,
    CalendarReminderDto, CalendarTaskDto, CardDavServerStatus, ContactDto, ContactsBookSettings,
    ContactsShareMode, ContactsShareStatus, DiscoverCalDavRequest, EventSuggestionDto,
    ExportBackupRequest, ExportBackupResponse, FolderPoliciesDto, FolderPolicyDto,
    ImportBackupRequest, ImportBackupResult, InvitationResponse, JobsTickReport, LabelDto,
    LdapSearchRequest, LdapSyncRequest, LdapSyncResult, LdapSyncSettings, ListCalendarRangeRequest,
    ListMessagesRequest, ListMessagesResponse, ListThreadsResponse, MailboxDto, MessageAiInsights,
    MessageSortBy, OptimizeDraftRequest, OptimizeDraftResponse, SortDirection,
    MessageDetailDto, MessageSummaryDto, MoveMessageRequest, OAuthExchangeRequest,
    OAuthExchangeResponse, OAuthTokensDto, OfflineMailboxAccountPolicy, OfflineMailboxMode,
    OfflineMailboxSettingsDto, OfflineOffloadReport, OfflinePromptEvent, OutboundQueueItemDto,
    OutboundStatus, PgpDecryptResult, PgpGenerateRequest, PgpImportRequest, PgpKeyDto,
    PgpVerifyResult, PlannedSummaryDto, ProviderPreset, RecipientSuggestion, RespondInvitationRequest,
    RetentionModeDto, RuleDto, SaveDraftRequest, SearchRequest, SearchResponse, SendLaterRequest,
    SendMessageRequest, SetContactsShareModeRequest, SetFlagsRequest, SetMessageLabelsRequest,
    SignatureDto, SnoozePreset, SnoozeRequest, SnoozedMessageDto, SpamScoreDto, SpamSettingsDto,
    SuggestRepliesMessageRequest, SuggestRepliesMessageResponse, SuggestReplyMessageRequest,
    SuggestReplyMessageResponse, SummarizeMessageRequest, SummarizeMessageResponse,
    SyncProgressEvent, SyncRequest, SyncResult, UpsertCalendarAccountRequest,
    UpsertCalendarCollectionRequest, UpsertCalendarEventRequest, UpsertCalendarTaskRequest,
    UpsertContactRequest, UpsertLabelRequest, UpsertRuleRequest, UpsertSignatureRequest,
};
use novamail_mail::{
    append_sent_remote, archive_remote, delete_remote, execute_delete_remote, move_remote,
    offload_message_remote, plan_delete_remote, probe_account_quota, refetch_message_content,
    save_draft_remote, set_flags_remote, set_flags_remote_batch, OAuthConfig, Pop3Client,
    SmtpClient, SyncEngine,
};
use novamail_rules::{evaluate_rules, Action, RuleDefinition, RuleMatchContext};
use novamail_search::SearchService;
use uuid::Uuid;

use crate::contacts_store::DbContactStore;
use crate::folder_policies::{
    FolderPolicies, FolderPolicy, RetentionMode, SETTINGS_KEY as FOLDER_POLICIES_KEY,
    SPAM_SETTINGS_KEY,
};
use crate::offline_mailbox::{
    self, OfflineMailboxSettings, SETTINGS_KEY as OFFLINE_MAILBOX_KEY,
};
use crate::paths::AppPaths;
use crate::sanitize::sanitize_html;
use crate::spam::{self, SpamSettings};
use crate::{CoreError, CoreResult};

pub struct AppState {
    pub paths: AppPaths,
    pub db: Database,
    pub secrets: SecretStore,
    ai: RwLock<Arc<dyn AiProvider>>,
    carddav: CardDavServer,
    ldap_server: LdapServer,
}

impl AppState {
    pub fn initialize(paths: AppPaths) -> CoreResult<Self> {
        paths.ensure().map_err(|e| CoreError::Message(e.to_string()))?;
        let db = Database::open(&paths.db_path)?;
        // Memory fallback keeps headless CI / missing Secret Service usable.
        let secrets = SecretStore::with_memory_fallback(true);
        let store = DbContactStore::new(db.clone());
        let carddav = CardDavServer::new(store.clone());
        let ldap_server = LdapServer::new(store);
        let settings = Self::load_ai_settings(&db);
        Ok(Self {
            paths,
            db,
            secrets,
            ai: RwLock::new(Self::provider_from_settings(&settings)),
            carddav,
            ldap_server,
        })
    }

    fn load_ai_settings(db: &Database) -> AiSettings {
        match db.get_setting("ai.settings") {
            Ok(Some(raw)) => serde_json::from_str(&raw).unwrap_or_else(|_| AiSettings {
                enabled: true,
                model: default_ollama_model(),
                base_url: default_ollama_url(),
                onboarding_completed: true,
            }),
            _ => AiSettings {
                enabled: false,
                model: default_ollama_model(),
                base_url: default_ollama_url(),
                onboarding_completed: false,
            },
        }
    }

    fn provider_from_settings(settings: &AiSettings) -> Arc<dyn AiProvider> {
        if settings.enabled {
            Arc::new(OllamaProvider::new(
                settings.base_url.clone(),
                settings.model.clone(),
            ))
        } else {
            Arc::new(NullAiProvider)
        }
    }

    fn ai_provider(&self) -> Arc<dyn AiProvider> {
        self.ai.read().clone()
    }

    fn offline_ai() -> NullAiProvider {
        NullAiProvider
    }

    pub fn ai_settings(&self) -> CoreResult<AiSettings> {
        Ok(Self::load_ai_settings(&self.db))
    }

    pub fn set_ai_settings(&self, mut settings: AiSettings) -> CoreResult<AiSettings> {
        if settings.model.trim().is_empty() {
            settings.model = DEFAULT_MODEL.to_string();
        }
        if settings.base_url.trim().is_empty() {
            settings.base_url = default_ollama_url();
        }
        settings.model = settings.model.trim().to_string();
        settings.base_url = settings.base_url.trim_end_matches('/').to_string();
        if settings.enabled {
            settings.onboarding_completed = true;
        }
        let raw =
            serde_json::to_string(&settings).map_err(|e| CoreError::Message(e.to_string()))?;
        self.db.set_setting("ai.settings", &raw)?;
        *self.ai.write() = Self::provider_from_settings(&settings);
        if settings.enabled {
            self.enqueue_missing_ai_insights(25);
        }
        Ok(settings)
    }

    pub async fn ai_runtime_status(&self) -> CoreResult<AiRuntimeStatus> {
        let settings = self.ai_settings()?;
        let nvidia_gpu = detect_nvidia_gpu();
        let presence = probe_ollama(&settings.base_url).await;
        let models = if presence.reachable {
            list_ollama_models(&settings.base_url)
                .await
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let recommended = recommended_model(nvidia_gpu, &models);
        let allow_pick = allow_model_pick(nvidia_gpu, &models);
        Ok(AiRuntimeStatus {
            ollama_reachable: presence.reachable,
            ollama_installed: presence.installed,
            ollama_binary: presence.binary_path,
            can_install_user: presence.can_install_user,
            can_install_system: presence.can_install_system,
            nvidia_gpu,
            models,
            recommended_model: recommended,
            allow_model_pick: allow_pick,
        })
    }

    pub async fn ai_install_ollama<F>(
        &self,
        request: AiInstallOllamaRequest,
        mut on_progress: F,
    ) -> CoreResult<AiInstallOllamaResponse>
    where
        F: FnMut(AiInstallProgressEvent) + Send,
    {
        let settings = self.ai_settings()?;
        let mode = request.mode.trim().to_ascii_lowercase();
        let binary_path = match mode.as_str() {
            "user" => {
                let path = install_ollama_user(|p| {
                    on_progress(AiInstallProgressEvent {
                        status: p.status,
                        done: p.done,
                    });
                })
                .await
                .map_err(|e| CoreError::Message(e.to_string()))?;
                Some(path.display().to_string())
            }
            "system" => {
                install_ollama_system(|p| {
                    on_progress(AiInstallProgressEvent {
                        status: p.status,
                        done: p.done,
                    });
                })
                .map_err(|e| CoreError::Message(e.to_string()))?;
                None
            }
            _ => {
                return Err(CoreError::Message(
                    "Install mode must be 'user' or 'system'".into(),
                ));
            }
        };

        on_progress(AiInstallProgressEvent {
            status: "starting".into(),
            done: false,
        });
        ensure_ollama_running(&settings.base_url)
            .await
            .map_err(|e| CoreError::Message(e.to_string()))?;
        on_progress(AiInstallProgressEvent {
            status: "ready".into(),
            done: true,
        });
        Ok(AiInstallOllamaResponse {
            binary_path,
            reachable: true,
        })
    }

    pub async fn ai_start_ollama(&self) -> CoreResult<AiInstallOllamaResponse> {
        let settings = self.ai_settings()?;
        ensure_ollama_running(&settings.base_url)
            .await
            .map_err(|e| CoreError::Message(e.to_string()))?;
        let presence = probe_ollama(&settings.base_url).await;
        Ok(AiInstallOllamaResponse {
            binary_path: presence.binary_path,
            reachable: presence.reachable,
        })
    }

    /// Pull a model (Ollama library name or Hugging Face / Ollama URL) via local Ollama.
    pub async fn ai_pull_model<F>(
        &self,
        request: AiPullModelRequest,
        mut on_progress: F,
    ) -> CoreResult<AiPullModelResponse>
    where
        F: FnMut(AiPullProgressEvent) + Send,
    {
        let settings = self.ai_settings()?;
        let model_ref = normalize_model_ref(&request.model)
            .map_err(|e| CoreError::Message(e.to_string()))?;
        let pulled = pull_ollama_model(&settings.base_url, &model_ref, |progress| {
            on_progress(AiPullProgressEvent {
                model: model_ref.clone(),
                status: progress.status,
                digest: progress.digest,
                total: progress.total,
                completed: progress.completed,
                done: progress.done,
            });
        })
        .await
        .map_err(|e| CoreError::Message(e.to_string()))?;
        Ok(AiPullModelResponse { model: pulled })
    }

    pub fn provider_presets(&self) -> Vec<ProviderPreset> {
        let mut presets = ProviderPreset::all();
        for preset in &mut presets {
            let oauth_env = match preset.provider {
                novamail_ipc::MailProvider::Gmail => "NOVAMAIL_GOOGLE_CLIENT_ID",
                novamail_ipc::MailProvider::Microsoft365 => "NOVAMAIL_MS_CLIENT_ID",
                novamail_ipc::MailProvider::Yahoo => "NOVAMAIL_YAHOO_CLIENT_ID",
                _ => continue,
            };
            let has_oauth = std::env::var(oauth_env)
                .map(|v| !v.trim().is_empty())
                .unwrap_or(false);
            match preset.provider {
                novamail_ipc::MailProvider::Yahoo if has_oauth => {
                    preset.auth_type = novamail_ipc::AuthType::OAuth2;
                    preset.oauth_authorize_url =
                        Some("https://api.login.yahoo.com/oauth2/request_auth".into());
                }
                novamail_ipc::MailProvider::Gmail | novamail_ipc::MailProvider::Microsoft365
                    if !has_oauth =>
                {
                    // Keep OAuth UI, but authorize will return a clear error.
                    // Gmail/Microsoft basic auth is broadly blocked; don't pretend password works.
                }
                _ => {}
            }
        }
        presets
    }

    pub fn list_accounts(&self) -> CoreResult<Vec<AccountDto>> {
        Ok(self.db.list_accounts()?)
    }

    pub async fn add_account_password(
        &self,
        request: AddAccountPasswordRequest,
    ) -> CoreResult<AccountDto> {
        let id = Uuid::new_v4();
        let account = AccountRecord {
            id,
            name: request.name,
            label: request.label,
            email: request.email,
            provider: request.provider,
            auth_type: novamail_ipc::AuthType::Password,
            imap_host: request.imap_host,
            imap_port: request.imap_port,
            imap_tls: request.imap_tls,
            smtp_host: request.smtp_host,
            smtp_port: request.smtp_port,
            smtp_tls: request.smtp_tls,
            created_at: chrono::Utc::now().timestamp(),
        };

        let credentials = AccountCredentials::Password {
            password: request.password,
        };

        SyncEngine::test_connection(&account, &credentials).await?;
        SmtpClient::test_connection(&account, &credentials).await?;
        self.secrets.store_credentials(id, &credentials)?;
        self.db.insert_account(&account)?;

        Ok(account_to_dto(&account))
    }

    pub async fn add_account_oauth(
        &self,
        request: AddAccountOAuthRequest,
    ) -> CoreResult<AccountDto> {
        let id = Uuid::new_v4();
        let account = AccountRecord {
            id,
            name: request.name,
            label: request.label,
            email: request.email,
            provider: request.provider,
            auth_type: novamail_ipc::AuthType::OAuth2,
            imap_host: request.imap_host,
            imap_port: request.imap_port,
            imap_tls: request.imap_tls,
            smtp_host: request.smtp_host,
            smtp_port: request.smtp_port,
            smtp_tls: request.smtp_tls,
            created_at: chrono::Utc::now().timestamp(),
        };

        let credentials = AccountCredentials::OAuth2 {
            tokens: OAuthTokens {
                access_token: request.access_token,
                refresh_token: request.refresh_token,
                expires_at: request.expires_at,
            },
        };

        SyncEngine::test_connection(&account, &credentials).await?;
        SmtpClient::test_connection(&account, &credentials).await?;
        self.secrets.store_credentials(id, &credentials)?;
        self.db.insert_account(&account)?;

        Ok(account_to_dto(&account))
    }

    pub async fn update_account(&self, request: UpdateAccountRequest) -> CoreResult<AccountDto> {
        let existing = self.db.get_account(request.id)?;
        let mut account = existing.clone();
        account.name = request.name;
        account.label = request.label;
        account.email = request.email;
        account.provider = request.provider;
        account.imap_host = request.imap_host;
        account.imap_port = request.imap_port;
        account.imap_tls = request.imap_tls;
        account.smtp_host = request.smtp_host;
        account.smtp_port = request.smtp_port;
        account.smtp_tls = request.smtp_tls;

        let password = request
            .password
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty());

        let credentials = if let Some(password) = password {
            if account.auth_type != novamail_ipc::AuthType::Password {
                return Err(CoreError::Message(
                    "password update is only supported for password-auth accounts".into(),
                ));
            }
            AccountCredentials::Password {
                password: password.to_string(),
            }
        } else {
            self.secrets
                .load_credentials(account.id)
                .map_err(|e| CoreError::Message(e.to_string()))?
        };

        let servers_changed = existing.imap_host != account.imap_host
            || existing.imap_port != account.imap_port
            || existing.imap_tls != account.imap_tls
            || existing.smtp_host != account.smtp_host
            || existing.smtp_port != account.smtp_port
            || existing.smtp_tls != account.smtp_tls
            || existing.email != account.email
            || password.is_some();

        if servers_changed {
            SyncEngine::test_connection(&account, &credentials).await?;
            SmtpClient::test_connection(&account, &credentials).await?;
        }

        if password.is_some() {
            self.secrets.store_credentials(account.id, &credentials)?;
        }
        self.db.update_account(&account)?;

        Ok(account_to_dto(&account))
    }

    pub fn remove_account(&self, account_id: Uuid) -> CoreResult<()> {
        let _ = self.secrets.delete_credentials(account_id);
        self.db.delete_account(account_id)?;
        Ok(())
    }

    pub fn list_mailboxes(&self, account_id: Option<Uuid>) -> CoreResult<Vec<MailboxDto>> {
        Ok(self.db.list_mailboxes(account_id)?)
    }

    pub fn list_messages(&self, request: ListMessagesRequest) -> CoreResult<ListMessagesResponse> {
        let (messages, total) = self.db.list_messages(&request)?;
        Ok(ListMessagesResponse { messages, total })
    }

    pub fn list_message_ids(&self, request: ListMessagesRequest) -> CoreResult<Vec<Uuid>> {
        Ok(self.db.list_message_ids(&request)?)
    }

    pub fn list_threads(&self, request: ListMessagesRequest) -> CoreResult<ListThreadsResponse> {
        Ok(self.db.list_threads(&request)?)
    }

    pub fn list_messages_by_thread(&self, thread_id: Uuid) -> CoreResult<Vec<MessageSummaryDto>> {
        Ok(self.db.list_messages_by_thread(thread_id)?)
    }

    pub fn get_message(&self, message_id: Uuid) -> CoreResult<MessageDetailDto> {
        tracing::info!(%message_id, "get_message start");
        let mut detail = self.db.get_message(message_id)?;
        if let Some(html) = detail.body_html.take() {
            let raw_len = html.len();
            // Never let sanitize panics or multi‑MB base64 kill the process on open.
            let cleaned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                sanitize_html(&html)
            }))
            .unwrap_or_else(|_| {
                tracing::error!(%message_id, "sanitize_html panicked; dropping HTML body");
                String::new()
            });
            tracing::info!(
                %message_id,
                raw_len,
                cleaned_len = cleaned.len(),
                "get_message html sanitized"
            );
            detail.body_html = if cleaned.trim().is_empty() {
                None
            } else {
                Some(cleaned)
            };
        }
        tracing::info!(%message_id, "get_message done");
        Ok(detail)
    }

    pub async fn set_flags(&self, request: SetFlagsRequest) -> CoreResult<()> {
        self.db
            .set_flags(request.message_id, request.unread, request.starred)?;
        // Mailbox unread/total counts are refreshed inside db.set_flags.
        // Push IMAP flags in the background so UI actions (quick sort, open-to-read) stay snappy.
        let db = self.db.clone();
        let secrets = self.secrets.clone();
        let message_id = request.message_id;
        let unread = request.unread;
        let starred = request.starred;
        spawn_background(async move {
            if let Err(err) = set_flags_remote(&db, &secrets, message_id, unread, starred).await {
                tracing::warn!(error = %err, "IMAP flag sync failed; local flags kept");
            }
        });
        Ok(())
    }

    /// Bulk flag updates with one IMAP session per mailbox (Yahoo-friendly).
    pub async fn set_flags_many(
        &self,
        message_ids: Vec<Uuid>,
        unread: Option<bool>,
        starred: Option<bool>,
    ) -> CoreResult<()> {
        for message_id in &message_ids {
            self.db.set_flags(*message_id, unread, starred)?;
        }
        let db = self.db.clone();
        let secrets = self.secrets.clone();
        spawn_background(async move {
            if let Err(err) =
                set_flags_remote_batch(&db, &secrets, &message_ids, unread, starred).await
            {
                tracing::warn!(error = %err, "IMAP batch flag sync failed; local flags kept");
            }
        });
        Ok(())
    }

    pub fn search(&self, request: SearchRequest) -> CoreResult<SearchResponse> {
        let service = SearchService::new(self.db.clone());
        Ok(service.search(request)?)
    }

    pub async fn sync<F, P>(
        &self,
        request: SyncRequest,
        mut on_progress: F,
        mut on_offline_prompt: P,
    ) -> CoreResult<Vec<SyncResult>>
    where
        F: FnMut(SyncProgressEvent) + Send,
        P: FnMut(OfflinePromptEvent) + Send,
    {
        let engine = SyncEngine::new(
            self.db.clone(),
            self.secrets.clone(),
            &self.paths.blobs_dir,
        );
        let account_ids = if let Some(id) = request.account_id {
            vec![id]
        } else {
            self.db
                .list_accounts()?
                .into_iter()
                .map(|a| a.id)
                .collect::<Vec<_>>()
        };

        let mut results = Vec::new();
        let mut new_ids = Vec::new();
        for account_id in account_ids {
            let report = engine
                .sync_account(account_id, |event| on_progress(event))
                .await?;
            new_ids.extend(report.new_message_ids.iter().copied());
            results.push(SyncResult {
                account_id: report.account_id,
                mailboxes_synced: report.mailboxes_synced,
                messages_fetched: report.messages_fetched,
            });
        }
        self.on_new_messages_synced(&new_ids);
        if let Err(err) = self.apply_folder_retention() {
            tracing::warn!(error = %err, "folder retention cleanup failed");
        }
        for result in &results {
            match self.apply_offline_mailbox_after_sync(result.account_id).await {
                Ok(Some(prompt)) => on_offline_prompt(prompt),
                Ok(None) => {}
                Err(err) => {
                    tracing::warn!(
                        account_id = %result.account_id,
                        error = %err,
                        "offline mailbox policy failed"
                    );
                }
            }
        }
        Ok(results)
    }

    /// Apply mail rules, spam filter, then enqueue background AI insights.
    pub fn on_new_messages_synced(&self, message_ids: &[Uuid]) {
        for message_id in message_ids {
            if let Err(err) = self.apply_rules_for_message(*message_id) {
                tracing::debug!(%message_id, error = %err, "rule apply skipped");
            }
            if let Err(err) = self.apply_spam_filter(*message_id) {
                tracing::debug!(%message_id, error = %err, "spam filter skipped");
            }
        }
        self.enqueue_ai_insights(message_ids);
    }

    pub async fn send_message(&self, request: SendMessageRequest) -> CoreResult<()> {
        let account = self.db.get_account(request.account_id)?;
        let draft_id = request.draft_id;
        let request = self.apply_pgp_to_send(request)?;
        SmtpClient::send_with_secrets(&account, &self.secrets, &request).await?;
        if !account.imap_host.trim().is_empty() {
            if let Err(err) =
                append_sent_remote(&self.db, &self.secrets, &account, &request).await
            {
                tracing::warn!(
                    account = %account.email,
                    error = %err,
                    "IMAP APPEND to Sent failed after SMTP accept"
                );
            }
        }
        if let Some(id) = draft_id {
            if !account.imap_host.trim().is_empty() {
                if let Err(err) = delete_remote(&self.db, &self.secrets, id).await {
                    tracing::warn!(error = %err, "IMAP draft delete after send failed");
                }
            }
            let _ = self.db.delete_message(id);
        }
        Ok(())
    }

    fn apply_pgp_to_send(&self, mut request: SendMessageRequest) -> CoreResult<SendMessageRequest> {
        if !request.pgp_sign && !request.pgp_encrypt {
            return Ok(request);
        }
        let account = self.db.get_account(request.account_id)?;
        let mut body = request.body_text.clone();
        if request.pgp_sign {
            let Some((_, secret)) = self.db.find_pgp_secret_for_email(&account.email)? else {
                return Err(CoreError::Message(format!(
                    "no OpenPGP secret key for {}",
                    account.email
                )));
            };
            body = sign_message(&body, &secret)?;
        }
        if request.pgp_encrypt {
            let mut publics = Vec::new();
            for addr in request.to.iter().chain(request.cc.iter()) {
                if let Some(pub_armored) = self.db.find_pgp_public_for_email(&addr.email)? {
                    publics.push(pub_armored);
                }
            }
            if publics.is_empty() {
                return Err(CoreError::Message(
                    "no OpenPGP public keys for recipients".into(),
                ));
            }
            let refs: Vec<&str> = publics.iter().map(String::as_str).collect();
            body = encrypt_message(&body, &refs)?;
            request.body_html = None;
        }
        request.body_text = body;
        Ok(request)
    }

    pub fn snooze_message(&self, request: SnoozeRequest) -> CoreResult<i64> {
        let detail = self.db.get_message(request.message_id)?;
        let wake_at = resolve_snooze_wake(request.preset, request.wake_at)?;
        self.db.snooze_message(
            request.message_id,
            detail.summary.account_id,
            wake_at,
            Some(detail.summary.mailbox_id),
        )?;
        Ok(wake_at)
    }

    pub fn unsnooze_message(&self, message_id: Uuid) -> CoreResult<()> {
        self.db.unsnooze_message(message_id)?;
        Ok(())
    }

    pub fn list_snoozed(&self, limit: u32) -> CoreResult<Vec<SnoozedMessageDto>> {
        Ok(self.db.list_snoozed_messages(limit)?)
    }

    pub fn wake_due_snoozes(&self) -> CoreResult<u32> {
        let now = chrono::Utc::now().timestamp();
        let due = self.db.list_due_snoozes(now)?;
        let mut woke = 0u32;
        for message_id in due {
            self.db.unsnooze_message(message_id)?;
            woke += 1;
        }
        Ok(woke)
    }

    pub fn enqueue_send_later(&self, request: SendLaterRequest) -> CoreResult<Uuid> {
        let now = chrono::Utc::now().timestamp();
        if request.send_at <= now {
            return Err(CoreError::Message(
                "send_at must be in the future".into(),
            ));
        }
        let _ = self.db.get_account(request.message.account_id)?;
        if request.message.to.is_empty() {
            return Err(CoreError::Message("at least one recipient required".into()));
        }
        let id = Uuid::new_v4();
        let payload = serde_json::to_string(&request.message)
            .map_err(|e| CoreError::Message(e.to_string()))?;
        self.db.enqueue_outbound(
            id,
            request.message.account_id,
            &payload,
            request.send_at,
        )?;
        Ok(id)
    }

    pub fn list_outbound_queue(&self, limit: u32) -> CoreResult<Vec<OutboundQueueItemDto>> {
        Ok(self.db.list_outbound_queue(limit)?)
    }

    pub fn cancel_outbound(&self, id: Uuid) -> CoreResult<()> {
        self.db.cancel_outbound(id)?;
        Ok(())
    }

    pub fn planned_summary(&self) -> CoreResult<PlannedSummaryDto> {
        Ok(PlannedSummaryDto {
            snoozed_count: self.db.count_active_snoozes()?,
            outbound_pending_count: self.db.count_pending_outbound()?,
        })
    }

    pub async fn flush_outbound_queue(&self) -> CoreResult<(u32, u32)> {
        let now = chrono::Utc::now().timestamp();
        let due = self.db.list_due_outbound(now, 20)?;
        let mut sent = 0u32;
        let mut failed = 0u32;
        for (id, _account_id, payload) in due {
            let request: SendMessageRequest = match serde_json::from_str(&payload) {
                Ok(r) => r,
                Err(err) => {
                    let _ = self.db.set_outbound_status(
                        id,
                        OutboundStatus::Failed,
                        Some(&err.to_string()),
                        None,
                    );
                    failed += 1;
                    continue;
                }
            };
            let _ = self
                .db
                .set_outbound_status(id, OutboundStatus::Sending, None, None);
            match self.send_message(request).await {
                Ok(()) => {
                    let _ = self.db.set_outbound_status(
                        id,
                        OutboundStatus::Sent,
                        None,
                        Some(chrono::Utc::now().timestamp()),
                    );
                    sent += 1;
                }
                Err(err) => {
                    let _ = self.db.set_outbound_status(
                        id,
                        OutboundStatus::Failed,
                        Some(&err.to_string()),
                        None,
                    );
                    failed += 1;
                }
            }
        }
        Ok((sent, failed))
    }

    pub async fn run_jobs_tick(&self) -> CoreResult<JobsTickReport> {
        let woke_snoozes = self.wake_due_snoozes()?;
        let (sent_later, failed_later) = self.flush_outbound_queue().await?;
        let reminder_titles = self.fire_due_calendar_reminders()?;
        let scanned_invites = self.db.scan_messages_for_invites(40).unwrap_or(0);
        Ok(JobsTickReport {
            woke_snoozes,
            sent_later,
            failed_later,
            calendar_reminders: reminder_titles.len() as u32,
            scanned_invites,
            reminder_titles,
        })
    }

    fn fire_due_calendar_reminders(&self) -> CoreResult<Vec<String>> {
        let now = chrono::Utc::now().timestamp();
        let due = self.db.list_due_calendar_reminders(now)?;
        let mut titles = Vec::new();
        for event in due {
            tracing::info!(
                title = %event.title,
                starts_at = event.starts_at,
                "calendar reminder due"
            );
            self.db.mark_calendar_reminder_fired(event.id, now)?;
            titles.push(event.title);
        }
        Ok(titles)
    }

    pub async fn save_draft(&self, request: SaveDraftRequest) -> CoreResult<MessageDetailDto> {
        let account = self.db.get_account(request.account_id)?;
        let drafts = self.db.ensure_mailbox(request.account_id, "Drafts", "drafts")?;
        let now = chrono::Utc::now().timestamp();
        let subject = if request.subject.trim().is_empty() {
            "(no subject)".to_string()
        } else {
            request.subject.trim().to_string()
        };
        let snippet: String = request
            .body_text
            .chars()
            .take(160)
            .collect::<String>()
            .replace('\n', " ");
        let from = AddressDto {
            name: Some(account.name.clone()),
            email: account.email.clone(),
        };

        let (message_id, rfc_message_id) = if let Some(existing_id) = request.id {
            let existing_rfc = self
                .db
                .get_message(existing_id)?
                .message_id
                .unwrap_or_else(|| format!("<{existing_id}@novamail.local>"));
            self.db.update_message_draft(
                existing_id,
                &subject,
                &request.to,
                &request.cc,
                &request.body_text,
                request.body_html.as_deref(),
                now,
                &snippet,
            )?;
            if let Ok(detail) = self.db.get_message(existing_id) {
                let _ = self.db.upsert_thread(&novamail_db::models::ThreadRecord {
                    id: detail.summary.thread_id,
                    account_id: request.account_id,
                    subject: subject.clone(),
                    last_message_at: now,
                    message_count: 1,
                    unread_count: 0,
                    participants: {
                        let mut p = vec![from.clone()];
                        p.extend(request.to.iter().cloned());
                        p
                    },
                    snippet: snippet.clone(),
                });
            }
            (existing_id, existing_rfc)
        } else {
            let thread_id = Uuid::new_v4();
            let message_id = Uuid::new_v4();
            let rfc_message_id = format!("<{message_id}@novamail.local>");
            self.db.upsert_thread(&novamail_db::models::ThreadRecord {
                id: thread_id,
                account_id: request.account_id,
                subject: subject.clone(),
                last_message_at: now,
                message_count: 1,
                unread_count: 0,
                participants: {
                    let mut p = vec![from.clone()];
                    p.extend(request.to.iter().cloned());
                    p
                },
                snippet: snippet.clone(),
            })?;
            self.db.insert_message(&novamail_db::models::MessageRecord {
                id: message_id,
                account_id: request.account_id,
                mailbox_id: drafts.id,
                thread_id,
                uid: None,
                message_id: Some(rfc_message_id.clone()),
                in_reply_to: request.in_reply_to.clone(),
                references: request.references.clone(),
                subject: subject.clone(),
                from: from.clone(),
                to: request.to.clone(),
                cc: request.cc.clone(),
                date: now,
                flags: novamail_db::models::FLAG_SEEN,
                snippet: snippet.clone(),
                body_text: Some(request.body_text.clone()),
                body_html: request.body_html.clone(),
                has_attachments: false,
                raw_path: None,
                local_only: false,
                offline_at: None,
                size_bytes: Some(
                    (request.body_text.len()
                        + request.body_html.as_deref().map(|s| s.len()).unwrap_or(0))
                        as i64,
                ),
            })?;
            (message_id, rfc_message_id)
        };

        let _ = self.db.set_message_rfc_id(message_id, &rfc_message_id);

        // IMAP: APPEND into the server Drafts / Entwürfe folder.
        if !account.imap_host.trim().is_empty() {
            if let Err(err) = save_draft_remote(
                &self.db,
                &self.secrets,
                &account,
                &request,
                message_id,
                &rfc_message_id,
            )
            .await
            {
                tracing::warn!(error = %err, "IMAP draft APPEND failed; local draft kept");
            }
        }

        Ok(self.get_message(message_id)?)
    }

    pub fn oauth_authorize_url(&self, provider: novamail_ipc::MailProvider) -> CoreResult<String> {
        let config = OAuthConfig::for_provider(&provider)?;
        let state = novamail_mail::OAuthFlow::new_state();
        Ok(config.authorize_url_with_state(&state)?)
    }

    pub async fn oauth_exchange_code(
        &self,
        request: OAuthExchangeRequest,
    ) -> CoreResult<OAuthExchangeResponse> {
        let config = OAuthConfig::for_provider(&request.provider)?;
        let response = config.exchange_code(&request.code).await?;
        Ok(OAuthExchangeResponse {
            tokens: OAuthTokensDto {
                access_token: response.tokens.access_token,
                refresh_token: response.tokens.refresh_token,
                expires_at: response.tokens.expires_at,
            },
            token_type: response.token_type,
            scope: response.scope,
        })
    }

    /// Starts the localhost callback listener and returns the authorization code.
    pub async fn oauth_wait_callback(
        &self,
        timeout_secs: u64,
    ) -> CoreResult<novamail_mail::OAuthCallbackResult> {
        Ok(novamail_mail::wait_for_oauth_callback(
            std::time::Duration::from_secs(timeout_secs.max(30)),
        )
        .await?)
    }

    pub async fn archive_message(&self, message_id: Uuid) -> CoreResult<()> {
        self.db.archive_message(message_id)?;
        if let Err(err) = archive_remote(&self.db, &self.secrets, message_id).await {
            tracing::warn!(error = %err, "IMAP archive failed; local archive kept");
        }
        Ok(())
    }

    pub fn build_forward_draft(&self, message_id: Uuid) -> CoreResult<SendMessageRequest> {
        let detail = self.get_message(message_id)?;
        let original = detail
            .body_text
            .unwrap_or_else(|| detail.summary.snippet.clone());
        let body = format!(
            "\n\n---------- Forwarded message ----------\nFrom: {}\nDate: {}\nSubject: {}\n\n{}",
            detail.summary.from.email,
            detail.summary.date,
            detail.summary.subject,
            original
        );
        Ok(SendMessageRequest {
            account_id: detail.summary.account_id,
            to: vec![],
            cc: vec![],
            bcc: vec![],
            subject: if detail.summary.subject.to_ascii_lowercase().starts_with("fwd:") {
                detail.summary.subject
            } else {
                format!("Fwd: {}", detail.summary.subject)
            },
            body_text: body,
            body_html: None,
            in_reply_to: None,
            references: vec![],
            attachments: Vec::new(),
            draft_id: None,
            pgp_sign: false,
            pgp_encrypt: false,
        })
    }

    pub async fn delete_message(&self, message_id: Uuid) -> CoreResult<()> {
        // Snapshot IMAP coordinates before local delete, then remove locally immediately
        // so quick-sort / keyboard triage never waits on the network.
        let plan = match plan_delete_remote(&self.db, message_id) {
            Ok(plan) => plan,
            Err(err) => {
                tracing::warn!(error = %err, "could not plan IMAP delete");
                None
            }
        };
        let detail = self.db.get_message(message_id)?;
        for attachment in detail.attachments {
            let _ = std::fs::remove_file(attachment.path);
        }
        self.db.delete_message(message_id)?;
        if let Some(plan) = plan {
            let secrets = self.secrets.clone();
            spawn_background(async move {
                if let Err(err) = execute_delete_remote(&secrets, plan).await {
                    tracing::warn!(error = %err, "IMAP delete failed; local copy already removed");
                }
            });
        }
        Ok(())
    }

    pub fn open_attachment_path(&self, attachment_id: Uuid) -> CoreResult<String> {
        let attachment = self.db.get_attachment(attachment_id)?;
        let path = std::path::PathBuf::from(&attachment.path);
        if path.is_file() {
            return Ok(std::fs::canonicalize(&path)
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned());
        }
        Err(CoreError::Message(format!(
            "attachment file missing on disk: {} ({})",
            attachment.filename, attachment.path
        )))
    }

    /// Ensure the attachment bytes exist locally (re-fetch from IMAP if needed), then return path.
    ///
    /// `message_id` / `filename` let us recover when the UI still holds a stale
    /// attachment UUID after a resync replaced rows.
    pub async fn ensure_attachment_path(
        &self,
        attachment_id: Uuid,
        message_id: Option<Uuid>,
        filename: Option<String>,
    ) -> CoreResult<String> {
        let attachment = match self.db.get_attachment(attachment_id) {
            Ok(a) => a,
            Err(novamail_db::DbError::NotFound(_)) => {
                let mid = message_id.ok_or_else(|| {
                    CoreError::Message(format!("not found: attachment {attachment_id}"))
                })?;
                let name = filename.clone().unwrap_or_default();
                tracing::warn!(
                    %attachment_id,
                    message_id = %mid,
                    filename = %name,
                    "stale attachment id; resolving via message"
                );
                self.resolve_attachment_for_message(mid, attachment_id, name.as_str())
                    .await?
            }
            Err(err) => return Err(err.into()),
        };
        let path = std::path::PathBuf::from(&attachment.path);
        let file_ok = path.is_file()
            && std::fs::metadata(&path)
                .map(|m| m.len() > 0 || attachment.size == 0)
                .unwrap_or(false);
        if file_ok {
            tracing::info!(
                attachment_id = %attachment.id,
                path = %path.display(),
                "opening attachment"
            );
            return Ok(std::fs::canonicalize(&path)
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned());
        }
        tracing::warn!(
            attachment_id = %attachment.id,
            message_id = %attachment.message_id,
            path = %attachment.path,
            expected_size = attachment.size,
            "attachment missing or empty; refetching message from IMAP"
        );
        let filename = attachment.filename.clone();
        let message_id = attachment.message_id;
        refetch_message_content(
            &self.db,
            &self.secrets,
            &self.paths.blobs_dir,
            message_id,
        )
        .await
        .map_err(|e| CoreError::Message(format!("could not refetch attachment: {e}")))?;
        let found = self
            .find_attachment_row(message_id, Some(attachment.id), &filename)?
            .ok_or_else(|| {
                CoreError::Message(format!(
                    "attachment '{filename}' still missing after IMAP refetch"
                ))
            })?;
        let path = std::path::PathBuf::from(&found.path);
        if !path.is_file() {
            return Err(CoreError::Message(format!(
                "attachment file not written: {}",
                found.path
            )));
        }
        tracing::info!(
            attachment_id = %found.id,
            path = %path.display(),
            "attachment restored from IMAP"
        );
        Ok(std::fs::canonicalize(&path)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned())
    }

    async fn resolve_attachment_for_message(
        &self,
        message_id: Uuid,
        preferred_id: Uuid,
        filename: &str,
    ) -> CoreResult<AttachmentDto> {
        if let Some(found) = self.find_attachment_row(message_id, Some(preferred_id), filename)? {
            return Ok(found);
        }
        refetch_message_content(
            &self.db,
            &self.secrets,
            &self.paths.blobs_dir,
            message_id,
        )
        .await
        .map_err(|e| CoreError::Message(format!("could not refetch attachment: {e}")))?;
        self.find_attachment_row(message_id, Some(preferred_id), filename)?
            .ok_or_else(|| {
                CoreError::Message(format!(
                    "not found: attachment {preferred_id} (message {message_id})"
                ))
            })
    }

    fn find_attachment_row(
        &self,
        message_id: Uuid,
        preferred_id: Option<Uuid>,
        filename: &str,
    ) -> CoreResult<Option<AttachmentDto>> {
        let rows = self.db.list_attachments(message_id)?;
        if let Some(id) = preferred_id {
            if let Some(hit) = rows.iter().find(|a| a.id == id) {
                return Ok(Some(hit.clone()));
            }
        }
        if !filename.is_empty() {
            if let Some(hit) = rows.iter().find(|a| a.filename == filename) {
                return Ok(Some(hit.clone()));
            }
        }
        if rows.len() == 1 {
            return Ok(Some(rows.into_iter().next().unwrap()));
        }
        Ok(None)
    }

    /// Stage attachment under a clean original filename for the OS opener.
    pub async fn stage_attachment_for_open(
        &self,
        attachment_id: Uuid,
        message_id: Option<Uuid>,
        filename: Option<String>,
    ) -> CoreResult<String> {
        let blob = self
            .ensure_attachment_path(attachment_id, message_id.clone(), filename.clone())
            .await?;
        let display_name = filename
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                self.db
                    .get_attachment(attachment_id)
                    .ok()
                    .map(|a| a.filename)
            })
            .or_else(|| {
                message_id.and_then(|mid| {
                    self.db
                        .list_attachments(mid)
                        .ok()
                        .and_then(|rows| rows.into_iter().next().map(|a| a.filename))
                })
            })
            .unwrap_or_else(|| "attachment.bin".into());
        let safe: String = display_name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                    c
                } else {
                    '_'
                }
            })
            .take(160)
            .collect();
        let safe = if safe.trim().is_empty() {
            "attachment.bin".into()
        } else {
            safe
        };
        // Per-attachment dir + original basename only — xdg-open / MIME helpers
        // key off the filename; UUID prefixes often make openers no-op.
        let stage_dir = self.paths.data_dir.join("open").join(attachment_id.to_string());
        std::fs::create_dir_all(&stage_dir)
            .map_err(|e| CoreError::Message(format!("cannot create open dir: {e}")))?;
        let staged = stage_dir.join(&safe);
        std::fs::copy(&blob, &staged)
            .map_err(|e| CoreError::Message(format!("cannot stage attachment: {e}")))?;
        Ok(staged.to_string_lossy().into_owned())
    }

    /// Reveal the attachment's folder in the system file manager.
    pub async fn reveal_attachment(
        &self,
        attachment_id: Uuid,
        message_id: Option<Uuid>,
        filename: Option<String>,
    ) -> CoreResult<String> {
        let path = self
            .ensure_attachment_path(attachment_id, message_id, filename)
            .await?;
        let parent = std::path::Path::new(&path)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone());
        Ok(parent)
    }

    pub fn total_inbox_unread(&self) -> CoreResult<u32> {
        Ok(self.db.total_inbox_unread()?)
    }

    /// Write an exported message rendering into the user's Downloads folder.
    pub fn export_message_bytes(
        &self,
        message_id: Uuid,
        format: &str,
        bytes: Vec<u8>,
    ) -> CoreResult<String> {
        let detail = self.db.get_message(message_id)?;
        let stem = sanitize_export_stem(&detail.summary.subject);
        let ext = match format {
            "pdf" | "jpeg" | "jpg" | "tiff" | "tif" | "html" => {
                if format == "jpg" {
                    "jpeg"
                } else if format == "tif" {
                    "tiff"
                } else {
                    format
                }
            }
            other => {
                return Err(CoreError::Message(format!("unsupported export format: {other}")));
            }
        };
        let dir = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        std::fs::create_dir_all(&dir)
            .map_err(|e| CoreError::Message(format!("cannot create export dir: {e}")))?;
        let path = unique_export_path(&dir, stem, ext);
        std::fs::write(&path, bytes)
            .map_err(|e| CoreError::Message(format!("write export failed: {e}")))?;
        tracing::info!(path = %path.display(), %ext, "message exported");
        Ok(path.to_string_lossy().into_owned())
    }

    /// Build a simple text PDF for the message (subject + plaintext body).
    pub fn export_message_pdf(&self, message_id: Uuid) -> CoreResult<String> {
        let detail = self.db.get_message(message_id)?;
        let body = detail
            .body_text
            .clone()
            .unwrap_or_else(|| {
                detail
                    .body_html
                    .as_deref()
                    .map(strip_html_rough)
                    .unwrap_or_else(|| detail.summary.snippet.clone())
            });
        let pdf = build_simple_pdf(
            &detail.summary.subject,
            &format!(
                "From: {}\nDate: {}\n\n{}",
                detail.summary.from.email,
                detail.summary.date,
                body
            ),
        );
        self.export_message_bytes(message_id, "pdf", pdf)
    }

    /// Export the message as a standalone HTML file into Downloads.
    pub fn export_message_html(&self, message_id: Uuid) -> CoreResult<String> {
        let detail = self.db.get_message(message_id)?;
        let subject = html_escape_attr(&detail.summary.subject);
        let from = html_escape_attr(&detail.summary.from.email);
        let date = detail.summary.date;
        let body_inner = if let Some(html) = detail.body_html.as_deref() {
            html.to_string()
        } else {
            let text = detail
                .body_text
                .clone()
                .unwrap_or_else(|| detail.summary.snippet.clone());
            format!(
                "<pre style=\"white-space:pre-wrap;font:14px/1.5 sans-serif\">{}</pre>",
                html_escape_text(&text)
            )
        };
        let doc = format!(
            "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>{subject}</title></head>\
             <body style=\"margin:2rem;font:15px/1.55 system-ui,sans-serif;color:#111\">\
             <h1 style=\"font-size:1.4rem\">{subject}</h1>\
             <p style=\"color:#555\">From: {from}<br>Date: {date}</p>\
             <hr style=\"border:none;border-top:1px solid #ddd;margin:1.25rem 0\">\
             {body_inner}</body></html>"
        );
        self.export_message_bytes(message_id, "html", doc.into_bytes())
    }

    pub fn list_contacts(&self, query: Option<String>) -> CoreResult<Vec<ContactDto>> {
        Ok(self.db.list_contacts(query.as_deref())?)
    }

    pub fn suggest_recipients(
        &self,
        query: String,
        limit: Option<u32>,
    ) -> CoreResult<Vec<RecipientSuggestion>> {
        Ok(self
            .db
            .suggest_recipients(&query, limit.unwrap_or(12) as usize)?)
    }

    pub fn spellcheck_status(&self) -> CoreResult<novamail_ipc::SpellcheckStatus> {
        crate::spellcheck::spellcheck_status()
    }

    pub async fn spellcheck_install(
        &self,
        code: String,
    ) -> CoreResult<novamail_ipc::SpellDictionaryDto> {
        crate::spellcheck::spellcheck_install(&code).await
    }

    pub async fn spellcheck_ensure_for_locale(
        &self,
        locale: String,
    ) -> CoreResult<novamail_ipc::SpellDictionaryDto> {
        crate::spellcheck::spellcheck_ensure_for_locale(&locale).await
    }

    pub fn spellcheck_suggest(
        &self,
        word: String,
        lang: String,
    ) -> CoreResult<novamail_ipc::SpellSuggestResult> {
        crate::spellcheck::spellcheck_suggest(&word, &lang)
    }

    pub fn spellcheck_learn_word(&self, word: String, lang: String) -> CoreResult<()> {
        crate::spellcheck::spellcheck_learn_word(&word, &lang)
    }

    pub fn spellcheck_ensure_personal_dicts(&self, languages: &[String]) -> CoreResult<()> {
        crate::spellcheck::ensure_personal_dictionaries(languages)
    }

    pub fn upsert_contact(&self, request: UpsertContactRequest) -> CoreResult<ContactDto> {
        let id = request.id.unwrap_or_else(Uuid::new_v4);
        let record = novamail_db::models::ContactRecord {
            id,
            display_name: request.display_name,
            given_name: request.given_name,
            family_name: request.family_name,
            emails: request.emails,
            phones: request.phones,
            faxes: request.faxes,
            organization: request.organization,
            job_title: request.job_title,
            addresses: request.addresses,
            custom_fields: request.custom_fields,
            photo_base64: request.photo_base64,
            ldap_dn: request.ldap_dn,
            notes: request.notes,
            updated_at: chrono::Utc::now().timestamp(),
        };
        self.db.upsert_contact(&record)?;
        Ok(self.db.get_contact(id)?)
    }

    pub fn contacts_book_settings(&self) -> CoreResult<ContactsBookSettings> {
        let raw = self.db.get_setting("contacts.book")?;
        if let Some(raw) = raw {
            Ok(serde_json::from_str(&raw).unwrap_or_default())
        } else {
            Ok(ContactsBookSettings::default())
        }
    }

    pub fn set_contacts_book_settings(
        &self,
        settings: ContactsBookSettings,
    ) -> CoreResult<ContactsBookSettings> {
        let raw =
            serde_json::to_string(&settings).map_err(|e| CoreError::Message(e.to_string()))?;
        self.db.set_setting("contacts.book", &raw)?;
        Ok(settings)
    }

    pub fn delete_contact(&self, contact_id: Uuid) -> CoreResult<()> {
        self.db.delete_contact(contact_id)?;
        Ok(())
    }

    pub async fn start_carddav(&self) -> CoreResult<CardDavServerStatus> {
        self.set_contacts_share_mode(SetContactsShareModeRequest {
            mode: ContactsShareMode::Server,
            client_url: None,
            client_bind_dn: None,
            client_password: None,
            client_base_dn: None,
        })
        .await?;
        Ok(self.carddav.status())
    }

    pub fn stop_carddav(&self) -> CoreResult<CardDavServerStatus> {
        let _ = self.ldap_server.stop();
        let status = self.carddav.stop();
        let _ = self.db.set_setting("contacts.share_mode", "local");
        Ok(status)
    }

    pub fn carddav_status(&self) -> CoreResult<CardDavServerStatus> {
        self.ensure_directory_credentials()?;
        Ok(self.carddav.status())
    }

    pub fn contacts_share_status(&self) -> CoreResult<ContactsShareStatus> {
        self.ensure_directory_credentials()?;
        let mode = self.contacts_share_mode()?;
        let client = if mode == ContactsShareMode::Client {
            Some(self.ldap_get_settings()?)
        } else {
            None
        };
        Ok(ContactsShareStatus {
            mode,
            carddav: self.carddav.status(),
            ldap_server: self.ldap_server.status(),
            client,
        })
    }

    pub async fn set_contacts_share_mode(
        &self,
        request: SetContactsShareModeRequest,
    ) -> CoreResult<ContactsShareStatus> {
        self.ensure_directory_credentials()?;
        match request.mode {
            ContactsShareMode::Local => {
                let _ = self.carddav.stop();
                let _ = self.ldap_server.stop();
                self.db.set_setting("contacts.share_mode", "local")?;
            }
            ContactsShareMode::Server => {
                // Persist mode first so the UI keeps "Server" even if a bind fails.
                self.db.set_setting("contacts.share_mode", "server")?;
                if let Err(err) = self.carddav.start().await {
                    tracing::warn!(error = %err, "CardDAV server failed to start");
                }
                if let Err(err) = self.ldap_server.start().await {
                    tracing::warn!(error = %err, "LDAP server failed to start");
                }
            }
            ContactsShareMode::Client => {
                let _ = self.carddav.stop();
                let _ = self.ldap_server.stop();
                let url = request
                    .client_url
                    .filter(|u| !u.trim().is_empty())
                    .or_else(|| {
                        self.db
                            .get_setting("ldap.sync")
                            .ok()
                            .flatten()
                            .and_then(|raw| {
                                serde_json::from_str::<LdapSyncSettings>(&raw)
                                    .ok()
                                    .map(|s| s.url)
                            })
                    })
                    .unwrap_or_else(|| "ldap://192.168.1.10:1389".into());
                let bind_dn = request
                    .client_bind_dn
                    .filter(|v| !v.trim().is_empty())
                    .unwrap_or_else(|| DEFAULT_BIND_DN.to_string());
                let base_dn = request
                    .client_base_dn
                    .filter(|v| !v.trim().is_empty())
                    .unwrap_or_else(|| DEFAULT_BASE_DN.to_string());
                let password = request.client_password.filter(|v| !v.trim().is_empty());
                self.ldap_save_settings(LdapSyncSettings {
                    url,
                    bind_dn: Some(bind_dn),
                    password,
                    base_dn,
                    filter: "(objectClass=inetOrgPerson)".into(),
                })?;
                self.db.set_setting("contacts.share_mode", "client")?;
            }
        }
        self.contacts_share_status()
    }

    pub async fn contacts_client_sync(&self) -> CoreResult<LdapSyncResult> {
        let mode = self.contacts_share_mode()?;
        if mode != ContactsShareMode::Client {
            return Err(CoreError::Message(
                "LDAP client sync is only available in Client mode".into(),
            ));
        }
        let settings = self.ldap_get_settings_with_password()?;
        self.ldap_sync(LdapSyncRequest {
            url: settings.url,
            bind_dn: settings.bind_dn,
            password: settings.password,
            base_dn: settings.base_dn,
            filter: settings.filter,
            save_settings: false,
        })
        .await
    }

    fn contacts_share_mode(&self) -> CoreResult<ContactsShareMode> {
        Ok(match self.db.get_setting("contacts.share_mode")?.as_deref() {
            Some("server") => ContactsShareMode::Server,
            Some("client") => ContactsShareMode::Client,
            _ => ContactsShareMode::Local,
        })
    }

    fn ldap_get_settings_with_password(&self) -> CoreResult<LdapSyncSettings> {
        let raw = self.db.get_setting("ldap.sync")?;
        if let Some(raw) = raw {
            Ok(serde_json::from_str(&raw).map_err(|e| CoreError::Message(e.to_string()))?)
        } else {
            Err(CoreError::Message("no LDAP client settings saved".into()))
        }
    }

    fn ensure_directory_credentials(&self) -> CoreResult<()> {
        const USER_KEY: &str = "carddav.username";
        const PASS_KEY: &str = "carddav.password";
        let username = match self.db.get_setting(USER_KEY)? {
            Some(u) if !u.trim().is_empty() => u,
            _ => {
                self.db.set_setting(USER_KEY, "novamail")?;
                "novamail".into()
            }
        };
        let password = match self.db.get_setting(PASS_KEY)? {
            Some(p) if !p.trim().is_empty() => p,
            _ => {
                let generated = Uuid::new_v4().simple().to_string();
                self.db.set_setting(PASS_KEY, &generated)?;
                generated
            }
        };
        self.carddav
            .set_credentials(username.clone(), password.clone());
        self.ldap_server.set_credentials(username, password);
        Ok(())
    }

    pub async fn ldap_search(&self, request: LdapSearchRequest) -> CoreResult<Vec<ContactDto>> {
        Ok(search_ldap(&request).await?)
    }

    /// Phase-1 org sync: passphrase-encrypted JSON backup (accounts, contacts, rules…).
    pub fn export_backup(&self, request: ExportBackupRequest) -> CoreResult<ExportBackupResponse> {
        let accounts = self.db.list_accounts()?;
        let mut backup_accounts = Vec::with_capacity(accounts.len());
        for account in &accounts {
            let mut entry = BackupAccount::from(account);
            if let Ok(creds) = self.secrets.load_credentials(account.id) {
                entry.credentials = Some(match creds {
                    AccountCredentials::Password { password } => BackupAccountCredentials {
                        kind: "password".into(),
                        password: Some(password),
                        access_token: None,
                        refresh_token: None,
                        expires_at: None,
                    },
                    AccountCredentials::OAuth2 { tokens } => BackupAccountCredentials {
                        kind: "oauth2".into(),
                        password: None,
                        access_token: Some(tokens.access_token),
                        refresh_token: tokens.refresh_token,
                        expires_at: tokens.expires_at,
                    },
                });
            }
            backup_accounts.push(entry);
        }

        let contacts = self.db.list_contacts(None)?;
        let payload = BackupPayload {
            version: 1,
            exported_at: chrono::Utc::now().timestamp(),
            accounts: backup_accounts,
            contacts: contacts.iter().map(BackupContact::from).collect(),
            labels: self.db.list_labels(None)?,
            rules: self.db.list_rules()?,
            signatures: self.db.list_signatures(None)?,
            contacts_book: Some(self.contacts_book_settings()?),
            ldap: Some(self.ldap_get_settings()?),
        };

        let plaintext = serde_json::to_vec(&payload)
            .map_err(|e| CoreError::Message(e.to_string()))?;
        let encrypted = encrypt_backup_payload(&request.passphrase, &plaintext)?;
        let envelope = serde_json::to_vec(&encrypted)
            .map_err(|e| CoreError::Message(e.to_string()))?;
        let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        Ok(ExportBackupResponse {
            filename: format!("novamail-backup-{stamp}.nmbak"),
            data_base64: B64.encode(envelope),
            accounts: payload.accounts.len() as u32,
            contacts: payload.contacts.len() as u32,
        })
    }

    pub fn import_backup(&self, request: ImportBackupRequest) -> CoreResult<ImportBackupResult> {
        let envelope_bytes = B64
            .decode(request.data_base64.as_bytes())
            .map_err(|_| CoreError::Message("invalid backup data encoding".into()))?;
        let file: EncryptedBackupFile = serde_json::from_slice(&envelope_bytes)
            .map_err(|_| CoreError::Message("invalid backup file".into()))?;
        let plaintext = decrypt_backup_payload(&request.passphrase, &file)?;
        let payload: BackupPayload = serde_json::from_slice(&plaintext)
            .map_err(|_| CoreError::Message("backup payload corrupt or unsupported".into()))?;

        let mut result = ImportBackupResult::default();
        let existing_contacts = self.db.list_contacts(None)?;

        for account in payload.accounts {
            let email = account.email.clone();
            let existing = self.db.find_account_by_email(&email)?;
            let id = existing
                .as_ref()
                .map(|a| a.id)
                .unwrap_or(account.id);
            let record = AccountRecord {
                id,
                name: account.name,
                label: account.label,
                email,
                provider: account.provider,
                auth_type: account.auth_type,
                imap_host: account.imap_host,
                imap_port: account.imap_port,
                imap_tls: account.imap_tls,
                smtp_host: account.smtp_host,
                smtp_port: account.smtp_port,
                smtp_tls: account.smtp_tls,
                created_at: existing
                    .as_ref()
                    .map(|a| a.created_at)
                    .unwrap_or(account.created_at),
            };
            if existing.is_some() {
                self.db.update_account(&record)?;
                result.accounts_updated += 1;
            } else {
                // Prefer original id; if clash, insert with fresh id.
                if self.db.get_account(id).is_ok() {
                    let mut fresh = record.clone();
                    fresh.id = Uuid::new_v4();
                    self.db.insert_account(&fresh)?;
                    if let Some(creds) = account.credentials {
                        self.store_backup_credentials(fresh.id, &creds)?;
                    }
                } else {
                    self.db.insert_account(&record)?;
                    if let Some(creds) = account.credentials {
                        self.store_backup_credentials(id, &creds)?;
                    }
                }
                result.accounts_imported += 1;
                continue;
            }
            if let Some(creds) = account.credentials {
                self.store_backup_credentials(id, &creds)?;
            }
        }

        for contact in payload.contacts {
            if let Some(existing) = find_matching_contact(&existing_contacts, &contact) {
                if existing.updated_at >= contact.updated_at
                    && existing.display_name == contact.display_name
                    && existing.emails == contact.emails
                {
                    result.contacts_skipped += 1;
                    continue;
                }
                let record = ContactRecord {
                    id: existing.id,
                    display_name: contact.display_name,
                    given_name: contact.given_name,
                    family_name: contact.family_name,
                    emails: contact.emails,
                    phones: contact.phones,
                    faxes: contact.faxes,
                    organization: contact.organization,
                    job_title: contact.job_title,
                    addresses: contact.addresses,
                    custom_fields: contact.custom_fields,
                    photo_base64: contact.photo_base64,
                    ldap_dn: contact.ldap_dn,
                    notes: contact.notes,
                    updated_at: contact.updated_at.max(chrono::Utc::now().timestamp()),
                };
                self.db.upsert_contact(&record)?;
                result.contacts_updated += 1;
            } else {
                let id = if self.db.get_contact(contact.id).is_ok() {
                    Uuid::new_v4()
                } else {
                    contact.id
                };
                let record = ContactRecord {
                    id,
                    display_name: contact.display_name,
                    given_name: contact.given_name,
                    family_name: contact.family_name,
                    emails: contact.emails,
                    phones: contact.phones,
                    faxes: contact.faxes,
                    organization: contact.organization,
                    job_title: contact.job_title,
                    addresses: contact.addresses,
                    custom_fields: contact.custom_fields,
                    photo_base64: contact.photo_base64,
                    ldap_dn: contact.ldap_dn,
                    notes: contact.notes,
                    updated_at: contact.updated_at,
                };
                self.db.upsert_contact(&record)?;
                result.contacts_imported += 1;
            }
        }

        for label in payload.labels {
            if self.db.get_account(label.account_id).is_err() {
                continue;
            }
            self.db.upsert_label(&LabelRecord {
                id: label.id,
                account_id: label.account_id,
                name: label.name,
                color: label.color,
            })?;
            result.labels_imported += 1;
        }
        for rule in payload.rules {
            self.db.upsert_rule(&RuleRecord {
                id: rule.id,
                account_id: rule.account_id,
                name: rule.name,
                enabled: rule.enabled,
                predicate_json: rule.predicate_json,
                action_json: rule.action_json,
            })?;
            result.rules_imported += 1;
        }
        for signature in payload.signatures {
            self.db.upsert_signature(&SignatureRecord {
                id: signature.id,
                account_id: signature.account_id,
                name: signature.name,
                body_text: signature.body_text,
                is_default: signature.is_default,
            })?;
            result.signatures_imported += 1;
        }

        if let Some(book) = payload.contacts_book {
            self.set_contacts_book_settings(book)?;
        }
        if let Some(ldap) = payload.ldap {
            self.ldap_save_settings(ldap)?;
        }

        Ok(result)
    }

    fn store_backup_credentials(
        &self,
        account_id: Uuid,
        creds: &BackupAccountCredentials,
    ) -> CoreResult<()> {
        let mapped = match creds.kind.as_str() {
            "oauth2" => AccountCredentials::OAuth2 {
                tokens: OAuthTokens {
                    access_token: creds.access_token.clone().unwrap_or_default(),
                    refresh_token: creds.refresh_token.clone(),
                    expires_at: creds.expires_at,
                },
            },
            _ => AccountCredentials::Password {
                password: creds.password.clone().unwrap_or_default(),
            },
        };
        self.secrets.store_credentials(account_id, &mapped)?;
        Ok(())
    }

    pub fn ldap_get_settings(&self) -> CoreResult<LdapSyncSettings> {
        let raw = self.db.get_setting("ldap.sync")?;
        if let Some(raw) = raw {
            let mut settings: LdapSyncSettings = serde_json::from_str(&raw)
                .map_err(|e| CoreError::Message(e.to_string()))?;
            settings.password = None;
            Ok(settings)
        } else {
            Ok(LdapSyncSettings {
                url: "ldaps://ldap.example.com".into(),
                bind_dn: None,
                password: None,
                base_dn: "ou=people,dc=example,dc=com".into(),
                filter: "(objectClass=inetOrgPerson)".into(),
            })
        }
    }

    pub fn ldap_save_settings(&self, mut settings: LdapSyncSettings) -> CoreResult<()> {
        // Keep previously stored password when the client omits it.
        if settings.password.as_deref().unwrap_or("").is_empty() {
            if let Some(raw) = self.db.get_setting("ldap.sync")? {
                if let Ok(prev) = serde_json::from_str::<LdapSyncSettings>(&raw) {
                    settings.password = prev.password;
                }
            }
        }
        let raw =
            serde_json::to_string(&settings).map_err(|e| CoreError::Message(e.to_string()))?;
        self.db.set_setting("ldap.sync", &raw)?;
        Ok(())
    }

    /// Pull contacts from LDAP and upsert by `ldap_dn` so other devices can consume
    /// them via the embedded CardDAV address book.
    pub async fn ldap_sync(&self, request: LdapSyncRequest) -> CoreResult<LdapSyncResult> {
        if request.save_settings {
            self.ldap_save_settings(LdapSyncSettings {
                url: request.url.clone(),
                bind_dn: request.bind_dn.clone(),
                password: request.password.clone(),
                base_dn: request.base_dn.clone(),
                filter: request.filter.clone(),
            })?;
        }

        let found = search_ldap(&LdapSearchRequest {
            url: request.url,
            bind_dn: request.bind_dn,
            password: request.password,
            base_dn: request.base_dn,
            filter: request.filter,
        })
        .await?;

        let mut imported = 0u32;
        let mut updated = 0u32;
        for contact in found {
            let ldap_dn = contact.ldap_dn.clone();
            let existing = if let Some(dn) = &ldap_dn {
                self.db.find_contact_by_ldap_dn(dn)?
            } else {
                None
            };
            let id = existing.as_ref().map(|c| c.id).unwrap_or(contact.id);
            if existing.is_some() {
                updated += 1;
            } else {
                imported += 1;
            }
            let record = novamail_db::models::ContactRecord {
                id,
                display_name: contact.display_name,
                given_name: contact.given_name,
                family_name: contact.family_name,
                emails: contact.emails,
                phones: contact.phones,
                faxes: contact.faxes,
                organization: contact.organization,
                job_title: contact.job_title,
                addresses: contact.addresses,
                custom_fields: contact.custom_fields,
                photo_base64: contact.photo_base64,
                ldap_dn,
                notes: contact.notes,
                updated_at: chrono::Utc::now().timestamp(),
            };
            self.db.upsert_contact(&record)?;
        }

        Ok(LdapSyncResult {
            imported,
            updated,
            total: imported + updated,
        })
    }

    pub fn list_attachments(&self, message_id: Uuid) -> CoreResult<Vec<AttachmentDto>> {
        Ok(self.db.list_attachments(message_id)?)
    }

    pub fn upsert_label(&self, request: UpsertLabelRequest) -> CoreResult<LabelDto> {
        let id = request.id.unwrap_or_else(Uuid::new_v4);
        let account_id = request.account_id;
        let record = novamail_db::models::LabelRecord {
            id,
            account_id,
            name: request.name,
            color: request.color,
        };
        self.db.upsert_label(&record)?;
        Ok(self
            .db
            .list_labels(Some(account_id))?
            .into_iter()
            .find(|l| l.id == id)
            .ok_or_else(|| CoreError::Message("label missing after upsert".into()))?)
    }

    pub fn list_labels(&self, account_id: Option<Uuid>) -> CoreResult<Vec<LabelDto>> {
        Ok(self.db.list_labels(account_id)?)
    }

    pub fn delete_label(&self, label_id: Uuid) -> CoreResult<()> {
        self.db.delete_label(label_id)?;
        Ok(())
    }

    pub fn set_message_labels(&self, request: SetMessageLabelsRequest) -> CoreResult<()> {
        self.db
            .set_message_labels(request.message_id, &request.label_ids)?;
        Ok(())
    }

    pub fn list_message_labels(&self, message_id: Uuid) -> CoreResult<Vec<LabelDto>> {
        Ok(self.db.list_message_labels(message_id)?)
    }

    pub fn upsert_rule(&self, request: UpsertRuleRequest) -> CoreResult<RuleDto> {
        let id = request.id.unwrap_or_else(Uuid::new_v4);
        let record = novamail_db::models::RuleRecord {
            id,
            account_id: request.account_id,
            name: request.name,
            enabled: request.enabled,
            predicate_json: request.predicate_json,
            action_json: request.action_json,
        };
        self.db.upsert_rule(&record)?;
        Ok(self
            .db
            .list_rules()?
            .into_iter()
            .find(|r| r.id == id)
            .ok_or_else(|| CoreError::Message("rule missing after upsert".into()))?)
    }

    pub fn list_rules(&self) -> CoreResult<Vec<RuleDto>> {
        Ok(self.db.list_rules()?)
    }

    pub fn delete_rule(&self, rule_id: Uuid) -> CoreResult<()> {
        self.db.delete_rule(rule_id)?;
        Ok(())
    }

    pub fn upsert_signature(&self, request: UpsertSignatureRequest) -> CoreResult<SignatureDto> {
        let id = request.id.unwrap_or_else(Uuid::new_v4);
        let account_id = request.account_id;
        let record = novamail_db::models::SignatureRecord {
            id,
            account_id,
            name: request.name,
            body_text: request.body_text,
            is_default: request.is_default,
        };
        self.db.upsert_signature(&record)?;
        Ok(self
            .db
            .list_signatures(account_id)?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| CoreError::Message("signature missing after upsert".into()))?)
    }

    pub fn list_signatures(&self, account_id: Option<Uuid>) -> CoreResult<Vec<SignatureDto>> {
        Ok(self.db.list_signatures(account_id)?)
    }

    pub fn delete_signature(&self, signature_id: Uuid) -> CoreResult<()> {
        self.db.delete_signature(signature_id)?;
        Ok(())
    }

    /// Test POP3 connectivity and return the number of messages on the server.
    pub async fn pop3_test(
        &self,
        host: String,
        port: u16,
        use_tls: bool,
        user: String,
        password: String,
    ) -> CoreResult<u32> {
        let mut client = Pop3Client::connect(&host, port, use_tls).await?;
        client.login(&user, &password).await?;
        let listed = client.list().await?;
        let count = listed.len() as u32;
        client.quit().await?;
        Ok(count)
    }

    pub fn db(&self) -> &Database {
        &self.db
    }

    pub fn secrets(&self) -> &SecretStore {
        &self.secrets
    }

    pub fn get_message_ai_insights(&self, message_id: Uuid) -> CoreResult<MessageAiInsights> {
        let summary = insight_text(&self.db, message_id, "summary")?;
        // v3 keys: greet the From contact; never copy body salutations / invent titles.
        let reply_a = insight_text(&self.db, message_id, "reply_v3_a")?;
        let reply_b = insight_text(&self.db, message_id, "reply_v3_b")?;
        let event_suggestions = insight_event_suggestions(&self.db, message_id)?;
        let provider = insight_provider(&self.db, message_id, "summary")?
            .or(insight_provider(&self.db, message_id, "event_suggestions")?)
            .or(insight_provider(&self.db, message_id, "reply_v3_a")?);
        let incomplete = summary.is_none()
            || reply_a.is_none()
            || reply_b.is_none()
            || !insight_event_suggestions_current(&self.db, message_id).unwrap_or(false);
        if incomplete {
            // Opening a mail should kick background generation if sync already missed it.
            self.enqueue_ai_insights(&[message_id]);
        }
        Ok(MessageAiInsights {
            message_id,
            summary,
            reply_suggestion: reply_a.clone(),
            reply_a,
            reply_b,
            event_suggestions,
            provider,
            incomplete,
        })
    }

    /// Backfill AI insights for recent messages that still lack them.
    pub fn enqueue_missing_ai_insights(&self, limit: u32) {
        let settings = Self::load_ai_settings(&self.db);
        if !settings.enabled {
            return;
        }
        let Ok((messages, _)) = self.db.list_messages(&ListMessagesRequest {
            account_id: None,
            mailbox_id: None,
            unified: true,
            mailbox_role: None,
            unread_only: false,
            starred_only: false,
            has_attachments: false,
            local_only: false,
            snoozed_only: false,
            limit: limit.max(1).min(100),
            offset: 0,
            query: None,
            sort_by: MessageSortBy::Date,
            sort_dir: SortDirection::Desc,
        }) else {
            return;
        };
        let mut missing = Vec::new();
        for msg in messages {
            let has_summary = self
                .db
                .has_ai_insight(msg.id, "summary")
                .unwrap_or(false);
            let has_replies = self.db.has_ai_insight(msg.id, "reply_v3_a").unwrap_or(false)
                && self.db.has_ai_insight(msg.id, "reply_v3_b").unwrap_or(false);
            let has_events =
                insight_event_suggestions_current(&self.db, msg.id).unwrap_or(false);
            if !(has_summary && has_replies && has_events) {
                missing.push(msg.id);
            }
        }
        if !missing.is_empty() {
            self.enqueue_ai_insights(&missing);
        }
    }

    pub async fn optimize_draft(
        &self,
        request: OptimizeDraftRequest,
    ) -> CoreResult<OptimizeDraftResponse> {
        let ai = self.ai_provider();
        let suggestion = match ai
            .optimize_draft(
                &request.subject,
                &request.body_text,
                request.preferred_language.as_deref(),
            )
            .await
        {
            Ok(text) => text,
            Err(err) => {
                tracing::warn!(error = %err, "optimize draft failed; using offline fallback");
                Self::offline_ai()
                    .optimize_draft(
                        &request.subject,
                        &request.body_text,
                        request.preferred_language.as_deref(),
                    )
                    .await?
            }
        };
        Ok(OptimizeDraftResponse {
            suggestion,
            provider: ai.name().into(),
        })
    }

    pub fn enqueue_ai_insights(&self, message_ids: &[Uuid]) {
        if message_ids.is_empty() {
            return;
        }
        let settings = Self::load_ai_settings(&self.db);
        // Prefer generating once AI is enabled; onboarding flag alone should not block.
        if !settings.enabled {
            return;
        }
        // Cap backlog so a large first sync does not saturate the CPU for hours.
        let ids: Vec<Uuid> = message_ids.iter().copied().take(25).collect();
        let db = self.db.clone();
        let ai = self.ai_provider();
        // Sync Tauri commands / scheduler callbacks have no Tokio reactor — never
        // call tokio::spawn directly here (that panic aborted the process on open).
        spawn_background(async move {
            static AI_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
            let _guard = AI_LOCK.lock().await;
            for message_id in ids {
                if let Err(err) =
                    Self::generate_and_store_insights(&db, ai.as_ref(), message_id).await
                {
                    tracing::debug!(%message_id, error = %err, "background AI insight skipped");
                }
            }
        });
    }

    async fn generate_and_store_insights(
        db: &Database,
        ai: &dyn AiProvider,
        message_id: Uuid,
    ) -> CoreResult<()> {
        let has_replies = db.has_ai_insight(message_id, "reply_v3_a")?
            && db.has_ai_insight(message_id, "reply_v3_b")?;
        let has_events = insight_event_suggestions_current(db, message_id)?;
        if db.has_ai_insight(message_id, "summary")? && has_replies && has_events {
            return Ok(());
        }
        let detail = db.get_message(message_id)?;
        let account = db.get_account(detail.summary.account_id).ok();
        let account_email = account
            .as_ref()
            .map(|a| a.email.clone())
            .unwrap_or_default();
        let account_name = account.as_ref().and_then(|a| {
            let n = a.name.trim();
            if n.is_empty() {
                None
            } else {
                Some(n.to_string())
            }
        });
        let body = detail
            .body_text
            .clone()
            .unwrap_or_else(|| detail.summary.snippet.clone());

        if !db.has_ai_insight(message_id, "summary")? {
            let result = match ai
                .summarize(SummarizeRequest {
                    subject: detail.summary.subject.clone(),
                    body_text: body.clone(),
                    preferred_language: None,
                })
                .await
            {
                Ok(r) => r,
                Err(_) => {
                    Self::offline_ai()
                        .summarize(SummarizeRequest {
                            subject: detail.summary.subject.clone(),
                            body_text: body.clone(),
                            preferred_language: None,
                        })
                        .await?
                }
            };
            let payload = serde_json::json!({
                "text": result.summary,
                "provider": result.provider,
            });
            db.upsert_ai_insight(message_id, "summary", &payload.to_string())?;
        }

        if !has_events {
            let ev_req = ExtractEventsRequest {
                subject: detail.summary.subject.clone(),
                body_text: body.clone(),
                reference_at: detail.summary.date,
            };
            let result = match ai.extract_event_suggestions(ev_req.clone()).await {
                Ok(r) => r,
                Err(_) => Self::offline_ai().extract_event_suggestions(ev_req).await?,
            };
            let suggestions: Vec<serde_json::Value> = result
                .suggestions
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "label": s.label,
                        "startsAt": s.starts_at,
                        "endsAt": s.ends_at,
                        "location": s.location,
                        "confidence": s.confidence,
                    })
                })
                .collect();
            let payload = serde_json::json!({
                "version": EVENT_SUGGESTIONS_VERSION,
                "suggestions": suggestions,
                "provider": result.provider,
            });
            db.upsert_ai_insight(message_id, "event_suggestions", &payload.to_string())?;
        }

        if !has_replies {
            let ai_request = SuggestReplyRequest {
                subject: detail.summary.subject.clone(),
                body_text: body,
                from_email: detail.summary.from.email.clone(),
                from_name: detail.summary.from.name.clone(),
                reply_as_email: Some(account_email),
                reply_as_name: account_name,
                facts: None,
                style: None,
                preferred_language: None,
            };
            let result = match ai.suggest_reply_variants(ai_request.clone()).await {
                Ok(r) => r,
                Err(_) => Self::offline_ai().suggest_reply_variants(ai_request).await?,
            };
            if let Some(a) = result.variants.first() {
                let payload = serde_json::json!({
                    "text": a,
                    "provider": result.provider,
                });
                db.upsert_ai_insight(message_id, "reply_v3_a", &payload.to_string())?;
            }
            if let Some(b) = result.variants.get(1) {
                let payload = serde_json::json!({
                    "text": b,
                    "provider": result.provider,
                });
                db.upsert_ai_insight(message_id, "reply_v3_b", &payload.to_string())?;
            }
        }
        Ok(())
    }

    pub async fn summarize_message(
        &self,
        request: SummarizeMessageRequest,
    ) -> CoreResult<SummarizeMessageResponse> {
        if let Some(cached) = self.db.get_ai_insight(request.message_id, "summary")? {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&cached) {
                if let (Some(text), provider) = (
                    value.get("text").and_then(|v| v.as_str()),
                    value
                        .get("provider")
                        .and_then(|v| v.as_str())
                        .unwrap_or("cache"),
                ) {
                    return Ok(SummarizeMessageResponse {
                        message_id: request.message_id,
                        summary: text.to_string(),
                        provider: provider.to_string(),
                    });
                }
            }
        }

        let detail = self.get_message(request.message_id)?;
        let body = detail
            .body_text
            .clone()
            .unwrap_or_else(|| detail.summary.snippet.clone());
        let ai = self.ai_provider();
        let result = match ai
            .summarize(SummarizeRequest {
                subject: detail.summary.subject.clone(),
                body_text: body.clone(),
                preferred_language: request.preferred_language.clone(),
            })
            .await
        {
            Ok(result) => result,
            Err(err) => {
                tracing::warn!(error = %err, "primary AI provider failed; using offline fallback");
                Self::offline_ai()
                    .summarize(SummarizeRequest {
                        subject: detail.summary.subject,
                        body_text: body,
                        preferred_language: request.preferred_language.clone(),
                    })
                    .await?
            }
        };
        let payload = serde_json::json!({
            "text": result.summary,
            "provider": result.provider,
        });
        let _ = self
            .db
            .upsert_ai_insight(request.message_id, "summary", &payload.to_string());
        Ok(SummarizeMessageResponse {
            message_id: request.message_id,
            summary: result.summary,
            provider: result.provider,
        })
    }

    pub async fn suggest_reply_message(
        &self,
        request: SuggestReplyMessageRequest,
    ) -> CoreResult<SuggestReplyMessageResponse> {
        let variants = self
            .suggest_replies_message(SuggestRepliesMessageRequest {
                message_id: request.message_id,
                facts: request.facts,
                preferred_language: request.preferred_language,
            })
            .await?;
        let suggestion = variants
            .variants
            .first()
            .cloned()
            .unwrap_or_default();
        Ok(SuggestReplyMessageResponse {
            message_id: request.message_id,
            suggestion,
            provider: variants.provider,
        })
    }

    pub async fn suggest_replies_message(
        &self,
        request: SuggestRepliesMessageRequest,
    ) -> CoreResult<SuggestRepliesMessageResponse> {
        let facts = request
            .facts
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        // Use cache only when no custom facts were requested (v3 greeting keys).
        if facts.is_none() {
            let a = insight_text(&self.db, request.message_id, "reply_v3_a")?;
            let b = insight_text(&self.db, request.message_id, "reply_v3_b")?;
            if let (Some(a), Some(b)) = (a, b) {
                let provider = insight_provider(&self.db, request.message_id, "reply_v3_a")?
                    .unwrap_or_else(|| "cache".into());
                return Ok(SuggestRepliesMessageResponse {
                    message_id: request.message_id,
                    variants: vec![a, b],
                    provider,
                });
            }
        }

        let detail = self.get_message(request.message_id)?;
        let account = self.db.get_account(detail.summary.account_id).ok();
        let account_email = account
            .as_ref()
            .map(|a| a.email.clone())
            .unwrap_or_default();
        let account_name = account.as_ref().and_then(|a| {
            let n = a.name.trim();
            if n.is_empty() {
                None
            } else {
                Some(n.to_string())
            }
        });
        let body = detail
            .body_text
            .clone()
            .unwrap_or_else(|| detail.summary.snippet.clone());
        let ai_request = SuggestReplyRequest {
            subject: detail.summary.subject.clone(),
            body_text: body,
            from_email: detail.summary.from.email.clone(),
            from_name: detail.summary.from.name.clone(),
            reply_as_email: Some(account_email),
            reply_as_name: account_name,
            facts: facts.clone(),
            style: None,
            preferred_language: request.preferred_language.clone(),
        };
        let ai = self.ai_provider();
        let result = match ai.suggest_reply_variants(ai_request.clone()).await {
            Ok(result) => result,
            Err(err) => {
                tracing::warn!(error = %err, "primary AI provider failed; using offline fallback");
                Self::offline_ai()
                    .suggest_reply_variants(ai_request)
                    .await?
            }
        };

        if facts.is_none() {
            if let Some(a) = result.variants.first() {
                let payload = serde_json::json!({
                    "text": a,
                    "provider": result.provider,
                });
                let _ = self
                    .db
                    .upsert_ai_insight(request.message_id, "reply_v3_a", &payload.to_string());
            }
            if let Some(b) = result.variants.get(1) {
                let payload = serde_json::json!({
                    "text": b,
                    "provider": result.provider,
                });
                let _ = self
                    .db
                    .upsert_ai_insight(request.message_id, "reply_v3_b", &payload.to_string());
            }
        }

        Ok(SuggestRepliesMessageResponse {
            message_id: request.message_id,
            variants: result.variants,
            provider: result.provider,
        })
    }

    /// Applies enabled rules from SQLite against a message.
    pub fn apply_rules_for_message(&self, message_id: Uuid) -> CoreResult<Vec<Action>> {
        let detail = self.get_message(message_id)?;
        let conn_rules = self.load_rule_definitions()?;
        let ctx = RuleMatchContext {
            message: &detail.summary,
            body_text: detail.body_text.as_deref().unwrap_or(""),
        };
        let matched = evaluate_rules(&conn_rules, &ctx);
        let mut actions = Vec::new();
        let mut remote_moves: Vec<(Uuid, String, String)> = Vec::new();
        let mut remote_deletes: Vec<Uuid> = Vec::new();
        let mut remote_flags: Vec<(Uuid, Option<bool>, Option<bool>)> = Vec::new();

        for (_id, acts) in matched {
            for action in acts {
                match &action {
                    Action::MarkRead => {
                        self.db.set_flags(message_id, Some(false), None)?;
                        remote_flags.push((message_id, Some(false), None));
                    }
                    Action::MarkUnread => {
                        self.db.set_flags(message_id, Some(true), None)?;
                        remote_flags.push((message_id, Some(true), None));
                    }
                    Action::Star => {
                        self.db.set_flags(message_id, None, Some(true))?;
                        remote_flags.push((message_id, None, Some(true)));
                    }
                    Action::Unstar => {
                        self.db.set_flags(message_id, None, Some(false))?;
                        remote_flags.push((message_id, None, Some(false)));
                    }
                    Action::AddLabel { label } => {
                        self.ensure_label_on_message(
                            detail.summary.account_id,
                            message_id,
                            label,
                        )?;
                    }
                    Action::MoveToMailbox { mailbox } => {
                        let (name, role) =
                            Self::resolve_mailbox_target(mailbox, detail.summary.account_id, &self.db)?;
                        let mb = self.db.ensure_mailbox(detail.summary.account_id, &name, &role)?;
                        self.db.set_message_mailbox(message_id, mb.id)?;
                        let _ = self.db.refresh_mailbox_counts(mb.id);
                        remote_moves.push((message_id, name, role));
                    }
                    Action::MoveToSpam => {
                        let (name, role) = self.ensure_spam_mailbox(detail.summary.account_id)?;
                        let mb = self.db.ensure_mailbox(detail.summary.account_id, &name, &role)?;
                        self.db.set_message_mailbox(message_id, mb.id)?;
                        let _ = self.db.refresh_mailbox_counts(mb.id);
                        remote_moves.push((message_id, name, role));
                    }
                    Action::Delete => {
                        remote_deletes.push(message_id);
                    }
                }
                actions.push(action);
            }
        }

        self.enqueue_remote_rule_side_effects(remote_moves, remote_deletes, remote_flags);
        Ok(actions)
    }

    fn ensure_label_on_message(
        &self,
        account_id: Uuid,
        message_id: Uuid,
        label_name: &str,
    ) -> CoreResult<()> {
        let label = if let Some(existing) = self.db.find_label_by_name(account_id, label_name)? {
            existing
        } else {
            self.upsert_label(UpsertLabelRequest {
                id: None,
                account_id,
                name: label_name.to_string(),
                color: "#b45309".into(),
            })?
        };
        let mut ids: Vec<Uuid> = self
            .db
            .list_message_labels(message_id)?
            .into_iter()
            .map(|l| l.id)
            .collect();
        if !ids.contains(&label.id) {
            ids.push(label.id);
            self.db.set_message_labels(message_id, &ids)?;
        }
        Ok(())
    }

    fn resolve_mailbox_target(
        target: &str,
        account_id: Uuid,
        db: &Database,
    ) -> CoreResult<(String, String)> {
        let lower = target.trim().to_ascii_lowercase();
        let role = match lower.as_str() {
            "spam" | "junk" => "junk",
            "trash" | "deleted" | "bin" => "trash",
            "archive" => "archive",
            "inbox" => "inbox",
            "sent" => "sent",
            "drafts" | "entwürfe" | "entwuerfe" => "drafts",
            _ => {
                if let Some(mb) = db.find_mailbox_by_name(account_id, target)? {
                    return Ok((
                        mb.name,
                        mb.role.unwrap_or_else(|| "archive".into()),
                    ));
                }
                if let Some(mb) = db.find_mailbox_by_role(account_id, &lower)? {
                    return Ok((mb.name, lower));
                }
                return Ok((target.to_string(), "archive".into()));
            }
        };
        if let Some(mb) = db.find_mailbox_by_role(account_id, role)? {
            return Ok((mb.name, role.into()));
        }
        let fallback_name = match role {
            "junk" => "Spam",
            "trash" => "Trash",
            "archive" => "Archive",
            "drafts" => "Drafts",
            "sent" => "Sent",
            _ => "INBOX",
        };
        Ok((fallback_name.into(), role.into()))
    }

    fn ensure_spam_mailbox(&self, account_id: Uuid) -> CoreResult<(String, String)> {
        if let Some(mb) = self.db.find_mailbox_by_role(account_id, "junk")? {
            return Ok((mb.name, "junk".into()));
        }
        for name in ["Spam", "Junk", "Junk E-mail", "Junk Email"] {
            if let Some(mb) = self.db.find_mailbox_by_name(account_id, name)? {
                let _ = self.db.ensure_mailbox(account_id, &mb.name, "junk");
                return Ok((mb.name, "junk".into()));
            }
        }
        let mb = self.db.ensure_mailbox(account_id, "Spam", "junk")?;
        Ok((mb.name, "junk".into()))
    }

    fn enqueue_remote_rule_side_effects(
        &self,
        moves: Vec<(Uuid, String, String)>,
        deletes: Vec<Uuid>,
        flags: Vec<(Uuid, Option<bool>, Option<bool>)>,
    ) {
        if moves.is_empty() && deletes.is_empty() && flags.is_empty() {
            return;
        }
        let db = self.db.clone();
        let secrets = self.secrets.clone();
        spawn_background(async move {
            for (message_id, name, role) in moves {
                if let Err(err) = move_remote(&db, &secrets, message_id, &name, Some(&role)).await {
                    tracing::warn!(%message_id, error = %err, "rule IMAP move failed");
                }
            }
            for message_id in deletes {
                if let Err(err) = delete_remote(&db, &secrets, message_id).await {
                    tracing::warn!(%message_id, error = %err, "rule IMAP delete failed");
                }
                let _ = db.delete_message(message_id);
            }
            for (message_id, unread, starred) in flags {
                if let Err(err) = set_flags_remote(&db, &secrets, message_id, unread, starred).await
                {
                    tracing::debug!(%message_id, error = %err, "rule IMAP flag sync failed");
                }
            }
        });
    }

    pub async fn move_message(&self, request: MoveMessageRequest) -> CoreResult<()> {
        let detail = self.get_message(request.message_id)?;
        let (name, role) =
            Self::resolve_mailbox_target(&request.target, detail.summary.account_id, &self.db)?;
        let mb = self
            .db
            .ensure_mailbox(detail.summary.account_id, &name, &role)?;
        self.db
            .set_message_mailbox(request.message_id, mb.id)?;
        let _ = self.db.refresh_mailbox_counts(mb.id);
        let _ = self.db.refresh_mailbox_counts(detail.summary.mailbox_id);
        if let Err(err) =
            move_remote(&self.db, &self.secrets, request.message_id, &name, Some(&role)).await
        {
            tracing::warn!(error = %err, "IMAP move failed; local mailbox updated");
        }
        Ok(())
    }

    pub async fn mark_spam(&self, message_id: Uuid) -> CoreResult<()> {
        let detail = self.get_message(message_id)?;
        let body = detail.body_text.as_deref().unwrap_or("");
        spam::train(
            &self.paths.data_dir,
            &detail.summary.subject,
            &detail.summary.from.email,
            body,
            true,
        )?;
        self.move_message(MoveMessageRequest {
            message_id,
            target: "junk".into(),
        })
        .await
    }

    pub async fn mark_not_spam(&self, message_id: Uuid) -> CoreResult<()> {
        let detail = self.get_message(message_id)?;
        let body = detail.body_text.as_deref().unwrap_or("");
        spam::train(
            &self.paths.data_dir,
            &detail.summary.subject,
            &detail.summary.from.email,
            body,
            false,
        )?;
        self.move_message(MoveMessageRequest {
            message_id,
            target: "inbox".into(),
        })
        .await
    }

    fn apply_spam_filter(&self, message_id: Uuid) -> CoreResult<()> {
        let settings = self.load_spam_settings();
        if !settings.enabled || !settings.auto_move {
            return Ok(());
        }
        let detail = self.get_message(message_id)?;
        // Skip messages already in junk/trash/drafts.
        if let Ok(mb) = self.db.get_mailbox(detail.summary.mailbox_id) {
            if matches!(
                mb.role.as_deref(),
                Some("junk" | "trash" | "drafts" | "sent")
            ) {
                return Ok(());
            }
        }
        let body = detail.body_text.as_deref().unwrap_or("");
        let verdict = spam::score_message(
            &self.paths.data_dir,
            &settings,
            &detail.summary.subject,
            &detail.summary.from.email,
            body,
        );
        if !verdict.is_spam {
            return Ok(());
        }
        tracing::info!(
            %message_id,
            score = verdict.score,
            reasons = ?verdict.reasons,
            "auto-moving message to spam"
        );
        let (name, role) = self.ensure_spam_mailbox(detail.summary.account_id)?;
        let mb = self
            .db
            .ensure_mailbox(detail.summary.account_id, &name, &role)?;
        self.db.set_message_mailbox(message_id, mb.id)?;
        let _ = self.db.refresh_mailbox_counts(mb.id);
        self.enqueue_remote_rule_side_effects(
            vec![(message_id, name, role)],
            Vec::new(),
            Vec::new(),
        );
        Ok(())
    }

    pub fn spam_settings(&self) -> CoreResult<SpamSettingsDto> {
        let settings = self.load_spam_settings();
        let (trained_spam, trained_ham) = spam::model_stats(&self.paths.data_dir);
        Ok(SpamSettingsDto {
            enabled: settings.enabled,
            auto_move: settings.auto_move,
            threshold: settings.threshold,
            strict_heuristics: settings.strict_heuristics,
            trained_spam,
            trained_ham,
        })
    }

    pub fn set_spam_settings(&self, dto: SpamSettingsDto) -> CoreResult<SpamSettingsDto> {
        let settings = SpamSettings {
            enabled: dto.enabled,
            auto_move: dto.auto_move,
            threshold: dto.threshold.clamp(0.4, 0.99),
            strict_heuristics: dto.strict_heuristics,
        };
        self.db.set_setting(
            SPAM_SETTINGS_KEY,
            &serde_json::to_string(&settings).map_err(|e| CoreError::Message(e.to_string()))?,
        )?;
        self.spam_settings()
    }

    pub fn score_spam(&self, message_id: Uuid) -> CoreResult<SpamScoreDto> {
        let detail = self.get_message(message_id)?;
        let settings = self.load_spam_settings();
        let body = detail.body_text.as_deref().unwrap_or("");
        let verdict = spam::score_message(
            &self.paths.data_dir,
            &settings,
            &detail.summary.subject,
            &detail.summary.from.email,
            body,
        );
        Ok(SpamScoreDto {
            message_id,
            score: verdict.score,
            is_spam: verdict.is_spam,
            reasons: verdict.reasons,
        })
    }

    fn load_spam_settings(&self) -> SpamSettings {
        self.db
            .get_setting(SPAM_SETTINGS_KEY)
            .ok()
            .flatten()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn folder_policies(&self) -> CoreResult<FolderPoliciesDto> {
        Ok(Self::policies_to_dto(&self.load_folder_policies()))
    }

    pub fn set_folder_policies(&self, dto: FolderPoliciesDto) -> CoreResult<FolderPoliciesDto> {
        let policies = FolderPolicies {
            policies: dto
                .policies
                .into_iter()
                .map(|p| FolderPolicy {
                    role: p.role,
                    mode: match p.mode {
                        RetentionModeDto::Keep => RetentionMode::Keep,
                        RetentionModeDto::DeleteAfterDays => RetentionMode::DeleteAfterDays,
                    },
                    days: p.days.max(1).min(3650),
                })
                .collect(),
        };
        self.db.set_setting(
            FOLDER_POLICIES_KEY,
            &serde_json::to_string(&policies).map_err(|e| CoreError::Message(e.to_string()))?,
        )?;
        Ok(Self::policies_to_dto(&policies))
    }

    fn load_folder_policies(&self) -> FolderPolicies {
        self.db
            .get_setting(FOLDER_POLICIES_KEY)
            .ok()
            .flatten()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    fn policies_to_dto(policies: &FolderPolicies) -> FolderPoliciesDto {
        FolderPoliciesDto {
            policies: policies
                .policies
                .iter()
                .map(|p| FolderPolicyDto {
                    role: p.role.clone(),
                    mode: match p.mode {
                        RetentionMode::Keep => RetentionModeDto::Keep,
                        RetentionMode::DeleteAfterDays => RetentionModeDto::DeleteAfterDays,
                    },
                    days: p.days,
                })
                .collect(),
        }
    }

    pub fn apply_folder_retention(&self) -> CoreResult<u32> {
        let policies = self.load_folder_policies();
        let now = chrono::Utc::now().timestamp();
        let mut deleted = 0u32;
        for policy in &policies.policies {
            if policy.mode != RetentionMode::DeleteAfterDays {
                continue;
            }
            let cutoff = now - (i64::from(policy.days) * 86_400);
            let ids = self
                .db
                .list_message_ids_in_role_older_than(&policy.role, cutoff)?;
            for id in ids {
                if self.db.delete_message(id).is_ok() {
                    deleted += 1;
                }
            }
        }
        Ok(deleted)
    }

    pub fn offline_mailbox_settings(&self) -> CoreResult<OfflineMailboxSettingsDto> {
        Ok(self.load_offline_mailbox_settings().to_dto())
    }

    pub fn set_offline_mailbox_settings(
        &self,
        dto: OfflineMailboxSettingsDto,
    ) -> CoreResult<OfflineMailboxSettingsDto> {
        let settings = OfflineMailboxSettings::from_dto(dto);
        self.db.set_setting(
            OFFLINE_MAILBOX_KEY,
            &serde_json::to_string(&settings).map_err(|e| CoreError::Message(e.to_string()))?,
        )?;
        Ok(settings.to_dto())
    }

    pub fn set_offline_account_policy(
        &self,
        policy: OfflineMailboxAccountPolicy,
    ) -> CoreResult<OfflineMailboxAccountPolicy> {
        let policy = offline_mailbox::sanitize_policy(policy);
        let mut settings = self.load_offline_mailbox_settings();
        settings.upsert(policy.clone());
        self.db.set_setting(
            OFFLINE_MAILBOX_KEY,
            &serde_json::to_string(&settings).map_err(|e| CoreError::Message(e.to_string()))?,
        )?;
        Ok(policy)
    }

    fn load_offline_mailbox_settings(&self) -> OfflineMailboxSettings {
        self.db
            .get_setting(OFFLINE_MAILBOX_KEY)
            .ok()
            .flatten()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub async fn account_quota(&self, account_id: Uuid) -> CoreResult<AccountQuotaDto> {
        Ok(probe_account_quota(&self.db, &self.secrets, account_id).await?)
    }

    pub fn local_only_count(&self, account_id: Option<Uuid>) -> CoreResult<u32> {
        Ok(self.db.count_local_only_messages(account_id)?)
    }

    pub async fn run_offline_offload(
        &self,
        account_id: Uuid,
        force: bool,
    ) -> CoreResult<OfflineOffloadReport> {
        let policy = self.load_offline_mailbox_settings().policy_for(account_id);
        let quota = probe_account_quota(&self.db, &self.secrets, account_id).await?;
        if !force && !offline_mailbox::should_offload(&policy, quota.percent) {
            return Ok(OfflineOffloadReport {
                account_id,
                candidates: 0,
                offloaded: 0,
                skipped_incomplete: 0,
                skipped_starred: 0,
                freed_bytes: 0,
                errors: 0,
            });
        }
        self.offload_with_policy(account_id, &policy).await
    }

    pub async fn process_offline_after_sync(
        &self,
        account_id: Uuid,
    ) -> CoreResult<Option<OfflinePromptEvent>> {
        self.apply_offline_mailbox_after_sync(account_id).await
    }

    async fn apply_offline_mailbox_after_sync(
        &self,
        account_id: Uuid,
    ) -> CoreResult<Option<OfflinePromptEvent>> {
        let policy = self.load_offline_mailbox_settings().policy_for(account_id);
        let quota = match probe_account_quota(&self.db, &self.secrets, account_id).await {
            Ok(q) => q,
            Err(err) => {
                tracing::debug!(%account_id, error = %err, "quota probe skipped");
                return Ok(None);
            }
        };

        if offline_mailbox::should_offload(&policy, quota.percent) {
            match self.offload_with_policy(account_id, &policy).await {
                Ok(report) if report.offloaded > 0 => {
                    tracing::info!(
                        %account_id,
                        offloaded = report.offloaded,
                        freed = report.freed_bytes,
                        "offline mailbox offload completed"
                    );
                }
                Ok(_) => {}
                Err(err) => tracing::warn!(%account_id, error = %err, "offload batch failed"),
            }
        }

        if offline_mailbox::should_prompt(&policy, quota.percent) {
            let account = self.db.get_account(account_id)?;
            return Ok(Some(OfflinePromptEvent {
                account_id,
                account_email: account.email,
                percent: quota.percent.unwrap_or(0.0),
                used_bytes: quota.used_bytes,
                limit_bytes: quota.limit_bytes,
            }));
        }
        Ok(None)
    }

    async fn offload_with_policy(
        &self,
        account_id: Uuid,
        policy: &OfflineMailboxAccountPolicy,
    ) -> CoreResult<OfflineOffloadReport> {
        let now = chrono::Utc::now().timestamp();
        let older_than = now - (i64::from(policy.min_age_days) * 86_400);
        let candidates = self.db.list_offload_candidates(
            account_id,
            older_than,
            policy.keep_starred_on_imap,
            policy.batch_limit,
        )?;
        let mut report = OfflineOffloadReport {
            account_id,
            candidates: candidates.len() as u32,
            offloaded: 0,
            skipped_incomplete: 0,
            skipped_starred: 0,
            freed_bytes: 0,
            errors: 0,
        };

        for message_id in candidates {
            if let Ok(detail) = self.db.get_message(message_id) {
                if detail.summary.starred && policy.keep_starred_on_imap {
                    report.skipped_starred += 1;
                    continue;
                }
            }
            match offload_message_remote(
                &self.db,
                &self.secrets,
                &self.paths.blobs_dir,
                message_id,
            )
            .await
            {
                Ok(freed) => {
                    report.offloaded += 1;
                    report.freed_bytes += freed;
                }
                Err(err) => {
                    let msg = err.to_string();
                    if msg.contains("incomplete") {
                        report.skipped_incomplete += 1;
                    } else {
                        report.errors += 1;
                        tracing::debug!(%message_id, error = %err, "offload skipped");
                    }
                }
            }
        }
        Ok(report)
    }

    pub fn dismiss_offline_prompt(&self, account_id: Uuid) -> CoreResult<()> {
        let mut settings = self.load_offline_mailbox_settings();
        let mut policy = settings.policy_for(account_id);
        policy.prompt_dismissed = true;
        settings.upsert(policy);
        self.db.set_setting(
            OFFLINE_MAILBOX_KEY,
            &serde_json::to_string(&settings).map_err(|e| CoreError::Message(e.to_string()))?,
        )?;
        Ok(())
    }

    pub fn enable_offline_from_prompt(
        &self,
        account_id: Uuid,
        mode: OfflineMailboxMode,
    ) -> CoreResult<OfflineMailboxAccountPolicy> {
        let mut policy = self.load_offline_mailbox_settings().policy_for(account_id);
        policy.mode = mode;
        policy.prompt_dismissed = true;
        if matches!(mode, OfflineMailboxMode::Off) {
            policy.mode = OfflineMailboxMode::Threshold;
        }
        self.set_offline_account_policy(policy)
    }


    fn load_rule_definitions(&self) -> CoreResult<Vec<RuleDefinition>> {
        let mut out = Vec::new();
        for rule in self.db.list_rules()? {
            if !rule.enabled {
                continue;
            }
            let predicate = serde_json::from_str(&rule.predicate_json).map_err(|e| {
                CoreError::Message(format!("invalid rule predicate {}: {e}", rule.id))
            })?;
            let actions = serde_json::from_str(&rule.action_json).map_err(|e| {
                CoreError::Message(format!("invalid rule actions {}: {e}", rule.id))
            })?;
            out.push(RuleDefinition {
                id: rule.id,
                account_id: rule.account_id,
                name: rule.name,
                enabled: rule.enabled,
                predicate,
                actions,
            });
        }
        Ok(out)
    }

    pub async fn prioritize_message(&self, message_id: Uuid) -> CoreResult<(f32, String, String)> {
        let detail = self.get_message(message_id)?;
        let request = PrioritizeRequest {
            subject: detail.summary.subject,
            snippet: detail.summary.snippet,
            from_email: detail.summary.from.email,
        };
        let ai = self.ai_provider();
        let result = match ai.prioritize(request.clone()).await {
            Ok(result) => result,
            Err(_) => Self::offline_ai().prioritize(request).await?,
        };
        Ok((result.score, result.rationale, result.provider))
    }

    // —— OpenPGP ——

    pub fn pgp_list_keys(&self) -> CoreResult<Vec<PgpKeyDto>> {
        Ok(self.db.list_pgp_keys()?)
    }

    pub fn pgp_generate(&self, request: PgpGenerateRequest) -> CoreResult<PgpKeyDto> {
        let info = generate_key(&request.user_id)?;
        self.db.upsert_pgp_key(
            &info.fingerprint,
            &info.user_ids,
            info.has_secret,
            &info.armored_public,
            info.armored_secret.as_deref(),
        )?;
        Ok(PgpKeyDto {
            fingerprint: info.fingerprint,
            user_ids: info.user_ids,
            has_secret: info.has_secret,
            created_at: chrono::Utc::now().timestamp(),
        })
    }

    pub fn pgp_import(&self, request: PgpImportRequest) -> CoreResult<PgpKeyDto> {
        let info = import_armored(&request.armored)?;
        self.db.upsert_pgp_key(
            &info.fingerprint,
            &info.user_ids,
            info.has_secret,
            &info.armored_public,
            info.armored_secret.as_deref(),
        )?;
        Ok(PgpKeyDto {
            fingerprint: info.fingerprint,
            user_ids: info.user_ids,
            has_secret: info.has_secret,
            created_at: chrono::Utc::now().timestamp(),
        })
    }

    pub fn pgp_delete(&self, fingerprint: &str) -> CoreResult<()> {
        self.db.delete_pgp_key(fingerprint)?;
        Ok(())
    }

    pub fn pgp_export_public(&self, fingerprint: &str) -> CoreResult<String> {
        self.db
            .get_pgp_public(fingerprint)?
            .ok_or_else(|| CoreError::Message("key not found".into()))
    }

    pub fn pgp_decrypt_text(&self, armored: &str) -> CoreResult<PgpDecryptResult> {
        let secrets = self.db.list_pgp_secrets()?;
        let publics = self.db.list_pgp_publics()?;
        let secret_refs: Vec<&str> = secrets.iter().map(String::as_str).collect();
        let public_refs: Vec<&str> = publics.iter().map(String::as_str).collect();
        let result = decrypt_message(armored, &secret_refs, &public_refs)?;
        Ok(PgpDecryptResult {
            plaintext: result.plaintext,
            signature_valid: result.signature_valid,
            signer_fpr: result.signer_fpr,
        })
    }

    pub fn pgp_verify_text(&self, armored: &str) -> CoreResult<PgpVerifyResult> {
        let publics = self.db.list_pgp_publics()?;
        let public_refs: Vec<&str> = publics.iter().map(String::as_str).collect();
        let result = verify_message(armored, &public_refs)?;
        Ok(PgpVerifyResult {
            plaintext: result.plaintext,
            valid: result.valid,
            signer_fpr: result.signer_fpr,
        })
    }

    pub fn pgp_inspect_message(&self, message_id: Uuid) -> CoreResult<Option<PgpDecryptResult>> {
        let detail = self.get_message(message_id)?;
        let text = detail
            .body_text
            .unwrap_or_else(|| detail.summary.snippet.clone());
        if !looks_like_pgp(&text) {
            return Ok(None);
        }
        if text.contains("-----BEGIN PGP MESSAGE-----") {
            Ok(Some(self.pgp_decrypt_text(&text)?))
        } else {
            let verified = self.pgp_verify_text(&text)?;
            Ok(Some(PgpDecryptResult {
                plaintext: verified.plaintext,
                signature_valid: Some(verified.valid),
                signer_fpr: verified.signer_fpr,
            }))
        }
    }

    // —— Calendar / Tasks ——

    const COLLECTION_COLORS: &'static [&'static str] = &[
        "#1e3a5f", "#0f766e", "#166534", "#b45309", "#be123c", "#334155",
    ];

    pub fn ensure_calendar_ready(&self) -> CoreResult<CalendarCollectionDto> {
        Ok(self.db.ensure_local_default_collection()?)
    }

    pub fn list_calendar_accounts(&self) -> CoreResult<Vec<CalendarAccountDto>> {
        Ok(self.db.list_calendar_accounts()?)
    }

    pub fn upsert_calendar_account(
        &self,
        request: UpsertCalendarAccountRequest,
    ) -> CoreResult<CalendarAccountDto> {
        let _ = self.ensure_calendar_ready()?;
        let id = request.id.unwrap_or_else(Uuid::new_v4);
        self.db.upsert_calendar_account(
            id,
            &request.name,
            &request.caldav_url,
            &request.username,
        )?;
        if let Some(password) = request.password {
            if !password.is_empty() {
                self.secrets.store_calendar_password(id, &password)?;
            }
        }
        if !request.collections.is_empty() {
            let make_default = self
                .db
                .list_calendar_collections()?
                .iter()
                .all(|c| c.calendar_account_id.is_none());
            for (idx, col) in request.collections.iter().enumerate() {
                let col_id = Uuid::new_v5(
                    &Uuid::NAMESPACE_URL,
                    format!("{}:{}", id, col.href).as_bytes(),
                );
                let color = Self::COLLECTION_COLORS[idx % Self::COLLECTION_COLORS.len()];
                self.db.upsert_calendar_collection(
                    col_id,
                    Some(id),
                    Some(&col.href),
                    &col.display_name,
                    color,
                    true,
                    make_default && idx == 0,
                )?;
            }
        } else {
            // Single-URL account: treat caldav_url as the collection.
            let col_id = Uuid::new_v5(&Uuid::NAMESPACE_URL, format!("{id}:root").as_bytes());
            let has_default = self
                .db
                .list_calendar_collections()?
                .iter()
                .any(|c| c.is_default);
            self.db.upsert_calendar_collection(
                col_id,
                Some(id),
                Some(&request.caldav_url),
                &request.name,
                Self::COLLECTION_COLORS[0],
                true,
                !has_default,
            )?;
        }
        self.db
            .list_calendar_accounts()?
            .into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| CoreError::Message("calendar account missing after upsert".into()))
    }

    pub fn delete_calendar_account(&self, id: Uuid) -> CoreResult<()> {
        let _ = self.secrets.delete_calendar_password(id);
        self.db.delete_calendar_account(id)?;
        let _ = self.ensure_calendar_ready()?;
        Ok(())
    }

    pub fn list_calendar_collections(&self) -> CoreResult<Vec<CalendarCollectionDto>> {
        let _ = self.ensure_calendar_ready()?;
        Ok(self.db.list_calendar_collections()?)
    }

    pub fn upsert_calendar_collection(
        &self,
        request: UpsertCalendarCollectionRequest,
    ) -> CoreResult<CalendarCollectionDto> {
        let id = request.id.unwrap_or_else(Uuid::new_v4);
        self.db.upsert_calendar_collection(
            id,
            request.calendar_account_id,
            request.href.as_deref(),
            &request.display_name,
            &request.color,
            request.is_visible,
            request.is_default,
        )?;
        self.db
            .get_calendar_collection(id)?
            .ok_or_else(|| CoreError::Message("calendar collection missing after upsert".into()))
    }

    pub fn set_default_calendar_collection(&self, id: Uuid) -> CoreResult<CalendarCollectionDto> {
        self.db.set_default_calendar_collection(id)?;
        self.db
            .get_calendar_collection(id)?
            .ok_or_else(|| CoreError::Message("calendar collection not found".into()))
    }

    pub fn delete_calendar_collection(&self, id: Uuid) -> CoreResult<()> {
        self.db.delete_calendar_collection(id)?;
        // Keep at least one default calendar available.
        let _ = self.ensure_calendar_ready()?;
        Ok(())
    }

    pub async fn sync_calendar_account(&self, id: Uuid) -> CoreResult<(u32, u32)> {
        let account = self
            .db
            .list_calendar_accounts()?
            .into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| CoreError::Message("calendar account not found".into()))?;
        let password = self
            .secrets
            .load_calendar_password(id)?
            .unwrap_or_default();
        let collections: Vec<_> = self
            .db
            .list_calendar_collections()?
            .into_iter()
            .filter(|c| c.calendar_account_id == Some(id) && c.is_visible)
            .collect();
        let targets = if collections.is_empty() {
            vec![(None, account.caldav_url.clone())]
        } else {
            collections
                .into_iter()
                .filter_map(|c| c.href.clone().map(|h| (Some(c.id), h)))
                .collect()
        };
        let mut events = 0u32;
        let mut tasks = 0u32;
        for (collection_id, href) in targets {
            let result =
                crate::caldav::sync_calendar(&href, &account.username, &password).await?;
            for event in result.events {
                let event_id = Uuid::new_v5(&Uuid::NAMESPACE_URL, event.uid.as_bytes());
                let reminders: Vec<CalendarReminderDto> = event
                    .reminder_minutes
                    .iter()
                    .map(|m| CalendarReminderDto { minutes: *m })
                    .collect();
                let reminders_json = serde_json::to_string(&reminders).unwrap_or_else(|_| "[]".into());
                self.db.upsert_calendar_event(
                    event_id,
                    Some(account.id),
                    collection_id,
                    Some(&event.uid),
                    &event.title,
                    event.starts_at,
                    event.ends_at,
                    event.location.as_deref(),
                    event.description.as_deref(),
                    event.all_day,
                    None,
                    &reminders_json,
                    &event.status,
                    event.organizer.as_deref(),
                    "[]",
                    event.etag.as_deref(),
                    event.href.as_deref(),
                )?;
                events += 1;
            }
            for task in result.tasks {
                let task_id = Uuid::new_v5(&Uuid::NAMESPACE_URL, task.uid.as_bytes());
                self.db.upsert_calendar_task(
                    task_id,
                    Some(account.id),
                    Some(&task.uid),
                    &task.title,
                    task.due_at,
                    task.completed,
                    &task.notes,
                    None,
                )?;
                tasks += 1;
            }
        }
        Ok((events, tasks))
    }

    pub fn list_calendar_events(
        &self,
        request: ListCalendarRangeRequest,
    ) -> CoreResult<Vec<CalendarEventDto>> {
        let _ = self.ensure_calendar_ready()?;
        Ok(self.db.list_calendar_events(request.from, request.to)?)
    }

    pub async fn discover_caldav(
        &self,
        request: DiscoverCalDavRequest,
    ) -> CoreResult<Vec<CalDavCollectionDto>> {
        let found = crate::caldav::discover_calendars(
            &request.caldav_url,
            &request.username,
            &request.password,
        )
        .await?;
        Ok(found
            .into_iter()
            .map(|c| CalDavCollectionDto {
                href: c.href,
                display_name: c.display_name,
            })
            .collect())
    }

    pub async fn upsert_calendar_event(
        &self,
        request: UpsertCalendarEventRequest,
    ) -> CoreResult<CalendarEventDto> {
        let default = self.ensure_calendar_ready()?;
        let collection_id = request.collection_id.or(Some(default.id));
        let collection = match collection_id {
            Some(cid) => self.db.get_calendar_collection(cid)?,
            None => None,
        };
        let calendar_account_id = request
            .calendar_account_id
            .or_else(|| collection.as_ref().and_then(|c| c.calendar_account_id));
        let id = request.id.unwrap_or_else(Uuid::new_v4);
        let existing = self.db.get_calendar_event(id)?;
        let ical_uid = existing
            .as_ref()
            .and_then(|e| e.ical_uid.clone())
            .unwrap_or_else(|| id.to_string());
        let mut etag = existing.as_ref().and_then(|e| e.etag.clone());
        let mut object_href = existing.as_ref().and_then(|e| e.href.clone());
        let status = request
            .status
            .clone()
            .unwrap_or_else(|| "confirmed".into());
        let reminders_json =
            serde_json::to_string(&request.reminders).unwrap_or_else(|_| "[]".into());
        let color = collection.as_ref().map(|c| c.color.clone());
        self.db.upsert_calendar_event(
            id,
            calendar_account_id,
            collection_id,
            Some(&ical_uid),
            &request.title,
            request.starts_at,
            request.ends_at,
            request.location.as_deref(),
            request.description.as_deref(),
            request.all_day,
            request.source_message_id,
            &reminders_json,
            &status,
            existing.as_ref().and_then(|e| e.organizer.as_deref()),
            "[]",
            etag.as_deref(),
            object_href.as_deref(),
        )?;
        if let (Some(account_id), Some(col)) = (calendar_account_id, collection.as_ref()) {
            if let Some(collection_url) = col.href.as_deref() {
                let minutes: Vec<i64> = request.reminders.iter().map(|r| r.minutes).collect();
                match self
                    .push_event_to_caldav(
                        account_id,
                        collection_url,
                        &ical_uid,
                        &request.title,
                        request.starts_at,
                        request.ends_at,
                        request.location.as_deref(),
                        request.description.as_deref(),
                        request.all_day,
                        &status,
                        &minutes,
                        etag.as_deref(),
                        object_href.as_deref(),
                    )
                    .await
                {
                    Ok(put) => {
                        etag = put.etag.or(etag);
                        object_href = Some(put.href);
                        let _ = self.db.upsert_calendar_event(
                            id,
                            calendar_account_id,
                            collection_id,
                            Some(&ical_uid),
                            &request.title,
                            request.starts_at,
                            request.ends_at,
                            request.location.as_deref(),
                            request.description.as_deref(),
                            request.all_day,
                            request.source_message_id,
                            &reminders_json,
                            &status,
                            existing.as_ref().and_then(|e| e.organizer.as_deref()),
                            "[]",
                            etag.as_deref(),
                            object_href.as_deref(),
                        );
                    }
                    Err(err) => {
                        tracing::warn!(error = %err, "CalDAV event upload failed; kept locally");
                    }
                }
            }
        }
        Ok(CalendarEventDto {
            id,
            calendar_account_id,
            collection_id,
            ical_uid: Some(ical_uid),
            title: request.title,
            starts_at: request.starts_at,
            ends_at: request.ends_at,
            location: request.location,
            description: request.description,
            all_day: request.all_day,
            source_message_id: request.source_message_id,
            reminders: request.reminders,
            status,
            organizer: existing.and_then(|e| e.organizer),
            attendees: vec![],
            color,
            etag,
            href: object_href,
        })
    }

    async fn push_event_to_caldav(
        &self,
        account_id: Uuid,
        collection_url: &str,
        ical_uid: &str,
        title: &str,
        starts_at: i64,
        ends_at: Option<i64>,
        location: Option<&str>,
        description: Option<&str>,
        all_day: bool,
        status: &str,
        reminder_minutes: &[i64],
        if_match: Option<&str>,
        object_url: Option<&str>,
    ) -> CoreResult<crate::caldav::PutVeventResult> {
        let account = self
            .db
            .list_calendar_accounts()?
            .into_iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| CoreError::Message("calendar account not found".into()))?;
        let password = self
            .secrets
            .load_calendar_password(account_id)?
            .unwrap_or_default();
        let ics = crate::caldav::build_vevent_ics(
            ical_uid,
            title,
            starts_at,
            ends_at,
            location,
            description,
            all_day,
            status,
            reminder_minutes,
        );
        crate::caldav::put_vevent(
            collection_url,
            &account.username,
            &password,
            ical_uid,
            &ics,
            if_match,
            object_url,
        )
        .await
    }

    pub async fn delete_calendar_event(&self, id: Uuid) -> CoreResult<()> {
        let existing = self.db.get_calendar_event(id)?;
        self.db.delete_calendar_event(id)?;
        if let Some(event) = existing {
            if let (Some(account_id), Some(uid), Some(collection_id)) = (
                event.calendar_account_id,
                event.ical_uid,
                event.collection_id,
            ) {
                if let Some(col) = self.db.get_calendar_collection(collection_id)? {
                    if let Some(href) = col.href {
                        let account = self
                            .db
                            .list_calendar_accounts()?
                            .into_iter()
                            .find(|a| a.id == account_id);
                        if let Some(account) = account {
                            let password = self
                                .secrets
                                .load_calendar_password(account_id)?
                                .unwrap_or_default();
                            if let Err(err) = crate::caldav::delete_object(
                                &href,
                                &account.username,
                                &password,
                                &uid,
                            )
                            .await
                            {
                                tracing::warn!(error = %err, "CalDAV event delete failed");
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn list_calendar_tasks(&self, include_completed: bool) -> CoreResult<Vec<CalendarTaskDto>> {
        Ok(self.db.list_calendar_tasks(include_completed)?)
    }

    pub fn upsert_calendar_task(
        &self,
        request: UpsertCalendarTaskRequest,
    ) -> CoreResult<CalendarTaskDto> {
        let id = request.id.unwrap_or_else(Uuid::new_v4);
        let now = chrono::Utc::now().timestamp();
        self.db.upsert_calendar_task(
            id,
            request.calendar_account_id,
            None,
            &request.title,
            request.due_at,
            request.completed,
            &request.notes,
            request.source_message_id,
        )?;
        Ok(CalendarTaskDto {
            id,
            calendar_account_id: request.calendar_account_id,
            ical_uid: None,
            title: request.title,
            due_at: request.due_at,
            completed: request.completed,
            notes: request.notes,
            source_message_id: request.source_message_id,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn delete_calendar_task(&self, id: Uuid) -> CoreResult<()> {
        self.db.delete_calendar_task(id)?;
        Ok(())
    }

    pub async fn create_event_from_message(
        &self,
        message_id: Uuid,
        starts_at: i64,
        ends_at: Option<i64>,
        calendar_account_id: Option<Uuid>,
    ) -> CoreResult<CalendarEventDto> {
        let detail = self.get_message(message_id)?;
        let default = self.ensure_calendar_ready()?;
        let collection = if let Some(aid) = calendar_account_id {
            self.db
                .list_calendar_collections()?
                .into_iter()
                .find(|c| c.calendar_account_id == Some(aid))
                .unwrap_or(default)
        } else {
            default
        };
        self.upsert_calendar_event(UpsertCalendarEventRequest {
            id: None,
            calendar_account_id: collection.calendar_account_id,
            collection_id: Some(collection.id),
            title: detail.summary.subject.clone(),
            starts_at,
            ends_at,
            location: None,
            description: Some(format!(
                "From mail: {}\n{}",
                detail.summary.from.email,
                detail.body_text.unwrap_or(detail.summary.snippet)
            )),
            all_day: false,
            source_message_id: Some(message_id),
            reminders: vec![CalendarReminderDto { minutes: 15 }],
            status: Some("confirmed".into()),
        })
        .await
    }

    pub fn create_task_from_message(
        &self,
        message_id: Uuid,
        due_at: Option<i64>,
    ) -> CoreResult<CalendarTaskDto> {
        let detail = self.get_message(message_id)?;
        self.upsert_calendar_task(UpsertCalendarTaskRequest {
            id: None,
            calendar_account_id: None,
            title: detail.summary.subject.clone(),
            due_at,
            completed: false,
            notes: format!("From mail: {}", detail.summary.from.email),
            source_message_id: Some(message_id),
        })
    }

    pub fn list_calendar_invitations(
        &self,
        pending_only: bool,
    ) -> CoreResult<Vec<CalendarInvitationDto>> {
        let _ = self.db.scan_messages_for_invites(40);
        Ok(self.db.list_calendar_invitations(pending_only)?)
    }

    pub async fn respond_calendar_invitation(
        &self,
        request: RespondInvitationRequest,
    ) -> CoreResult<CalendarInvitationDto> {
        let (invite, _ics) = self
            .db
            .get_calendar_invitation(request.id)?
            .ok_or_else(|| CoreError::Message("invitation not found".into()))?;
        let partstat = match request.response {
            InvitationResponse::Accept => "accepted",
            InvitationResponse::Decline => "declined",
            InvitationResponse::Tentative => "tentative",
        };
        if matches!(
            request.response,
            InvitationResponse::Accept | InvitationResponse::Tentative
        ) {
            let default = self.ensure_calendar_ready()?;
            let status = if matches!(request.response, InvitationResponse::Tentative) {
                "tentative"
            } else {
                "confirmed"
            };
            let _ = self
                .upsert_calendar_event(UpsertCalendarEventRequest {
                    id: None,
                    calendar_account_id: default.calendar_account_id,
                    collection_id: Some(default.id),
                    title: invite.title.clone(),
                    starts_at: invite.starts_at,
                    ends_at: invite.ends_at,
                    location: invite.location.clone(),
                    description: invite.description.clone(),
                    all_day: false,
                    source_message_id: invite.message_id,
                    reminders: vec![CalendarReminderDto { minutes: 15 }],
                    status: Some(status.into()),
                })
                .await?;
        }
        if let Err(err) = self.send_imip_reply(&invite, partstat).await {
            tracing::warn!(error = %err, "iMIP REPLY send failed; RSVP kept locally");
        }
        self.db.set_invitation_partstat(request.id, partstat)?;
        self.db
            .list_calendar_invitations(false)?
            .into_iter()
            .find(|i| i.id == request.id)
            .ok_or_else(|| CoreError::Message("invitation missing after respond".into()))
    }

    async fn send_imip_reply(
        &self,
        invite: &CalendarInvitationDto,
        partstat: &str,
    ) -> CoreResult<()> {
        let Some(organizer) = invite.organizer.as_ref().filter(|s| !s.is_empty()) else {
            return Ok(());
        };
        let (account_id, attendee_email) = if let Some(message_id) = invite.message_id {
            let detail = self.db.get_message(message_id)?;
            let rec = self.db.get_account(detail.summary.account_id)?;
            (rec.id, rec.email)
        } else {
            let account = self
                .db
                .list_accounts()?
                .into_iter()
                .next()
                .ok_or_else(|| CoreError::Message("no mail account for iMIP reply".into()))?;
            (account.id, account.email)
        };
        let reply_ics = crate::caldav::build_imip_reply(
            &invite.ical_uid,
            &invite.title,
            invite.starts_at,
            invite.ends_at,
            organizer,
            &attendee_email,
            partstat,
        );
        let data_base64 = B64.encode(reply_ics.as_bytes());
        let subject = match partstat {
            "declined" => format!("Declined: {}", invite.title),
            "tentative" => format!("Tentative: {}", invite.title),
            _ => format!("Accepted: {}", invite.title),
        };
        self.send_message(SendMessageRequest {
            account_id,
            to: vec![AddressDto {
                name: None,
                email: organizer.clone(),
            }],
            cc: vec![],
            bcc: vec![],
            subject,
            body_text: format!(
                "NovaMail RSVP ({partstat}) for \"{}\".\n\n",
                invite.title
            ),
            body_html: None,
            in_reply_to: None,
            references: vec![],
            attachments: vec![novamail_ipc::OutgoingAttachment {
                filename: "invite-reply.ics".into(),
                mime: "text/calendar; method=REPLY".into(),
                data_base64,
            }],
            draft_id: None,
            pgp_sign: false,
            pgp_encrypt: false,
        })
        .await
    }
}

fn account_to_dto(account: &novamail_db::models::AccountRecord) -> AccountDto {
    AccountDto {
        id: account.id,
        name: account.name.clone(),
        label: account.label.clone(),
        email: account.email.clone(),
        provider: account.provider.clone(),
        auth_type: account.auth_type.clone(),
        imap_host: account.imap_host.clone(),
        imap_port: account.imap_port,
        imap_tls: account.imap_tls,
        smtp_host: account.smtp_host.clone(),
        smtp_port: account.smtp_port,
        smtp_tls: account.smtp_tls,
        created_at: account.created_at,
    }
}

fn html_escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn html_escape_attr(s: &str) -> String {
    html_escape_text(s).replace('"', "&quot;")
}

fn sanitize_export_stem(subject: &str) -> String {
    let trimmed = subject.trim();
    let base = if trimmed.is_empty() {
        "message"
    } else {
        trimmed
    };
    let mut out = String::with_capacity(base.len().min(80));
    for ch in base.chars().take(80) {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else if ch.is_whitespace() || ch == '.' || ch == ',' {
            if !out.ends_with('_') {
                out.push('_');
            }
        }
    }
    let stem = out.trim_matches('_').to_string();
    if stem.is_empty() {
        "message".into()
    } else {
        stem
    }
}

fn unique_export_path(dir: &std::path::Path, stem: String, ext: &str) -> std::path::PathBuf {
    let mut path = dir.join(format!("{stem}.{ext}"));
    if !path.exists() {
        return path;
    }
    for i in 2..10_000 {
        path = dir.join(format!("{stem}-{i}.{ext}"));
        if !path.exists() {
            return path;
        }
    }
    dir.join(format!(
        "{stem}-{}.{ext}",
        chrono::Utc::now().timestamp_millis()
    ))
}

fn strip_html_rough(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Minimal single-page PDF with Helvetica text (ASCII-safe body).
fn build_simple_pdf(title: &str, body: &str) -> Vec<u8> {
    fn pdf_escape(s: &str) -> String {
        s.chars()
            .map(|c| match c {
                '\\' => "\\\\".to_string(),
                '(' => "\\(".to_string(),
                ')' => "\\)".to_string(),
                c if c.is_ascii() && !c.is_control() => c.to_string(),
                c => format!("\\{:03o}", (c as u32).min(255)),
            })
            .collect()
    }

    let mut lines: Vec<String> = Vec::new();
    lines.push(title.chars().take(90).collect());
    for paragraph in body.split('\n') {
        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            if current.is_empty() {
                current = word.to_string();
            } else if current.len() + 1 + word.len() <= 90 {
                current.push(' ');
                current.push_str(word);
            } else {
                lines.push(current);
                current = word.to_string();
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
        if lines.len() >= 48 {
            break;
        }
    }
    if lines.len() > 48 {
        lines.truncate(48);
    }

    let mut content = String::from("BT\n/F1 11 Tf\n14 TL\n50 780 Td\n");
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            content.push_str("T*\n");
        }
        content.push_str(&format!("({}) Tj\n", pdf_escape(line)));
    }
    content.push_str("ET");

    let objects: Vec<String> = vec![
        "1 0 obj<< /Type /Catalog /Pages 2 0 R >>endobj\n".into(),
        "2 0 obj<< /Type /Pages /Kids [3 0 R] /Count 1 >>endobj\n".into(),
        "3 0 obj<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources<< /Font<< /F1 5 0 R >> >> >>endobj\n".into(),
        format!(
            "4 0 obj<< /Length {} >>stream\n{}\nendstream\nendobj\n",
            content.len(),
            content
        ),
        "5 0 obj<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>endobj\n".into(),
    ];

    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = vec![0usize];
    for obj in &objects {
        offsets.push(pdf.len());
        pdf.push_str(obj);
    }
    let xref_pos = pdf.len();
    pdf.push_str(&format!("xref\n0 {}\n", objects.len() + 1));
    pdf.push_str("0000000000 65535 f \n");
    for off in offsets.iter().skip(1) {
        pdf.push_str(&format!("{off:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        objects.len() + 1,
        xref_pos
    ));
    pdf.into_bytes()
}

fn resolve_snooze_wake(preset: SnoozePreset, custom: Option<i64>) -> CoreResult<i64> {
    use chrono::{Datelike, Duration, Local};
    let now = Local::now();
    let wake = match preset {
        SnoozePreset::Custom => {
            let Some(ts) = custom else {
                return Err(CoreError::Message(
                    "custom snooze requires wake_at".into(),
                ));
            };
            if ts <= now.timestamp() {
                return Err(CoreError::Message(
                    "wake_at must be in the future".into(),
                ));
            }
            return Ok(ts);
        }
        SnoozePreset::LaterToday => {
            let later = now + Duration::hours(3);
            let evening = now
                .date_naive()
                .and_hms_opt(18, 0, 0)
                .and_then(|ndt| ndt.and_local_timezone(Local).single())
                .unwrap_or(later);
            if evening > now {
                evening
            } else {
                later
            }
        }
        SnoozePreset::TomorrowMorning => {
            let tomorrow = (now + Duration::days(1)).date_naive();
            tomorrow
                .and_hms_opt(9, 0, 0)
                .and_then(|ndt| ndt.and_local_timezone(Local).single())
                .unwrap_or(now + Duration::days(1))
        }
        SnoozePreset::NextMonday => {
            let weekday = now.weekday().num_days_from_monday();
            let days_ahead = if weekday == 0 { 7 } else { 7 - weekday };
            let target = (now + Duration::days(i64::from(days_ahead))).date_naive();
            target
                .and_hms_opt(9, 0, 0)
                .and_then(|ndt| ndt.and_local_timezone(Local).single())
                .unwrap_or(now + Duration::days(i64::from(days_ahead)))
        }
    };
    Ok(wake.timestamp())
}

fn insight_text(db: &Database, message_id: Uuid, kind: &str) -> CoreResult<Option<String>> {
    Ok(db.get_ai_insight(message_id, kind)?.and_then(|raw| {
        serde_json::from_str::<serde_json::Value>(&raw)
            .ok()
            .and_then(|v| v.get("text")?.as_str().map(|s| s.to_string()))
    }))
}

fn insight_provider(db: &Database, message_id: Uuid, kind: &str) -> CoreResult<Option<String>> {
    Ok(db.get_ai_insight(message_id, kind)?.and_then(|raw| {
        serde_json::from_str::<serde_json::Value>(&raw)
            .ok()
            .and_then(|v| v.get("provider")?.as_str().map(|s| s.to_string()))
    }))
}

/// Bump when event extraction logic changes so stale cached suggestions re-run.
const EVENT_SUGGESTIONS_VERSION: i64 = 2;

fn insight_event_suggestions_current(db: &Database, message_id: Uuid) -> CoreResult<bool> {
    let Some(raw) = db.get_ai_insight(message_id, "event_suggestions")? else {
        return Ok(false);
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Ok(false);
    };
    Ok(value.get("version").and_then(|v| v.as_i64()) == Some(EVENT_SUGGESTIONS_VERSION))
}

fn insight_event_suggestions(
    db: &Database,
    message_id: Uuid,
) -> CoreResult<Vec<EventSuggestionDto>> {
    let Some(raw) = db.get_ai_insight(message_id, "event_suggestions")? else {
        return Ok(Vec::new());
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Ok(Vec::new());
    };
    // Ignore stale extractions that used quote-header timestamps.
    if value.get("version").and_then(|v| v.as_i64()) != Some(EVENT_SUGGESTIONS_VERSION) {
        return Ok(Vec::new());
    }
    let Some(arr) = value.get("suggestions").and_then(|v| v.as_array()) else {
        return Ok(Vec::new());
    };
    Ok(arr
        .iter()
        .filter_map(|item| {
            Some(EventSuggestionDto {
                label: item.get("label")?.as_str()?.to_string(),
                starts_at: item.get("startsAt")?.as_i64()?,
                ends_at: item.get("endsAt").and_then(|v| v.as_i64()),
                location: item
                    .get("location")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                confidence: item
                    .get("confidence")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.5) as f32,
            })
        })
        .collect())
}

/// Match backup contacts to local ones: LDAP DN, then shared email, then id.
fn find_matching_contact<'a>(
    existing: &'a [ContactDto],
    contact: &BackupContact,
) -> Option<&'a ContactDto> {
    if let Some(dn) = contact.ldap_dn.as_deref().filter(|s| !s.is_empty()) {
        if let Some(found) = existing
            .iter()
            .find(|c| c.ldap_dn.as_deref() == Some(dn))
        {
            return Some(found);
        }
    }
    for email in &contact.emails {
        let email = email.trim();
        if email.is_empty() {
            continue;
        }
        if let Some(found) = existing
            .iter()
            .find(|c| c.emails.iter().any(|e| e.eq_ignore_ascii_case(email)))
        {
            return Some(found);
        }
    }
    existing.iter().find(|c| c.id == contact.id)
}

/// Spawn async work from sync or async contexts without panicking.
///
/// Sync Tauri commands have no Tokio reactor; `tokio::spawn` there aborts the process
/// (`there is no reactor running`). Prefer the current handle when present, otherwise
/// run the future on a short-lived background thread runtime.
fn spawn_background<F>(fut: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.spawn(fut);
        }
        Err(_) => {
            let _ = std::thread::Builder::new()
                .name("novamail-bg".into())
                .spawn(move || match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => {
                        rt.block_on(fut);
                    }
                    Err(err) => {
                        tracing::error!(error = %err, "failed to build background runtime");
                    }
                });
        }
    }
}

#[cfg(test)]
mod share_mode_tests {
    use super::*;
    use tempfile::tempdir;

    fn test_app() -> AppState {
        let dir = tempdir().unwrap();
        let paths = AppPaths::from_data_dir(dir.path().join("data"));
        // leak tempdir so path stays valid for test duration
        std::mem::forget(dir);
        AppState::initialize(paths).unwrap()
    }

    #[tokio::test]
    async fn can_set_server_and_client_mode() {
        let app = test_app();
        let server = app
            .set_contacts_share_mode(SetContactsShareModeRequest {
                mode: ContactsShareMode::Server,
                client_url: None,
                client_bind_dn: None,
                client_password: None,
                client_base_dn: None,
            })
            .await
            .expect("server mode");
        assert_eq!(server.mode, ContactsShareMode::Server);
        println!(
            "server carddav={} ldap={} url={}",
            server.carddav.running,
            server.ldap_server.running,
            server.ldap_server.listen_url
        );
        assert!(
            server.carddav.running || server.ldap_server.running,
            "at least one service should run"
        );

        let client = app
            .set_contacts_share_mode(SetContactsShareModeRequest {
                mode: ContactsShareMode::Client,
                client_url: Some("ldap://127.0.0.1:1389".into()),
                client_bind_dn: Some("cn=novamail,dc=novamail".into()),
                client_password: Some("x".into()),
                client_base_dn: Some("ou=people,dc=novamail".into()),
            })
            .await
            .expect("client mode");
        assert_eq!(client.mode, ContactsShareMode::Client);
        assert!(!client.carddav.running);
        assert!(!client.ldap_server.running);

        let local = app
            .set_contacts_share_mode(SetContactsShareModeRequest {
                mode: ContactsShareMode::Local,
                client_url: None,
                client_bind_dn: None,
                client_password: None,
                client_base_dn: None,
            })
            .await
            .expect("local mode");
        assert_eq!(local.mode, ContactsShareMode::Local);
    }
}
