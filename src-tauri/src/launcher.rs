use crate::config::{AppState, LauncherConfig};
use crate::util;
use std::path::PathBuf;
use tauri::{Manager, State};

// ==================== Config Commands ====================

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = util::lock_config(&state)?;
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
    let cfg = util::lock_config(&state)?;
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
    let cfg = util::lock_config(&state)?;
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
    util::update_config(&state, |cfg| {
        cfg.last_nickname = nickname;
        Ok(())
    })
}

#[tauri::command]
pub fn update_server(state: State<'_, AppState>, server_number: u32) -> Result<serde_json::Value, String> {
    util::update_config(&state, |cfg| {
        cfg.last_server = server_number;
        Ok(())
    })
}

#[tauri::command]
pub fn update_launch_params(state: State<'_, AppState>, params: serde_json::Value) -> Result<serde_json::Value, String> {
    util::update_config(&state, |cfg| {
        cfg.launch_params = serde_json::from_value(params).map_err(|e| e.to_string())?;
        Ok(())
    })
}

#[tauri::command]
pub fn read_launch_params(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = util::lock_config(&state)?;
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
        let mut cfg = util::lock_config(&state)?;
        auto_detect_game_paths_inner(&mut cfg)
    };
    if found {
        state.save_config()?;
    }
    let cfg = util::lock_config(&state)?;
    Ok(serde_json::json!({
        "success": found,
        "message": if found { "Found" } else { "Not found" },
        "game_path": cfg.game_path,
        "launcher_path": cfg.launcher_path
    }))
}

#[tauri::command]
pub fn set_game_paths(state: State<'_, AppState>, game: String, launcher: String) -> Result<serde_json::Value, String> {
    util::update_config(&state, |cfg| {
        cfg.game_path = game;
        cfg.launcher_path = launcher;
        Ok(())
    })?;
    Ok(serde_json::json!({"success": true, "message": "Paths set"}))
}

#[tauri::command]
pub fn select_game_path() -> Result<serde_json::Value, String> {
    // TODO: native folder dialog via tauri::api::dialog
    Ok(serde_json::json!({"success": false, "message": "Not implemented yet"}))
}

// ==================== Game Launch ====================

/// Python `get_launcher_version`: v8 > v7 > v6 по файлам в папке игры, иначе v8.
fn detect_launcher_version(state: &AppState) -> String {
    let Some(game_dir) = util::game_dir(state) else {
        return "v8".to_string();
    };
    let plugins_dir = game_dir.join("preloading_plugins");
    let has_plugins = std::fs::read_dir(&plugins_dir)
        .map(|mut rd| rd.next().is_some())
        .unwrap_or(false);
    if has_plugins {
        if game_dir.join("ArizonaLauncher8.0_byAIR.exe").exists() {
            return "v8".to_string();
        }
        if game_dir.join("ArizonaLauncher7.0_byAIR.exe").exists() {
            return "v7".to_string();
        }
        if game_dir.join("ArizonaLauncher6_byAIR.exe").exists() {
            return "v6".to_string();
        }
    }
    "v8".to_string()
}

/// Python `kill_all_launchers`: убивает точные имена exe (не substring — чтобы не убить себя).
fn kill_all_launchers() {
    const LAUNCHER_EXES: [&str; 3] = [
        "arizonalauncher6_byair.exe",
        "arizonalauncher7.0_byair.exe",
        "arizonalauncher8.0_byair.exe",
    ];
    let my_pid = std::process::id();
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let mut killed = 0u32;
    let mut denied = 0u32;
    for (pid, proc) in sys.processes() {
        if pid.as_u32() == my_pid {
            continue;
        }
        let name = proc
            .name()
            .to_string_lossy()
            .to_lowercase();
        if LAUNCHER_EXES.contains(&name.as_str()) {
            if proc.kill() {
                killed += 1;
            } else {
                denied += 1;
                log::warn!("kill_all_launchers: AccessDenied pid={} name={}", pid, name);
            }
        }
    }
    if denied > 0 {
        log::warn!("kill_all_launchers: {} процессов не удалось убить (нет прав)", denied);
    }
    log::info!("kill_all_launchers: killed={}, denied={}", killed, denied);
    std::thread::sleep(std::time::Duration::from_secs(1));
}

/// Python: санитайзер ника — убрать control-символы, обрезать до 20.
fn sanitize_nickname(nick: &str) -> String {
    let stripped: String = nick
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    stripped.chars().take(20).collect()
}

/// Python: валидация CDN, формат `\d+,\d+,\d+`, иначе fallback "1,1,1".
fn validate_cdn(raw: &str) -> String {
    let re = match regex::Regex::new(r"^\d+,\d+,\d+$") {
        Ok(r) => r,
        Err(_) => return "1,1,1".to_string(),
    };
    if re.is_match(raw) {
        raw.to_string()
    } else {
        log::warn!("launch_game: некорректный CDN '{}', использую '1,1,1'", raw);
        "1,1,1".to_string()
    }
}

#[tauri::command]
pub async fn start_game(
    state: State<'_, AppState>,
    nickname: String,
    server_data: Option<serde_json::Value>,
    launch_params: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    // Валидации (порядок как в Python)
    let (launcher_path, game_path) = {
        let cfg = util::lock_config(&state)?;
        (cfg.launcher_path.clone(), cfg.game_path.clone())
    };
    if launcher_path.is_empty() || !std::path::Path::new(&launcher_path).exists() {
        return Ok(serde_json::json!({"success": false, "message": "Launcher not found", "code": "LAUNCHER_MISSING"}));
    }
    if game_path.is_empty() || !std::path::Path::new(&game_path).exists() {
        return Ok(serde_json::json!({"success": false,
            "message": "Game path is missing or invalid — set it in Settings", "code": "GAME_MISSING"}));
    }
    if nickname.trim().is_empty() {
        return Ok(serde_json::json!({"success": false, "message": "Enter nickname", "code": "NICKNAME_EMPTY"}));
    }
    let nickname = sanitize_nickname(&nickname);
    if nickname.is_empty() {
        return Ok(serde_json::json!({"success": false,
            "message": "Nickname contains only invalid characters", "code": "NICKNAME_INVALID"}));
    }

    let srv_ip = server_data
        .as_ref()
        .and_then(|s| s.get("ip"))
        .and_then(|v| v.as_str())
        .unwrap_or("payson.arizona-rp.com")
        .to_string();
    let srv_port = server_data
        .as_ref()
        .and_then(|s| s.get("port"))
        .map(|v| v.to_string().trim_matches('"').to_string())
        .unwrap_or_else(|| "7777".to_string());

    let params = match launch_params {
        Some(p) => serde_json::from_value::<crate::config::LaunchParams>(p).ok(),
        None => None,
    };
    let params = match params {
        Some(p) => p,
        None => util::lock_config(&state)?.launch_params.clone(),
    };

    let launcher_ver = detect_launcher_version(&state);
    let is_v7plus = launcher_ver == "v7" || launcher_ver == "v8";
    let cdn_value = validate_cdn(&params.cdn);

    // Базовая команда
    let mut cmd = vec![
        "-c".to_string(),
        "-h".to_string(),
        srv_ip,
        "-p".to_string(),
        srv_port,
        "-mem".to_string(),
        params.memory.to_string(),
        "-n".to_string(),
        nickname.clone(),
        "-arizona".to_string(),
    ];

    // Общие флаги для всех версий (v6/v7/v8)
    let common_flags: [(&bool, &str); 8] = [
        (&params.widescreen, "-widescreen"),
        (&params.texture_mode, "-t"),
        (&params.color_depth_16, "-16bpp"),
        (&params.allow_hdr, "-allow_hdr"),
        (&params.enable_grass, "-enable_grass"),
        (&params.ldo, "-ldo"),
        (&params.auth_cef_enable, "-auth_cef_enable"),
        (&params.autologin, "-x"),
    ];
    for (flag_on, name) in &common_flags {
        if **flag_on {
            cmd.push(name.to_string());
        }
    }

    // Оконный режим: -window и -old_window взаимоисключающие
    if is_v7plus && params.old_window {
        cmd.push("-old_window".to_string());
    } else if params.window_mode {
        cmd.push("-window".to_string());
    }

    // Доп. флаги v7+
    if is_v7plus {
        let v7plus_flags: [(&bool, &str); 6] = [
            (&params.enable_new_grass, "-enable_new_grass"),
            (&params.use_d3dx9_43, "-use_d3dx9_43"),
            (&params.show_dialog_ids, "-show_dialog_ids"),
            (&params.offcef, "-offcef"),
            (&params.modern_scale, "-modern_scale"),
            (&params.trees_new, "-trees_new"),
        ];
        for (flag_on, name) in &v7plus_flags {
            if **flag_on {
                cmd.push(name.to_string());
            }
        }
    }

    cmd.push("-cdn".to_string());
    cmd.push(cdn_value);

    // kill_all_launchers перед стартом (в Python — синхронно с sleep 1s)
    tokio::task::spawn_blocking(kill_all_launchers).await.ok();

    let exe = std::path::PathBuf::from(&launcher_path);
    let cwd = exe
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let child = std::process::Command::new(&exe)
        .args(&cmd)
        .current_dir(&cwd)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();

    let mut proc = match child {
        Ok(p) => p,
        Err(e) => {
            log::error!("launch_game: {e}");
            return Ok(serde_json::json!({"success": false, "message": e.to_string(), "code": "EXCEPTION"}));
        }
    };

    // Poll до 2 секунд: stub-лаунчеры (Air v6/v7/v8) могут завершиться с кодом 0
    // сразу после спавна дочернего процесса игры — это НЕ краш.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        match proc.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
            Err(_) => break,
        }
    }
    let rc_now = proc.try_wait().ok().flatten().and_then(|st| st.code());

    if rc_now.is_none() || rc_now == Some(0) {
        // Сохраняем ник/сервер/параметры
        util::update_config(&state, |cfg| {
            cfg.last_nickname = nickname.clone();
            if let Some(ref sd) = server_data {
                if let Some(num) = sd.get("number").and_then(|v| v.as_u64()) {
                    cfg.last_server = num as u32;
                }
            }
            cfg.launch_params = params.clone();
            Ok(())
        })?;
        let pid = proc.id();
        // Detach: не держим handle
        std::mem::forget(proc);
        Ok(serde_json::json!({"success": true, "message": format!("Launching for {}", nickname), "pid": pid}))
    } else {
        Ok(serde_json::json!({"success": false,
            "message": format!("Launcher crashed (exit code {})", rc_now.unwrap_or(-1)),
            "code": "LAUNCHER_CRASHED"}))
    }
}

#[tauri::command]
pub fn is_gta_running() -> Result<serde_json::Value, String> {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let running = sys
        .processes()
        .values()
        .any(|p| p.name().to_string_lossy().eq_ignore_ascii_case("gta_sa.exe"));
    Ok(serde_json::json!({"running": running, "was_running": false}))
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

    let resp = match util::http_request(
        reqwest::Method::GET,
        "https://arizona-ping.react.group/desktop/ping/Arizona/ping.json",
        headers,
        std::time::Duration::from_secs(10),
    )
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
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CACHE_CONTROL,
            reqwest::header::HeaderValue::from_static("no-cache"),
        );
        headers.insert(
            reqwest::header::PRAGMA,
            reqwest::header::HeaderValue::from_static("no-cache"),
        );
        headers.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_static("Mozilla/5.0"),
        );
        let resp = match util::http_request(
            reqwest::Method::GET,
            url,
            headers,
            std::time::Duration::from_secs(20),
        )
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
        let text = util::decode_text_bytes(&bytes);
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
    let cfg = util::lock_config(&state)?;
    Ok(serde_json::json!({"success": true, "ids": cfg.read_news_ids}))
}

#[tauri::command]
pub fn save_read_news_ids(state: State<'_, AppState>, ids: Vec<String>) -> Result<serde_json::Value, String> {
    {
        let mut cfg = util::lock_config(&state)?;
        cfg.read_news_ids = ids;
    }
    state.save_config()?;
    util::ok_true()
}

// ==================== Launcher Version ====================

#[tauri::command]
pub fn get_launcher_version(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": true, "version": detect_launcher_version(&state)}))
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
    let cfg = util::lock_config(&state)?;
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
    util::ok_true()
}

#[tauri::command]
pub fn minimize_to_tray(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }
    util::ok_true()
}

#[tauri::command]
pub fn restore_window(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    util::ok_true()
}

#[tauri::command]
pub fn open_game_folder(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = util::lock_config(&state)?;
    if cfg.game_path.is_empty() {
        return Ok(serde_json::json!({"success": false, "message": "Path not set"}));
    }
    let game_dir = std::path::Path::new(&cfg.game_path)
        .parent()
        .ok_or("Invalid game path")?;
    open::that(game_dir).map_err(|e| e.to_string())?;
    util::ok_true()
}

#[tauri::command]
pub fn open_data_folder(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    std::fs::create_dir_all(&state.documents_path).ok();
    open::that(&state.documents_path).map_err(|e| e.to_string())?;
    util::ok_true()
}

#[tauri::command]
pub fn open_log_file() -> Result<serde_json::Value, String> {
    // TODO: find log path
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

// ==================== Settings ====================

#[tauri::command]
pub fn get_launcher_settings(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = util::lock_config(&state)?;
    Ok(serde_json::json!({"success": true, "data": cfg.launcher_settings}))
}

#[tauri::command]
pub fn save_launcher_settings(state: State<'_, AppState>, settings: serde_json::Value) -> Result<serde_json::Value, String> {
    {
        let mut cfg = util::lock_config(&state)?;
        cfg.launcher_settings = serde_json::from_value(settings).map_err(|e| e.to_string())?;
    }
    state.save_config()?;
    util::ok_true()
}

#[tauri::command]
pub fn get_window_title(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = util::lock_config(&state)?;
    let title = cfg.launcher_settings.window_title.clone().unwrap_or_else(|| "Arizona RP Launcher".to_string());
    Ok(serde_json::json!({"success": true, "title": title}))
}

#[tauri::command]
pub fn set_window_title(state: State<'_, AppState>, title: String) -> Result<serde_json::Value, String> {
    {
        let mut cfg = util::lock_config(&state)?;
        cfg.launcher_settings.window_title = Some(title);
    }
    state.save_config()?;
    util::ok_true()
}

#[tauri::command]
pub fn get_window_size(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = util::lock_config(&state)?;
    let size = cfg.launcher_settings.window_size.clone().unwrap_or_else(|| vec![1285, 732]);
    Ok(serde_json::json!(size))
}

#[tauri::command]
pub fn save_layout_order(state: State<'_, AppState>, order: Vec<String>) -> Result<serde_json::Value, String> {
    {
        let mut cfg = util::lock_config(&state)?;
        cfg.launcher_settings.layout_order = Some(order);
    }
    state.save_config()?;
    util::ok_true()
}

#[tauri::command]
pub fn get_layout_order(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = util::lock_config(&state)?;
    let order = cfg.launcher_settings.layout_order.clone().unwrap_or_default();
    Ok(serde_json::json!(order))
}

// ==================== Profiles ====================

#[tauri::command]
pub fn get_profiles(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = util::lock_config(&state)?;
    Ok(serde_json::json!({
        "success": true,
        "profiles": cfg.profiles,
        "active_id": cfg.active_profile_id
    }))
}

#[tauri::command]
pub fn save_profiles(state: State<'_, AppState>, profiles: Vec<serde_json::Value>, active_id: String) -> Result<serde_json::Value, String> {
    {
        let mut cfg = util::lock_config(&state)?;
        cfg.profiles = Some(profiles);
        cfg.active_profile_id = Some(active_id);
    }
    state.save_config()?;
    util::ok_true()
}

// ==================== Patches ====================

/// Папка preloading_plugins рядом с игрой (пустой путь, если game_path не задан).
fn plugins_dir(state: &AppState) -> PathBuf {
    util::game_dir(state)
        .map(|d| d.join("preloading_plugins"))
        .unwrap_or_default()
}

fn patches_json_path(state: &AppState) -> PathBuf {
    plugins_dir(state).join("#ArizonaPatches.json")
}

/// Python: версия DLL через win32api.GetFileVersionInfo → "1.0.0.0" | None.
#[cfg(windows)]
fn get_dll_version(path: &std::path::Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use winapi::um::winver::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};

    // Layout VS_FIXEDFILEINFO (winver.h) — определяем локально, чтобы не тянуть структуру winapi
    #[repr(C)]
    #[derive(Default)]
    struct FixedFileInfo {
        signature: u32,
        struc_version: u32,
        file_version_ms: u32,
        file_version_ls: u32,
        product_ms: u32,
        product_ls: u32,
        file_flags_mask: u32,
        file_flags: u32,
        file_os: u32,
        file_type: u32,
        file_subtype: u32,
        file_date_ms: u32,
        file_date_ls: u32,
    }

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let root: Vec<u16> = "\\".encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let mut handle = 0u32;
        let size = GetFileVersionInfoSizeW(wide.as_ptr(), &mut handle);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        if GetFileVersionInfoW(wide.as_ptr(), 0, size, buf.as_mut_ptr() as *mut _) == 0 {
            return None;
        }
        let mut out: *mut winapi::ctypes::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        if VerQueryValueW(buf.as_ptr() as *const _, root.as_ptr(), &mut out, &mut len) == 0
            || out.is_null()
        {
            return None;
        }
        let ffi = out as *const FixedFileInfo;
        let ms = (*ffi).file_version_ms;
        let ls = (*ffi).file_version_ls;
        Some(format!(
            "{}.{}.{}.{}",
            ms >> 16,
            ms & 0xFFFF,
            ls >> 16,
            ls & 0xFFFF
        ))
    }
}

#[cfg(not(windows))]
fn get_dll_version(_path: &std::path::Path) -> Option<String> {
    None
}

/// Python: _create_patches_backup — бэкап перед записью, максимум 15 штук.
fn create_patches_backup(state: &AppState, label: &str) -> Option<PathBuf> {
    let src = patches_json_path(state);
    if !src.exists() {
        return None;
    }
    let backup_dir = state.documents_path.join("backups");
    std::fs::create_dir_all(&backup_dir).ok()?;

    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let suffix = if label.is_empty() {
        String::new()
    } else {
        format!("_{}", label)
    };
    let dst = backup_dir.join(format!("patches_{}{}.json", ts, suffix));
    std::fs::copy(&src, &dst).ok()?;

    // Чистим старые бэкапы: максимум 15, сортировка по mtime
    const MAX_BACKUPS: usize = 15;
    let mut backups: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&backup_dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("patches_") && name.ends_with(".json") {
                if let Ok(md) = e.metadata() {
                    if let Ok(mtime) = md.modified() {
                        backups.push((mtime, e.path()));
                    }
                }
            }
        }
    }
    backups.sort_by_key(|(t, _)| *t);
    while backups.len() > MAX_BACKUPS {
        if let Some((_, p)) = backups.first() {
            std::fs::remove_file(p).ok();
        }
        backups.remove(0);
    }
    Some(dst)
}

#[tauri::command]
pub fn check_patches_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let files = check_patches_files_inner(&state);
    let installed_version = state
        .config
        .lock()
        .map(|c| c.patches_version.clone())
        .unwrap_or_default();
    let url_v2 = patches_download_url("v2");

    let both_exist = files
        .get("both_exist")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // 1. Missing: нет json ИЛИ нет dll
    if !both_exist {
        let plugins = plugins_dir(&state);
        let mut missing: Vec<&str> = Vec::new();
        if !plugins.join("#ArizonaPatches.json").exists() {
            missing.push("#ArizonaPatches.json");
        }
        if !plugins.join("#ArizonaPatches.dll").exists() {
            missing.push("#ArizonaPatches.dll");
        }
        return Ok(serde_json::json!({
            "status": "missing",
            "version": serde_json::Value::Null,
            "message": format!("Патчи недоступны: отсутствуют файлы: {}", missing.join(", ")),
            "message_detail": "Файлы патчей не найдены в папке preloading_plugins. Необходимо скачать патчи.",
            "can_update": true,
            "download_url": url_v2,
            "installed_version": serde_json::Value::Null
        }));
    }

    // 2. Версия DLL 1.0.0.0 — блокируем настройки и отключаем старый DLL
    let dll_version = files
        .get("dll_version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    if dll_version.as_deref() == Some("1.0.0.0") {
        if let Some(dll_path) = files.get("dll_path").and_then(|v| v.as_str()) {
            disable_old_dll(std::path::Path::new(dll_path));
        }
        return Ok(serde_json::json!({
            "status": "outdated_dll_v1",
            "version": "1.0.0.0",
            "message": "Патчи недоступны: устаревший ArizonaPatches.dll (1.0.0.0)",
            "message_detail": "Установлен старый ArizonaPatches.dll (1.0.0.0), он вызывает краши игры. Ждите выхода v2 от EIR — тогда кнопка «Скачать» станет активной.",
            "can_update": false,
            "download_url": serde_json::Value::Null,
            "installed_version": "1.0.0.0"
        }));
    }

    // 4. Версия не задана, но файлы на месте и DLL не 1.0.0.0
    if installed_version.is_none() {
        if dll_version.is_some() {
            return Ok(serde_json::json!({
                "status": "current",
                "version": "v2",
                "message": "Патчи актуальны",
                "message_detail": "Патчи установлены и работают корректно.",
                "can_update": false,
                "download_url": serde_json::Value::Null,
                "installed_version": "v2"
            }));
        }
        return Ok(serde_json::json!({
            "status": "missing",
            "version": serde_json::Value::Null,
            "message": "Патчи не установлены",
            "message_detail": "Патчи не были установлены через лаунчер. Необходимо скачать и установить патчи.",
            "can_update": true,
            "download_url": url_v2,
            "installed_version": serde_json::Value::Null
        }));
    }

    let installed = installed_version.unwrap();

    // 5. outdated_v1
    if installed == "v1" {
        return Ok(serde_json::json!({
            "status": "outdated_v1",
            "version": "v1",
            "message": "Патчи v1 не работают после глобальной обновы игры",
            "message_detail": "У вас установлены патчи v1 — они не работают после глобальной обновы игры (февраль 2026). EIR выпустил исправленную версию v2.",
            "can_update": true,
            "download_url": url_v2,
            "installed_version": "v1"
        }));
    }

    // current
    Ok(serde_json::json!({
        "status": "current",
        "version": installed,
        "message": "Патчи актуальны",
        "message_detail": "Патчи v2 установлены и работают корректно.",
        "can_update": false,
        "download_url": serde_json::Value::Null,
        "installed_version": installed
    }))
}

/// Python: _disable_old_dll — переименовывает старый DLL в .dll1.
fn disable_old_dll(dll_path: &std::path::Path) -> Option<PathBuf> {
    let s = dll_path.to_string_lossy().to_string();
    // Уже переименован? (.1, .1.1, .1.2, ...)
    let re = regex::Regex::new(r"\.1(\.\d+)?$").ok()?;
    if re.is_match(&s) {
        return None;
    }
    if !dll_path.exists() {
        return None;
    }
    let backup = std::path::PathBuf::from(format!("{}.1", s));
    if backup.exists() {
        std::fs::remove_file(&backup).ok();
    }
    std::fs::rename(dll_path, &backup).ok()?;
    log::info!("Old DLL renamed to .1: {}", backup.display());
    Some(backup)
}

fn patches_download_url(version: &str) -> String {
    format!(
        "https://raw.githubusercontent.com/worteng/ArizonaLauncher/main/patches/arizonapatches_{}.zip",
        version
    )
}

fn check_patches_files_inner(state: &AppState) -> serde_json::Value {
    if util::game_path(state).is_empty() {
        return serde_json::json!({
            "json_exists": false, "dll_exists": false, "dll_version": serde_json::Value::Null,
            "message": "game_path не установлен", "both_exist": false
        });
    }
    let plugins = plugins_dir(state);
    if !plugins.is_dir() {
        return serde_json::json!({
            "json_exists": false, "dll_exists": false, "dll_version": serde_json::Value::Null,
            "message": "preloading_plugins не найдена", "both_exist": false
        });
    }

    let json_path = plugins.join("#ArizonaPatches.json");
    let dll_path = plugins.join("#ArizonaPatches.dll");
    let json_exists = json_path.exists();

    // Ищем DLL: основной файл или любой бэкап (.1, .1.1, .1.2, etc.)
    let mut dll_candidates: Vec<PathBuf> = Vec::new();
    if dll_path.exists() {
        dll_candidates.push(dll_path.clone());
    }
    if let Ok(rd) = std::fs::read_dir(&plugins) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("#ArizonaPatches.dll") && name != "#ArizonaPatches.dll" {
                dll_candidates.push(e.path());
            }
        }
    }

    let mut dll_exists = false;
    let mut dll_version: Option<String> = None;
    let mut dll_path_checked: Option<PathBuf> = None;
    for cand in dll_candidates {
        if let Some(ver) = get_dll_version(&cand) {
            dll_exists = true;
            dll_version = Some(ver);
            dll_path_checked = Some(cand);
            break;
        }
    }

    serde_json::json!({
        "json_exists": json_exists,
        "dll_exists": dll_exists,
        "dll_version": dll_version,
        "json_path": if json_exists { serde_json::json!(json_path.to_string_lossy()) } else { serde_json::Value::Null },
        "dll_path": dll_path_checked.map(|p| serde_json::json!(p.to_string_lossy())).unwrap_or(serde_json::Value::Null),
        "plugins_dir": dll_path.parent().map(|p| serde_json::json!(p.to_string_lossy())).unwrap_or(serde_json::Value::Null),
        "both_exist": json_exists && dll_exists
    })
}

#[tauri::command]
pub fn check_patches_files(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    Ok(check_patches_files_inner(&state))
}

#[tauri::command]
pub fn read_patches(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let path = patches_json_path(&state);
    let path_str = path.to_string_lossy().to_string();
    if path_str.is_empty() {
        return Ok(serde_json::json!({"success": false, "message": "patches_path пустой", "path": ""}));
    }
    if !path.exists() {
        return Ok(serde_json::json!({"success": false, "message": format!("Файл не найден: {}", path_str), "path": path_str}));
    }
    match util::read_patches_file(&path) {
        Ok(data) => {
            let keys_count = data.as_object().map(|o| o.len()).unwrap_or(0);
            Ok(serde_json::json!({"success": true, "data": data, "path": path_str, "keys_count": keys_count}))
        }
        Err(e) => Ok(serde_json::json!({"success": false, "message": e, "path": path_str})),
    }
}

#[tauri::command]
pub fn write_patches(state: State<'_, AppState>, data: serde_json::Value) -> Result<serde_json::Value, String> {
    Ok(write_patches_inner(&state, &data))
}

#[tauri::command]
pub fn list_patches_backups(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let backup_dir = state.documents_path.join("backups");
    if !backup_dir.exists() {
        return Ok(serde_json::json!([]));
    }
    let mut items: Vec<(std::time::SystemTime, serde_json::Value)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&backup_dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !(name.starts_with("patches_") && name.ends_with(".json")) {
                continue;
            }
            let Ok(md) = e.metadata() else { continue };
            let Ok(mtime) = md.modified() else { continue };
            let date = chrono::DateTime::<chrono::Local>::from(mtime)
                .format("%d.%m.%Y %H:%M:%S")
                .to_string();
            items.push((
                mtime,
                serde_json::json!({
                    "filename": name,
                    "path": e.path().to_string_lossy(),
                    "date": date,
                    "size": md.len(),
                }),
            ));
        }
    }
    // Новые первыми
    items.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(serde_json::Value::Array(items.into_iter().map(|(_, v)| v).collect()))
}

#[tauri::command]
pub fn restore_patches_backup(state: State<'_, AppState>, filename: String) -> Result<serde_json::Value, String> {
    let backup_path = match util::safe_backup_path(&state, &filename) {
        Ok(p) => p,
        Err(m) => return util::fail_message(m),
    };
    if !backup_path.exists() {
        return Ok(serde_json::json!({"success": false, "message": format!("Бэкап не найден: {}", filename)}));
    }
    let data = match util::read_patches_file(&backup_path) {
        Ok(d) => d,
        Err(e) => return util::fail_message(e),
    };

    // Бэкап текущего перед восстановлением
    let patches_path = patches_json_path(&state);
    if patches_path.exists() {
        create_patches_backup(&state, "before_restore");
    }
    let result = write_patches_inner(&state, &data);
    if result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
        log::info!("restore_patches_backup: восстановлен {}", filename);
        return Ok(serde_json::json!({"success": true, "data": data, "message": format!("Восстановлен: {}", filename)}));
    }
    Ok(result)
}

/// Общая тело записи патчей (используется и write_patches, и restore).
fn write_patches_inner(state: &AppState, data: &serde_json::Value) -> serde_json::Value {
    let path = patches_json_path(state);
    if path.as_os_str().is_empty() {
        return serde_json::json!({"success": false, "message": "Path not set"});
    }
    if path.exists() {
        create_patches_backup(state, "");
    }
    let mut buf: Vec<u8> = Vec::new();
    {
        use serde::Serialize;
        let mut ser = serde_json::Serializer::with_formatter(
            &mut buf,
            serde_json::ser::PrettyFormatter::with_indent(b"    "),
        );
        if let Err(e) = data.serialize(&mut ser) {
            return serde_json::json!({"success": false, "message": e.to_string()});
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    match std::fs::write(&path, &buf) {
        Ok(_) => serde_json::json!({"success": true, "message": "Saved"}),
        Err(e) => serde_json::json!({"success": false, "message": e.to_string()}),
    }
}

#[tauri::command]
pub fn delete_patches_backup(state: State<'_, AppState>, filename: String) -> Result<serde_json::Value, String> {
    let backup_path = match util::safe_backup_path(&state, &filename) {
        Ok(p) => p,
        Err(m) => return util::fail_message(m),
    };
    if !backup_path.exists() {
        return Ok(serde_json::json!({"success": false, "message": "Файл не найден"}));
    }
    match std::fs::remove_file(&backup_path) {
        Ok(_) => util::ok_true(),
        Err(e) => Ok(serde_json::json!({"success": false, "message": e.to_string()})),
    }
}

#[tauri::command]
pub fn preview_patches_backup(state: State<'_, AppState>, filename: String) -> Result<serde_json::Value, String> {
    let backup_path = match util::safe_backup_path(&state, &filename) {
        Ok(p) => p,
        Err(m) => return util::fail_message(m),
    };
    if !backup_path.exists() {
        return Ok(serde_json::json!({"success": false, "message": "Файл не найден"}));
    }
    let size = std::fs::metadata(&backup_path).map(|m| m.len()).unwrap_or(0);
    let data = match util::read_patches_file(&backup_path) {
        Ok(d) => d,
        Err(_) => return Ok(serde_json::json!({"success": false, "message": "Файл повреждён (не JSON)"})),
    };

    // Бэкап может быть плоским (bool-значения сверху) или вложенным ({"patches": {...}})
    let obj = data.as_object();
    let is_nested = obj
        .map(|o| o.get("patches").map(|p| p.is_object()).unwrap_or(false))
        .unwrap_or(false);
    let mut patches: Vec<(String, bool)> = Vec::new();
    let mut launch_params = serde_json::json!({});
    if is_nested {
        let o = obj.unwrap();
        for (k, v) in o["patches"].as_object().unwrap() {
            patches.push((k.clone(), v.as_bool().unwrap_or(false)));
        }
        launch_params = o.get("launch_params").cloned().unwrap_or_else(|| serde_json::json!({}));
    } else if let Some(o) = obj {
        for (k, v) in o {
            if let Some(b) = v.as_bool() {
                patches.push((k.clone(), b));
            } else {
                launch_params[k] = v.clone();
            }
        }
    }
    let enabled: Vec<&String> = patches.iter().filter(|(_, b)| *b).map(|(k, _)| k).collect();
    let disabled: Vec<&String> = patches.iter().filter(|(_, b)| !*b).map(|(k, _)| k).collect();

    let r#type = if filename.contains("before_import") {
        "before_import"
    } else if filename.contains("before_restore") {
        "before_restore"
    } else {
        "save"
    };

    Ok(serde_json::json!({
        "success": true,
        "filename": filename,
        "type": r#type,
        "size": size,
        "patches_total": patches.len(),
        "patches_enabled": enabled,
        "patches_disabled": disabled,
        "launch_params": launch_params,
        "saved_at": obj.and_then(|o| o.get("saved_at")).cloned().unwrap_or_else(|| serde_json::json!("")),
    }))
}

#[tauri::command]
pub async fn download_and_install_patches(version: Option<String>) -> Result<serde_json::Value, String> {
    // TODO
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[tauri::command]
pub async fn get_patches_latest_version() -> Result<serde_json::Value, String> {
    // Python: чтение patches/version.txt с GitHub
    let url = "https://raw.githubusercontent.com/worteng/ArizonaLauncher/main/patches/version.txt";
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static("Mozilla/5.0"),
    );
    let resp = match util::http_request(
        reqwest::Method::GET,
        url,
        headers,
        std::time::Duration::from_secs(15),
    )
    .await
    {
        Ok(r) => r,
        Err(e) => return Ok(serde_json::json!({"success": false, "message": e.to_string()})),
    };
    let status = resp.status();
    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => return Ok(serde_json::json!({"success": false, "message": e.to_string()})),
    };
    let version = util::decode_text_bytes(&bytes).trim().to_string();
    if status == reqwest::StatusCode::OK && !version.is_empty() && version.starts_with('v') {
        return Ok(serde_json::json!({
            "success": true,
            "version": version,
            "download_url": patches_download_url(&version)
        }));
    }
    Ok(serde_json::json!({"success": false, "message": format!("HTTP {}", status)}))
}

#[tauri::command]
pub async fn check_patches_available(version: Option<String>) -> Result<serde_json::Value, String> {
    let version = version.unwrap_or_else(|| "v2".to_string());
    let url = patches_download_url(&version);
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static("Mozilla/5.0"),
    );
    match util::http_request(
        reqwest::Method::HEAD,
        &url,
        headers,
        std::time::Duration::from_secs(10),
    )
    .await
    {
        Ok(resp) => {
            if resp.status() == reqwest::StatusCode::OK {
                Ok(serde_json::json!({"success": true, "available": true, "url": url, "version": version}))
            } else {
                Ok(serde_json::json!({"success": true, "available": false,
                    "message": format!("Патчи {} пока недоступны. Следите за обновлениями!", version)}))
            }
        }
        Err(e) => {
            log::warn!("check_patches_available error: {e}");
            Ok(serde_json::json!({"success": true, "available": false,
                "message": "Не удалось проверить доступность. Проверьте подключение к интернету."}))
        }
    }
}

#[tauri::command]
pub async fn check_patches_update_status() -> Result<serde_json::Value, String> {
    // Python: манифест updates/patches.json на GitHub
    let url = "https://raw.githubusercontent.com/worteng/ArizonaLauncher/main/updates/patches.json";
    let resp = match util::http_request(
        reqwest::Method::GET,
        url,
        reqwest::header::HeaderMap::new(),
        std::time::Duration::from_secs(15),
    )
    .await
    {
        Ok(r) => r,
        Err(e) => return Ok(serde_json::json!({"success": false, "message": e.to_string()})),
    };
    if resp.status() != reqwest::StatusCode::OK {
        return Ok(serde_json::json!({"success": false, "message": format!("HTTP {}", resp.status())}));
    }
    let text = match resp.text().await {
        Ok(t) => t,
        Err(e) => return Ok(serde_json::json!({"success": false, "message": e.to_string()})),
    };
    let data: serde_json::Value = match serde_json::from_str(text.trim()) {
        Ok(d) => d,
        Err(_) => return Ok(serde_json::json!({"success": false, "message": "Некорректный манифест"})),
    };
    Ok(serde_json::json!({
        "success": true,
        "released": data.get("released").and_then(|v| v.as_bool()).unwrap_or(false),
        "latest_version": data.get("latest_version").map(|v| v.to_string().trim_matches('"').to_string()).unwrap_or_default(),
        "notes": data.get("notes").map(|v| v.to_string().trim_matches('"').to_string()).unwrap_or_default(),
    }))
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
    util::ok_true()
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
