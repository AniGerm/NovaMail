mod commands;
mod state;

use std::time::Duration;

use novamail_mail::SyncScheduler;
use state::DesktopState;
use tauri::{Emitter, Manager};
use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("novamail=info".parse().unwrap()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let state = DesktopState::new()?;
            let scheduler = SyncScheduler::new(
                state.app.db().clone(),
                state.app.secrets().clone(),
                state.app.paths.blobs_dir.clone(),
                Duration::from_secs(300),
            );
            let handle = app.handle().clone();
            let handle_offline = app.handle().clone();
            let app_for_ai = state.app.clone();
            let app_for_offline = state.app.clone();
            let app_for_jobs = state.app.clone();
            let handle_jobs = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Immediate catch-up after restart, then every 60s.
                loop {
                    match app_for_jobs.run_jobs_tick().await {
                        Ok(report) => {
                            if report.woke_snoozes > 0
                                || report.sent_later > 0
                                || report.failed_later > 0
                            {
                                let _ = handle_jobs.emit("jobs://tick", &report);
                            }
                        }
                        Err(err) => {
                            tracing::warn!(error = %err, "jobs tick failed");
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(60)).await;
                }
            });
            let _sync_task = scheduler.spawn(
                move |event| {
                    let _ = handle.emit("sync://progress", &event);
                },
                move |new_ids| {
                    app_for_ai.on_new_messages_synced(&new_ids);
                },
                move |account_ids| {
                    let app = app_for_offline.clone();
                    let emit = handle_offline.clone();
                    tauri::async_runtime::spawn(async move {
                        for account_id in account_ids {
                            match app.process_offline_after_sync(account_id).await {
                                Ok(Some(prompt)) => {
                                    let _ = emit.emit("offline://prompt", &prompt);
                                }
                                Ok(None) => {}
                                Err(err) => {
                                    tracing::warn!(
                                        %account_id,
                                        error = %err,
                                        "scheduled offline mailbox failed"
                                    );
                                }
                            }
                        }
                    });
                },
            );
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::provider_presets,
            commands::accounts_list,
            commands::accounts_add_password,
            commands::accounts_add_oauth,
            commands::accounts_remove,
            commands::mailboxes_list,
            commands::messages_list,
            commands::threads_list,
            commands::messages_list_by_thread,
            commands::messages_get,
            commands::messages_set_flags,
            commands::messages_search,
            commands::messages_send,
            commands::messages_save_draft,
            commands::mail_sync,
            commands::oauth_authorize_url,
            commands::oauth_exchange_code,
            commands::oauth_wait_callback,
            commands::ai_summarize_message,
            commands::ai_suggest_reply,
            commands::ai_suggest_replies,
            commands::ai_message_insights,
            commands::ai_get_settings,
            commands::ai_set_settings,
            commands::ai_runtime_status,
            commands::ai_pull_model,
            commands::ai_install_ollama,
            commands::ai_start_ollama,
            commands::messages_archive,
            commands::messages_delete,
            commands::messages_forward_draft,
            commands::attachments_list,
            commands::attachments_open_path,
            commands::contacts_list,
            commands::recipients_suggest,
            commands::spellcheck_status,
            commands::spellcheck_install,
            commands::spellcheck_ensure_for_locale,
            commands::contacts_upsert,
            commands::contacts_delete,
            commands::carddav_start,
            commands::carddav_stop,
            commands::carddav_status,
            commands::contacts_share_status,
            commands::contacts_set_share_mode,
            commands::contacts_client_sync,
            commands::ldap_search,
            commands::ldap_get_settings,
            commands::ldap_sync,
            commands::contacts_book_settings,
            commands::contacts_set_book_settings,
            commands::labels_list,
            commands::labels_upsert,
            commands::labels_delete,
            commands::messages_set_labels,
            commands::messages_list_labels,
            commands::rules_list,
            commands::rules_upsert,
            commands::rules_delete,
            commands::messages_move,
            commands::messages_mark_spam,
            commands::messages_mark_not_spam,
            commands::spam_get_settings,
            commands::spam_set_settings,
            commands::spam_score_message,
            commands::folder_policies_get,
            commands::folder_policies_set,
            commands::folder_policies_apply,
            commands::offline_mailbox_get,
            commands::offline_mailbox_set,
            commands::offline_mailbox_set_policy,
            commands::offline_mailbox_quota,
            commands::offline_mailbox_local_count,
            commands::offline_mailbox_run,
            commands::offline_mailbox_dismiss_prompt,
            commands::offline_mailbox_enable_from_prompt,
            commands::messages_snooze,
            commands::messages_unsnooze,
            commands::messages_list_snoozed,
            commands::messages_send_later,
            commands::outbound_list,
            commands::outbound_cancel,
            commands::planned_summary,
            commands::jobs_tick,
            commands::signatures_list,
            commands::signatures_upsert,
            commands::signatures_delete,
            commands::pop3_test,
            commands::backup_export,
            commands::backup_import,
            commands::pgp_list_keys,
            commands::pgp_generate,
            commands::pgp_import,
            commands::pgp_delete,
            commands::pgp_export_public,
            commands::pgp_decrypt_text,
            commands::pgp_verify_text,
            commands::pgp_inspect_message,
            commands::calendar_accounts_list,
            commands::calendar_accounts_upsert,
            commands::calendar_accounts_delete,
            commands::calendar_accounts_sync,
            commands::calendar_events_list,
            commands::calendar_events_upsert,
            commands::calendar_events_delete,
            commands::calendar_tasks_list,
            commands::calendar_tasks_upsert,
            commands::calendar_tasks_delete,
            commands::calendar_event_from_message,
            commands::calendar_task_from_message,
        ])
        .run(tauri::generate_context!())
        .expect("error while running NovaMail");
}
