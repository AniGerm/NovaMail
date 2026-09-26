use std::sync::Arc;

use novamail_ai::{
    AiProvider, NullAiProvider, OllamaProvider, PrioritizeRequest, SuggestReplyRequest,
    SummarizeRequest,
};
use novamail_contacts::{search_ldap, CardDavServer};
use novamail_crypto::{AccountCredentials, OAuthTokens, SecretStore};
use novamail_db::{AccountRecord, Database};
use novamail_ipc::{
    AccountDto, AddAccountOAuthRequest, AddAccountPasswordRequest, AttachmentDto,
    CardDavServerStatus, ContactDto, LabelDto, LdapSearchRequest, ListMessagesRequest,
    ListMessagesResponse, ListThreadsResponse, MailboxDto, MessageDetailDto, MessageSummaryDto,
    OAuthExchangeRequest, OAuthExchangeResponse, OAuthTokensDto, ProviderPreset, RuleDto,
    SearchRequest, SearchResponse, SendMessageRequest, SetFlagsRequest, SetMessageLabelsRequest,
    SignatureDto, SuggestReplyMessageRequest, SuggestReplyMessageResponse,
    SummarizeMessageRequest, SummarizeMessageResponse, SyncProgressEvent, SyncRequest, SyncResult,
    UpsertContactRequest, UpsertLabelRequest, UpsertRuleRequest, UpsertSignatureRequest,
};
use novamail_mail::{OAuthConfig, Pop3Client, SmtpClient, SyncEngine};
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
    ai: Arc<dyn AiProvider>,
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
        Ok(Self {
            paths,
            db,
            secrets,
            ai: Self::select_ai_provider(),
            carddav,
        })
    }

    fn select_ai_provider() -> Arc<dyn AiProvider> {
        // Primary provider talks to local Ollama. Per-request helpers fall back
        // to NullAiProvider when the daemon is unreachable.
        Arc::new(OllamaProvider::default())
    }

    fn offline_ai() -> NullAiProvider {
        NullAiProvider
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

    pub fn set_flags(&self, request: SetFlagsRequest) -> CoreResult<()> {
        self.db
            .set_flags(request.message_id, request.unread, request.starred)?;
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
        for account_id in account_ids {
            let report = engine
                .sync_account(account_id, |event| on_progress(event))
                .await?;
            results.push(SyncResult {
                account_id: report.account_id,
                mailboxes_synced: report.mailboxes_synced,
                messages_fetched: report.messages_fetched,
            });
        }
        Ok(results)
    }

    pub async fn send_message(&self, request: SendMessageRequest) -> CoreResult<()> {
        let account = self.db.get_account(request.account_id)?;
        let credentials = self.secrets.load_credentials(request.account_id)?;
        SmtpClient::send(&account, &credentials, &request).await?;
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

    pub fn archive_message(&self, message_id: Uuid) -> CoreResult<()> {
        self.db.archive_message(message_id)?;
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

    pub fn delete_message(&self, message_id: Uuid) -> CoreResult<()> {
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
            emails: request.emails,
            phones: request.phones,
            notes: request.notes,
            updated_at: chrono::Utc::now().timestamp(),
        };
        self.db.upsert_contact(&record)?;
        Ok(self.db.get_contact(id)?)
    }

    pub fn delete_contact(&self, contact_id: Uuid) -> CoreResult<()> {
        self.db.delete_contact(contact_id)?;
        Ok(())
    }

    pub async fn start_carddav(&self) -> CoreResult<CardDavServerStatus> {
        Ok(self.carddav.start().await?)
    }

    pub fn stop_carddav(&self) -> CoreResult<CardDavServerStatus> {
        Ok(self.carddav.stop())
    }

    pub fn carddav_status(&self) -> CardDavServerStatus {
        self.carddav.status()
    }

    pub async fn ldap_search(&self, request: LdapSearchRequest) -> CoreResult<Vec<ContactDto>> {
        Ok(search_ldap(&request).await?)
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

    pub async fn summarize_message(
        &self,
        request: SummarizeMessageRequest,
    ) -> CoreResult<SummarizeMessageResponse> {
        let detail = self.get_message(request.message_id)?;
        let body = detail
            .body_text
            .clone()
            .unwrap_or_else(|| detail.summary.snippet.clone());
        let result = match self
            .ai
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
        let detail = self.get_message(request.message_id)?;
        let body = detail
            .body_text
            .clone()
            .unwrap_or_else(|| detail.summary.snippet.clone());
        let ai_request = SuggestReplyRequest {
            subject: detail.summary.subject.clone(),
            body_text: body,
            from_email: detail.summary.from.email.clone(),
        };
        let result = match self.ai.suggest_reply(ai_request.clone()).await {
            Ok(result) => result,
            Err(err) => {
                tracing::warn!(error = %err, "primary AI provider failed; using offline fallback");
                Self::offline_ai().suggest_reply(ai_request).await?
            }
        };
        Ok(SuggestReplyMessageResponse {
            message_id: request.message_id,
            suggestion: result.suggestion,
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
        let result = match self.ai.prioritize(request.clone()).await {
            Ok(result) => result,
            Err(_) => Self::offline_ai().prioritize(request).await?,
        };
        Ok((result.score, result.rationale, result.provider))
    }
}

