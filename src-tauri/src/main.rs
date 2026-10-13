#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod launcher;
mod util;

use config::AppState;
use tauri::Manager;

fn main() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let state = AppState::new();
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            launcher::get_config,
            launcher::get_saved_data,
            launcher::get_init_data,
            launcher::auto_detect_paths,
            launcher::set_game_paths,
            launcher::select_game_path,
            launcher::start_game,
            launcher::update_nickname,
            launcher::update_server,
            launcher::update_launch_params,
            launcher::read_launch_params,
            launcher::get_launch_method,
            launcher::set_launch_method,
            launcher::get_servers,
            launcher::get_servers_and_news,
            launcher::get_launcher_news,
            launcher::get_read_news_ids,
            launcher::save_read_news_ids,
            launcher::get_launcher_version,
            launcher::check_launcher_update,
            launcher::get_install_action,
            launcher::get_install_action_api,
            launcher::check_patches_available_api,
            launcher::fetch_image_as_base64,
            launcher::get_debug_info,
            launcher::minimize_window,
            launcher::minimize_to_tray,
            launcher::restore_window,
            launcher::open_game_folder,
            launcher::open_data_folder,
            launcher::open_log_file,
            launcher::is_gta_running,
            launcher::get_wallpapers,
            launcher::get_wallpaper_thumb,
            launcher::get_wallpaper_full,
            launcher::select_bg_image,
            launcher::get_launcher_settings,
            launcher::save_launcher_settings,
            launcher::get_window_title,
            launcher::set_window_title,
            launcher::get_window_size,
            launcher::save_layout_order,
            launcher::get_layout_order,
            launcher::get_profiles,
            launcher::save_profiles,
            launcher::check_patches_status,
            launcher::check_patches_files,
            launcher::read_patches,
            launcher::write_patches,
            launcher::list_patches_backups,
            launcher::restore_patches_backup,
            launcher::delete_patches_backup,
            launcher::preview_patches_backup,
            launcher::download_and_install_patches,
            launcher::get_patches_latest_version,
            launcher::check_patches_available,
            launcher::check_patches_update_status,
            launcher::fetch_patch_presets,
            launcher::install_patch_preset,
            launcher::fetch_patches_schema,
            launcher::export_patches,
            launcher::import_patches,
            launcher::install_patches_file,
            launcher::check_update_api,
            launcher::download_update_api,
            launcher::start_launcher_install,
            launcher::get_moonloader_scripts,
            launcher::toggle_moonloader_script,
            launcher::delete_moonloader_script,
            launcher::open_moonloader_folder,
            launcher::fetch_moonloader_catalog,
            launcher::install_moonloader_script,
            launcher::install_lua_file,
            launcher::fetch_libraries_catalog,
            launcher::list_installed_libraries,
            launcher::install_library,
            launcher::uninstall_library,
            launcher::fetch_others_catalog,
            launcher::install_other_file,
            launcher::remove_other_file,
            launcher::check_other_installed,
            launcher::open_telegram_share,
            launcher::open_url,
            launcher::check_tray_support,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
