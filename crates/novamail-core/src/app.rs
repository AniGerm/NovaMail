use std::sync::Arc;

use novamail_ai::{
    allow_model_pick, default_ollama_model, default_ollama_url, detect_nvidia_gpu,
    ensure_ollama_running, install_ollama_system, install_ollama_user, list_ollama_models,
    normalize_model_ref, probe_ollama, pull_ollama_model, recommended_model, AiProvider,
    NullAiProvider, OllamaProvider, PrioritizeRequest, SuggestReplyRequest, SummarizeRequest,
    DEFAULT_MODEL,
};
use parking_lot::RwLock;
use novamail_contacts::{search_ldap, CardDavServer};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use novamail_crypto::{
    decrypt_backup_payload, encrypt_backup_payload, AccountCredentials, EncryptedBackupFile,
    OAuthTokens, SecretStore,
};
use novamail_db::{AccountRecord, ContactRecord, Database, LabelRecord, RuleRecord, SignatureRecord};
use novamail_ipc::{
    AccountDto, AddAccountOAuthRequest, AddAccountPasswordRequest, AiInstallOllamaRequest,
    AiInstallOllamaResponse, AiInstallProgressEvent, AiPullModelRequest, AiPullModelResponse,
    AiPullProgressEvent, AiRuntimeStatus, AiSettings, AttachmentDto, BackupAccount,
    BackupAccountCredentials, BackupContact, BackupPayload,
    CardDavServerStatus, ContactDto, ContactsBookSettings, ExportBackupRequest,
    ExportBackupResponse, ImportBackupRequest, ImportBackupResult, LabelDto, LdapSearchRequest,
    LdapSyncRequest, LdapSyncResult, LdapSyncSettings, ListMessagesRequest, ListMessagesResponse,
    ListThreadsResponse, MailboxDto, MessageAiInsights, MessageDetailDto, MessageSummaryDto,
    OAuthExchangeRequest, OAuthExchangeResponse, OAuthTokensDto, ProviderPreset, RuleDto,
    SearchRequest, SearchResponse, SendMessageRequest, SetFlagsRequest, SetMessageLabelsRequest,
    SignatureDto, SuggestRepliesMessageRequest, SuggestRepliesMessageResponse,
    SuggestReplyMessageRequest, SuggestReplyMessageResponse, SummarizeMessageRequest,
    SummarizeMessageResponse, SyncProgressEvent, SyncRequest, SyncResult, UpsertContactRequest,
    UpsertLabelRequest, UpsertRuleRequest, UpsertSignatureRequest,
};
use novamail_mail::{
    archive_remote, delete_remote, set_flags_remote, OAuthConfig, Pop3Client, SmtpClient,
    SyncEngine,
};
use novamail_rules::{evaluate_rules, Action, RuleDefinition, RuleMatchContext};
use novamail_search::SearchService;
use uuid::Uuid;

use crate::contacts_store::DbContactStore;
use crate::paths::AppPaths;
use crate::sanitize::sanitize_html;
use crate::{CoreError, CoreResult};

pub struct AppState {
    pub paths: AppPaths,
    pub db: Database,
    pub secrets: SecretStore,
    ai: RwLock<Arc<dyn AiProvider>>,
    carddav: CardDavServer,
}

impl AppState {
    pub fn initialize(paths: AppPaths) -> CoreResult<Self> {
        paths.ensure().map_err(|e| CoreError::Message(e.to_string()))?;
        let db = Database::open(&paths.db_path)?;
        // Memory fallback keeps headless CI / missing Secret Service usable.
        let secrets = SecretStore::with_memory_fallback(true);
        let store = DbContactStore::new(db.clone());
        let carddav = CardDavServer::new(store);
        let settings = Self::load_ai_settings(&db);
        Ok(Self {
            paths,
            db,
            secrets,
            ai: RwLock::new(Self::provider_from_settings(&settings)),
            carddav,
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
        let raw =
            serde_json::to_string(&settings).map_err(|e| CoreError::Message(e.to_string()))?;
        self.db.set_setting("ai.settings", &raw)?;
        *self.ai.write() = Self::provider_from_settings(&settings);
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
        ProviderPreset::all()
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
        self.secrets.store_credentials(id, &credentials)?;
        self.db.insert_account(&account)?;

        Ok(AccountDto {
            id: account.id,
            name: account.name,
            email: account.email,
            provider: account.provider,
            auth_type: account.auth_type,
            imap_host: account.imap_host,
            imap_port: account.imap_port,
            imap_tls: account.imap_tls,
            smtp_host: account.smtp_host,
            smtp_port: account.smtp_port,
            smtp_tls: account.smtp_tls,
            created_at: account.created_at,
        })
    }

    pub async fn add_account_oauth(
        &self,
        request: AddAccountOAuthRequest,
    ) -> CoreResult<AccountDto> {
        let id = Uuid::new_v4();
        let account = AccountRecord {
            id,
            name: request.name,
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
        self.secrets.store_credentials(id, &credentials)?;
        self.db.insert_account(&account)?;

        Ok(AccountDto {
            id: account.id,
            name: account.name,
            email: account.email,
            provider: account.provider,
            auth_type: account.auth_type,
            imap_host: account.imap_host,
            imap_port: account.imap_port,
            imap_tls: account.imap_tls,
            smtp_host: account.smtp_host,
            smtp_port: account.smtp_port,
            smtp_tls: account.smtp_tls,
            created_at: account.created_at,
        })
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

    pub fn list_threads(&self, request: ListMessagesRequest) -> CoreResult<ListThreadsResponse> {
        Ok(self.db.list_threads(&request)?)
    }

    pub fn list_messages_by_thread(&self, thread_id: Uuid) -> CoreResult<Vec<MessageSummaryDto>> {
        Ok(self.db.list_messages_by_thread(thread_id)?)
    }

    pub fn get_message(&self, message_id: Uuid) -> CoreResult<MessageDetailDto> {
        let mut detail = self.db.get_message(message_id)?;
        if let Some(html) = detail.body_html.take() {
            let cleaned = sanitize_html(&html);
            detail.body_html = if cleaned.trim().is_empty() {
                None
            } else {
                Some(cleaned)
            };
        }
        Ok(detail)
    }

    pub async fn set_flags(&self, request: SetFlagsRequest) -> CoreResult<()> {
        self.db
            .set_flags(request.message_id, request.unread, request.starred)?;
        if let Err(err) = set_flags_remote(
            &self.db,
            &self.secrets,
            request.message_id,
            request.unread,
            request.starred,
        )
        .await
        {
            tracing::warn!(error = %err, "IMAP flag sync failed; local flags kept");
        }
        Ok(())
    }

    pub fn search(&self, request: SearchRequest) -> CoreResult<SearchResponse> {
        let service = SearchService::new(self.db.clone());
        Ok(service.search(request)?)
    }

    pub async fn sync<F>(
        &self,
        request: SyncRequest,
        mut on_progress: F,
    ) -> CoreResult<Vec<SyncResult>>
    where
        F: FnMut(SyncProgressEvent) + Send,
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
        Ok(results)
    }

    /// Apply mail rules + enqueue background AI insights for newly synced messages.
    pub fn on_new_messages_synced(&self, message_ids: &[Uuid]) {
        for message_id in message_ids {
            if let Err(err) = self.apply_rules_for_message(*message_id) {
                tracing::debug!(%message_id, error = %err, "rule apply skipped");
            }
        }
        self.enqueue_ai_insights(message_ids);
    }

    pub async fn send_message(&self, request: SendMessageRequest) -> CoreResult<()> {
        let account = self.db.get_account(request.account_id)?;
        SmtpClient::send_with_secrets(&account, &self.secrets, &request).await?;
        Ok(())
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
        })
    }

    pub async fn delete_message(&self, message_id: Uuid) -> CoreResult<()> {
        if let Err(err) = delete_remote(&self.db, &self.secrets, message_id).await {
            tracing::warn!(error = %err, "IMAP delete failed; removing local copy anyway");
        }
        let detail = self.get_message(message_id)?;
        for attachment in detail.attachments {
            let _ = std::fs::remove_file(attachment.path);
        }
        self.db.delete_message(message_id)?;
        Ok(())
    }

    pub fn open_attachment_path(&self, attachment_id: Uuid) -> CoreResult<String> {
        let attachment = self.db.get_attachment(attachment_id)?;
        Ok(attachment.path)
    }

    pub fn list_contacts(&self, query: Option<String>) -> CoreResult<Vec<ContactDto>> {
        Ok(self.db.list_contacts(query.as_deref())?)
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
        self.ensure_carddav_credentials()?;
        Ok(self.carddav.start().await?)
    }

    pub fn stop_carddav(&self) -> CoreResult<CardDavServerStatus> {
        Ok(self.carddav.stop())
    }

    pub fn carddav_status(&self) -> CoreResult<CardDavServerStatus> {
        self.ensure_carddav_credentials()?;
        Ok(self.carddav.status())
    }

    fn ensure_carddav_credentials(&self) -> CoreResult<()> {
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
        self.carddav.set_credentials(username, password);
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
        let reply_a = insight_text(&self.db, message_id, "reply_a")?
            .or(insight_text(&self.db, message_id, "reply")?);
        let reply_b = insight_text(&self.db, message_id, "reply_b")?;
        let provider = insight_provider(&self.db, message_id, "summary")?
            .or(insight_provider(&self.db, message_id, "reply_a")?)
            .or(insight_provider(&self.db, message_id, "reply")?);
        Ok(MessageAiInsights {
            message_id,
            summary,
            reply_suggestion: reply_a.clone(),
            reply_a,
            reply_b,
            provider,
        })
    }

    pub fn enqueue_ai_insights(&self, message_ids: &[Uuid]) {
        if message_ids.is_empty() {
            return;
        }
        let settings = Self::load_ai_settings(&self.db);
        if !settings.enabled || !settings.onboarding_completed {
            return;
        }
        // Cap backlog so a large first sync does not saturate the CPU for hours.
        let ids: Vec<Uuid> = message_ids.iter().copied().take(25).collect();
        let db = self.db.clone();
        let ai = self.ai_provider();
        tokio::spawn(async move {
            static AI_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
            let _guard = AI_LOCK.lock().await;
            for message_id in ids {
                if let Err(err) = Self::generate_and_store_insights(&db, ai.as_ref(), message_id).await
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
        let has_replies = db.has_ai_insight(message_id, "reply_a")?
            && db.has_ai_insight(message_id, "reply_b")?;
        if db.has_ai_insight(message_id, "summary")? && has_replies {
            return Ok(());
        }
        let detail = db.get_message(message_id)?;
        let body = detail
            .body_text
            .clone()
            .unwrap_or_else(|| detail.summary.snippet.clone());

        if !db.has_ai_insight(message_id, "summary")? {
            let result = match ai
                .summarize(SummarizeRequest {
                    subject: detail.summary.subject.clone(),
                    body_text: body.clone(),
                })
                .await
            {
                Ok(r) => r,
                Err(_) => {
                    Self::offline_ai()
                        .summarize(SummarizeRequest {
                            subject: detail.summary.subject.clone(),
                            body_text: body.clone(),
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

        if !has_replies {
            let ai_request = SuggestReplyRequest {
                subject: detail.summary.subject.clone(),
                body_text: body,
                from_email: detail.summary.from.email.clone(),
                facts: None,
                style: None,
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
                db.upsert_ai_insight(message_id, "reply_a", &payload.to_string())?;
                db.upsert_ai_insight(message_id, "reply", &payload.to_string())?;
            }
            if let Some(b) = result.variants.get(1) {
                let payload = serde_json::json!({
                    "text": b,
                    "provider": result.provider,
                });
                db.upsert_ai_insight(message_id, "reply_b", &payload.to_string())?;
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

        // Use cache only when no custom facts were requested.
        if facts.is_none() {
            let a = insight_text(&self.db, request.message_id, "reply_a")?
                .or(insight_text(&self.db, request.message_id, "reply")?);
            let b = insight_text(&self.db, request.message_id, "reply_b")?;
            if let (Some(a), Some(b)) = (a, b) {
                let provider = insight_provider(&self.db, request.message_id, "reply_a")?
                    .unwrap_or_else(|| "cache".into());
                return Ok(SuggestRepliesMessageResponse {
                    message_id: request.message_id,
                    variants: vec![a, b],
                    provider,
                });
            }
        }

        let detail = self.get_message(request.message_id)?;
        let body = detail
            .body_text
            .clone()
            .unwrap_or_else(|| detail.summary.snippet.clone());
        let ai_request = SuggestReplyRequest {
            subject: detail.summary.subject.clone(),
            body_text: body,
            from_email: detail.summary.from.email.clone(),
            facts: facts.clone(),
            style: None,
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
                    .upsert_ai_insight(request.message_id, "reply_a", &payload.to_string());
                let _ = self
                    .db
                    .upsert_ai_insight(request.message_id, "reply", &payload.to_string());
            }
            if let Some(b) = result.variants.get(1) {
                let payload = serde_json::json!({
                    "text": b,
                    "provider": result.provider,
                });
                let _ = self
                    .db
                    .upsert_ai_insight(request.message_id, "reply_b", &payload.to_string());
            }
        }

        Ok(SuggestRepliesMessageResponse {
            message_id: request.message_id,
            variants: result.variants,
            provider: result.provider,
        })
    }

    /// Applies enabled rules from SQLite against a message (first version).
    pub fn apply_rules_for_message(&self, message_id: Uuid) -> CoreResult<Vec<Action>> {
        let detail = self.get_message(message_id)?;
        let conn_rules = self.load_rule_definitions()?;
        let ctx = RuleMatchContext {
            message: &detail.summary,
            body_text: detail.body_text.as_deref().unwrap_or(""),
        };
        let matched = evaluate_rules(&conn_rules, &ctx);
        let mut actions = Vec::new();
        for (_id, acts) in matched {
            for action in acts {
                match &action {
                    Action::MarkRead => {
                        self.db.set_flags(message_id, Some(false), None)?;
                    }
                    Action::Star => {
                        self.db.set_flags(message_id, None, Some(true))?;
                    }
                    Action::AddLabel { .. } | Action::MoveToMailbox { .. } => {
                        // Labels/mailbox moves land with P1 UI; actions are recorded.
                    }
                }
                actions.push(action);
            }
        }
        Ok(actions)
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
