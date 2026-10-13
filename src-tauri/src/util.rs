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

// ==================== Ресурс версии exe (Windows) ====================

/// Чтение строковых полей VS_VERSION_INFO из exe (ProductVersion, ProductName и т.п.).
/// Возвращает строки в порядке `keys`, "" если поле отсутствует.
/// Работает с любым именем файла — берёт данные из ресурса версии.
pub fn exe_version_fields(path: &Path, keys: &[&str]) -> Vec<String> {
    use std::os::windows::ffi::OsStrExt;
    use winapi::um::winver::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut result: Vec<String> = vec![String::new(); keys.len()];

    unsafe {
        let mut handle = 0u32; // DWORD
        let size = GetFileVersionInfoSizeW(wide.as_ptr(), &mut handle);
        if size == 0 {
            return result;
        }
        let mut buf = vec![0u8; size as usize];
        if GetFileVersionInfoW(wide.as_ptr(), handle, size, buf.as_mut_ptr() as *mut _) == 0 {
            return result;
        }

        // Перечень переводов строки: пары (lang, codepage)
        let mut trans = std::ptr::null_mut();
        let mut trans_len = 0u32;
        let sub: Vec<u16> = "\\VarFileInfo\\Translation"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut pairs: Vec<(u16, u16)> = Vec::new();
        if VerQueryValueW(buf.as_ptr() as *const _, sub.as_ptr(), &mut trans, &mut trans_len) != 0
            && !trans.is_null()
        {
            let n = trans_len as usize / 4;
            let t = trans as *const u16;
            for i in 0..n {
                pairs.push((*t.add(i * 2), *t.add(i * 2 + 1)));
            }
        }
        if pairs.is_empty() {
            pairs.push((0x0409, 0x04B0)); // US English — частый fallback
        }

        for (lang, cp) in pairs {
            for (i, key) in keys.iter().enumerate() {
                if !result[i].is_empty() {
                    continue;
                }
                let subkey = format!("\\StringFileInfo\\{:04x}{:04x}\\{}", lang, cp, key);
                let w: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
                let mut val = std::ptr::null_mut();
                let mut vlen = 0u32;
                if VerQueryValueW(buf.as_ptr() as *const _, w.as_ptr(), &mut val, &mut vlen) == 0
                    || val.is_null()
                {
                    continue;
                }
                let u = val as *const u16;
                // vlen у разных Windows значит то байты, то символы — берём
                // щедрый лимит и всё равно читаем ровно до null-терминатора,
                // но не дальше буфера ресурса
                let base = buf.as_ptr() as usize;
                let off = val as usize;
                let remain_u16 = if off >= base && off - base < buf.len() {
                    (buf.len() - (off - base)) / 2
                } else {
                    0
                };
                let max = ((vlen as usize).max(4) * 2 + 2).min(remain_u16);
                let mut len = 0usize;
                while len < max && *u.add(len) != 0 {
                    len += 1;
                }
                if len > 0 {
                    result[i] = String::from_utf16_lossy(std::slice::from_raw_parts(u, len));
                }
            }
        }
    }
    result
}

/// Это exe лаунчера Arizona? Определяется по ресурсу версии, НЕ по имени файла:
/// ProductName/FileDescription = "Launcher for Arizona GTA:SA-MP",
/// CompanyName = "AIR ChannelT", OriginalFilename = "ArizonaLauncher...exe"
/// (OriginalFilename остаётся исходным даже если файл переименовали).
pub fn is_arizona_launcher(path: &Path) -> bool {
    let f = exe_version_fields(
        path,
        &[
            "ProductName",
            "FileDescription",
            "CompanyName",
            "OriginalFilename",
        ],
    );
    let norm = |s: &str| s.to_lowercase();
    let (pn, fd, cn, of) = (norm(&f[0]), norm(&f[1]), norm(&f[2]), norm(&f[3]));
    let arizona_launcher = |s: &str| s.contains("arizona") && s.contains("launcher");
    arizona_launcher(&pn)
        || arizona_launcher(&fd)
        || cn.contains("air channel")
        || of.contains("arizonalauncher")
}

/// Мажорная версия из ProductVersion (fallback — FileVersion), 0 если нет данных.
pub fn exe_major_version(path: &Path) -> u32 {
    let f = exe_version_fields(path, &["ProductVersion", "FileVersion"]);
    for s in &f {
        if let Some(num) = s.split('.').next() {
            if let Ok(v) = num.trim().parse::<u32>() {
                return v;
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_real_launcher_exe() {
        // Реальный exe лаунчера с рабочего стола (если файл есть)
        let p = Path::new(r"C:\Users\halfik\Desktop\ArizonaLauncher8.0_byAIR.exe");
        if !p.exists() {
            eprintln!("файл не найден — пропуск");
            return;
        }
        let f = exe_version_fields(p, &["ProductName", "FileDescription", "CompanyName", "ProductVersion"]);
        assert_eq!(f[0], "Launcher for Arizona GTA:SA-MP", "ProductName: {:?}", f);
        assert_eq!(f[1], "Launcher for Arizona GTA:SA-MP");
        assert_eq!(f[2], "AIR Channel™");
        assert_eq!(f[3], "8.0.0.0");
        assert!(is_arizona_launcher(p), "лаунчер не распознан");
        assert_eq!(exe_major_version(p), 8);

        // Не-лаунчер (обычный exe) не должен распознаваться как лаунчер
        let not_launcher = Path::new(r"C:\Windows\System32\notepad.exe");
        if not_launcher.exists() {
            assert!(!is_arizona_launcher(not_launcher), "notepad распознан как лаунчер!");
        }
    }
}
