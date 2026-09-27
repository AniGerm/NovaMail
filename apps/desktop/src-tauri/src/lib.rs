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
            let app_for_ai = state.app.clone();
            let _sync_task = scheduler.spawn(
                move |event| {
                    let _ = handle.emit("sync://progress", &event);
                },
                move |new_ids| {
                    app_for_ai.on_new_messages_synced(&new_ids);
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
            commands::mail_sync,
            commands::oauth_authorize_url,
            commands::oauth_exchange_code,
            commands::oauth_wait_callback,
            commands::ai_summarize_message,
            commands::ai_suggest_reply,
            commands::ai_suggest_replies,
            commands::ai_message_insights,
            commands::messages_archive,
            commands::messages_delete,
            commands::messages_forward_draft,
            commands::attachments_list,
            commands::attachments_open_path,
            commands::contacts_list,
            commands::contacts_upsert,
            commands::contacts_delete,
            commands::carddav_start,
            commands::carddav_stop,
            commands::carddav_status,
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
            commands::signatures_list,
            commands::signatures_upsert,
            commands::signatures_delete,
            commands::pop3_test,
            commands::backup_export,
            commands::backup_import,
        ])
        .run(tauri::generate_context!())
        .expect("error while running NovaMail");
}
