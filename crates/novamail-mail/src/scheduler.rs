//! Background sync scheduler.
//!
//! Runs periodic IMAP sync for all accounts without blocking the UI thread.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use novamail_crypto::SecretStore;
use novamail_db::Database;
use novamail_ipc::{SyncProgressEvent, SyncRequest};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::sync::SyncEngine;

#[derive(Clone)]
pub struct SyncScheduler {
    db: Database,
    secrets: SecretStore,
    blobs_dir: PathBuf,
    interval: Duration,
    running: Arc<Mutex<bool>>,
}

impl SyncScheduler {
    pub fn new(
        db: Database,
        secrets: SecretStore,
        blobs_dir: PathBuf,
        interval: Duration,
    ) -> Self {
        Self {
            db,
            secrets,
            blobs_dir,
            interval,
            running: Arc::new(Mutex::new(false)),
        }
    }

    pub fn spawn<F>(self, mut on_progress: F) -> tokio::task::JoinHandle<()>
    where
        F: FnMut(SyncProgressEvent) + Send + 'static,
    {
        tokio::spawn(async move {
            loop {
                {
                    let mut guard = self.running.lock().await;
                    if *guard {
                        // previous cycle still running — skip overlap
                    } else {
                        *guard = true;
                        drop(guard);
                        let engine = SyncEngine::new(
                            self.db.clone(),
                            self.secrets.clone(),
                            &self.blobs_dir,
                        );
                        let accounts = match self.db.list_accounts() {
                            Ok(list) => list,
                            Err(err) => {
                                warn!(error = %err, "scheduler failed to list accounts");
                                *self.running.lock().await = false;
                                tokio::time::sleep(self.interval).await;
                                continue;
                            }
                        };
                        for account in accounts {
                            info!(account = %account.email, "scheduled sync starting");
                            if let Err(err) = engine
                                .sync_account(account.id, |event| on_progress(event))
                                .await
                            {
                                warn!(account = %account.email, error = %err, "scheduled sync failed");
                            }
                        }
                        *self.running.lock().await = false;
                    }
                }
                tokio::time::sleep(self.interval).await;
            }
        })
    }

    pub async fn run_once<F>(&self, request: SyncRequest, on_progress: F) -> crate::MailResult<u32>
    where
        F: FnMut(SyncProgressEvent) + Send,
    {
        let engine = SyncEngine::new(self.db.clone(), self.secrets.clone(), &self.blobs_dir);
        let account_ids = if let Some(id) = request.account_id {
            vec![id]
        } else {
            self.db
                .list_accounts()?
                .into_iter()
                .map(|a| a.id)
                .collect()
        };
        let mut total = 0u32;
        let mut on_progress = on_progress;
        for account_id in account_ids {
            let report = engine.sync_account(account_id, |e| on_progress(e)).await?;
            total += report.messages_fetched;
        }
        Ok(total)
    }
}
