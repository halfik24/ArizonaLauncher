#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod game_update;
mod launcher;
mod util;

use config::AppState;
use tauri::Manager;

/// Логирование: в dev — консоль (RUST_LOG), в релизе — файл
/// `<Documents>/ArizonaLauncher/launcher.log` (stderr у windows_subsystem теряется).
/// При старте старый лог > 2 МБ сдвигается в `launcher.log.old`.
fn init_logging() {
    let log_path = util::log_file_path();
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if let Ok(meta) = std::fs::metadata(&log_path) {
        if meta.len() > 2 * 1024 * 1024 {
            let _ = std::fs::rename(&log_path, log_path.with_extension("log.old"));
        }
    }

    #[cfg(debug_assertions)]
    {
        let _ = log_path; // в dev пишем в консоль, файл не используется
        env_logger::init();
    }
    #[cfg(not(debug_assertions))]
    {
        use std::io::Write;
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            Ok(file) => {
                env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
                    .target(env_logger::Target::Pipe(Box::new(file)))
                    .format_timestamp_secs()
                    .init();
            }
            Err(_) => env_logger::init(), // файл недоступен — молча, как раньше
        }
    }
}

fn main() {
    init_logging();
    log::info!(
        "Arizona RP Launcher {} запуск",
        env!("CARGO_PKG_VERSION")
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let state = AppState::new();
            // Рамка окна до первого показа — без мигания системной рамки
            let frame_mode = {
                let cfg = state.config.lock().map_err(|e| e.to_string())?;
                cfg.launcher_settings
                    .window_frame
                    .clone()
                    .unwrap_or_else(|| "system".to_string())
            };
            app.manage(state);
            if let Some(window) = app.get_webview_window("main") {
                launcher::apply_window_frame(&window, &frame_mode);
                let _ = window.show();
            }

            // Трей: левый клик — показать окно, контекстное меню — выход
            if let Some(tray) = app.tray_by_id("main") {
                tray.on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(win) = tray.app_handle().get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.unminimize();
                            let _ = win.set_focus();
                        }
                    }
                });
                let quit =
                    tauri::menu::MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
                let menu = tauri::menu::Menu::with_items(app, &[&quit])?;
                tray.set_menu(Some(menu))?;
                tray.on_menu_event(|app, event| {
                    if event.id() == "quit" {
                        app.exit(0);
                    }
                });
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // Alt+F4 / системное закрытие в нашей рамке — прячем в трей
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let custom = window
                    .state::<AppState>()
                    .config
                    .lock()
                    .map(|c| c.launcher_settings.window_frame.as_deref() == Some("custom"))
                    .unwrap_or(false);
                if custom {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
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
            launcher::get_window_frame,
            launcher::set_window_frame,
            launcher::toggle_maximize,
            launcher::is_maximized,
            launcher::start_dragging,
            launcher::start_resize,
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
            launcher::repair_game_api,
            launcher::cancel_game_update,
            launcher::game_update_status,
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
