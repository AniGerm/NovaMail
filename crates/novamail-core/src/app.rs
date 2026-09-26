use novamail_crypto::{AccountCredentials, OAuthTokens, SecretStore};
use novamail_db::{AccountRecord, Database};
use novamail_ipc::{
    AccountDto, AddAccountOAuthRequest, AddAccountPasswordRequest, ListMessagesRequest,
    ListMessagesResponse, MailboxDto, MessageDetailDto, ProviderPreset, SearchRequest,
    SearchResponse, SendMessageRequest, SetFlagsRequest, SyncProgressEvent, SyncRequest,
    SyncResult,
};
use novamail_mail::{SmtpClient, SyncEngine};
use novamail_search::SearchService;
use uuid::Uuid;

use crate::paths::AppPaths;
use crate::{CoreError, CoreResult};

pub struct AppState {
    pub paths: AppPaths,
    pub db: Database,
    pub secrets: SecretStore,
}

impl AppState {
    pub fn initialize(paths: AppPaths) -> CoreResult<Self> {
        paths.ensure().map_err(|e| CoreError::Message(e.to_string()))?;
        let db = Database::open(&paths.db_path)?;
        // Memory fallback keeps headless CI / missing Secret Service usable.
        let secrets = SecretStore::with_memory_fallback(true);
        Ok(Self {
            paths,
            db,
            secrets,
        })
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

    pub fn get_message(&self, message_id: Uuid) -> CoreResult<MessageDetailDto> {
        Ok(self.db.get_message(message_id)?)
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
        let engine = SyncEngine::new(self.db.clone(), self.secrets.clone());
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
        let config = novamail_mail::OAuthConfig::for_provider(&provider)?;
        let state = novamail_mail::OAuthFlow::new_state();
        Ok(config.authorize_url_with_state(&state)?)
    }
}
