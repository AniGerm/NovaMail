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
                Duration::from_secs(300),
            );
            let handle = app.handle().clone();
            let _sync_task = scheduler.spawn(move |event| {
                let _ = handle.emit("sync://progress", &event);
            });
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
            commands::messages_archive,
            commands::messages_forward_draft,
        ])
        .run(tauri::generate_context!())
        .expect("error while running NovaMail");
}
