mod commands;
mod profiles;
mod state;

use crate::state::AppState;
use tracing_subscriber::{EnvFilter, fmt};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,tower_http=warn")),
        )
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::lgs_status,
            commands::start_lgs,
            commands::stop_lgs,
            commands::list_games,
            commands::inspect_game_folder,
            commands::launch_game,
            commands::build_launch_url,
            commands::ca_status,
            commands::install_ca,
            commands::uninstall_ca,
            commands::prepare_session,
            commands::open_test_browser,
            commands::list_profiles,
            commands::save_profile,
            commands::delete_profile,
            commands::get_settings,
            commands::toggle_resolution,
            commands::add_custom_resolution,
            commands::delete_custom_resolution,
            commands::replace_resolutions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
