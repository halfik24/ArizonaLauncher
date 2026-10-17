//! HTTP и файловые операции для установки игры.
//! Транспорт — reqwest (как во всём бэкенде), качка потоком в `.part`,
//! затем переименование. MD5 — потоково, в отдельном потоке (spawn_blocking).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use anyhow::{bail, Context};

use super::manifest::Root;

const UA: &str = "ArizonaLauncher/2.0";

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .user_agent(UA)
        .build()
        .expect("reqwest Client с дефолтными настройками не может не собраться")
}

/// Манифест: GET {cdn}/game.json (общий клиент, суммарный таймаут 15 с).
async fn try_manifest(http: &reqwest::Client, cdn: &str) -> anyhow::Result<Root> {
    let url = format!("{}/game.json", cdn.trim_end_matches('/'));
    let resp = http
        .get(&url)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("GET {url} — статус ответа"))?;
    let body = resp.text().await.context("чтение game.json")?;
    let root: Root =
        serde_json::from_str(&body).context("game.json не распарсился")?;
    Ok(root)
}

/// Основной CDN, при неудаче — резервный. Возвращает манифест.
pub async fn fetch_manifest(
    http: &reqwest::Client,
    main: &str,
    reserve: &str,
    mut log: impl FnMut(String),
) -> anyhow::Result<Root> {
    match try_manifest(http, main).await {
        Ok(r) => {
            log(format!("Манифест: {main}/game.json"));
            return Ok(r);
        }
        Err(e) => log(format!("Основной CDN не ответил ({main}): {e:#}")),
    }
    let r = try_manifest(http, reserve)
        .await
        .with_context(|| format!("резервный CDN {reserve}"))?;
    log(format!("Манифест: {reserve}/game.json (резерв)"));
    Ok(r)
}

/// Процентное кодирование пути: всё, кроме [A-Za-z0-9-._~/], в %XX.
pub fn encode_rel(rel: &str) -> String {
    let mut out = String::with_capacity(rel.len() + 8);
    for b in rel.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn file_url(cdn: &str, rel: &str) -> String {
    format!("{}/game/{}", cdn.trim_end_matches('/'), encode_rel(rel))
}

/// Путь временного файла рядом с целью: `foo.img` → `foo.img.part`.
fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    name.push_str(".part");
    dest.with_file_name(name)
}

/// Качает url в dest через `.part`, сообщает прогресс (байт файла), умеет отменяться.
pub async fn download_to(
    http: &reqwest::Client,
    url: &str,
    dest: &Path,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(u64) + Send,
) -> anyhow::Result<u64> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("создание папки {}", parent.display()))?;
    }
    let tmp = part_path(dest);
    if tmp.exists() {
        let _ = fs::remove_file(&tmp);
    }

    let resp = http
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("GET {url} — статус ответа"))?;

    let mut f = fs::File::create(&tmp)
        .with_context(|| format!("создание {}", tmp.display()))?;
    let mut total: u64 = 0;
    use std::io::Write;
    let mut resp = resp;
    loop {
        let chunk = resp
            .chunk()
            .await
            .context("чтение ответа")?;
        let Some(chunk) = chunk else {
            break;
        };
        if cancel.load(Ordering::Relaxed) {
            drop(f);
            let _ = fs::remove_file(&tmp);
            bail!("отменено");
        }
        f.write_all(&chunk).context("запись файла")?;
        total += chunk.len() as u64;
        on_progress(total);
    }
    f.flush().context("flush")?;
    drop(f);

    if dest.exists() {
        fs::remove_file(dest).ok();
    }
    fs::rename(&tmp, dest).with_context(|| format!("переименование в {}", dest.display()))?;
    Ok(total)
}

/// MD5 файла (потоково, буфер 64 КБ на стеке). Блокирующая — вызывать
/// из spawn_blocking.
pub fn md5_file_blocking(path: &Path) -> anyhow::Result<String> {
    use md5::{Digest, Md5};
    let mut f = fs::File::open(path).with_context(|| format!("открытие {}", path.display()))?;
    let mut h = Md5::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = std::io::Read::read(&mut f, &mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}

/// MD5 файла в отдельном потоке — чтобы не блокировать async-исполнитель.
pub async fn md5_file(path: PathBuf) -> anyhow::Result<String> {
    tokio::task::spawn_blocking(move || md5_file_blocking(&path))
        .await
        .map_err(|e| anyhow::anyhow!("md5 поток умер: {e}"))?
}

/// Ставит mtime файла = unix seconds (как в манифесте).
pub fn set_mtime(path: &Path, unix: i64) -> std::io::Result<()> {
    if unix <= 0 {
        return Ok(());
    }
    let f = fs::OpenOptions::new().write(true).open(path)?;
    f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(unix as u64))?;
    Ok(())
}

pub fn mtime_secs(md: &fs::Metadata) -> i64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Свободно байт на диске, где лежит path (Windows: GetDiskFreeSpaceExW).
pub fn free_space(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    // Если путь ещё не существует (свежая установка) — берём ближайшего
    // существующего родителя: диск тот же.
    let mut probe = path.to_path_buf();
    loop {
        if probe.parent().is_none() {
            break;
        }
        if probe.exists() {
            break;
        }
        probe = probe.parent()?.to_path_buf();
    }

    let wide: Vec<u16> = probe
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut avail: u64 = 0;
    let mut total: u64 = 0;
    let mut free: u64 = 0;
    let ok = unsafe {
        GetDiskFreeSpaceExW(wide.as_ptr(), &mut avail, &mut total, &mut free) != 0
    };
    ok.then_some(avail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_rel_keeps_safe_and_escapes_rest() {
        assert_eq!(encode_rel("models/trees.img"), "models/trees.img");
        assert_eq!(encode_rel("a b/c,d"), "a%20b/c%2Cd");
        assert_eq!(encode_rel("кириллица"), "%D0%BA%D0%B8%D1%80%D0%B8%D0%BB%D0%BB%D0%B8%D1%86%D0%B0");
    }

    #[test]
    fn file_url_joins_without_double_slash() {
        assert_eq!(
            file_url("https://pc.az-ins.com/release/", "models/trees.img"),
            "https://pc.az-ins.com/release/game/models/trees.img"
        );
    }

    #[test]
    fn part_path_appends_suffix() {
        assert_eq!(
            part_path(Path::new(r"G:\game\models\trees.img")),
            PathBuf::from(r"G:\game\models\trees.img.part")
        );
        assert_eq!(
            part_path(Path::new(r"G:\game\readme")),
            PathBuf::from(r"G:\game\readme.part")
        );
    }
}
