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
use uuid::Uuid;

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

    /// Start the periodic sync loop on a dedicated Tokio runtime thread.
    /// Safe to call from Tauri's sync `setup` hook (no ambient runtime required).
    ///
    /// `on_new_messages` receives local message IDs newly inserted in a cycle
    /// (for AI insights + mail rules).
    /// `on_accounts_synced` receives account IDs that completed sync (offline quota).
    /// `on_cycle_done` runs after every finished cycle (UI refresh / AI backfill).
    pub fn spawn<F, N, A, C>(
        self,
        mut on_progress: F,
        mut on_new_messages: N,
        mut on_accounts_synced: A,
        mut on_cycle_done: C,
    ) -> std::thread::JoinHandle<()>
    where
        F: FnMut(SyncProgressEvent) + Send + 'static,
        N: FnMut(Vec<Uuid>) + Send + 'static,
        A: FnMut(Vec<Uuid>) + Send + 'static,
        C: FnMut() + Send + 'static,
    {
        std::thread::Builder::new()
            .name("novamail-sync-scheduler".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(err) => {
                        warn!(error = %err, "failed to build scheduler runtime");
                        return;
                    }
                };
                runtime.block_on(async move {
                    // Immediate first sync, then wait between cycles.
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
                                        on_cycle_done();
                                        tokio::time::sleep(self.interval).await;
                                        continue;
                                    }
                                };
                                let mut new_ids = Vec::new();
                                let mut synced_accounts = Vec::new();
                                for account in accounts {
                                    info!(account = %account.email, "scheduled sync starting");
                                    match engine
                                        .sync_account(account.id, |event| on_progress(event))
                                        .await
                                    {
                                        Ok(report) => {
                                            new_ids.extend(report.new_message_ids);
                                            synced_accounts.push(account.id);
                                        }
                                        Err(err) => {
                                            warn!(
                                                account = %account.email,
                                                error = %err,
                                                "scheduled sync failed"
                                            );
                                        }
                                    }
                                }
                                if !new_ids.is_empty() {
                                    on_new_messages(new_ids);
                                }
                                if !synced_accounts.is_empty() {
                                    on_accounts_synced(synced_accounts);
                                }
                                *self.running.lock().await = false;
                                on_cycle_done();
                            }
                        }
                        tokio::time::sleep(self.interval).await;
                    }
                });
            })
            .expect("failed to spawn sync scheduler thread")
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
