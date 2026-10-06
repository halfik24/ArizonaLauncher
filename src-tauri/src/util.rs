//! Общие хелперы Tauri-команд: доступ к конфигу, JSON-ответы, HTTP, чтение файлов.
//!
//! Вынесены из `launcher.rs`, чтобы убрать копипасту. Поведение 1-в-1:
//! все ответы фронту — байт в байт как раньше.

use crate::config::{AppState, LauncherConfig};
use std::path::{Path, PathBuf};
use std::sync::MutexGuard;

/// Read-доступ к конфигу (замена `state.config.lock().map_err(...)`).
pub fn lock_config(state: &AppState) -> Result<MutexGuard<'_, LauncherConfig>, String> {
    state.config.lock().map_err(|e| e.to_string())
}

/// Прочитать `game_path` из конфига (ядовитый мьютекс → пустая строка, как раньше).
pub fn game_path(state: &AppState) -> String {
    state
        .config
        .lock()
        .map(|c| c.game_path.clone())
        .unwrap_or_default()
}

/// Папка игры = parent от `game_path`; `None`, если путь не задан.
pub fn game_dir(state: &AppState) -> Option<PathBuf> {
    let gp = game_path(state);
    if gp.is_empty() {
        return None;
    }
    Some(
        Path::new(&gp)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default(),
    )
}

/// Мутация конфига + сохранение: `{ lock; f(&mut cfg); save } → {success:true}`.
pub fn update_config(
    state: &AppState,
    f: impl FnOnce(&mut LauncherConfig) -> Result<(), String>,
) -> Result<serde_json::Value, String> {
    {
        let mut cfg = lock_config(state)?;
        f(&mut cfg)?;
    }
    state.save_config()?;
    Ok(serde_json::json!({"success": true}))
}

/// `Ok({"success": true})`.
pub fn ok_true() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": true}))
}

/// `Ok({"success": false, "message": ...})` — бизнес-ошибка без `Err`.
pub fn fail_message(msg: impl Into<String>) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({"success": false, "message": msg.into()}))
}

/// Один HTTP-запрос с таймаутом (замена копий `Client::builder` в `launcher.rs`).
pub async fn http_request(
    method: reqwest::Method,
    url: &str,
    headers: reqwest::header::HeaderMap,
    timeout: std::time::Duration,
) -> Result<reqwest::Response, String> {
    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| e.to_string())?;
    client
        .request(method, url)
        .headers(headers)
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| e.to_string())
}

/// Декодирование текстовых ответов: utf-8, иначе windows-1251 (как errors='replace').
pub fn decode_text_bytes(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let (cow, _, _) = encoding_rs::WINDOWS_1251.decode(bytes);
            cow.into_owned()
        }
    }
}

/// Python: чтение `#ArizonaPatches.json` со срезанием строк-комментариев `//`.
pub fn clean_patches_text(raw: &str) -> Result<String, String> {
    let re = regex::Regex::new(r"(?m)^\s*//.*").map_err(|e| e.to_string())?;
    let no_comments = re.replace_all(raw, "");
    Ok(no_comments
        .split('\n')
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n"))
}

/// `read_to_string → clean → parse` (общее для `read_patches` и `restore_patches_backup`).
pub fn read_patches_file(path: &Path) -> Result<serde_json::Value, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let cleaned = clean_patches_text(&raw)?;
    serde_json::from_str(&cleaned).map_err(|e| e.to_string())
}

/// Проверка path traversal для имени файла бэкапа.
pub fn safe_backup_path(state: &AppState, filename: &str) -> Result<PathBuf, String> {
    if filename.is_empty()
        || filename.contains('/')
        || filename.contains('\\')
        || filename == "."
        || filename == ".."
    {
        return Err("Недопустимое имя файла".to_string());
    }
    let dir = state.documents_path.join("backups");
    let path = dir.join(filename);
    // Имя файла обязано совпадать (защита от "..", путей и т.п.)
    if path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .as_deref()
        != Some(filename)
    {
        return Err("Недопустимое имя файла".to_string());
    }
    Ok(path)
}
