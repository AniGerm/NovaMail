mod commands;
mod state;

use state::DesktopState;
use tauri::Manager;
use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("novamail=info".parse().unwrap()))
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let state = DesktopState::new()?;
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running NovaMail");
}
