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

use crate::config::ServersCache;
use base64::Engine as _;

/// Директория кэша иконок серверов: %LOCALAPPDATA%\ArizonaLauncher\icons
fn icon_cache_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(dirs::cache_dir)
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default());
    let dir = base.join("ArizonaLauncher").join("icons");
    std::fs::create_dir_all(&dir).ok();
    dir
}

/// Расширение файла иконки из URL (как в Python: Path(url).suffix.split('?')[0]).
/// Допустимые значения — как в Python, иначе '.png'.
fn icon_file_ext(url: &str) -> String {
    let no_query = url.split('?').next().unwrap_or("");
    let ext = no_query.rsplit('.').next().unwrap_or("");
    match ext.to_ascii_lowercase().as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "ico" => format!(".{}", ext.to_ascii_lowercase()),
        _ => ".png".to_string(),
    }
}

fn icon_mime(ext: &str) -> &'static str {
    match ext.trim_start_matches('.').to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        _ => "image/png",
    }
}

/// Python `_download_icon`: читает из кэша (b64 или исходный файл) и возвращает data: URI.
/// Ничего не качает — скачивание делает prefetch в фоне.
fn icon_data_uri(icon_url: &str, server_id: &str) -> String {
    if icon_url.is_empty() {
        return String::new();
    }
    let cache = icon_cache_dir();
    let ext = icon_file_ext(icon_url);

    // 1) Готовый base64 data URI
    let b64_path = cache.join(format!("server_{}.b64", server_id));
    if let Ok(s) = std::fs::read_to_string(&b64_path) {
        let s = s.trim().to_string();
        if !s.is_empty() {
            return s;
        }
    }

    // 2) Локальный файл — кодируем и сохраняем data URI
    let local_path = cache.join(format!("server_{}{}", server_id, ext));
    if let Ok(bytes) = std::fs::read(&local_path) {
        if !bytes.is_empty() {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            let data_uri = format!("data:{};base64,{}", icon_mime(&ext), b64);
            std::fs::write(&b64_path, &data_uri).ok();
            return data_uri;
        }
    }

    String::new() // Нет в кэше — фоллбэк покажет номер
}

/// Python `_prefetch_all_icons`: фоновая докачка недостающих иконок в файловый кэш.
async fn prefetch_icons(servers: Vec<serde_json::Value>) {
    for s in &servers {
        let icon_url = s
            .get("original_icon")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if icon_url.is_empty() {
            continue;
        }
        let server_id = s
            .get("number")
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default();
        if server_id.is_empty() {
            continue;
        }

        let local_path =
            icon_cache_dir().join(format!("server_{}{}", server_id, icon_file_ext(icon_url)));
        if let Ok(md) = std::fs::metadata(&local_path) {
            if md.len() > 0 {
                continue;
            }
        }

        if let Ok(resp) = reqwest::get(icon_url).await {
            if resp.status().is_success() {
                if let Ok(bytes) = resp.bytes().await {
                    if !bytes.is_empty() {
                        std::fs::write(&local_path, &bytes).ok();
                    }
                }
            }
        }
    }
}

/// Python-семантика truthy для подбора полей (`a or b or c`).
fn truthy(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        serde_json::Value::String(s) => !s.is_empty(),
        serde_json::Value::Array(a) => !a.is_empty(),
        _ => true,
    }
}

/// Первое truthy-значение среди ключей, как `s.get('a') or s.get('b', default)`.
fn pick<'a>(v: &'a serde_json::Value, keys: &[&str]) -> Option<&'a serde_json::Value> {
    let obj = v.as_object()?;
    keys.iter().find_map(|k| obj.get(*k).filter(|x| truthy(x)))
}

fn jstr(v: &serde_json::Value, keys: &[&str], default: &str) -> String {
    pick(v, keys)
        .and_then(|x| x.as_str())
        .unwrap_or(default)
        .to_string()
}

fn jnum(v: &serde_json::Value, keys: &[&str], default: serde_json::Value) -> serde_json::Value {
    pick(v, keys).cloned().unwrap_or(default)
}

/// Нормализация списка серверов Python `get_servers` (number/name/online/queue/...).
fn normalize_servers(server_list: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let mut servers = Vec::new();
    for s in server_list {
        if !s.is_object() {
            continue;
        }
        let server_id = pick(s, &["number", "serverNumber", "id"])
            .cloned()
            .unwrap_or_else(|| serde_json::json!(1));
        let id_str = match &server_id {
            serde_json::Value::String(x) => x.clone(),
            other => other.to_string(),
        };
        let icon_url = jstr(s, &["icon"], "");
        let icon = if icon_url.is_empty() {
            String::new()
        } else {
            icon_data_uri(&icon_url, &id_str)
        };
        servers.push(serde_json::json!({
            "number": server_id,
            "name": jstr(s, &["name"], "Server"),
            "online": jnum(s, &["online", "playersOnline"], serde_json::json!(0)),
            "queue": jnum(s, &["queue", "queueLength"], serde_json::json!(0)),
            "recommended": pick(s, &["recomend", "recommended"]).is_some(),
            "ip": jstr(s, &["ip"], &format!("server{}.arizona-rp.com", id_str)),
            "port": jnum(s, &["port"], serde_json::json!(7777)),
            "maxplayers": jnum(s, &["maxplayers", "maxPlayers"], serde_json::json!(1000)),
            "original_icon": icon_url,
            "icon": icon,
        }));
    }
    servers
}

/// Тело `get_servers`: кэш 60s, ETag/If-Modified-Since, 304 → кэш, нормализация, prefetch.
async fn fetch_servers(state: &AppState) -> serde_json::Value {
    const TTL: std::time::Duration = std::time::Duration::from_secs(60);

    // Читаем состояние кэша (не держим лок через await)
    let (cached_data, etag, last_modified, fresh) = {
        let guard = match state.servers_cache.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(), // отравленный мьютекс — данные всё ещё читаемы
        };
        match guard.as_ref() {
            Some(c) => (
                Some(c.data.clone()),
                c.etag.clone(),
                c.last_modified.clone(),
                c.ts.elapsed() < TTL,
            ),
            None => (None, None, None, false),
        }
    };

    if fresh {
        // Обновляем иконки в кэшированных данных, если они теперь доступны локально
        if let Some(mut servers) = cached_data {
            for s in servers.iter_mut() {
                let has_icon = s.get("icon").and_then(|v| v.as_str()).map(|x| !x.is_empty()).unwrap_or(false);
                if !has_icon {
                    let icon_url = s.get("original_icon").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let id = pick(s, &["number", "serverNumber", "id"])
                        .map(|v| match v {
                            serde_json::Value::String(x) => x.clone(),
                            other => other.to_string(),
                        })
                        .unwrap_or_default();
                    if !icon_url.is_empty() && !id.is_empty() {
                        let uri = icon_data_uri(&icon_url, &id);
                        if let Some(obj) = s.as_object_mut() {
                            obj.insert("icon".into(), serde_json::Value::String(uri));
                        }
                    }
                }
            }
            return serde_json::Value::Array(servers);
        }
    }

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static("ArizonaLauncher/1.0"),
    );
    if let Some(ref e) = etag {
        if let Ok(v) = reqwest::header::HeaderValue::from_str(e) {
            headers.insert(reqwest::header::IF_NONE_MATCH, v);
        }
    }
    if let Some(ref lm) = last_modified {
        if let Ok(v) = reqwest::header::HeaderValue::from_str(lm) {
            headers.insert(reqwest::header::IF_MODIFIED_SINCE, v);
        }
    }

    let client = match reqwest::Client::builder().build() {
        Ok(c) => c,
        Err(_) => return cached_data.map(serde_json::Value::Array).unwrap_or(serde_json::Value::Null),
    };

    let resp = match client
        .get("https://arizona-ping.react.group/desktop/ping/Arizona/ping.json")
        .headers(headers)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            log::error!("Server fetch error: {e}");
            return cached_data.map(serde_json::Value::Array).unwrap_or(serde_json::Value::Null);
        }
    };

    let now = std::time::Instant::now();

    // 304 Not Modified — отдаём кэшированные данные
    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        if let Some(mut guard) = state.servers_cache.lock().ok() {
            if let Some(ref mut c) = guard.as_mut() {
                c.ts = now;
            }
        }
        return cached_data.map(serde_json::Value::Array).unwrap_or(serde_json::Value::Null);
    }

    if !resp.status().is_success() {
        log::error!("Server fetch HTTP {}", resp.status());
        return cached_data.map(serde_json::Value::Array).unwrap_or(serde_json::Value::Null);
    }

    let etag_new = resp
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let lm_new = resp
        .headers()
        .get(reqwest::header::LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let data: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            log::error!("Server fetch parse error: {e}");
            return cached_data.map(serde_json::Value::Array).unwrap_or(serde_json::Value::Null);
        }
    };

    let server_list: &Vec<serde_json::Value> = if data.is_object() {
        match data.get("query") {
            Some(q) if q.is_array() => q.as_array().unwrap(),
            _ => match data.as_array() {
                Some(a) => a,
                None => {
                    return cached_data.map(serde_json::Value::Array).unwrap_or(serde_json::Value::Null)
                }
            },
        }
    } else if let Some(a) = data.as_array() {
        a
    } else {
        return cached_data.map(serde_json::Value::Array).unwrap_or(serde_json::Value::Null);
    };

    let servers = normalize_servers(server_list);

    if let Ok(mut guard) = state.servers_cache.lock() {
        *guard = Some(ServersCache {
            ts: now,
            data: servers.clone(),
            etag: etag_new,
            last_modified: lm_new,
        });
    }

    // Предзагружаем иконки в фоне
    let spawn_servers = servers.clone();
    tauri::async_runtime::spawn(async move {
        prefetch_icons(spawn_servers).await;
    });

    serde_json::Value::Array(servers)
}

#[tauri::command]
pub async fn get_servers(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    Ok(fetch_servers(&state).await)
}

#[tauri::command]
pub async fn get_servers_and_news(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let (servers, news) = tokio::join!(fetch_servers(&state), fetch_launcher_news());
    Ok(serde_json::json!({
        "success": true,
        "servers": if servers.is_null() { serde_json::json!([]) } else { servers },
        "news": news,
    }))
}

/// Тело `get_launcher_news`: 3 fallback URL на GitHub, utf-8 → windows-1251.
async fn fetch_launcher_news() -> serde_json::Value {
    let urls = [
        "https://raw.githubusercontent.com/worteng/ArizonaLauncher/main/updatenews.txt",
        "https://raw.githubusercontent.com/worteng/ArizonaLauncher/refs/heads/main/updatenews.txt",
        "https://github.com/worteng/ArizonaLauncher/raw/main/updatenews.txt",
    ];

    for url in urls {
        log::info!("Пробуем загрузить новости: {url}");
        let client = match reqwest::Client::builder().build() {
            Ok(c) => c,
            Err(_) => continue,
        };
        let resp = match client
            .get(url)
            .timeout(std::time::Duration::from_secs(20))
            .header(reqwest::header::CACHE_CONTROL, "no-cache")
            .header(reqwest::header::PRAGMA, "no-cache")
            .header(reqwest::header::USER_AGENT, "Mozilla/5.0")
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                log::error!("Ошибка при загрузке с {url}: {e}");
                continue;
            }
        };
        log::info!("Статус ответа: {}", resp.status());
        let status = resp.status();
        let bytes = match resp.bytes().await {
            Ok(b) => b,
            Err(e) => {
                log::error!("Ошибка при чтении с {url}: {e}");
                continue;
            }
        };
        // utf-8, иначе windows-1251 (с заменой невалидных байтов, как errors='replace')
        let text = match std::str::from_utf8(&bytes) {
            Ok(s) => s.to_string(),
            Err(_) => {
                let (cow, _, _) = encoding_rs::WINDOWS_1251.decode(&bytes);
                cow.into_owned()
            }
        };
        if status == reqwest::StatusCode::OK && !text.trim().is_empty() {
            return serde_json::json!({"success": true, "text": text});
        }
    }

    serde_json::json!({
        "success": false,
        "message": "Не удалось загрузить новости (проверьте интернет или блокировку GitHub)"
    })
}

#[tauri::command]
pub async fn get_launcher_news() -> Result<serde_json::Value, String> {
    Ok(fetch_launcher_news().await)
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
