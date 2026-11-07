//! Проверка локальных файлов против манифеста (логика официального
//! лаунчера, main.js: k/we). Ничего не пишет — только возвращает план действий.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use super::downloader::mtime_secs;
use super::manifest::{Entry, Flat, Kind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Mkdir,
    Delete,
    DateFix,
    Download,
}

#[derive(Debug, Clone)]
pub struct Item {
    pub entry: Entry,
    pub action: Action,
    pub reason: String,
}

#[derive(Debug, Default)]
pub struct Plan {
    /// только то, что нужно сделать (Mkdir/Delete/DateFix/Download)
    pub items: Vec<Item>,
    pub download_count: usize,
    pub download_bytes: u64,
    pub date_fix: usize,
    pub delete_count: usize,
    pub mkdir_count: usize,
    /// файлы, которые уже в порядке
    pub ok: usize,
    /// сколько md5 реально пересчитано
    pub hashed: usize,
}

/// repair=true — считать md5 у всех файлов (режим «Починить»).
/// cache — rel -> md5 из прошлых прогонов (наш собственный кэш).
/// on_scan — прогресс по хэшируемым файлам: (bytes_done, bytes_total, current).
/// cancel — досрочный выход (частичный план; вызывающий обязан проверить
/// флаг сам и отбросить результат — без этого «Отменить» не действует,
/// пока MD5-скан больших папок не закончится).
/// on_log — лог перед 2-й фазой: сколько md5 предстоит посчитать.
pub fn plan(
    flat: &Flat,
    target: &Path,
    repair: bool,
    cache: &HashMap<String, String>,
    on_scan: Option<&(dyn Fn(u64, u64, &str) + Send + Sync)>,
    cancel: Option<&AtomicBool>,
    on_log: Option<&(dyn Fn(String) + Send + Sync)>,
) -> Plan {
    let cancelled = || cancel.is_some_and(|c| c.load(Ordering::Relaxed));
    let mut p = Plan::default();
    let mut scanned_bytes: u64 = 0;
    /// Файлы, чей md5 ещё не посчитан (2-я фаза, параллельные воркеры).
    struct Pending {
        path: PathBuf,
        expect: String,
        size: u64,
        mtime: i64,
        entry: Entry,
    }
    let mut pending: Vec<Option<Pending>> = Vec::new();

    for e in &flat.entries {
        if cancelled() {
            break;
        }
        // res-записи лаунчер берёт только при починке
        if e.res && !repair {
            continue;
        }
        let path = target.join(&e.rel);

        match e.kind {
            Kind::Dir => {
                if path.is_dir() {
                    p.ok += 1;
                } else {
                    p.mkdir_count += 1;
                    p.items.push(Item {
                        entry: e.clone(),
                        action: Action::Mkdir,
                        reason: "нет папки".into(),
                    });
                }
            }
            Kind::Delete => {
                if let Ok(md) = fs::metadata(&path) {
                    if md.is_file() && mtime_secs(&md) < e.date {
                        p.delete_count += 1;
                        p.items.push(Item {
                            entry: e.clone(),
                            action: Action::Delete,
                            reason: "манифест просит удалить".into(),
                        });
                    }
                }
            }
            Kind::File | Kind::Check => {
                if let Some(cb) = on_scan {
                    cb(scanned_bytes, flat.bytes, &e.rel);
                }
                let md = match fs::metadata(&path) {
                    Ok(m) if m.is_file() => m,
                    Ok(_) => {
                        p.download_count += 1;
                        p.download_bytes += e.size;
                        p.items.push(Item {
                            entry: e.clone(),
                            action: Action::Download,
                            reason: "на месте папка, а не файл".into(),
                        });
                        continue;
                    }
                    Err(_) => {
                        p.download_count += 1;
                        p.download_bytes += e.size;
                        p.items.push(Item {
                            entry: e.clone(),
                            action: Action::Download,
                            reason: "нет файла".into(),
                        });
                        continue;
                    }
                };

                if md.len() != e.size {
                    p.download_count += 1;
                    p.download_bytes += e.size;
                    p.items.push(Item {
                        entry: e.clone(),
                        action: Action::Download,
                        reason: format!("размер {} вместо {}", md.len(), e.size),
                    });
                    continue;
                }

                let mtime = mtime_secs(&md);
                // когда лаунчер считает md5:
                //  - repair: всегда
                //  - check-узел с разной датой
                //  - файл, чья дата в манифесте НОВЕЕ локальной
                let need_hash = repair
                    || (e.kind == Kind::Check && mtime != e.date)
                    || (e.kind == Kind::File && e.date > mtime);

                let mut queued_now = false;
                if need_hash {
                    let cached_ok = cache.get(&e.rel).map(|h| h == &e.hash).unwrap_or(false);
                    if cached_ok {
                        // кэш уже верит файлу (размер и дата не менялись)
                        p.ok += 1;
                        if mtime != e.date {
                            p.date_fix += 1;
                            p.items.push(Item {
                                entry: e.clone(),
                                action: Action::DateFix,
                                reason: "хэш верный, нужна только дата".into(),
                            });
                        }
                    } else {
                        // хэш посчитают воркеры параллельно ниже;
                        // размер queued-файла вернём в прогресс на 2-й фазе
                        p.hashed += 1;
                        queued_now = true;
                        pending.push(Some(Pending {
                            path: path.clone(),
                            expect: e.hash.clone(),
                            size: e.size,
                            mtime,
                            entry: e.clone(),
                        }));
                    }
                } else if e.kind == Kind::File && mtime > e.date {
                    // файл новее манифеста: лаунчер только переставляет дату
                    p.date_fix += 1;
                    p.items.push(Item {
                        entry: e.clone(),
                        action: Action::DateFix,
                        reason: "дата новее манифеста".into(),
                    });
                } else {
                    p.ok += 1;
                }
                if !queued_now {
                    scanned_bytes += e.size;
                }
            }
        }
    }

    // 2-я фаза: md5 у queued-файлов считают параллельно.
    // Воркеры забирают индексы из атомарного счётчика; результаты пишут
    // в flags по своему индексу, план в конце собирается В ИСХОДНОМ
    // порядке очереди — итог детерминированный и совпадает с однопотоком.
    if pending.iter().any(|x| x.is_some()) {
        let workers = std::thread::available_parallelism()
            .map(|v| v.get())
            .unwrap_or(4)
            .min(8)
            .min(pending.len().max(1));
        // Лог перед сканом: иначе в логе непонятно, сколько md5 предстоит
        // посчитать и почему отмена «догоняет» столько секунд
        if !cancelled() {
            if let Some(cb) = on_log {
                let files = pending.iter().flatten().count();
                let mb = pending.iter().flatten().map(|x| x.size).sum::<u64>() / 1_048_576;
                cb(format!("md5-скан: {files} файлов ({mb} MB), воркеров {workers}"));
            }
        }
        let next = AtomicUsize::new(0);
        let bytes_done = AtomicU64::new(0);
        let mut flags: Vec<bool> = Vec::with_capacity(pending.len());
        flags.resize(pending.len(), false);
        let flags = std::sync::Mutex::new(flags);
        std::thread::scope(|s| {
            for _ in 0..workers {
                s.spawn(|| loop {
                    if cancelled() {
                        break;
                    }
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= pending.len() {
                        break;
                    }
                    let Some(pw) = pending[i].as_ref() else { continue };
                    // Отменяемый md5: обычный md5_file_blocking прервать
                    // нельзя, и один большой файл досчитывался бы целиком
                    let ok = match md5_file_blocking_if(&pw.path, &cancelled) {
                        Ok(h) => h == pw.expect,
                        // отмена — выходим, файл не считаем
                        Err(_) if cancelled() => break,
                        Err(_) => false,
                    };
                    if let Ok(mut f) = flags.lock() {
                        f[i] = ok;
                    }
                    if let Some(cb) = on_scan {
                        let d = bytes_done.fetch_add(pw.size, Ordering::Relaxed) + pw.size;
                        cb(
                            scanned_bytes /* база без queued */ + d,
                            flat.bytes,
                            &pw.entry.rel,
                        );
                    }
                });
            }
        });
        // Паника воркера отравила бы лок — тогда все непроверенные файлы
        // честно считаем несовпавшими (false нужной длины), иначе flags[i]
        // ниже — index out of bounds
        let flags = match flags.into_inner() {
            Ok(f) => f,
            Err(_) => vec![false; pending.len()],
        };
        for (i, pw) in pending.iter_mut().enumerate() {
            let Some(item) = pw.take() else { continue };
            let hash_ok = flags[i];
            let e = &item.entry;
            if !hash_ok {
                p.download_count += 1;
                p.download_bytes += e.size;
                p.items.push(Item {
                    entry: e.clone(),
                    action: Action::Download,
                    reason: "md5 не совпал".into(),
                });
            } else {
                p.ok += 1;
                if item.mtime != e.date {
                    p.date_fix += 1;
                    p.items.push(Item {
                        entry: e.clone(),
                        action: Action::DateFix,
                        reason: "хэш верный, нужна только дата".into(),
                    });
                }
            }
        }
    }
    p
}

/// MD5 с проверкой отмены: читает по 1 МБ, каждые ~16 МБ зовёт `cancelled`,
/// при отмене — Err. Нужен воркерам 2-й фазы `plan`: обычный
/// `md5_file_blocking` прервать нельзя, и отмена «догоняла» скан
/// только после досчёта всех больших файлов.
pub fn md5_file_blocking_if(
    path: &Path,
    cancelled: &(dyn Fn() -> bool + Send + Sync),
) -> anyhow::Result<String> {
    use md5::{Digest, Md5};
    if cancelled() {
        anyhow::bail!("отменено пользователем");
    }
    let mut f = fs::File::open(path)
        .map_err(|e| anyhow::anyhow!("открытие {}: {e}", path.display()))?;
    let mut h = Md5::new();
    let mut buf = [0u8; 1024 * 1024];
    let mut since_check: u64 = 0;
    loop {
        let n = std::io::Read::read(&mut f, &mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        since_check += n as u64;
        if since_check >= 16 * 1024 * 1024 {
            if cancelled() {
                anyhow::bail!("отменено пользователем");
            }
            since_check = 0;
        }
    }
    Ok(format!("{:x}", h.finalize()))
}

/// Проверка, что в папку можно писать (аналог accessTestFile.txt у лаунчера).
pub fn access_test(target: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(target)
        .map_err(|e| anyhow::anyhow!("не удалось создать {}: {e}", target.display()))?;
    let probe = target.join(".__az_write_test");
    fs::write(&probe, b"writable?")
        .map_err(|e| anyhow::anyhow!("нет прав на запись в {}: {e}", target.display()))?;
    let _ = fs::remove_file(&probe);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_update::downloader::md5_file_blocking;
    use crate::game_update::manifest;

    fn entry(rel: &str, size: u64, hash: &str, date: i64) -> Entry {
        Entry {
            rel: rel.into(),
            kind: Kind::File,
            size,
            hash: hash.into(),
            date,
            res: false,
        }
    }

    #[test]
    fn plan_downloads_missing_and_size_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        // file_ok — совпадает размер, дата совпадает -> ok
        std::fs::write(dir.path().join("ok.img"), b"12345").unwrap();
        // file_bad — размер не тот
        std::fs::write(dir.path().join("bad.img"), b"123").unwrap();

        let flat = Flat {
            entries: vec![
                entry("ok.img", 5, "x", 0),  // date 0: mtime файла новее -> DateFix
                entry("bad.img", 9, "x", 0), // размер != -> Download
                entry("gone.img", 4, "x", 0), // нет -> Download
            ],
            files: 3,
            bytes: 18,
            ..Default::default()
        };
        let p = plan(&flat, dir.path(), false, &HashMap::new(), None, None, None);
        assert_eq!(p.download_count, 2);
        assert_eq!(p.download_bytes, 13);
        // ok.img: дата в манифесте 0 не новее локальной, локальная новее -> DateFix
        assert_eq!(p.date_fix, 1);
    }

    #[test]
    fn plan_uses_hash_cache_and_respects_mtime_rules() {
        let dir = tempfile::tempdir().unwrap();
        let content = b"hello";
        std::fs::write(dir.path().join("a.img"), content).unwrap();
        let md5 = md5_file_blocking(&dir.path().join("a.img")).unwrap();

        let mut flat = Flat::default();
        // date_change = 0 -> need_hash = false (дата манифеста не новее),
        // значит хэш не считается вовсе
        flat.entries.push(entry("a.img", 5, "whatever", 0));
        let p = plan(&flat, dir.path(), false, &HashMap::new(), None, None, None);
        assert_eq!(p.hashed, 0);
        assert_eq!(p.date_fix, 1); // mtime новее -> DateFix

        // дата манифеста новее локальной (год 2096) -> считаем md5; кэш от ускоряет
        const FUTURE_DATE: i64 = 4_000_000_000;
        let mut flat2 = Flat::default();
        flat2.entries.push(entry("a.img", 5, &md5, FUTURE_DATE));
        let p2 = plan(&flat2, dir.path(), false, &HashMap::new(), None, None, None);
        assert_eq!(p2.hashed, 1);
        assert_eq!(p2.ok, 1);
        assert_eq!(p2.date_fix, 1); // хэш верный, ставим дату

        let p3 = plan(&flat2, dir.path(), false, &HashMap::from([("a.img".to_string(), md5.clone())]), None, None, None);
        assert_eq!(p3.hashed, 0); // из кэша
        assert_eq!(p3.ok, 1);
    }

    #[test]
    fn res_entries_only_in_repair() {
        let dir = tempfile::tempdir().unwrap();
        let flat = Flat {
            entries: vec![Entry { res: true, ..entry("res/c.ini", 3, "x", 0) }],
            res_entries: 1,
            ..Default::default()
        };
        let p = plan(&flat, dir.path(), false, &HashMap::new(), None, None, None);
        assert_eq!(p.download_count, 0);
        let p = plan(&flat, dir.path(), true, &HashMap::new(), None, None, None);
        assert_eq!(p.download_count, 1); // нет файла + repair
    }

    #[test]
    fn access_test_reports_unwritable() {
        let dir = tempfile::tempdir().unwrap();
        assert!(access_test(dir.path()).is_ok());
        // Несуществующий корень диска создать нельзя
        assert!(access_test(Path::new(r"Q:\не\существует")).is_err());
    }

    #[test]
    fn manifest_and_verify_smoke() {
        // Пара manifest->verify на пустой папке: всё качать
        let json = r#"{"data":[{"type":"file","name":"a.txt","size":2,"hash":"h","date_change":1}]}"#;
        let root: manifest::Root = serde_json::from_str(json).unwrap();
        let flat = manifest::flatten(&root, true);
        let dir = tempfile::tempdir().unwrap();
        let p = plan(&flat, dir.path(), false, &HashMap::new(), None, None, None);
        assert_eq!(p.download_count, 1);
    }
}
