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

/// Сколько готовы ждать один чанк ответа («залипшее» соединение).
const CHUNK_TIMEOUT: Duration = Duration::from_secs(30);

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
/// `cancel` проверяется до и после каждого CDN-прохода: сам запрос
/// ограничен таймаутом 15 с (`try_manifest`), но без опроса флага
/// отмена во время манифеста ждала бы до двух проходов подряд.
pub async fn fetch_manifest(
    http: &reqwest::Client,
    main: &str,
    reserve: &str,
    mut log: impl FnMut(String),
    cancel: &AtomicBool,
) -> anyhow::Result<Root> {
    if cancel.load(Ordering::Relaxed) {
        bail!("отменено пользователем");
    }
    let main_res = try_manifest(http, main).await;
    if cancel.load(Ordering::Relaxed) {
        bail!("отменено пользователем");
    }
    match main_res {
        Ok(r) => {
            log(format!("Манифест: {main}/game.json"));
            return Ok(r);
        }
        Err(e) => log(format!("Основной CDN не ответил ({main}): {e:#}")),
    }
    let reserve_res = try_manifest(http, reserve).await;
    if cancel.load(Ordering::Relaxed) {
        bail!("отменено пользователем");
    }
    let r = reserve_res.with_context(|| format!("резервный CDN {reserve}"))?;
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

/// Ждать, пока пауза снята. Проверяет и cancel: во время паузы отмена тоже
/// работает (иначе кнопка «Отменить» зависнет). true — вышли из-за отмены.
fn wait_while_paused(
    pause: &AtomicBool,
    cancel: &AtomicBool,
    on_tick: &mut (dyn FnMut() + Send),
) -> bool {
    while pause.load(Ordering::Relaxed) {
        if cancel.load(Ordering::Relaxed) {
            return true;
        }
        on_tick();
        std::thread::sleep(Duration::from_millis(120));
    }
    cancel.load(Ordering::Relaxed)
}

/// Для `select!` вокруг запроса: резолвится сразу, как только поднят флаг
/// отмены (опрос раз в 150 мс). Без этого ожидание заголовков отмену
/// не видит вовсе — `connect_timeout` обрывает только установку соединения.
async fn wait_until_cancelled(cancel: &AtomicBool) {
    while !cancel.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
}

/// Качает url в dest через `.part`, сообщает прогресс (байт файла),
/// умеет отменяться и ставиться на паузу.
/// rate_bytes — лимит скорости (байт/с); None — качаем без ограничения.
pub async fn download_to(
    http: &reqwest::Client,
    url: &str,
    dest: &Path,
    rate_bytes: u64,
    cancel: &AtomicBool,
    pause: &AtomicBool,
    mut on_progress: impl FnMut(u64) + Send,
) -> anyhow::Result<u64> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("создание папки {}", parent.display()))?;
    }
    let tmp = part_path(dest);

    // Докачка с места обрыва: если .part уже есть — просим Range и
    // дописываем хвост; сервер проигнорировал (200) или битый Range (416) —
    // начинаем с нуля. Валидность конечного файла всё равно решает MD5.
    let mut existing = fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);
    let mut want_range = existing > 0;
    let mut attempts = 0u8;
    let (resp, base, append) = loop {
        // Отмена в начале каждой попытки: иначе запрос ушел бы впустую
        // (в т.ч. когда сюда зашли после отменённого md5)
        if cancel.load(Ordering::Relaxed) {
            bail!("отменено");
        }
        attempts += 1;
        if attempts > 3 {
            bail!("не удалось открыть {url}: более 3 попыток запроса");
        }
        let mut req = http.get(url);
        if want_range {
            req = req.header(reqwest::header::RANGE, format!("bytes={existing}-"));
        }
        // Ожидание заголовков раньше не было ничем ограничено и отмены
        // не видело: ждём либо ответ, либо сигнал отмены
        let r = tokio::select! {
            res = req.send() => res.with_context(|| format!("GET {url}"))?,
            _ = wait_until_cancelled(cancel) => bail!("отменено"),
        };
        let st = r.status().as_u16();
        if st == 206 && existing > 0 {
            break (r, existing, true);
        }
        if st == 416 && existing > 0 {
            // .part оказался длиннее файла или битый — чистим, пробуем заново
            let _ = fs::remove_file(&tmp);
            existing = 0;
            want_range = false;
            continue;
        }
        if st >= 400 {
            bail!("GET {url} — статус ответа {st}");
        }
        // 200 (сервер проигнорировал Range, либо файла раньше не было)
        let _ = fs::remove_file(&tmp);
        break (r, 0u64, false);
    };

    use std::io::Write;
    let mut f = if append {
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&tmp)
            .with_context(|| format!("дописывание {}", tmp.display()))?
    } else {
        fs::File::create(&tmp)
            .with_context(|| format!("создание {}", tmp.display()))?
    };
    let mut total: u64 = base;
    on_progress(total);
    // Токен-ведро: базовая точка времени + total/rate = «когда позволено».
    // Проседания сервера компенсируются (сервер отдал стоп — не досыпаем).
    // Любой rate > 0 троттлит: раньше тут был порог >= 1 МБ/с и мелкие
    // лимиты (Кбит/с, КБ/с) молча игнорировались.
    let rate = (rate_bytes > 0).then_some(rate_bytes);
    let mut due: Option<std::time::Instant> = None;
    let mut resp = resp;
    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(f);
            // .part НЕ трогаем — при повторном запуске допишем с места обрыва
            bail!("отменено");
        }
        // Пауза: стоим на месте, пока фронт не снимет. .part не трогаем —
        // после снятия качаем дальше с того же байта.
        if wait_while_paused(pause, cancel, &mut || {}) {
            drop(f);
            bail!("отменено");
        }
        // Таймаут на каждый чанк: connect_timeout не спасает от «залипшего»
        // соединения, когда TCP жив, но сервер молчит — без этого качка
        // (и кнопка «Отменить», которая проверяется между чанками) висит часами
        let chunk = match tokio::time::timeout(CHUNK_TIMEOUT, resp.chunk()).await {
            Ok(c) => c.context("чтение ответа")?,
            Err(_) => {
                drop(f);
                bail!("чтение ответа: сервер молчит дольше {} с", CHUNK_TIMEOUT.as_secs());
            }
        };
        let Some(chunk) = chunk else {
            break;
        };
        if cancel.load(Ordering::Relaxed) {
            drop(f);
            // .part НЕ трогаем — при повторном запуске допишем с места обрыва
            bail!("отменено");
        }
        f.write_all(&chunk).context("запись файла")?;
        total += chunk.len() as u64;
        on_progress(total);
        if let Some(rate) = rate {
            let started = *due.get_or_insert(std::time::Instant::now());
            let deadline =
                started + Duration::from_secs_f64(total as f64 / rate as f64);
            while deadline > std::time::Instant::now() {
                let sleep = (deadline - std::time::Instant::now()).min(Duration::from_millis(200));
                tokio::time::sleep(sleep).await;
                if cancel.load(Ordering::Relaxed) {
                    drop(f);
                    bail!("отменено");
                }
                if pause.load(Ordering::Relaxed) {
                    // На паузе «дедлайн» токенов замораживаем: базу не двигаем,
                    // чтобы после снятия не пытаться «догнать» время паузы.
                    if wait_while_paused(pause, cancel, &mut || {}) {
                        drop(f);
                        bail!("отменено");
                    }
                    due = None;
                    break;
                }
            }
        }
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

    /// Регресс порога rate > 0: лимит меньше 1 МБ/с реально троттлит.
    /// Раньше downloader брал rate только >= 1_048_576 — выбор «Кбит/с»
    /// и «КБ/с» в настройках молча игнорировался (localhost без лимита
    /// отдаёт 300 КБ практически мгновенно — это и ловит assert).
    #[tokio::test]
    async fn sub_mb_rate_throttles_download() {
        use std::io::{Read, Write};
        const SIZE: usize = 300_000;
        const RATE: u64 = 150_000; // 150 КБ/с → ~2 c
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let Ok((mut s, _)) = listener.accept() else { return };
            // читаем запрос до конца заголовков
            let mut req = [0u8; 512];
            let mut got = 0;
            while got < req.len() {
                let n = s.read(&mut req[got..]).unwrap_or(0);
                if n == 0 { break; }
                got += n;
                if req[..got].windows(4).any(|w | w == b"\r\n\r\n") { break; }
            }
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {SIZE}\r\nConnection: close\r\n\r\n"
            );
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(&vec![0u8; SIZE]);
            let _ = s.flush();
        });

        let dir = std::env::temp_dir().join(format!("az-dl-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let dest = dir.join("file.bin");
        let cancel = AtomicBool::new(false);
        let pause = AtomicBool::new(false);
        let url = format!("http://{addr}/file.bin");

        let started = std::time::Instant::now();
        let got = download_to(&client(), &url, &dest, RATE, &cancel, &pause, |_| {})
            .await
            .expect("качка с лимитом 150 КБ/с должна пройти");
        let elapsed = started.elapsed();

        assert_eq!(got, SIZE as u64);
        assert!(
            elapsed >= Duration::from_millis(1400),
            "300 КБ при лимите 150 КБ/с ушли за {elapsed:?} — \
             rate < 1 МБ/с снова игнорируется?"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
