use crate::config::{AppState, LauncherConfig};
use crate::util;
use std::path::PathBuf;
use tauri::{Emitter, Manager, State};

// ==================== Config Commands ====================

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = util::lock_config(&state)?;
    // Достаточно папки игры: лаунчер Air не обязателен — при нативном запуске
    // он не используется, а при Air его отсутствие покажет кнопка «Установить».
    let paths_ok = !cfg.game_path.is_empty()
        && std::path::Path::new(&cfg.game_path).exists();
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
    // Достаточно папки игры: лаунчер Air не обязателен — при нативном запуске
    // он не используется, а при Air его отсутствие покажет кнопка «Установить».
    let paths_ok = !cfg.game_path.is_empty();
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
    // Достаточно папки игры (см. get_saved_data)
    let paths_ok = !cfg.game_path.is_empty();
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

// ==================== IPC-граница фронт ↔ бэк ====================
//
// Фронт (и легаси-версия на Python) говорит в `snake_case`, а структуры
// конфига помечены `#[serde(rename_all = "camelCase")]`. Без конвертации:
//   * `update_launch_params` падал с «missing field `textureMode`» —
//     параметры запуска не сохранялись вообще;
//   * `save_launcher_settings` молча обнулял поля (неизвестные ключи
//     serde игнорирует) — например, выбранные обои не переживали рестарт.
const IPC_KEY_PAIRS: &[(&str, &str)] = &[
    ("texture_mode", "textureMode"),
    ("color_depth_16", "colorDepth16"),
    ("allow_hdr", "allowHdr"),
    ("enable_grass", "enableGrass"),
    ("auth_cef_enable", "authCefEnable"),
    ("window_mode", "windowMode"),
    ("enable_new_grass", "enableNewGrass"),
    ("old_window", "oldWindow"),
    ("use_d3dx9_43", "useD3dx943"),
    ("show_dialog_ids", "showDialogIds"),
    ("modern_scale", "modernScale"),
    ("trees_new", "treesNew"),
    ("bg_image", "bgImage"),
    ("window_title", "windowTitle"),
    ("window_size", "windowSize"),
    ("window_frame", "windowFrame"),
    ("layout_order", "layoutOrder"),
];

fn map_ipc_keys(value: serde_json::Value, to_camel: bool) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, val)| {
                    let renamed = IPC_KEY_PAIRS
                        .iter()
                        .find(|(snake, camel)| if to_camel { *snake == key.as_str() } else { *camel == key.as_str() })
                        .map(|(snake, camel)| if to_camel { (*camel).to_string() } else { (*snake).to_string() })
                        .unwrap_or(key);
                    (renamed, map_ipc_keys(val, to_camel))
                })
                .collect(),
        ),
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(|v| map_ipc_keys(v, to_camel)).collect())
        }
        other => other,
    }
}

/// Ключи из фронта (snake_case) → в формат структур конфига (camelCase).
fn ipc_keys_to_camel(value: serde_json::Value) -> serde_json::Value {
    map_ipc_keys(value, true)
}

/// Ключи из структур конфига (camelCase) → в формат фронта (snake_case).
fn ipc_keys_to_snake(value: serde_json::Value) -> serde_json::Value {
    map_ipc_keys(value, false)
}

#[tauri::command]
pub fn update_launch_params(state: State<'_, AppState>, params: serde_json::Value) -> Result<serde_json::Value, String> {
    util::update_config(&state, |cfg| {
        cfg.launch_params =
            serde_json::from_value(ipc_keys_to_camel(params)).map_err(|e| e.to_string())?;
        Ok(())
    })
}

#[tauri::command]
pub fn read_launch_params(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = util::lock_config(&state)?;
    let value = serde_json::to_value(&cfg.launch_params).map_err(|e| e.to_string())?;
    Ok(ipc_keys_to_snake(value))
}

/// Способ запуска: "native" (напрямую gta_sa.exe) или "air" (через Air-лаунчер).
#[tauri::command]
pub fn get_launch_method(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = util::lock_config(&state)?;
    Ok(serde_json::json!({"success": true, "method": cfg.launch_method}))
}

#[tauri::command]
pub fn set_launch_method(state: State<'_, AppState>, method: String) -> Result<serde_json::Value, String> {
    if method != "native" && method != "air" {
        return Ok(serde_json::json!({"success": false, "message": format!("Неизвестный способ запуска: {method}")}));
    }
    util::update_config(&state, |cfg| {
        cfg.launch_method = method.clone();
        Ok(())
    })
}

// ==================== Game Path Commands ====================

/// Ищет exe лаунчера в папке по ресурсу версии (не по имени файла):
/// подойдёт любой exe, у которого ProductName/OriginalFilename указывает
/// на лаунчер Arizona. Если их несколько — возвращает самый новый.
fn find_launcher_exe(folder: &std::path::Path) -> Option<(PathBuf, u32)> {
    let entries = std::fs::read_dir(folder).ok()?;
    let mut best: Option<(PathBuf, u32)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        let is_exe = path
            .extension()
            .map(|e| e.to_string_lossy().eq_ignore_ascii_case("exe"))
            .unwrap_or(false);
        if !is_exe || !util::is_arizona_launcher(&path) {
            continue;
        }
        let major = util::exe_major_version(&path);
        if best.as_ref().map(|(_, m)| major > *m).unwrap_or(true) {
            best = Some((path, major));
        }
    }
    best
}

/// Мажорная версия лаунчера → "v6" / "v7" / "v8" (неизвестное и новее 8 → "v8").
fn major_to_version(major: u32) -> &'static str {
    match major {
        6 => "v6",
        7 => "v7",
        _ => "v8",
    }
}

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
            if let Some((launcher, _)) = find_launcher_exe(&game_dir) {
                cfg.launcher_path = launcher.to_string_lossy().to_string();
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

/// Нативный диалог выбора папки игры: проверяем gta_sa.exe, сохраняем пути,
/// возвращаем {success, message, folder, install_info} — как Python.
#[tauri::command]
pub async fn select_game_path(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    use tauri_plugin_dialog::DialogExt;

    let picked = app
        .dialog()
        .file()
        .set_title("Выберите папку с игрой (где находится gta_sa.exe)")
        .blocking_pick_folder();

    let Some(picked) = picked else {
        return util::fail_message("Папка не выбрана");
    };
    let folder = match picked {
        tauri_plugin_dialog::FilePath::Path(p) => p,
        tauri_plugin_dialog::FilePath::Url(u) => u
            .to_file_path()
            .map_err(|_| "Некорректный путь".to_string())?,
    };
    let folder_str = folder.to_string_lossy().to_string();

    let game_exe = folder.join("gta_sa.exe");
    if !game_exe.exists() {
        return util::fail_message("gta_sa.exe не найден в выбранной папке");
    }

    // Сохраняем пути: лаунчер ищем по ресурсу версии среди всех exe папки
    let launcher_exe = find_launcher_exe(&folder)
        .map(|(p, _)| p.to_string_lossy().to_string())
        .unwrap_or_default();
    util::update_config(&state, |cfg| {
        cfg.game_path = game_exe.to_string_lossy().to_string();
        cfg.launcher_path = launcher_exe;
        Ok(())
    })?;

    let install_info = get_install_action(state, folder_str.clone())?;
    Ok(serde_json::json!({
        "success": true,
        "message": format!("Путь установлен: {}", folder_str),
        "folder": folder_str,
        "install_info": install_info
    }))
}

// ==================== Game Launch ====================

/// Python `get_launcher_version`: v8 > v7 > v6 среди exe в папке игры
/// (по ресурсу версии, не по имени файла), иначе v8.
fn detect_launcher_version(state: &AppState) -> String {
    let Some(game_dir) = util::game_dir(state) else {
        return "v8".to_string();
    };
    let plugins_dir = game_dir.join("preloading_plugins");
    let has_plugins = std::fs::read_dir(&plugins_dir)
        .map(|mut rd| rd.next().is_some())
        .unwrap_or(false);
    if has_plugins {
        if let Some((_, major)) = find_launcher_exe(&game_dir) {
            return major_to_version(major).to_string();
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

/// Паритет с оригиналом (03 §1, 04 §1): при запуске игры пишем ник и память
/// в HKCU\Software\SAMP — тот же ключ, который читает сам клиент SA-MP.
/// `cdn` не пишем: его DWORD-семантика в разборе лаунчера «не проверена».
fn write_samp_registry(nickname: &str, mem_mb: u32) {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let Ok((key, _)) = RegKey::predef(HKEY_CURRENT_USER).create_subkey("Software\\SAMP") else {
        log::warn!("write_samp_registry: не удалось открыть HKCU\\Software\\SAMP");
        return;
    };
    if let Err(e) = key.set_value("PlayerName", &nickname) {
        log::warn!("write_samp_registry: PlayerName: {e}");
    }
    if let Err(e) = key.set_value("mem", &mem_mb) {
        log::warn!("write_samp_registry: mem: {e}");
    }
}

/// Собирает аргументы запуска. База одинакова, хвост отличается
/// (03 §3 — так командную строку получает игра от Air-ретранслятора):
///   нативно:  ... -cdn <n,n,n> -arizona [доп. флаги]
///   через Air: ... -arizona [доп. флаги] -cdn <n,n,n>  (порядок легаси worteng)
fn build_launch_args(
    native: bool,
    is_v7plus: bool,
    srv_ip: &str,
    srv_port: &str,
    nick: &str,
    params: &crate::config::LaunchParams,
) -> Vec<String> {
    let cdn_value = validate_cdn(&params.cdn);
    let mut cmd: Vec<String> = vec![
        "-c".to_string(),
        "-h".to_string(),
        srv_ip.to_string(),
        "-p".to_string(),
        srv_port.to_string(),
        "-mem".to_string(),
        params.memory.to_string(),
        "-n".to_string(),
        nick.to_string(),
    ];
    if native {
        cmd.push("-cdn".to_string());
        cmd.push(cdn_value.clone());
    }
    cmd.push("-arizona".to_string());

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

    if !native {
        cmd.push("-cdn".to_string());
        cmd.push(cdn_value);
    }
    cmd
}

#[cfg(test)]
mod launch_tests {
    use super::*;

    /// Дефолтные параметры: autologin → "-x", window_mode → "-window".
    #[test]
    fn native_args_order() {
        let p = crate::config::LaunchParams::default();
        let args = build_launch_args(true, true, "host.example", "7777", "Nick", &p);
        let got: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            got,
            vec![
                "-c", "-h", "host.example", "-p", "7777", "-mem", "4096", "-n", "Nick",
                "-cdn", "1,1,1", "-arizona", "-x", "-window"
            ]
        );
    }

    #[test]
    fn air_args_order() {
        let p = crate::config::LaunchParams::default();
        let args = build_launch_args(false, true, "host.example", "7777", "Nick", &p);
        let got: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            got,
            vec![
                "-c", "-h", "host.example", "-p", "7777", "-mem", "4096", "-n", "Nick",
                "-arizona", "-x", "-window", "-cdn", "1,1,1"
            ]
        );
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
    let (launcher_path, game_path, launch_method) = {
        let cfg = util::lock_config(&state)?;
        (
            cfg.launcher_path.clone(),
            cfg.game_path.clone(),
            cfg.launch_method.clone(),
        )
    };
    // "native" — запускаем gta_sa.exe напрямую; "air" — через ArizonaLauncher*.exe
    let native = launch_method != "air";
    if !native && (launcher_path.is_empty() || !std::path::Path::new(&launcher_path).exists()) {
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
    // При нативном запуске поддержка только v8 → все доп. флаги без проверок
    let is_v7plus = native || launcher_ver == "v7" || launcher_ver == "v8";
    let cmd = build_launch_args(native, is_v7plus, &srv_ip, &srv_port, &nickname, &params);

    // kill_all_launchers перед стартом (в Python — синхронно с sleep 1s)
    tokio::task::spawn_blocking(kill_all_launchers).await.ok();

    let (exe, cwd) = if native {
        // Реестр — паритет с оригиналом (03 §1, 04): SA-MP читает оттуда ник
        write_samp_registry(&nickname, params.memory);
        let game = std::path::PathBuf::from(&game_path);
        let dir = game
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        (game, dir)
    } else {
        let exe = std::path::PathBuf::from(&launcher_path);
        let cwd = exe
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        (exe, cwd)
    };
    log::info!(
        "start_game[{}]: {:?} {:?}",
        if native { "native" } else { "air" },
        exe,
        cmd
    );

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

/// `consume_exit: false` — «съесть» edge выхода нельзя (так опрашивает
/// статус-бар); edge тогда останется для GTA-watcher'а, который по нему
/// разворачивает окно лаунчера после закрытия игры.
#[tauri::command]
pub fn is_gta_running(consume_exit: Option<bool>) -> Result<serde_json::Value, String> {
    static GTA_WAS_RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let running = sys
        .processes()
        .values()
        .any(|p| p.name().to_string_lossy().eq_ignore_ascii_case("gta_sa.exe"));
    // was_running: игра была запущена на прошлый вызов, а сейчас вышла —
    // фронт по этому флагу возвращает окно лаунчера после выхода из GTA.
    if consume_exit.unwrap_or(true) {
        let was_running = GTA_WAS_RUNNING.swap(running, std::sync::atomic::Ordering::Relaxed);
        Ok(serde_json::json!({"running": running, "was_running": was_running && !running}))
    } else {
        let was_running = GTA_WAS_RUNNING.load(std::sync::atomic::Ordering::Relaxed);
        Ok(serde_json::json!({"running": running, "was_running": was_running && !running}))
    }
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

// ==================== Disk cache (stale-while-revalidate) ====================
// Дисковый кэш переживает перезапуск: на старте отдаём его мгновенно,
// а свежие данные тянем фоном. Иначе каждый запуск ждёт 160-700ms сети.

fn disk_cache_path(state: &AppState, name: &str) -> PathBuf {
    state.documents_path.join("cache").join(name)
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Атомарная запись кэш-файла: tmp → rename (паттерн save_config).
fn write_cache_file(state: &AppState, name: &str, payload: &serde_json::Value) {
    let dir = state.documents_path.join("cache");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join(name);
    let tmp = dir.join(format!("{name}.tmp"));
    let Ok(json) = serde_json::to_string(payload) else { return };
    if std::fs::write(&tmp, json).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

fn read_cache_file(state: &AppState, name: &str) -> Option<serde_json::Value> {
    let raw = std::fs::read_to_string(disk_cache_path(state, name)).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Дисковый кэш серверов: (servers, etag, last_modified).
fn read_disk_servers(state: &AppState) -> Option<(Vec<serde_json::Value>, Option<String>, Option<String>)> {
    let v = read_cache_file(state, "servers.json")?;
    let servers = v.get("servers")?.as_array()?.clone();
    let etag = v.get("etag").and_then(|x| x.as_str()).map(str::to_string);
    let lm = v.get("last_modified").and_then(|x| x.as_str()).map(str::to_string);
    Some((servers, etag, lm))
}

fn write_disk_servers(state: &AppState, servers: &[serde_json::Value], etag: Option<&str>, last_modified: Option<&str>) {
    write_cache_file(
        state,
        "servers.json",
        &serde_json::json!({
            "ts_ms": now_ms(),
            "etag": etag,
            "last_modified": last_modified,
            "servers": servers,
        }),
    );
}

fn read_disk_news(state: &AppState) -> Option<String> {
    let v = read_cache_file(state, "news.json")?;
    v.get("text").and_then(|x| x.as_str()).map(str::to_string)
}

fn write_disk_news(state: &AppState, text: &str) {
    write_cache_file(state, "news.json", &serde_json::json!({ "ts_ms": now_ms(), "text": text }));
}

/// Обновить серверы в фоне — не более одной задачи за раз.
fn spawn_servers_revalidate(app: &tauri::AppHandle) {
    use std::sync::atomic::Ordering;
    if app.state::<AppState>().servers_revalidating.swap(true, Ordering::SeqCst) {
        return; // уже обновляется
    }
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let st = app2.state::<AppState>();
        fetch_servers(&st).await;
        st.servers_revalidating.store(false, Ordering::SeqCst);
    });
}

/// Обновить новости в фоне — не более одной задачи за раз и не чаще
/// раза в 60s (частые вызовы команды не должны сыпать запросами в GitHub).
fn spawn_news_revalidate(app: &tauri::AppHandle) {
    use std::sync::atomic::Ordering;
    if app.state::<AppState>().news_revalidating.swap(true, Ordering::SeqCst) {
        return;
    }
    // Диск свежий (<60s) — обновлять нечего
    if let Some(v) = read_cache_file(&app.state::<AppState>(), "news.json") {
        let ts = v.get("ts_ms").and_then(|x| x.as_u64()).unwrap_or(0u64);
        let age = now_ms().saturating_sub(ts as u128);
        if age < 60_000 {
            app.state::<AppState>().news_revalidating.store(false, Ordering::SeqCst);
            return;
        }
    }
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let st = app2.state::<AppState>();
        let r = fetch_launcher_news().await;
        if r.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
            if let Some(text) = r.get("text").and_then(|v| v.as_str()) {
                write_disk_news(&st, text);
            }
        }
        st.news_revalidating.store(false, Ordering::SeqCst);
    });
}

/// stale-while-revalidate для серверов:
/// - память есть (TTL 60s) → обычная логика fetch_servers;
/// - памяти нет, но есть диск → отдаём диск мгновенно, фоном fetch_servers;
/// - ничего нет (первый запуск) → ждём сеть, как раньше.
async fn fetch_servers_sw(app: &tauri::AppHandle, state: &AppState) -> serde_json::Value {
    let memory_empty = {
        let guard = match state.servers_cache.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        guard.is_none()
    };
    log::info!("fetch_servers_sw: memory_empty={memory_empty}");

    if !memory_empty {
        return fetch_servers(state).await;
    }
    if let Some((servers, _, _)) = read_disk_servers(state) {
        spawn_servers_revalidate(app);
        log::info!("servers: дисковый кэш отдан мгновенно ({} серверов)", servers.len());
        return serde_json::Value::Array(servers);
    }
    log::info!("servers: ни памяти, ни диска — сетевой запрос");
    fetch_servers(state).await
}

/// stale-while-revalidate для новостей: диск → сразу + фоновое обновление,
/// иначе — сеть (первый запуск).
async fn fetch_news_sw(app: &tauri::AppHandle, state: &AppState) -> serde_json::Value {
    if let Some(text) = read_disk_news(state) {
        spawn_news_revalidate(app);
        log::info!("news: дисковый кэш отдан мгновенно ({} байт)", text.len());
        return serde_json::json!({"success": true, "text": text});
    }
    let r = fetch_launcher_news().await;
    if r.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
        if let Some(text) = r.get("text").and_then(|v| v.as_str()) {
            write_disk_news(state, text);
        }
    }
    r
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

    log::info!("fetch_servers: fresh={fresh}, has_cached_data={}", cached_data.is_some());

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
            etag: etag_new.clone(),
            last_modified: lm_new.clone(),
        });
    }
    // Дублируем на диск — переживёт перезапуск (мгновенный старт)
    write_disk_servers(state, &servers, etag_new.as_deref(), lm_new.as_deref());

    // Предзагружаем иконки в фоне
    let spawn_servers = servers.clone();
    tauri::async_runtime::spawn(async move {
        prefetch_icons(spawn_servers).await;
    });

    serde_json::Value::Array(servers)
}

#[tauri::command]
pub async fn get_servers(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    Ok(fetch_servers_sw(&app, &state).await)
}

#[tauri::command]
pub async fn get_servers_and_news(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let (servers, news) = tokio::join!(fetch_servers_sw(&app, &state), fetch_news_sw(&app, &state));
    Ok(serde_json::json!({
        "success": true,
        "servers": if servers.is_null() { serde_json::json!([]) } else { servers },
        "news": news,
    }))
}

/// Тело `get_launcher_news`: 3 fallback URL на GitHub, utf-8 → windows-1251.
async fn fetch_launcher_news() -> serde_json::Value {
    let urls = [
        "https://raw.githubusercontent.com/halfik24/ArzLaunchRepo/main/updatenews.txt",
        "https://raw.githubusercontent.com/halfik24/ArzLaunchRepo/refs/heads/main/updatenews.txt",
        "https://github.com/halfik24/ArzLaunchRepo/raw/main/updatenews.txt",
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
pub async fn get_launcher_news(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    Ok(fetch_news_sw(&app, &state).await)
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

/// Python `get_install_action`, но лаунчер определяется по ресурсу версии среди
/// всех exe папки (не по имени файла):
/// - action: 'none' | 'choose_version' | 'install_launcher_only'
///           | 'install_preloading_only' | 'upgrade_to_v8' | 'error'
///
/// Чистая файловая логика (без учёта способа запуска) — используется и
/// `start_launcher_install` для проверки «уже установлен?».
fn install_action_for_folder(folder_path: &str) -> Result<serde_json::Value, String> {
    let folder = std::path::Path::new(folder_path);
    if !folder.join("gta_sa.exe").exists() {
        return Ok(serde_json::json!({"action": "error", "message": "gta_sa.exe не найден"}));
    }
    // Что реально лежит в папке: найденный лаунчер и его версия
    let launcher = find_launcher_exe(folder); // Option<(PathBuf, major)>
    let has_launcher = launcher.is_some();
    let launcher_version: Option<&'static str> =
        launcher.as_ref().map(|(_, major)| major_to_version(*major));
    let has_v8 = launcher_version == Some("v8");
    let plugins = folder.join("preloading_plugins");
    let has_preloading = plugins.is_dir()
        && std::fs::read_dir(&plugins)
            .map(|mut rd| rd.next().is_some())
            .unwrap_or(false);

    // 1. preloading + v8 → ничего делать не надо
    if has_preloading && has_v8 {
        return Ok(serde_json::json!({
            "action": "none",
            "message": "Лаунчер v8 уже установлен",
            "has_launcher": true,
            "has_preloading": true,
            "launcher_version": "v8"
        }));
    }
    // 2/3. preloading + старый лаунчер (v7/v6) → обновление до v8
    if has_preloading && has_launcher && !has_v8 {
        return Ok(serde_json::json!({
            "action": "upgrade_to_v8",
            "message": "Доступно обновление до v8",
            "has_launcher": true,
            "has_preloading": true,
            "launcher_version": launcher_version
        }));
    }
    // 4. нет preloading, но лаунчер есть
    if !has_preloading && has_launcher {
        if has_v8 {
            // Для v8 ставку preloading решает удалённый флаг патчера
            return Ok(serde_json::json!({
                "action": "none",
                "message": "Лаунчер v8 уже установлен",
                "has_launcher": true,
                "has_preloading": false,
                "launcher_version": "v8"
            }));
        }
        return Ok(serde_json::json!({
            "action": "install_preloading_only",
            "message": "Требуется установка папки preloading_plugins",
            "has_launcher": true,
            "has_preloading": false,
            "launcher_version": launcher_version
        }));
    }
    // 5. preloading есть, лаунчера нет
    if has_preloading && !has_launcher {
        return Ok(serde_json::json!({
            "action": "install_launcher_only",
            "message": "Требуется установка лаунчера v8",
            "has_launcher": false,
            "has_preloading": true,
            "launcher_version": null
        }));
    }
    // 6. Полная установка: нет ни лаунчера, ни preloading_plugins
    Ok(serde_json::json!({
        "action": "choose_version",
        "message": "Выберите версию лаунчера для установки",
        "has_launcher": false,
        "has_preloading": false,
        "launcher_version": null
    }))
}

/// Команда для фронта: какое действие по установке требуется с учётом
/// способа запуска. При нативном запуске лаунчер Air и preloading не нужны —
/// всегда 'none', чтобы не пилить «Установить» без причины.
#[tauri::command]
pub fn get_install_action(state: State<'_, AppState>, folder_path: String) -> Result<serde_json::Value, String> {
    let method = { util::lock_config(&state)?.launch_method.clone() };
    if method != "air" {
        return Ok(serde_json::json!({
            "action": "none",
            "message": "Нативный запуск — лаунчер Air не требуется",
            "launch_method": "native"
        }));
    }
    install_action_for_folder(&folder_path)
}

/// API-обёртка для JS: {success, install_info} (как в Python get_install_action_api)
#[tauri::command]
pub fn get_install_action_api(state: State<'_, AppState>, folder_path: String) -> Result<serde_json::Value, String> {
    match get_install_action(state, folder_path) {
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
            "app_version": format!("v{}", env!("CARGO_PKG_VERSION")),
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
    let path = util::log_file_path();
    if !path.exists() {
        return util::fail_message("Лог-файл ещё не создан — запустите лаунчер и повторите");
    }
    open::that(&path).map_err(|e| e.to_string())?;
    util::ok_true()
}

/// Открыть ссылку во внешнем браузере (новости, релизы, Telegram).
/// WebView2 не даёт фронту `window.open`, поэтому это делает бэкенд.
#[tauri::command]
pub fn open_url(url: String) -> Result<serde_json::Value, String> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Ok(serde_json::json!({"success": false, "message": "Недопустимый адрес"}));
    }
    open::that(url).map_err(|e| e.to_string())?;
    util::ok_true()
}

// ==================== Settings ====================

#[tauri::command]
pub fn get_launcher_settings(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = util::lock_config(&state)?;
    let data = serde_json::to_value(&cfg.launcher_settings).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"success": true, "data": ipc_keys_to_snake(data)}))
}

#[tauri::command]
pub fn save_launcher_settings(state: State<'_, AppState>, settings: serde_json::Value) -> Result<serde_json::Value, String> {
    {
        let mut cfg = util::lock_config(&state)?;
        cfg.launcher_settings =
            serde_json::from_value(ipc_keys_to_camel(settings)).map_err(|e| e.to_string())?;
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

// ==================== Рамка окна (system / custom) ====================

/// Windows 11: скруглить углы окна (DWMWA_WINDOW_CORNER_PREFERENCE = 33,
/// DWMWCP_ROUND = 2). На Windows 10 вызов молча не сработает — ок.
#[cfg(windows)]
pub fn apply_rounded_corners(window: &tauri::WebviewWindow) {
    use winapi::shared::minwindef::DWORD;
    use winapi::um::dwmapi::DwmSetWindowAttribute;
    if let Ok(hwnd) = window.hwnd() {
        let preference: DWORD = 2; // DWMWCP_ROUND
        unsafe {
            DwmSetWindowAttribute(
                hwnd.0 as _,
                33, // DWMWA_WINDOW_CORNER_PREFERENCE
                &preference as *const _ as *const _,
                std::mem::size_of::<DWORD>() as u32,
            );
        }
    }
}

#[cfg(not(windows))]
pub fn apply_rounded_corners(_window: &tauri::WebviewWindow) {}

/// Добавить окну стиль WS_THICKFRAME (как у обычных окон Windows).
/// Без него frameless-окно при перетаскивании рисует мусор/чёрные полосы
/// (WebView2 не перерисовывается в стандартном drag-loop), а с ним —
/// всё перерисовывается нормально + бесплатно даёт нативный ресайз
/// по краям и снап к краям экрана.
#[cfg(windows)]
pub fn add_thick_frame(window: &tauri::WebviewWindow) {
    use winapi::um::winuser::{
        SetWindowLongPtrW, SetWindowPos, GetWindowLongPtrW, GWL_STYLE, SWP_FRAMECHANGED,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_THICKFRAME,
    };
    if let Ok(hwnd) = window.hwnd() {
        unsafe {
            let hwnd = hwnd.0 as _;
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            SetWindowLongPtrW(hwnd, GWL_STYLE, style | WS_THICKFRAME as isize);
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }
}

#[cfg(not(windows))]
pub fn add_thick_frame(_window: &tauri::WebviewWindow) {}

/// Применить режим рамки к окну: custom = без системной рамки + скруглённые
/// углы + WS_THICKFRAME (против чёрных полос при drag), system = стандартная
/// рамка Windows (углы её own).
pub fn apply_window_frame(window: &tauri::WebviewWindow, mode: &str) {
    let custom = mode == "custom";
    let _ = window.set_decorations(!custom);
    if custom {
        apply_rounded_corners(window);
        add_thick_frame(window);
    }
}

#[tauri::command]
pub fn get_window_frame(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = util::lock_config(&state)?;
    let mode = cfg
        .launcher_settings
        .window_frame
        .clone()
        .unwrap_or_else(|| "system".to_string());
    Ok(serde_json::json!({"success": true, "mode": mode}))
}

/// Переключить рамку окна на лету (без перезапуска) и сохранить выбор.
#[tauri::command]
pub fn set_window_frame(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    mode: String,
) -> Result<serde_json::Value, String> {
    let mode = if mode == "custom" { "custom" } else { "system" };
    {
        let mut cfg = util::lock_config(&state)?;
        cfg.launcher_settings.window_frame = Some(mode.to_string());
    }
    state.save_config()?;
    if let Some(window) = app.get_webview_window("main") {
        apply_window_frame(&window, mode);
    }
    util::ok_true()
}

#[tauri::command]
pub fn toggle_maximize(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_maximized().unwrap_or(false) {
            window.unmaximize().map_err(|e| e.to_string())?;
        } else {
            window.maximize().map_err(|e| e.to_string())?;
        }
    }
    util::ok_true()
}

#[tauri::command]
pub fn is_maximized(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let maximized = app
        .get_webview_window("main")
        .and_then(|w| w.is_maximized().ok())
        .unwrap_or(false);
    Ok(serde_json::json!({"success": true, "maximized": maximized}))
}

/// Перетаскивание окна из JS (вызывается по mousedown на фоне).
/// JS-вызов window.startDragging() блокируется capabilities, а Rust — нет.
#[tauri::command]
pub fn start_dragging(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    if let Some(window) = app.get_webview_window("main") {
        window.start_dragging().map_err(|e| e.to_string())?;
    }
    util::ok_true()
}

/// Резайз frameless-окна по краю/углу: edge = left|right|top|bottom|
/// top-left|top-right|bottom-left|bottom-right. Windows сама ловит
/// WM_NCLBUTTONDOWN с кодом HT* и уходит в режим ресайза.
#[tauri::command]
pub fn start_resize(app: tauri::AppHandle, edge: String) -> Result<serde_json::Value, String> {
    #[cfg(windows)]
    {
        use winapi::shared::minwindef::LPARAM;
        use winapi::shared::minwindef::WPARAM;
        use winapi::um::winuser::{
            ReleaseCapture, SendMessageW, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTLEFT, HTRIGHT,
            HTTOP, HTTOPLEFT, HTTOPRIGHT, WM_NCLBUTTONDOWN,
        };
        let Some(window) = app.get_webview_window("main") else {
            return util::ok_true();
        };
        let Ok(hwnd) = window.hwnd() else {
            return util::ok_true();
        };
        let ht = match edge.as_str() {
            "left" => HTLEFT,
            "right" => HTRIGHT,
            "top" => HTTOP,
            "bottom" => HTBOTTOM,
            "top-left" => HTTOPLEFT,
            "top-right" => HTTOPRIGHT,
            "bottom-left" => HTBOTTOMLEFT,
            _ => HTBOTTOMRIGHT,
        };
        unsafe {
            ReleaseCapture();
            SendMessageW(hwnd.0 as _, WM_NCLBUTTONDOWN, ht as WPARAM, 0 as LPARAM);
        }
    }
    #[cfg(not(windows))]
    let _ = (app, edge);
    util::ok_true()
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
pub fn save_profiles(state: State<'_, AppState>, profiles: Vec<serde_json::Value>, active_id: Option<String>) -> Result<serde_json::Value, String> {
    {
        let mut cfg = util::lock_config(&state)?;
        cfg.profiles = Some(profiles);
        cfg.active_profile_id = active_id.filter(|s| !s.is_empty());
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

/// Diff кандидата патчей (импортируемого файла/бэкапа) против текущего
/// #ArizonaPatches.json. Возвращает {changed:[{key,from,to}], added:[{key,to}],
/// removed:[{key,from}], same_count}. Плоский и вложенный формат нормализуются.
fn diff_vs_current_patches(state: &AppState, candidate: &serde_json::Value) -> serde_json::Value {
    // Текущий конфиг → плоский map
    let current_flat = util::read_patches_file(&patches_json_path(state))
        .ok()
        .and_then(|d| patches_flat(&d))
        .unwrap_or_default();
    // Кандидат → плоский map
    let cand_flat = patches_flat(candidate).unwrap_or_default();

    let mut changed = Vec::new();
    let mut added = Vec::new();
    let mut same_count = 0usize;
    for (k, v) in &cand_flat {
        match current_flat.get(k) {
            Some(cur) if cur == v => same_count += 1,
            Some(cur) => changed.push(serde_json::json!({
                "key": k,
                "from": cur,
                "to": v,
            })),
            None => added.push(serde_json::json!({"key": k, "to": v})),
        }
    }
    let mut removed = Vec::new();
    for (k, v) in &current_flat {
        if !cand_flat.contains_key(k) {
            removed.push(serde_json::json!({"key": k, "from": v}));
        }
    }

    serde_json::json!({
        "changed": changed,
        "added": added,
        "removed": removed,
        "same_count": same_count,
        "current_exists": !current_flat.is_empty(),
    })
}

/// Нормализация данных патчей к плоскому map: вложенный формат
/// {"patches": {...}} разворачивается, плоский — как есть.
/// (нет файла/пусто → None)
fn patches_flat(data: &serde_json::Value) -> Option<serde_json::Map<String, serde_json::Value>> {
    let obj = data.as_object()?;
    if let Some(nested) = obj.get("patches").and_then(|p| p.as_object()) {
        return Some(nested.clone());
    }
    Some(obj.clone())
}

/// Краткая сводка конфига патчей из плоского map: {total, on, off}.
fn patches_summary(flat: &serde_json::Map<String, serde_json::Value>) -> serde_json::Value {
    let total = flat.len();
    let on = flat.values().filter(|v| v.as_bool().unwrap_or(false)).count();
    serde_json::json!({"total": total, "on": on, "off": total - on})
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
        "https://raw.githubusercontent.com/halfik24/ArzLaunchRepo/main/patches/arizonapatches_{}.zip",
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
    // Текущий конфиг — чтобы каждому бэкапу сразу посчитать отличия
    let current_flat = util::read_patches_file(&patches_json_path(&state))
        .ok()
        .and_then(|d| patches_flat(&d))
        .unwrap_or_default();

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

            // Сводка бэкапа: сколько патчей вкл/выкл и сколько отличий
            // от текущего конфига (для бейджа в списке).
            let flat = util::read_patches_file(&e.path())
                .ok()
                .and_then(|d| patches_flat(&d));
            let (summary, diff_count) = match &flat {
                Some(f) => {
                    let diff = diff_vs_current_patches(&state, &serde_json::Value::Object(f.clone()));
                    let n = diff["changed"].as_array().map(|a| a.len()).unwrap_or(0)
                        + diff["added"].as_array().map(|a| a.len()).unwrap_or(0)
                        + diff["removed"].as_array().map(|a| a.len()).unwrap_or(0);
                    (patches_summary(f), serde_json::json!(n))
                }
                None => (serde_json::Value::Null, serde_json::Value::Null),
            };
            let r#type = if name.contains("before_import") {
                "before_import"
            } else if name.contains("before_restore") {
                "before_restore"
            } else {
                "save"
            };

            items.push((
                mtime,
                serde_json::json!({
                    "filename": name,
                    "path": e.path().to_string_lossy(),
                    "date": date,
                    "size": md.len(),
                    "type": r#type,
                    "summary": summary,
                    "diff_count": diff_count,
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

    // Diff против текущего конфига — фронт показывает «что изменится при
    // восстановлении» прямо в превью.
    let diff = diff_vs_current_patches(&state, &data);

    Ok(serde_json::json!({
        "success": true,
        "filename": filename,
        "type": r#type,
        "size": size,
        "patches_total": patches.len(),
        "patches_enabled": enabled,
        "patches_disabled": disabled,
        "launch_params": launch_params,
        "diff": diff,
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
    let url = "https://raw.githubusercontent.com/halfik24/ArzLaunchRepo/main/patches/version.txt";
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
    let url = "https://raw.githubusercontent.com/halfik24/ArzLaunchRepo/main/updates/patches.json";
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

/// Экспорт настроек патчей в JSON-файл через системный диалог сохранения.
#[tauri::command]
pub fn export_patches(app: tauri::AppHandle, data: serde_json::Value) -> Result<serde_json::Value, String> {
    use tauri_plugin_dialog::DialogExt;

    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let picked = app
        .dialog()
        .file()
        .set_title("Сохранить настройки патчей")
        .set_file_name(&format!("ArizonaPatches_{}.json", ts))
        .add_filter("JSON файл", &["json"])
        .blocking_save_file();

    let Some(picked) = picked else {
        return util::fail_message("Отменено");
    };
    let path = match picked {
        tauri_plugin_dialog::FilePath::Path(p) => p,
        tauri_plugin_dialog::FilePath::Url(u) => u
            .to_file_path()
            .map_err(|_| "Некорректный путь".to_string())?,
    };

    // Pretty JSON — тем же сериализатором, что и write_patches_inner
    let mut buf: Vec<u8> = Vec::new();
    {
        use serde::Serialize;
        let mut ser = serde_json::Serializer::with_formatter(
            &mut buf,
            serde_json::ser::PrettyFormatter::with_indent(b"    "),
        );
        data.serialize(&mut ser).map_err(|e| e.to_string())?;
    }
    match std::fs::write(&path, &buf) {
        Ok(_) => {
            log::info!("export_patches: сохранён {}", path.display());
            Ok(serde_json::json!({"success": true, "message": format!("Сохранено: {}", path.to_string_lossy())}))
        }
        Err(e) => Ok(serde_json::json!({"success": false, "message": e.to_string()})),
    }
}

/// Импорт настроек патчей из JSON-файла через системный диалог открытия.
/// Только читает и валидирует файл — запись делает фронтенд (после показа
/// diff-подтверждения), бэкап перед записью создаёт write_patches_inner.
#[tauri::command]
pub fn import_patches(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    use tauri_plugin_dialog::DialogExt;

    let picked = app
        .dialog()
        .file()
        .set_title("Открыть файл настроек патчей")
        .add_filter("JSON файл", &["json"])
        .blocking_pick_file();

    let Some(picked) = picked else {
        return util::fail_message("Отменено");
    };
    let path = match picked {
        tauri_plugin_dialog::FilePath::Path(p) => p,
        tauri_plugin_dialog::FilePath::Url(u) => u
            .to_file_path()
            .map_err(|_| "Некорректный путь".to_string())?,
    };

    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            return Ok(serde_json::json!({"success": false, "message": format!("Не удалось прочитать файл: {}", e)}))
        }
    };
    // Python: уб строки, начинающиеся с // (JSON с комментариями)
    let cleaned: String = raw
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    let data: serde_json::Value = match serde_json::from_str(&cleaned) {
        Ok(d) => d,
        Err(e) => return Ok(serde_json::json!({"success": false, "message": format!("Ошибка JSON: {}", e)})),
    };
    let Some(obj) = data.as_object() else {
        return util::fail_message("Файл не содержит настроек патчей");
    };

    // Вложенный формат {"patches": {...}} — разворачиваем до плоского:
    // в #ArizonaPatches.json пишется только плоский map патчей.
    let is_nested = obj.get("patches").map(|p| p.is_object()).unwrap_or(false);
    let flat: serde_json::Map<String, serde_json::Value> = if is_nested {
        obj["patches"].as_object().cloned().unwrap_or_default()
    } else {
        obj.clone()
    };
    let bool_count = flat.values().filter(|v| v.as_bool().unwrap_or(false)).count();
    if bool_count == 0 {
        return util::fail_message("Файл не содержит настроек патчей");
    }

    let flat_val = serde_json::Value::Object(flat);
    let diff = diff_vs_current_patches(&state, &flat_val);
    log::info!(
        "import_patches: выбран {}, настроек {}, diff changed={} added={} removed={}",
        path.display(),
        bool_count,
        diff["changed"].as_array().map(|a| a.len()).unwrap_or(0),
        diff["added"].as_array().map(|a| a.len()).unwrap_or(0),
        diff["removed"].as_array().map(|a| a.len()).unwrap_or(0),
    );
    Ok(serde_json::json!({
        "success": true,
        "data": flat_val,
        "keys_count": bool_count,
        "filename": path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default(),
        "diff": diff
    }))
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

// ==================== Установка / обновление / починка игры ====================
//
// Ядро — модуль game_update (порт az-proto): манифест game.json с CDN,
// правила проверки официального лаунчера (нет файла / размер / mtime /
// md5 с кэшем), качка по одному файлу с фоллбэком на резервный CDN.

use crate::game_update::engine::{self, Event, Job, Mode};

/// Вариант графики из настроек (`game_update_future` в launcher_settings,
/// default true) — ручной переключатель пользователя.
fn game_update_future(state: &AppState) -> bool {
    state
        .config
        .lock()
        .ok()
        .and_then(|c| {
            c.launcher_settings
                .extra
                .get("game_update_future")
                .and_then(|v| v.as_bool())
        })
        .unwrap_or(true)
}

/// Целевая папка игры: game_path в конфиге — путь до exe, файлы манифеста
/// лежат рядом с ним. Если game_path — папка, используем её как есть.
/// Если путь вообще не существует (файл переименовали/удалили), но он с
/// расширением — считаем это путём до exe и берём родителя, иначе все
/// файлы манифеста «не найдутся» и проверка потребует качать всё.
fn resolve_game_target(gp: &str) -> Option<PathBuf> {
    if gp.is_empty() {
        return None;
    }
    let p = PathBuf::from(gp);
    if p.is_file() {
        return p.parent().map(|x| x.to_path_buf());
    }
    if p.is_dir() || p.extension().is_none() {
        return Some(p);
    }
    p.parent()
        .filter(|x| !x.as_os_str().is_empty())
        .map(|x| x.to_path_buf())
}

/// Захватить единственную задачу установки/обновления. Повторный вызов
/// до завершения — ошибка.
fn claim_game_job(
    state: &AppState,
) -> Result<std::sync::Arc<std::sync::atomic::AtomicBool>, String> {
    use std::sync::atomic::AtomicBool;
    let mut g = state
        .game_update_cancel
        .lock()
        .map_err(|e| e.to_string())?;
    if g.is_some() {
        return Err("Установка/обновление игры уже выполняется".into());
    }
    let flag = std::sync::Arc::new(AtomicBool::new(false));
    *g = Some(flag.clone());
    Ok(flag)
}

fn release_game_job(state: &AppState) {
    if let Ok(mut g) = state.game_update_cancel.lock() {
        *g = None;
    }
}

/// Проверка: манифест + план (только чтение). Фронт ждёт ответ синхронно
/// и показывает кнопку «Обновить» при needs_update.
#[tauri::command]
pub async fn check_update_api(
    state: State<'_, AppState>,
    game_path: Option<String>,
) -> Result<serde_json::Value, String> {
    let gp = game_path
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| util::game_path(&state));
    let target = resolve_game_target(&gp)
        .ok_or_else(|| "game_path не установлен".to_string())?;
    let documents = state.documents_path.clone();

    let job = Job {
        mode: Mode::Check,
        cdn_main: engine::DEFAULT_CDN_MAIN.into(),
        cdn_reserve: engine::DEFAULT_CDN_RESERVE.into(),
        target,
        // Вариант графики: ручной переключатель из настроек.
        future: game_update_future(&state),
        documents,
    };

    let (_flat, plan) = crate::game_update::engine::build_plan(
        &job,
        &crate::game_update::downloader::client(),
        &|ev| {
            if let Event::Log(m) = ev {
                log::info!("[game-check] {m}");
            }
        },
        &std::sync::atomic::AtomicBool::new(false),
    )
    .await
    .map_err(|e| format!("{e:#}"))?;

    let needs_update = plan.download_count > 0;
    let message = if needs_update {
        format!(
            "Требуется обновление: {} файлов ({:.2} ГБ)",
            plan.download_count,
            engine::gb(plan.download_bytes)
        )
    } else {
        "Игра в актуальном состоянии".to_string()
    };
    log::info!(
        "check_update_api: {message} (дат поправить {}, папок создать {})",
        plan.date_fix,
        plan.mkdir_count
    );

    Ok(serde_json::json!({
        "success": true,
        "needs_update": needs_update,
        "message": message,
        "files": plan.download_count,
        "bytes": plan.download_bytes,
        "date_fix": plan.date_fix,
        "mkdir": plan.mkdir_count,
        "deletes": plan.delete_count,
    }))
}

/// Игра запущена? (независимая проверка — не трогает edge выхода
/// was_running из is_gta_running, чтобы watcher статуса-строки
/// по-прежнему увидел «игра вышла» сам).
fn gta_process_running() -> bool {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys.processes()
        .values()
        .any(|p| p.name().to_string_lossy().eq_ignore_ascii_case("gta_sa.exe"))
}

/// Запуск Apply/Repair в фоне. События — `launcher-install-progress`
/// (stage: 'dl' | 'done' | 'error'), фронт уже слушает этот хаб.
fn spawn_game_job(
    app: tauri::AppHandle,
    state: &AppState,
    mode: Mode,
    future: bool,
) -> Result<serde_json::Value, String> {
    let gp = util::game_path(state);
    let target = resolve_game_target(&gp)
        .ok_or_else(|| "game_path не установлен".to_string())?;
    if gta_process_running() {
        return Ok(serde_json::json!({
            "success": false,
            "message": "Игра запущена — закрой её перед установкой/обновлением"
        }));
    }
    let cancel = claim_game_job(state)?;

    let job = Job {
        mode,
        cdn_main: engine::DEFAULT_CDN_MAIN.into(),
        cdn_reserve: engine::DEFAULT_CDN_RESERVE.into(),
        target,
        future,
        documents: state.documents_path.clone(),
    };

    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let started = std::time::Instant::now();
        log::info!("game_update: старт ({})", job.mode.title());

        let on_event = |ev: Event| match ev {
            Event::Log(m) => log::info!("[game] {m}"),
            Event::Progress {
                bytes_done,
                bytes_total,
                current,
                speed,
                ..
            } => {
                let pct = if bytes_total > 0 {
                    ((bytes_done as f64 / bytes_total as f64) * 100.0) as u32
                } else {
                    0
                };
                let speed_mb = speed as f64 / 1_048_576.0;
                let message = if speed > 0 {
                    format!("{current} — {speed_mb:.1} МБ/с")
                } else {
                    current.clone()
                };
                let _ = app.emit(
                    "launcher-install-progress",
                    serde_json::json!({
                        "stage": "dl",
                        "progress": pct.min(99),
                        "message": message,
                    }),
                );
            }
            Event::Finished { ok, text } => {
                let _ = app.emit(
                    "launcher-install-progress",
                    serde_json::json!({
                        "stage": if ok { "done" } else { "error" },
                        "progress": if ok { 100 } else { 0 },
                        "message": text,
                    }),
                );
            }
        };

        let result = engine::run(&job, &on_event, &cancel).await;
        let (ok, text) = match &result {
            Ok(text) => {
                log::info!(
                    "game_update: готово за {:.1} с — {text}",
                    started.elapsed().as_secs_f64()
                );
                (true, text.clone())
            }
            Err(e) => {
                let msg = format!("{e:#}");
                log::warn!(
                    "game_update: ошибка за {:.1} с — {msg}",
                    started.elapsed().as_secs_f64()
                );
                (false, msg)
            }
        };
        on_event(Event::Finished { ok, text });
        release_game_job(&state);
    });

    Ok(serde_json::json!({"success": true, "started": true}))
}

#[tauri::command]
pub async fn download_update_api(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    spawn_game_job(app, &state, Mode::Apply, game_update_future(&state))
}

/// Починка: MD5 у всех файлов + res-записи.
#[tauri::command]
pub async fn repair_game_api(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    future: Option<bool>,
) -> Result<serde_json::Value, String> {
    let future = future.unwrap_or_else(|| game_update_future(&state));
    spawn_game_job(app, &state, Mode::Repair, future)
}

/// Отмена активной установки/обновления/починки.
#[tauri::command]
pub fn cancel_game_update(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let flag = state
        .game_update_cancel
        .lock()
        .map_err(|e| e.to_string())?
        .clone();
    match flag {
        Some(f) => {
            f.store(true, std::sync::atomic::Ordering::Relaxed);
            log::info!("game_update: запрошена отмена");
            Ok(serde_json::json!({"success": true}))
        }
        None => Ok(serde_json::json!({"success": false, "message": "Нечего отменять"})),
    }
}

/// Статус: выполняется ли сейчас установка/обновление игры.
#[tauri::command]
pub fn game_update_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let busy = state
        .game_update_cancel
        .lock()
        .map(|g| g.is_some())
        .unwrap_or(false);
    Ok(serde_json::json!({"busy": busy}))
}

// ==================== Установка лаунчера Air v8 ====================
//
// Архив лаунчера: единственный ArizonaLauncher8.0_byAIR.exe, метод Store,
// шифрование ZipCrypto (пароль 1111) — распаковка через
// `ZipArchive::by_index_decrypt` (crate zip 2.x, ZipCrypto не требует
// фичи aes-crypto). Отдельный архив preloading_plugins.zip — без пароля.

/// Архив лаунчера Air v8 (ZipCrypto, пароль `LAUNCHER_V8_ZIP_PASSWORD`).
const LAUNCHER_V8_ZIP_URL: &str =
    "https://raw.githubusercontent.com/halfik24/ArzLaunchRepo/main/others/ArizonaLauncher8.0_byAIR.zip";
const LAUNCHER_V8_ZIP_PASSWORD: &[u8] = b"1111";
/// Архив папки прелоадинга (без пароля, содержит `preloading_plugins/...`).
const PRELOADING_ZIP_URL: &str =
    "https://raw.githubusercontent.com/halfik24/ArzLaunchRepo/main/others/preloading_plugins.zip";
/// Максимум файлов в архиве (аналог `_MAX_ZIP_FILES` в Python).
const MAX_ZIP_FILES: usize = 5000;

/// Прогресс для фронта: событие `launcher-install-progress` →
/// `window._onLauncherInstallProgress` (слушатель в src/tauri-bridge.js).
/// stage: 'dl' | 'extract' | 'install' | 'done' | 'error', progress 0-100.
fn emit_install_progress(app: &tauri::AppHandle, stage: &str, progress: u32, message: &str) {
    use tauri::Emitter;
    if let Err(e) = app.emit(
        "launcher-install-progress",
        serde_json::json!({
            "stage": stage,
            "progress": progress,
            "message": message,
        }),
    ) {
        log::warn!("emit_install_progress({stage}): {e}");
    }
}

/// Скачивает URL во временный zip-файл, эмитя прогресс `pct_from..=pct_to`
/// не чаще раза в ~200мс. HTTP-таймауты: connect 15с, весь запрос 600с.
async fn download_to_temp(
    app: &tauri::AppHandle,
    url: &str,
    pct_from: u32,
    pct_to: u32,
    message: &str,
) -> Result<tempfile::NamedTempFile, String> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(600))
        .user_agent("Mozilla/5.0")
        .build()
        .map_err(|e| format!("Не удалось создать HTTP-клиент: {e}"))?;

    let mut resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Ошибка загрузки: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Ошибка загрузки: HTTP {}", resp.status()));
    }
    let total = resp.content_length().unwrap_or(0);

    let mut tmp = tempfile::Builder::new()
        .prefix("arizona_launcher_install_")
        .suffix(".zip")
        .tempfile()
        .map_err(|e| format!("Не удалось создать временный файл: {e}"))?;

    emit_install_progress(app, "dl", pct_from, message);

    let span = (pct_to - pct_from) as u64;
    let mut downloaded: u64 = 0;
    let mut last_emit = std::time::Instant::now();
    loop {
        match resp.chunk().await {
            Ok(Some(chunk)) => {
                use std::io::Write;
                tmp.write_all(&chunk)
                    .map_err(|e| format!("Не удалось записать временный файл: {e}"))?;
                downloaded += chunk.len() as u64;
                if last_emit.elapsed() >= std::time::Duration::from_millis(200) {
                    last_emit = std::time::Instant::now();
                    let mb = downloaded as f64 / 1_048_576.0;
                    let pct = if total > 0 {
                        (pct_from + (downloaded * span / total) as u32).min(pct_to)
                    } else {
                        pct_from
                    };
                    emit_install_progress(app, "dl", pct, &format!("Скачано {mb:.1} МБ..."));
                }
            }
            Ok(None) => break,
            Err(e) => return Err(format!("Ошибка загрузки: {e}")),
        }
    }
    log::info!("download_to_temp: {url} → {downloaded} байт");
    emit_install_progress(app, "dl", pct_to, message);
    Ok(tmp)
}

/// Что распаковывать из архива лаунчера.
enum ZipExtractMode {
    /// Полная установка — все файлы.
    All,
    /// Только exe (установка/обновление лаунчера).
    ExeOnly,
    /// Только записи внутри preloading_plugins/ (сохранением структуры).
    PreloadingOnly,
}

/// Распаковка zip во folder_path. `password` — Some для ZipCrypto-архива
/// (для незашифрованных записей пароль игнорируется). Защита от zip-slip:
/// в путях архива не бывает "..", итоговый путь обязан быть внутри folder.
fn extract_launcher_zip(
    zip_path: &std::path::Path,
    folder: &std::path::Path,
    password: Option<&[u8]>,
    mode: ZipExtractMode,
) -> Result<usize, String> {
    let file =
        std::fs::File::open(zip_path).map_err(|e| format!("Не удалось открыть архив: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Некорректный архив: {e}"))?;
    if archive.len() > MAX_ZIP_FILES {
        return Err(format!(
            "Слишком много файлов в архиве: {} > {MAX_ZIP_FILES}",
            archive.len()
        ));
    }
    let folder_real = folder
        .canonicalize()
        .map_err(|e| format!("Папка игры недоступна: {e}"))?;

    let mut extracted = 0usize;
    for i in 0..archive.len() {
        // Имя/тип записи — в отдельном блоке, чтобы не держить мут-заем архива
        let (name, is_dir) = {
            let f = archive
                .by_index(i)
                .map_err(|e| format!("Не удалось прочитать запись архива: {e}"))?;
            (f.name().to_string(), f.is_dir())
        };
        if is_dir {
            continue;
        }
        let norm = name.replace('\\', "/");
        if norm.split('/').any(|seg| seg == "..") || norm.starts_with('/') {
            return Err(format!("Недопустимый путь в архиве: {name}"));
        }

        let is_preloading = norm.contains("preloading_plugins");
        let is_exe = norm.to_ascii_lowercase().ends_with(".exe");
        let should = match mode {
            ZipExtractMode::All => true,
            ZipExtractMode::ExeOnly => is_exe,
            ZipExtractMode::PreloadingOnly => is_preloading,
        };
        if !should {
            continue;
        }

        // preloading_plugins — сохраняем структуру каталогов, остальное — basename
        let rel = if is_preloading {
            norm[norm.find("preloading_plugins").unwrap()..].to_string()
        } else {
            match std::path::Path::new(&norm).file_name() {
                Some(base) => base.to_string_lossy().to_string(),
                None => continue,
            }
        };
        let dest = folder_real.join(&rel);
        if !dest.starts_with(&folder_real) {
            return Err(format!("Zip-slip обнаружен: {name}"));
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Не удалось создать папку: {e}"))?;
        }
        let mut src = match password {
            Some(pw) => archive
                .by_index_decrypt(i, pw)
                .map_err(|e| format!("Не удалось распаковать '{name}': {e}"))?,
            None => archive
                .by_index(i)
                .map_err(|e| format!("Не удалось распаковать '{name}': {e}"))?,
        };
        let mut dst =
            std::fs::File::create(&dest).map_err(|e| format!("Не удалось записать файл: {e}"))?;
        std::io::copy(&mut src, &mut dst)
            .map_err(|e| format!("Ошибка распаковки '{name}': {e}"))?;
        log::info!("extract_launcher_zip: {}", dest.display());
        extracted += 1;
    }
    Ok(extracted)
}

/// Удаляет старые exe лаунчеров перед установкой v8: известные имена
/// `ArizonaLauncher*.exe` и любые exe, распознанные по ресурсу версии как
/// лаунчер Arizona со старой версией (<8). gta_sa.exe и переименованный v8
/// не трогаются.
fn remove_old_launcher_exes(folder: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name_os) = path.file_name() else {
            continue;
        };
        let name = name_os.to_string_lossy().to_lowercase();
        if !name.ends_with(".exe") || name == "gta_sa.exe" {
            continue;
        }
        let known_old_name = name.starts_with("arizonalauncher")
            && name != "arizonalauncher8.0_byair.exe";
        let renamed_old_launcher =
            util::is_arizona_launcher(&path) && util::exe_major_version(&path) < 8;
        if !known_old_name && !renamed_old_launcher {
            continue;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => log::info!("remove_old_launcher_exes: удалён {name}"),
            Err(e) => log::warn!("remove_old_launcher_exes: не удалось удалить {name}: {e}"),
        }
    }
}

/// Папка preloading_plugins существует и не пуста (как в get_install_action).
fn has_preloading_plugins(folder: &std::path::Path) -> bool {
    let plugins = folder.join("preloading_plugins");
    plugins.is_dir()
        && std::fs::read_dir(&plugins)
            .map(|mut rd| rd.next().is_some())
            .unwrap_or(false)
}

/// Тело установки: скачивание → распаковка → обновление launcher_path.
/// Ошибка — текст для фронта (эмит 'error' делает вызывающая команда).
async fn launcher_install_run(
    app: &tauri::AppHandle,
    state: &AppState,
    folder_path: &str,
    install_type: &str,
) -> Result<(), String> {
    let folder = std::path::Path::new(folder_path);
    // preloading ставим при полной установке и по запросу; upgrade_to_v8 —
    // только если папки вдруг нет (по action её быть не должно)
    let install_launcher = install_type != "install_preloading_only";
    let need_preloading = matches!(install_type, "choose_version" | "install_preloading_only")
        || !has_preloading_plugins(folder);

    if install_type == "upgrade_to_v8" {
        // Старые процессы лаунчеров блокируют удаление их exe
        kill_all_launchers();
    }
    if install_launcher {
        remove_old_launcher_exes(folder);
    }

    // ── Этап 1: архив лаунчера (ZipCrypto, пароль) ──
    if install_launcher {
        emit_install_progress(app, "dl", 1, "Подключение к GitHub...");
        let zip = download_to_temp(
            app,
            LAUNCHER_V8_ZIP_URL,
            0,
            80,
            "Скачивание архива лаунчера...",
        )
        .await?;
        emit_install_progress(app, "extract", 82, "Распаковка архива...");
        let mode = if install_type == "choose_version" {
            ZipExtractMode::All
        } else {
            ZipExtractMode::ExeOnly
        };
        let extracted = extract_launcher_zip(
            zip.path(),
            folder,
            Some(LAUNCHER_V8_ZIP_PASSWORD),
            mode,
        )?;
        log::info!("start_launcher_install: извлечено файлов лаунчера: {extracted}");
        if extracted == 0 {
            return Err("Исполняемый файл лаунчера не найден в архиве".to_string());
        }
    }

    // ── Этап 2: preloading_plugins (без пароля), если его нет ──
    if need_preloading {
        emit_install_progress(app, "dl", 84, "Скачивание preloading_plugins...");
        let zip =
            download_to_temp(app, PRELOADING_ZIP_URL, 84, 92, "Скачивание preloading_plugins...")
                .await?;
        emit_install_progress(app, "extract", 94, "Распаковка preloading_plugins...");
        let extracted =
            extract_launcher_zip(zip.path(), folder, None, ZipExtractMode::PreloadingOnly)?;
        log::info!("start_launcher_install: извлечено файлов preloading: {extracted}");
        if extracted == 0 || !has_preloading_plugins(folder) {
            return Err("Папка preloading_plugins не найдена после распаковки".to_string());
        }
    }

    // ── Этап 3: финализация — путь к лаунчеру в конфиг ──
    emit_install_progress(app, "install", 96, "Установка файлов...");
    match find_launcher_exe(folder) {
        Some((exe, _)) => {
            let exe_str = exe.to_string_lossy().to_string();
            log::info!("start_launcher_install: launcher_path = {exe_str}");
            util::update_config(state, |cfg| {
                cfg.launcher_path = exe_str.clone();
                Ok(())
            })?;
        }
        None => log::warn!("start_launcher_install: exe лаунчера не найден в {folder_path}"),
    }
    Ok(())
}

#[tauri::command]
pub async fn start_launcher_install(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    folder_path: String,
    version: Option<String>,
    install_type: Option<String>,
) -> Result<serde_json::Value, String> {
    let version = version.unwrap_or_else(|| "v8".to_string());
    if version != "v8" {
        return util::fail_message(format!("Поддерживается только версия v8 (получена: {version})"));
    }
    let install_type = install_type.unwrap_or_else(|| "choose_version".to_string());
    const VALID_TYPES: [&str; 4] = [
        "choose_version",
        "install_launcher_only",
        "install_preloading_only",
        "upgrade_to_v8",
    ];
    if !VALID_TYPES.contains(&install_type.as_str()) {
        return util::fail_message(format!("Установка не требуется (action: {install_type})"));
    }

    let folder = std::path::Path::new(&folder_path);
    if !folder.is_dir() {
        return util::fail_message("Папка игры не найдена");
    }
    // Дополнительная проверка: нужная версия уже стоит — не качаем
    // (чисто файловая проверка: установку вызывают явно, метод не важен)
    if let Ok(info) = install_action_for_folder(&folder_path) {
        if info["action"] == "none" {
            let msg = info["message"].as_str().unwrap_or("Лаунчер уже установлен");
            return util::fail_message(msg);
        }
    }

    log::info!("start_launcher_install: folder={folder_path} version={version} type={install_type}");
    match launcher_install_run(&app, &state, &folder_path, &install_type).await {
        Ok(()) => {
            emit_install_progress(&app, "done", 100, "Установка завершена");
            log::info!("start_launcher_install: УСТАНОВКА ЗАВЕРШЕНА УСПЕШНО ({folder_path})");
            util::ok_true()
        }
        Err(e) => {
            log::error!("start_launcher_install: {e}");
            emit_install_progress(&app, "error", 0, &e);
            util::fail_message(e)
        }
    }
}

// ==================== Moonloader ====================

/// Путь к `moonloader/` рядом с gta_sa.exe — 1-в-1 как `_get_moonloader_dir` в Python.
/// Ошибка — текст для фронта («Путь к игре не установлен» / «Папка moonloader не найдена»).
fn moonloader_dir(state: &AppState) -> Result<PathBuf, String> {
    let gp = util::game_path(state);
    if gp.is_empty() {
        return Err("Путь к игре не установлен".to_string());
    }
    let dir = std::path::Path::new(&gp)
        .parent()
        .map(|p| p.join("moonloader"))
        .ok_or_else(|| "Invalid game path".to_string())?;
    if !dir.is_dir() {
        return Err("Папка moonloader не найдена".to_string());
    }
    Ok(dir)
}

/// Имя файла без обходов путей: только basename, никаких `..`, `/`, `\`.
fn safe_script_name(name: &str) -> Result<&str, String> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name == "."
        || name == ".."
    {
        return Err("Недопустимое имя файла".to_string());
    }
    Ok(name)
}

#[tauri::command]
pub fn get_moonloader_scripts(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let dir = match moonloader_dir(&state) {
        Ok(d) => d,
        Err(e) => return util::fail_message(e),
    };
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => return util::fail_message(e.to_string()),
    };
    let mut scripts: Vec<serde_json::Value> = Vec::new();
    for entry in entries.flatten() {
        // Только файлы, без подпапок
        match entry.file_type() {
            Ok(t) if t.is_file() => {}
            _ => continue,
        }
        let name = entry.file_name().to_string_lossy().to_string();
        // Активный = не заканчивается на .disabled
        let (real_name, enabled) = match name.strip_suffix(".disabled") {
            Some(base) => (base.to_string(), false),
            None => (name, true),
        };
        // Только скриптовые расширения
        let ext = match std::path::Path::new(&real_name).extension() {
            Some(e) => e.to_string_lossy().to_lowercase(),
            None => continue,
        };
        if !matches!(ext.as_str(), "lua" | "cs" | "asi" | "luac") {
            continue;
        }
        scripts.push(serde_json::json!({
            "name": real_name,
            "ext": ext,
            "enabled": enabled,
            "full_path": entry.path().to_string_lossy(),
        }));
    }
    scripts.sort_by(|a, b| {
        a["name"]
            .as_str()
            .unwrap_or("")
            .to_lowercase()
            .cmp(&b["name"].as_str().unwrap_or("").to_lowercase())
    });
    Ok(serde_json::json!({
        "success": true,
        "scripts": scripts,
        "dir": dir.to_string_lossy(),
    }))
}

#[tauri::command]
pub fn toggle_moonloader_script(
    state: State<'_, AppState>,
    script_name: String,
    enable: bool,
) -> Result<serde_json::Value, String> {
    let dir = match moonloader_dir(&state) {
        Ok(d) => d,
        Err(e) => return util::fail_message(e),
    };
    let name = match safe_script_name(&script_name) {
        Ok(n) => n,
        Err(e) => return util::fail_message(e),
    };
    let enabled_path = dir.join(name);
    let disabled_path = dir.join(format!("{}.disabled", name));

    let result = if enable {
        // Включить: убрать .disabled
        if disabled_path.exists() {
            std::fs::rename(&disabled_path, &enabled_path)
        } else if !enabled_path.exists() {
            return util::fail_message(format!("Файл не найден: {}", name));
        } else {
            return util::ok_true();
        }
    } else {
        // Выключить: добавить .disabled
        if enabled_path.exists() {
            std::fs::rename(&enabled_path, &disabled_path)
        } else if !disabled_path.exists() {
            return util::fail_message(format!("Файл не найден: {}", name));
        } else {
            return util::ok_true();
        }
    };
    match result {
        Ok(_) => util::ok_true(),
        Err(e) => util::fail_message(e.to_string()),
    }
}

#[tauri::command]
pub fn delete_moonloader_script(
    state: State<'_, AppState>,
    filename: String,
) -> Result<serde_json::Value, String> {
    let dir = match moonloader_dir(&state) {
        Ok(d) => d,
        Err(e) => return util::fail_message(e),
    };
    let name = match safe_script_name(&filename) {
        Ok(n) => n,
        Err(e) => return util::fail_message(e),
    };
    // Пробуем оба варианта: обычный и .disabled
    let candidates = [dir.join(name), dir.join(format!("{}.disabled", name))];
    let mut deleted: Vec<String> = Vec::new();
    for path in &candidates {
        if path.exists() {
            if let Err(e) = std::fs::remove_file(path) {
                return util::fail_message(e.to_string());
            }
            deleted.push(path.file_name().unwrap_or_default().to_string_lossy().to_string());
        }
    }
    if deleted.is_empty() {
        return util::fail_message(format!("Файл не найден: {}", name));
    }
    Ok(serde_json::json!({
        "success": true,
        "message": format!("Удалён: {}", deleted.join(", ")),
    }))
}

#[tauri::command]
pub fn open_moonloader_folder(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let dir = match moonloader_dir(&state) {
        Ok(d) => d,
        Err(e) => return util::fail_message(e),
    };
    open::that(&dir).map_err(|e| e.to_string())?;
    util::ok_true()
}

/// Парсер блоков `[секция]` с парами `key = value` — как `_parse_catalog_txt` в v1.
/// Комментарии (`#`), пустые строки и строки без `=` пропускаются.
fn parse_catalog_txt(text: &str) -> Vec<serde_json::Value> {
    let mut blocks: Vec<serde_json::Value> = Vec::new();
    let mut current: Option<serde_json::Map<String, serde_json::Value>> = None;
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            if let Some(block) = current.take() {
                blocks.push(serde_json::Value::Object(block));
            }
            current = Some(serde_json::Map::new());
            continue;
        }
        if let Some(block) = current.as_mut() {
            if let Some(eq) = line.find('=') {
                let key = line[..eq].trim().to_string();
                let val = line[eq + 1..].trim().to_string();
                block.insert(key, serde_json::Value::String(val));
            }
        }
    }
    if let Some(block) = current {
        blocks.push(serde_json::Value::Object(block));
    }
    blocks
}

/// Безопасное имя файла — аналог `_safe_basename` из v1:
/// только basename (без путей/`..`) и whitelist символов.
fn safe_script_filename(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Пустое имя файла".to_string());
    }
    let cleaned = trimmed.rsplit(['/', '\\']).next().unwrap_or("");
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        return Err(format!("Недопустимое имя файла: {name}"));
    }
    let ok = cleaned
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ' | '(' | ')'));
    if !ok {
        return Err(format!("Недопустимые символы в имени файла: {cleaned}"));
    }
    Ok(cleaned.to_string())
}

#[tauri::command]
pub async fn fetch_moonloader_catalog() -> Result<serde_json::Value, String> {
    let urls = [
        "https://raw.githubusercontent.com/halfik24/ArzLaunchRepo/main/Lua/lua.txt",
        "https://raw.githubusercontent.com/halfik24/ArzLaunchRepo/refs/heads/main/Lua/lua.txt",
        "https://github.com/halfik24/ArzLaunchRepo/raw/main/Lua/lua.txt",
        // Fallback из v1 (старый репо, теперь под halfik24)
        "https://raw.githubusercontent.com/halfik24/ArizonaLauncher/main/moonloader.txt",
    ];
    let mut last_err = String::new();
    for url in urls {
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
                log::error!("fetch_moonloader_catalog: {url}: {e}");
                last_err = e;
                continue;
            }
        };
        let status = resp.status();
        let bytes = match resp.bytes().await {
            Ok(b) => b,
            Err(e) => {
                last_err = e.to_string();
                continue;
            }
        };
        if status != reqwest::StatusCode::OK {
            last_err = format!("HTTP {status}");
            continue;
        }
        let text = util::decode_text_bytes(&bytes);
        let data = parse_catalog_txt(&text);
        if data.is_empty() {
            last_err = "пустой каталог".to_string();
            continue;
        }
        log::info!("fetch_moonloader_catalog: {} скриптов с {url}", data.len());
        return Ok(serde_json::json!({"success": true, "data": data}));
    }
    Ok(serde_json::json!({
        "success": false,
        "message": if last_err.is_empty() {
            "Каталог недоступен".to_string()
        } else {
            format!("Не удалось загрузить каталог: {last_err}")
        }
    }))
}

#[tauri::command]
pub async fn install_moonloader_script(
    state: State<'_, AppState>,
    url: String,
    filename: String,
) -> Result<serde_json::Value, String> {
    // Папка moonloader рядом с gta_sa.exe (та же логика, что в v1)
    let ml_dir = match moonloader_dir(&state) {
        Ok(d) => d,
        Err(e) => return util::fail_message(e),
    };
    // Безопасное имя файла (whitelist как в v1)
    let safe_name = match safe_script_filename(&filename) {
        Ok(n) => n,
        Err(e) => return util::fail_message(e),
    };
    // Скачиваем скрипт
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static("Mozilla/5.0"),
    );
    let resp = match util::http_request(
        reqwest::Method::GET,
        &url,
        headers,
        std::time::Duration::from_secs(30),
    )
    .await
    {
        Ok(r) => r,
        Err(e) => return util::fail_message(format!("Ошибка загрузки: {e}")),
    };
    let status = resp.status();
    if status != reqwest::StatusCode::OK {
        return util::fail_message(format!("Ошибка загрузки: HTTP {status}"));
    }
    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => return util::fail_message(format!("Ошибка загрузки: {e}")),
    };
    let dest = ml_dir.join(&safe_name);
    if let Err(e) = std::fs::write(&dest, &bytes) {
        return util::fail_message(format!("Не удалось записать файл: {e}"));
    }
    log::info!("Скрипт установлен: {}", dest.display());
    Ok(serde_json::json!({"success": true, "message": format!("Установлен: {safe_name}")}))
}

#[tauri::command]
pub fn install_lua_file(filename: String, content_b64: String) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

// ==================== Libraries ====================

/// Каталог библиотек `libraries.txt`: ArzLaunchRepo, фолбэк — старый ArizonaLauncher (v1).
#[tauri::command]
pub async fn fetch_libraries_catalog() -> Result<serde_json::Value, String> {
    let urls = [
        "https://raw.githubusercontent.com/halfik24/ArzLaunchRepo/main/libraries.txt",
        "https://raw.githubusercontent.com/halfik24/ArzLaunchRepo/refs/heads/main/libraries.txt",
        "https://github.com/halfik24/ArzLaunchRepo/raw/main/libraries.txt",
        // Fallback из v1
        "https://raw.githubusercontent.com/halfik24/ArizonaLauncher/main/libraries.txt",
    ];
    let mut last_err = String::new();
    for url in urls {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CACHE_CONTROL,
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
                log::error!("fetch_libraries_catalog: {url}: {e}");
                last_err = e;
                continue;
            }
        };
        let status = resp.status();
        let bytes = match resp.bytes().await {
            Ok(b) => b,
            Err(e) => {
                last_err = e.to_string();
                continue;
            }
        };
        if status != reqwest::StatusCode::OK {
            last_err = format!("HTTP {status}");
            continue;
        }
        let text = util::decode_text_bytes(&bytes);
        let data = parse_catalog_txt(&text);
        if data.is_empty() {
            last_err = "пустой каталог".to_string();
            continue;
        }
        log::info!("fetch_libraries_catalog: {} библиотек с {url}", data.len());
        return Ok(serde_json::json!({"success": true, "data": data}));
    }
    Ok(serde_json::json!({
        "success": false,
        "message": if last_err.is_empty() {
            "Каталог библиотек недоступен".to_string()
        } else {
            format!("Не удалось загрузить каталог библиотек: {last_err}")
        }
    }))
}

/// Выбор канонической папки библиотек внутри `moonloader/`: существующая
/// непустая `lib/` либо `libraries/`, иначе создаётся `lib/` (приоритет —
/// 1-в-1 как `_get_lib_dir` в v1). Чистая функция — тестируется напрямую.
fn moonloader_lib_dir_from(ml_dir: &std::path::Path) -> Result<PathBuf, String> {
    for cand in ["lib", "libraries"] {
        let p = ml_dir.join(cand);
        if p.is_dir() {
            let non_empty = std::fs::read_dir(&p)
                .map(|mut it| it.next().is_some())
                .unwrap_or(false);
            if non_empty {
                return Ok(p);
            }
        }
    }
    let canon = ml_dir.join("lib");
    std::fs::create_dir_all(&canon)
        .map_err(|e| format!("Не удалось создать {}: {e}", canon.display()))?;
    Ok(canon)
}

/// Каноническая папка библиотек из состояния приложения.
fn moonloader_lib_dir(state: &AppState) -> Result<PathBuf, String> {
    let ml = moonloader_dir(state)?;
    moonloader_lib_dir_from(&ml)
}

/// Расширения файлов-библиотек (прочие файлы при скане пропускаем).
fn is_lib_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("lua") | Some("luac") | Some("dll") | Some("so") | Some("cs") | Some("asi")
    )
}

/// Имя библиотеки из имени файла: `mimgui.lua` → `mimgui`, `x.luac` → `x`,
/// `GifPlayerLua.dll` → `GifPlayerLua` (иначе имя либы из dll не совпадёт с каталогом).
fn lib_file_stem(fname: &str) -> String {
    let lower = fname.to_ascii_lowercase();
    for ext in [".luac", ".lua", ".dll", ".asi", ".cs", ".so"] {
        if lower.ends_with(ext) && fname.len() > ext.len() {
            return fname[..fname.len() - ext.len()].to_string();
        }
    }
    fname.to_string()
}

/// Сканирует папку либ. Файлы в корне: `mimgui.lua` → «mimgui».
/// Подпапки: `samp/events.lua` → «samp.events», а `mimgui/mimgui.lua` и
/// `foo/init.lua` → «mimgui» / «foo» (чинит рассинхрон имён с каталогом).
fn scan_installed_libs(lib_dir: &std::path::Path) -> Vec<serde_json::Value> {
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<String, (Vec<String>, u64)> = BTreeMap::new();
    let entries = match std::fs::read_dir(lib_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    for entry in entries.flatten() {
        let fname = entry.file_name().to_string_lossy().to_string();
        if fname.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_file() {
            if !is_lib_file(&path) {
                continue;
            }
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let g = groups.entry(lib_file_stem(&fname)).or_default();
            g.0.push(fname);
            g.1 += size;
        } else if ft.is_dir() {
            let Ok(sub) = std::fs::read_dir(&path) else { continue };
            for f in sub.flatten() {
                let sfname = f.file_name().to_string_lossy().to_string();
                if sfname.starts_with('.') {
                    continue;
                }
                let spath = f.path();
                if !spath.is_file() || !is_lib_file(&spath) {
                    continue;
                }
                let stem = lib_file_stem(&sfname);
                let key = if stem.eq_ignore_ascii_case(&fname) || stem == "init" {
                    fname.clone()
                } else {
                    format!("{fname}.{stem}")
                };
                let size = std::fs::metadata(&spath).map(|m| m.len()).unwrap_or(0);
                let g = groups.entry(key).or_default();
                g.0.push(format!("{fname}/{sfname}"));
                g.1 += size;
            }
        }
    }
    groups
        .into_iter()
        .map(|(name, (files, size))| serde_json::json!({"name": name, "files": files, "size": size}))
        .collect()
}

#[tauri::command]
pub fn list_installed_libraries(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let dir = match moonloader_lib_dir(&state) {
        Ok(d) => d,
        Err(e) => return util::fail_message(e),
    };
    Ok(serde_json::json!({"success": true, "data": scan_installed_libs(&dir)}))
}

/// Безопасная распаковка zip-библиотеки: ВСЁ кладётся в `lib/` с сохранением
/// внутренних путей (префикс `moonloader/` отрезается — такие архивы
/// упакованы под извлечение в корень игры). Пути с `..`, абсолютные и с
/// диском пропускаются. Возвращает записанные относительные пути.
fn extract_lib_zip(bytes: &[u8], lib_dir: &std::path::Path) -> Result<Vec<String>, String> {
    const MAX_ENTRIES: usize = 2000;
    const MAX_TOTAL: u64 = 500 * 1024 * 1024;
    let cursor = std::io::Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("Некорректный архив: {e}"))?;
    if archive.len() > MAX_ENTRIES {
        return Err(format!("Слишком много файлов в архиве: {}", archive.len()));
    }
    let mut written: Vec<String> = Vec::new();
    let mut total: u64 = 0;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if !entry.is_file() {
            continue;
        }
        let raw = entry.name().replace('\\', "/");
        // Защита от zip-slip
        if raw.starts_with('/')
            || raw.contains("..")
            || raw.chars().take(2).any(|c| c == ':')
        {
            log::warn!("extract_lib_zip: пропущен небезопасный путь: {raw}");
            continue;
        }
        total += entry.size();
        if total > MAX_TOTAL {
            return Err("Архив слишком большой".to_string());
        }
        // Архивы вида `moonloader/lib/x.lua` распаковываются как `lib/x.lua`,
        // а `moonloader/x.lua` — как просто `x.lua` (архив упакован под корень игры)
        let rel = raw
            .strip_prefix("moonloader/lib/")
            .or_else(|| raw.strip_prefix("moonloader/"))
            .unwrap_or(&raw);
        if rel.is_empty() || rel.ends_with('/') {
            continue;
        }
        let dest = lib_dir.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = std::fs::File::create(&dest)
            .map_err(|e| format!("Не удалось записать {rel}: {e}"))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| format!("Ошибка записи {rel}: {e}"))?;
        written.push(rel.to_string());
    }
    if written.is_empty() {
        return Err("В архиве нет файлов".to_string());
    }
    Ok(written)
}

/// Уже ли установлен целевой файл одиночной либы (`path` относительно lib/).
fn lib_path_is_installed(lib_dir: &std::path::Path, rel: &str) -> bool {
    lib_dir.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR))
        .is_file()
}

#[tauri::command]
pub async fn install_library(
    state: State<'_, AppState>,
    name: String,
    url: String,
    path: Option<String>,
) -> Result<serde_json::Value, String> {
    let name = name.trim();
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
    {
        return util::fail_message("Недопустимое имя библиотеки");
    }
    let lib_dir = match moonloader_lib_dir(&state) {
        Ok(d) => d,
        Err(e) => return util::fail_message(e),
    };
    let rel_path = path.as_deref().map(str::trim).filter(|p| !p.is_empty());
    if let Some(p) = rel_path {
        if p.starts_with('/') || p.starts_with('\\') || p.contains("..") || p.contains(':') {
            return util::fail_message(format!("Недопустимый путь: {p}"));
        }
    }
    // Уже установлено? Сначала по целевому файлу (точно), затем по имени (как в v1).
    if let Some(p) = rel_path {
        if lib_path_is_installed(&lib_dir, p) {
            return Ok(serde_json::json!({
                "success": true, "skipped": true,
                "message": format!("{name} уже установлена")
            }));
        }
    } else if scan_installed_libs(&lib_dir)
        .iter()
        .any(|l| l["name"].as_str().map(|n| n.eq_ignore_ascii_case(name)).unwrap_or(false))
    {
        return Ok(serde_json::json!({
            "success": true, "skipped": true,
            "message": format!("{name} уже установлена")
        }));
    }
    // Скачивание
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static("Mozilla/5.0"),
    );
    let resp = match util::http_request(
        reqwest::Method::GET,
        &url,
        headers,
        std::time::Duration::from_secs(30),
    )
    .await
    {
        Ok(r) => r,
        Err(e) => return util::fail_message(format!("Ошибка загрузки: {e}")),
    };
    let status = resp.status();
    if status != reqwest::StatusCode::OK {
        return util::fail_message(format!("Ошибка загрузки: HTTP {status}"));
    }
    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => return util::fail_message(format!("Ошибка загрузки: {e}")),
    };
    if bytes.is_empty() {
        return util::fail_message("Пустой файл библиотеки");
    }
    let written: Vec<String> = if bytes.starts_with(b"PK") {
        match extract_lib_zip(&bytes, &lib_dir) {
            Ok(w) => w,
            Err(e) => return util::fail_message(e),
        }
    } else {
        // Одиночный файл: явный путь из каталога либо имя из url / имени библиотеки
        let fname = match rel_path {
            Some(p) => p.to_string(),
            None => {
                let from_url = url.rsplit(['/', '\\']).next().and_then(|b| {
                    let s = b.split(['?', '#']).next().unwrap_or("");
                    if s.is_empty() {
                        None
                    } else {
                        safe_script_filename(s).ok()
                    }
                });
                from_url.unwrap_or_else(|| format!("{name}.lua"))
            }
        };
        let dest_path = lib_dir.join(fname.replace('/', std::path::MAIN_SEPARATOR_STR));
        if let Some(parent) = dest_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return util::fail_message(format!("Не удалось создать папку: {e}"));
            }
        }
        if let Err(e) = std::fs::write(&dest_path, &bytes) {
            return util::fail_message(format!("Не удалось записать файл: {e}"));
        }
        vec![fname]
    };
    log::info!("Библиотека {name} установлена: {}", written.join(", "));
    Ok(serde_json::json!({
        "success": true,
        "message": format!("Установлено: {name} ({} файлов)", written.len()),
        "files": written,
    }))
}

#[tauri::command]
pub fn uninstall_library(
    state: State<'_, AppState>,
    name: String,
) -> Result<serde_json::Value, String> {
    let name = name.trim();
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return util::fail_message("Недопустимое имя библиотеки");
    }
    let lib_dir = match moonloader_lib_dir(&state) {
        Ok(d) => d,
        Err(e) => return util::fail_message(e),
    };
    let mut deleted: Vec<String> = Vec::new();
    // 1) Файлы в корне lib: name / name.lua / name.luac
    for cand in [name.to_string(), format!("{name}.lua"), format!("{name}.luac")] {
        let p = lib_dir.join(&cand);
        if p.is_file() {
            if let Err(e) = std::fs::remove_file(&p) {
                return util::fail_message(e.to_string());
            }
            deleted.push(cand);
        }
    }
    // 2) Точечное имя: samp.events → lib/samp/events.lua(.luac), папка подметается
    if let Some((dir, rest)) = name.split_once('.') {
        if let Ok(dir_safe) = safe_script_name(dir) {
            let sub = lib_dir.join(dir_safe);
            if sub.is_dir() {
                for cand in [format!("{rest}.lua"), format!("{rest}.luac")] {
                    let p = sub.join(&cand);
                    if p.is_file() {
                        if let Err(e) = std::fs::remove_file(&p) {
                            return util::fail_message(e.to_string());
                        }
                        deleted.push(format!("{dir_safe}/{cand}"));
                    }
                }
                let _ = std::fs::remove_dir(&sub); // сработает только для пустой папки
            }
        }
    }
    // 3) Папка-библиотека (набор файлов)
    let plain_dir = lib_dir.join(name);
    if plain_dir.is_dir() {
        if let Err(e) = std::fs::remove_dir_all(&plain_dir) {
            return util::fail_message(e.to_string());
        }
        deleted.push(format!("{name}/"));
    }
    if deleted.is_empty() {
        return util::fail_message(format!("Библиотека не найдена: {name}"));
    }
    Ok(serde_json::json!({
        "success": true,
        "message": format!("Удалено: {}", deleted.join(", ")),
        "deleted": deleted,
    }))
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

// ==================== Wallpapers ====================

/// Расширения обоев — те же, что поддерживала легаси-версия на Python.
const WALLPAPER_EXTS: [&str; 5] = ["jpg", "jpeg", "png", "webp", "bmp"];

/// Папки с обоями, в порядке приоритета:
/// 1. `wallpapers/` рядом с exe — пользовательская, создаётся автоматически;
/// 2. `src/wallpapers/` — встроенные обои из ресурсов (видны в dev-сборке).
fn wallpapers_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(exe) = std::env::current_exe().ok() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("wallpapers"));
        }
    }
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("src")
        .join("wallpapers");
    if bundled.is_dir() {
        dirs.push(bundled);
    }
    dirs
}

fn wallpaper_mime(filename: &str) -> &'static str {
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        _ => "image/jpeg",
    }
}

/// Ищет файл обоев в известных папках (запрет path traversal: только имя файла).
fn find_wallpaper(filename: &str) -> Option<PathBuf> {
    if filename.is_empty()
        || filename.contains('/')
        || filename.contains('\\')
        || filename.contains("..")
    {
        return None;
    }
    wallpapers_dirs()
        .into_iter()
        .map(|dir| dir.join(filename))
        .find(|path| path.is_file())
}

fn wallpaper_data_url(filename: &str) -> Result<serde_json::Value, String> {
    let path = match find_wallpaper(filename) {
        Some(p) => p,
        None => return Ok(serde_json::json!({"success": false, "message": "File not found"})),
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => return Ok(serde_json::json!({"success": false, "message": e.to_string()})),
    };
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(serde_json::json!({
        "success": true,
        "data_url": format!("data:{};base64,{}", wallpaper_mime(filename), b64)
    }))
}

/// Список обоев. Если пользовательской папки нет — она создаётся автоматически.
#[tauri::command]
pub fn get_wallpapers() -> Result<serde_json::Value, String> {
    let dirs = wallpapers_dirs();
    // Пользовательская папка создаётся всегда — в неё можно просто положить картинки.
    if let Some(user_dir) = dirs.first() {
        if let Err(e) = std::fs::create_dir_all(user_dir) {
            return Ok(serde_json::json!({"success": false, "message": e.to_string(), "wallpapers": []}));
        }
    }

    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for dir in dirs {
        let entries = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            if !path.is_file() {
                continue;
            }
            let ext = match path.extension().and_then(|e| e.to_str()) {
                Some(e) => e.to_ascii_lowercase(),
                None => continue,
            };
            if !WALLPAPER_EXTS.contains(&ext.as_str()) {
                continue;
            }
            let filename = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            // одноимённый файл пользовательской папки перекрывает встроенный
            if !seen.insert(filename.clone()) {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&filename)
                .to_string();
            result.push(serde_json::json!({"name": name, "filename": filename}));
        }
    }
    Ok(serde_json::json!({"success": true, "wallpapers": result}))
}

#[tauri::command]
pub fn get_wallpaper_thumb(filename: String) -> Result<serde_json::Value, String> {
    // Легаси ресайзил превью в 500px через PIL; без crate `image` отдаём оригинал —
    // это тот же fallback, что срабатывал в Python-версии при отсутствии PIL.
    wallpaper_data_url(&filename)
}

#[tauri::command]
pub fn get_wallpaper_full(filename: String) -> Result<serde_json::Value, String> {
    wallpaper_data_url(&filename)
}

#[tauri::command]
pub fn select_bg_image() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": "Not implemented"}))
}

#[cfg(test)]
mod catalog_tests {
    use super::{parse_catalog_txt, safe_script_filename};

    #[test]
    fn parses_script_blocks_like_v1() {
        let text = "\
# комментарий пропускается

[script]
name        = Удаление надписи в ЦГ
author      = halfik
description = Убирает надпись.
url         = https://example.com/delCG.lua
file        = delCG.lua

[script]
name = Второй
file = two.lua
";
        let blocks = parse_catalog_txt(text);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0]["name"], "Удаление надписи в ЦГ");
        assert_eq!(blocks[0]["author"], "halfik");
        assert_eq!(blocks[0]["url"], "https://example.com/delCG.lua");
        assert_eq!(blocks[1]["name"], "Второй");
        // строки без `=` (продолжения описания) не ломают парсинг
        assert!(blocks[0].get("description").is_some());
    }

    #[test]
    fn parser_ignores_lines_before_first_block() {
        let blocks = parse_catalog_txt("stray = line\n[file]\nname = x\n");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["name"], "x");
        assert!(blocks[0].get("stray").is_none());
    }

    #[test]
    fn safe_filename_keeps_normal_names() {
        assert_eq!(
            safe_script_filename("Chat-Calculator v3.lua").unwrap(),
            "Chat-Calculator v3.lua"
        );
        assert_eq!(safe_script_filename(" delCG.lua ").unwrap(), "delCG.lua");
    }

    #[test]
    fn safe_filename_strips_paths_to_basename() {
        // Как в v1 (os.path.basename): путь отбрасывается, пишем в корень moonloader
        assert_eq!(safe_script_filename("../../evil.lua").unwrap(), "evil.lua");
        assert_eq!(safe_script_filename("..\\..\\evil.lua").unwrap(), "evil.lua");
        assert_eq!(safe_script_filename("C:\\Windows\\evil.lua").unwrap(), "evil.lua");
        assert!(safe_script_filename("..").is_err());
        assert!(safe_script_filename(".").is_err());
        assert!(safe_script_filename("").is_err());
        assert!(safe_script_filename("  ").is_err());
    }

    #[test]
    fn safe_filename_whitelist_rejects_exotic_chars() {
        assert!(safe_script_filename("#ArzMarket[3_57].lua").is_err());
        assert!(safe_script_filename("a&b.lua").is_err());
        assert!(safe_script_filename("lua\ninject.lua").is_err());
    }
}

#[cfg(test)]
mod library_tests {
    use super::*;
    use std::io::Write as _;

    fn lib_file_stem_pub(f: &str) -> String {
        lib_file_stem(f)
    }

    #[test]
    fn stem_strips_lib_extensions() {
        assert_eq!(lib_file_stem_pub("mimgui.lua"), "mimgui");
        assert_eq!(lib_file_stem_pub("requests.luac"), "requests");
        // dll/asi тоже отрезаются: имя либы должно совпадать с каталогом
        assert_eq!(lib_file_stem_pub("GifPlayerLua.dll"), "GifPlayerLua");
        assert_eq!(lib_file_stem_pub("mimgui.asi"), "mimgui");
        assert_eq!(lib_file_stem_pub("noext"), "noext");
        // короткие имена с точкой в конце не обрезаются в пустоту
        assert_eq!(lib_file_stem_pub(".lua"), ".lua");
    }

    #[test]
    fn lib_dir_creates_canon_when_none() {
        // Нет ни lib, ни libraries → создаётся lib
        let tmp = std::env::temp_dir().join(format!("arl_libtest_a_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let d = moonloader_lib_dir_from(&tmp).unwrap();
        assert!(d.ends_with("lib"));
        assert!(d.is_dir());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn lib_dir_prefers_existing_nonempty_libraries_over_empty_lib() {
        let tmp = std::env::temp_dir().join(format!("arl_libtest_b_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("lib")).unwrap();
        std::fs::create_dir_all(tmp.join("libraries")).unwrap();
        std::fs::write(tmp.join("libraries/samp.lua"), "--x").unwrap();
        let d = moonloader_lib_dir_from(&tmp).unwrap();
        assert!(d.ends_with("libraries"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn lib_dir_prefers_nonempty_lib_over_libraries_when_both() {
        let tmp = std::env::temp_dir().join(format!("arl_libtest_c_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("lib")).unwrap();
        std::fs::create_dir_all(tmp.join("libraries")).unwrap();
        std::fs::write(tmp.join("lib/mimgui.lua"), "--x").unwrap();
        std::fs::write(tmp.join("libraries/old.lua"), "--y").unwrap();
        let d = moonloader_lib_dir_from(&tmp).unwrap();
        assert!(d.ends_with("lib"), "lib имеет приоритет: {:?}", d);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn scan_names_flat_and_dotted() {
        let tmp = std::env::temp_dir().join(format!("arl_scan_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("samp")).unwrap();
        std::fs::create_dir_all(tmp.join("mimgui")).unwrap();
        std::fs::write(tmp.join("requests.lua"), "--r").unwrap();
        std::fs::write(tmp.join("samp/events.lua"), "--e").unwrap();
        std::fs::write(tmp.join("mimgui/mimgui.lua"), "--m").unwrap();
        std::fs::write(tmp.join("junk.txt"), "skip").unwrap();
        let libs = scan_installed_libs(&tmp);
        let names: Vec<&str> = libs
            .iter()
            .filter_map(|v| v["name"].as_str())
            .collect();
        assert!(names.contains(&"requests"), "flat файл: {names:?}");
        assert!(names.contains(&"samp.events"), "подпапка: {names:?}");
        assert!(names.contains(&"mimgui"), "папка==имя файла: {names:?}");
        assert!(!names.contains(&"junk"), "не-lua файл пропускается: {names:?}");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn zip_extracts_everything_into_lib_stripping_moonloader_prefix() {
        let tmp = std::env::temp_dir().join(format!("arl_zip_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let lib = tmp.join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        // Собираем zip в памяти: раскладки mimgui, samp-lua и moon-imgui
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut zw = zip::ZipWriter::new(&mut cursor);
            let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
            zw.start_file("mimgui/init.lua", opts).unwrap();
            zw.write_all(b"-- init").unwrap();
            zw.start_file("mimgui/cimguidx9.dll", opts).unwrap();
            zw.write_all(b"dll-bytes").unwrap();
            zw.start_file("samp/events.lua", opts).unwrap();
            zw.write_all(b"-- events").unwrap();
            zw.start_file("moonloader/lib/imgui.lua", opts).unwrap();
            zw.write_all(b"-- imgui").unwrap();
            zw.start_file("moonloader/lib/MoonImGui.dll", opts).unwrap();
            zw.write_all(b"dll2").unwrap();
            zw.finish().unwrap();
        }
        let bytes = cursor.into_inner();
        let written = extract_lib_zip(&bytes, &lib).unwrap();
        assert_eq!(written.len(), 5);
        // Всё живёт в lib/, раскладка архива сохраняется
        assert!(lib.join("mimgui/init.lua").is_file());
        assert!(lib.join("mimgui/cimguidx9.dll").is_file(), "dll остаётся в lib/mimgui/");
        assert!(lib.join("samp/events.lua").is_file());
        // Префикс moonloader/ отрезается (архив упакован под корень игры)
        assert!(lib.join("imgui.lua").is_file());
        assert!(lib.join("MoonImGui.dll").is_file());
        assert!(!lib.join("moonloader").exists());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn zip_rejects_path_traversal() {
        let tmp = std::env::temp_dir().join(format!("arl_ziptrav_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let lib = tmp.join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut zw = zip::ZipWriter::new(&mut cursor);
            let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
            zw.start_file("../evil.lua", opts).unwrap();
            zw.write_all(b"evil").unwrap();
            zw.start_file("good.lua", opts).unwrap();
            zw.write_all(b"ok").unwrap();
            zw.finish().unwrap();
        }
        let bytes = cursor.into_inner();
        let written = extract_lib_zip(&bytes, &lib).unwrap();
        assert_eq!(written, vec!["good.lua".to_string()], "evil.lua пропущен");
        assert!(!tmp.join("evil.lua").exists());
        assert!(!lib.join("../evil.lua").exists());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn lib_path_is_installed_checks_file() {
        let tmp = std::env::temp_dir().join(format!("arl_chk_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let lib = tmp.join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::write(lib.join("mimgui.lua"), "--x").unwrap();
        assert!(lib_path_is_installed(&lib, "mimgui.lua"));
        assert!(!lib_path_is_installed(&lib, "missing.lua"));
        // путь с подпапкой (canary-файл из каталога)
        std::fs::create_dir_all(lib.join("samp")).unwrap();
        std::fs::write(lib.join("samp/events.lua"), "--e").unwrap();
        assert!(lib_path_is_installed(&lib, "samp/events.lua"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn parse_libraries_catalog() {
        let text = "\
# комментарий
[library]
name = mimgui
description = ImGui bindings.
url = https://example.com/mimgui.zip
version = 1.0.0
path = mimgui/init.lua

[library]
name = samp.events
url = https://example.com/samp/events.lua
path = samp/events.lua
";
        let blocks = parse_catalog_txt(text);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0]["name"], "mimgui");
        assert_eq!(blocks[0]["path"], "mimgui/init.lua");
        assert_eq!(blocks[1]["path"], "samp/events.lua");
    }
}

#[cfg(test)]
mod game_target_tests {
    use super::resolve_game_target;

    #[test]
    fn exe_path_reduces_to_parent_even_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("gta_sa.exe");
        // exe существует
        assert_eq!(
            resolve_game_target(&exe.to_string_lossy()).unwrap(),
            dir.path()
        );
        // exe переименовали/удалили — путь с расширением всё равно exe
        assert_eq!(
            resolve_game_target(&exe.to_string_lossy()).unwrap(),
            dir.path()
        );
    }

    #[test]
    fn folder_path_without_ext_stays_folder() {
        let dir = tempfile::tempdir().unwrap();
        let no_ext = dir.path().join("папка_без_расширения");
        assert_eq!(
            resolve_game_target(&no_ext.to_string_lossy()).unwrap(),
            no_ext
        );
    }

    #[test]
    fn empty_is_none() {
        assert!(resolve_game_target("").is_none());
    }
}
