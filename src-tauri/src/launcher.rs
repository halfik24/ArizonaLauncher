use crate::config::{AppState, LauncherConfig};
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::{Manager, State};

// ==================== Config Commands ====================

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    let paths_ok = !cfg.game_path.is_empty()
        && !cfg.launcher_path.is_empty()
        && std::path::Path::new(&cfg.game_path).exists()
        && std::path::Path::new(&cfg.launcher_path).exists();
    Ok(serde_json::json!({
        "launcher_path": cfg.launcher_path,
        "game_path": cfg.game_path,
        "last_nickname": cfg.last_nickname,
        "last_server": cfg.last_server,
        "launch_params": cfg.launch_params,
        "paths_configured": paths_ok,
        "launcher_version": "v8"
    }))
}

#[tauri::command]
pub fn get_saved_data(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    let paths_ok = !cfg.game_path.is_empty() && !cfg.launcher_path.is_empty();
    Ok(serde_json::json!({
        "success": true,
        "nickname": cfg.last_nickname,
        "server": cfg.last_server,
        "game_path": cfg.game_path,
        "paths_configured": paths_ok
    }))
}

#[tauri::command]
pub fn get_init_data(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    let paths_ok = !cfg.game_path.is_empty() && !cfg.launcher_path.is_empty();
    Ok(serde_json::json!({
        "success": true,
        "saved": {
            "success": true,
            "nickname": cfg.last_nickname,
            "server": cfg.last_server,
            "game_path": cfg.game_path,
            "paths_configured": paths_ok
        }
    }))
}

#[tauri::command]
pub fn update_nickname(state: State<'_, AppState>, nickname: String) -> Result<serde_json::Value, String> {
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.last_nickname = nickname;
    }
    state.save_config()?;
    Ok(serde_json::json!({"success": true}))
}

#[tauri::command]
pub fn update_server(state: State<'_, AppState>, server_number: u32) -> Result<serde_json::Value, String> {
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.last_server = server_number;
    }
    state.save_config()?;
    Ok(serde_json::json!({"success": true}))
}

#[tauri::command]
pub fn update_launch_params(state: State<'_, AppState>, params: serde_json::Value) -> Result<serde_json::Value, String> {
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.launch_params = serde_json::from_value(params).map_err(|e| e.to_string())?;
    }
    state.save_config()?;
    Ok(serde_json::json!({"success": true}))
}

#[tauri::command]
pub fn read_launch_params(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(&cfg.launch_params).map_err(|e| e.to_string())?)
}

// ==================== Game Path Commands ====================

fn auto_detect_game_paths_inner(cfg: &mut LauncherConfig) -> bool {
    let home = dirs::home_dir().unwrap_or_default();
    let game_subpath = std::path::PathBuf::from("bin").join("arizona");

    let base_candidates: Vec<PathBuf> = vec![
        home.join("AppData").join("Local").join("Programs").join("Arizona Games Launcher"),
        PathBuf::from("C:/Program Files/Arizona Games Launcher"),
        PathBuf::from("C:/Program Files (x86)/Arizona Games Launcher"),
    ];

    for base in &base_candidates {
        let game_dir = base.join(&game_subpath);
        let gta = game_dir.join("gta_sa.exe");
        if gta.exists() {
            cfg.game_path = gta.to_string_lossy().to_string();
            let v8 = game_dir.join("ArizonaLauncher8.0_byAIR.exe");
            let v7 = game_dir.join("ArizonaLauncher7.0_byAIR.exe");
            let v6 = game_dir.join("ArizonaLauncher6_byAIR.exe");
            if v8.exists() {
                cfg.launcher_path = v8.to_string_lossy().to_string();
            } else if v7.exists() {
                cfg.launcher_path = v7.to_string_lossy().to_string();
            } else if v6.exists() {
                cfg.launcher_path = v6.to_string_lossy().to_string();
            }
            return true;
        }
    }
    false
}

#[tauri::command]
pub fn auto_detect_paths(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let found = {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        auto_detect_game_paths_inner(&mut cfg)
    };
    if found {
        state.save_config()?;
    }
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "success": found,
        "message": if found { "Found" } else { "Not found" },
        "game_path": cfg.game_path,
        "launcher_path": cfg.launcher_path
    }))
}

#[tauri::command]
pub fn set_game_paths(state: State<'_, AppState>, game: String, launcher: String) -> Result<serde_json::Value, String> {
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.game_path = game;
        cfg.launcher_path = launcher;
    }
    state.save_config()?;
    Ok(serde_json::json!({"success": true, "message": "Paths set"}))
}

#[tauri::command]
pub fn select_game_path() -> Result<serde_json::Value, String> {
    // TODO: native folder dialog via tauri::api::dialog
    Ok(serde_json::json!({"success": false, "message": "Not implemented yet"}))
}

// ==================== Game Launch ====================

#[tauri::command]
pub fn start_game(
    state: State<'_, AppState>,
    nickname: String,
    server_data: Option<serde_json::Value>,
    launch_params: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;

    if cfg.launcher_path.is_empty() || !std::path::Path::new(&cfg.launcher_path).exists() {
        return Ok(serde_json::json!({"success": false, "message": "Launcher not found", "code": "LAUNCHER_MISSING"}));
    }
    if cfg.game_path.is_empty() || !std::path::Path::new(&cfg.game_path).exists() {
        return Ok(serde_json::json!({"success": false, "message": "Game path is missing or invalid", "code": "GAME_MISSING"}));
    }
    if nickname.trim().is_empty() {
        return Ok(serde_json::json!({"success": false, "message": "Enter nickname", "code": "NICKNAME_EMPTY"}));
    }

    // TODO: spawn process, build cmdline args
    Ok(serde_json::json!({"success": true, "message": format!("Launching for {}", nickname)}))
}

#[tauri::command]
pub fn is_gta_running() -> Result<serde_json::Value, String> {
    // TODO: check process list for gta_sa.exe
    Ok(serde_json::json!({"running": false, "was_running": false}))
}

// ==================== Server / News ====================

#[tauri::command]
pub async fn get_servers() -> Result<serde_json::Value, String> {
    // TODO: fetch from arizona-ping API
    Ok(serde_json::json!([]))
}

#[tauri::command]
pub async fn get_servers_and_news() -> Result<serde_json::Value, String> {
    // TODO: parallel fetch servers + news
    Ok(serde_json::json!({"success": true, "servers": [], "news": {"success": false}}))
}

#[tauri::command]
pub async fn get_launcher_news() -> Result<serde_json::Value, String> {
    // TODO: fetch updatenews.txt from GitHub
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub fn get_read_news_ids(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"success": true, "ids": cfg.read_news_ids}))
}

#[tauri::command]
pub fn save_read_news_ids(state: State<'_, AppState>, ids: Vec<String>) -> Result<serde_json::Value, String> {
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.read_news_ids = ids;
    }
    state.save_config()?;
    Ok(serde_json::json!({"success": true}))
}

// ==================== Launcher Version ====================

#[tauri::command]
pub fn get_launcher_version(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": true, "version": "v8"}))
}

#[tauri::command]
pub fn check_launcher_update(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": true, "needs_update": false, "message": "Лаунчер v8 уже установлен"}))
}

#[tauri::command]
pub fn get_install_action(folder_path: String) -> Result<serde_json::Value, String> {
    // TODO: check game folder
    Ok(serde_json::json!({"action": "none", "message": "OK"}))
}

/// API-обёртка для JS: {success, install_info} (как в Python get_install_action_api)
#[tauri::command]
pub fn get_install_action_api(folder_path: String) -> Result<serde_json::Value, String> {
    match get_install_action(folder_path) {
        Ok(install_info) => Ok(serde_json::json!({"success": true, "install_info": install_info})),
        Err(e) => Ok(serde_json::json!({"success": false, "message": e})),
    }
}

/// API-обёртка для JS поверх check_patches_available (как в Python check_patches_available_api)
#[tauri::command]
pub async fn check_patches_available_api(version: Option<String>) -> Result<serde_json::Value, String> {
    match check_patches_available(version).await {
        Ok(v) => Ok(v),
        Err(e) => Ok(serde_json::json!({"success": false, "available": false, "message": e})),
    }
}

/// Скачивать картинку по URL и отдавать как data: URL в base64
#[tauri::command]
pub async fn fetch_image_as_base64(url: String) -> Result<serde_json::Value, String> {
    let resp = match reqwest::get(&url).await {
        Ok(r) => r,
        Err(e) => return Ok(serde_json::json!({"success": false, "message": e.to_string()})),
    };
    if !resp.status().is_success() {
        return Ok(serde_json::json!({"success": false, "message": format!("HTTP {}", resp.status())}));
    }
    let mime = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/png")
        .to_string();
    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => return Ok(serde_json::json!({"success": false, "message": e.to_string()})),
    };
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(serde_json::json!({"success": true, "data_url": format!("data:{};base64,{}", mime, b64)}))
}

// ==================== Debug ====================

#[tauri::command]
pub fn get_debug_info(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "success": true,
        "data": {
            "config_path": state.config_path.to_string_lossy(),
            "docs_path": state.documents_path.to_string_lossy(),
            "game_path": cfg.game_path,
            "launcher_path": cfg.launcher_path,
            "app_version": "v2.0.0",
            "launcher_version": "v8",
            "last_nickname": cfg.last_nickname,
            "last_server": cfg.last_server,
            "memory": cfg.launch_params.memory,
            "os_name": std::env::consts::OS,
            "architecture": std::env::consts::ARCH,
            "rust_version": env!("CARGO_PKG_VERSION"),
        }
    }))
}

// ==================== Window ====================

#[tauri::command]
pub fn minimize_window(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    if let Some(window) = app.get_webview_window("main") {
        window.minimize().map_err(|e| e.to_string())?;
    }
    Ok(serde_json::json!({"success": true}))
}

#[tauri::command]
pub fn minimize_to_tray(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(serde_json::json!({"success": true}))
}

#[tauri::command]
pub fn restore_window(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(serde_json::json!({"success": true}))
}

#[tauri::command]
pub fn open_game_folder(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    if cfg.game_path.is_empty() {
        return Ok(serde_json::json!({"success": false, "message": "Path not set"}));
    }
    let game_dir = std::path::Path::new(&cfg.game_path)
        .parent()
        .ok_or("Invalid game path")?;
    open::that(game_dir).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"success": true}))
}

#[tauri::command]
pub fn open_data_folder(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    std::fs::create_dir_all(&state.documents_path).ok();
    open::that(&state.documents_path).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"success": true}))
}

#[tauri::command]
pub fn open_log_file() -> Result<serde_json::Value, String> {
    // TODO: find log path
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

// ==================== Settings ====================

#[tauri::command]
pub fn get_launcher_settings(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"success": true, "data": cfg.launcher_settings}))
}

#[tauri::command]
pub fn save_launcher_settings(state: State<'_, AppState>, settings: serde_json::Value) -> Result<serde_json::Value, String> {
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.launcher_settings = serde_json::from_value(settings).map_err(|e| e.to_string())?;
    }
    state.save_config()?;
    Ok(serde_json::json!({"success": true}))
}

#[tauri::command]
pub fn get_window_title(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    let title = cfg.launcher_settings.window_title.clone().unwrap_or_else(|| "Arizona RP Launcher".to_string());
    Ok(serde_json::json!({"success": true, "title": title}))
}

#[tauri::command]
pub fn set_window_title(state: State<'_, AppState>, title: String) -> Result<serde_json::Value, String> {
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.launcher_settings.window_title = Some(title);
    }
    state.save_config()?;
    Ok(serde_json::json!({"success": true}))
}

#[tauri::command]
pub fn get_window_size(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    let size = cfg.launcher_settings.window_size.clone().unwrap_or_else(|| vec![1285, 732]);
    Ok(serde_json::json!(size))
}

#[tauri::command]
pub fn save_layout_order(state: State<'_, AppState>, order: Vec<String>) -> Result<serde_json::Value, String> {
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.launcher_settings.layout_order = Some(order);
    }
    state.save_config()?;
    Ok(serde_json::json!({"success": true}))
}

#[tauri::command]
pub fn get_layout_order(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    let order = cfg.launcher_settings.layout_order.clone().unwrap_or_default();
    Ok(serde_json::json!(order))
}

// ==================== Profiles ====================

#[tauri::command]
pub fn get_profiles(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "success": true,
        "profiles": cfg.profiles,
        "active_id": cfg.active_profile_id
    }))
}

#[tauri::command]
pub fn save_profiles(state: State<'_, AppState>, profiles: Vec<serde_json::Value>, active_id: String) -> Result<serde_json::Value, String> {
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.profiles = Some(profiles);
        cfg.active_profile_id = Some(active_id);
    }
    state.save_config()?;
    Ok(serde_json::json!({"success": true}))
}

// ==================== Patches ====================

#[tauri::command]
pub fn check_patches_status() -> Result<serde_json::Value, String> {
    // TODO
    Ok(serde_json::json!({"success": true, "status": {"status": "missing", "message": "Not implemented"}}))
}

#[tauri::command]
pub fn check_patches_files() -> Result<serde_json::Value, String> {
    // TODO
    Ok(serde_json::json!({"json_exists": false, "dll_exists": false, "both_exist": false}))
}

#[tauri::command]
pub fn read_patches() -> Result<serde_json::Value, String> {
    // TODO
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub fn write_patches(data: serde_json::Value) -> Result<serde_json::Value, String> {
    // TODO
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub fn list_patches_backups() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!([]))
}

#[tauri::command]
pub fn restore_patches_backup(filename: String) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub fn delete_patches_backup(filename: String) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub fn preview_patches_backup(filename: String) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub async fn download_and_install_patches(version: Option<String>) -> Result<serde_json::Value, String> {
    // TODO
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub async fn get_patches_latest_version() -> Result<serde_json::Value, String> {
    // TODO
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub async fn check_patches_available(version: Option<String>) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": true, "available": false}))
}

#[tauri::command]
pub async fn check_patches_update_status() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": true, "released": false}))
}

#[tauri::command]
pub fn export_patches(data: serde_json::Value) -> Result<serde_json::Value, String> {
    let _ = data; // TODO: save-file dialog (tauri-plugin-dialog) + write JSON
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub fn import_patches() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub fn install_patches_file(filename: String, content_b64: String) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

// ==================== Presets ====================

#[tauri::command]
pub async fn fetch_patch_presets() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub async fn install_patch_preset(url: String) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub async fn fetch_patches_schema() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

// ==================== Update ====================

#[tauri::command]
pub async fn check_update_api(game_path: Option<String>) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": true, "needs_update": false}))
}

#[tauri::command]
pub async fn download_update_api() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub async fn start_launcher_install(folder_path: String, version: Option<String>, install_type: Option<String>) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

// ==================== Moonloader ====================

#[tauri::command]
pub fn get_moonloader_scripts() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub fn toggle_moonloader_script(script_name: String, enable: bool) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub fn delete_moonloader_script(filename: String) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub fn open_moonloader_folder() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub async fn fetch_moonloader_catalog() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub async fn install_moonloader_script(url: String, filename: String) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub fn install_lua_file(filename: String, content_b64: String) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

// ==================== Libraries ====================

#[tauri::command]
pub async fn fetch_libraries_catalog() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub fn list_installed_libraries() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": true, "data": []}))
}

#[tauri::command]
pub async fn install_library(name: String, url: String) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub fn uninstall_library(name: String) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

// ==================== Others ====================

#[tauri::command]
pub async fn fetch_others_catalog() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub async fn install_other_file(url: String, filename: String, destination: String) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub fn remove_other_file(filename: String, destination: String) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub fn check_other_installed(filename: String, destination: String) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"installed": false}))
}

// ==================== Misc ====================

#[tauri::command]
pub fn check_tray_support() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": true, "available": true}))
}

#[tauri::command]
pub fn open_telegram_share() -> Result<serde_json::Value, String> {
    open::that("https://t.me/halfik24").map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"success": true}))
}

#[tauri::command]
pub fn get_wallpapers() -> Result<serde_json::Value, String> {
    // TODO: scan wallpapers dir
    Ok(serde_json::json!({"success": true, "wallpapers": []}))
}

#[tauri::command]
pub fn get_wallpaper_thumb(filename: String) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub fn get_wallpaper_full(filename: String) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub fn select_bg_image() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}
