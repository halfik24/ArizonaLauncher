//! Проверка локальных файлов против манифеста (логика официального
//! лаунчера, main.js: k/we). Ничего не пишет — только возвращает план действий.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::downloader::{md5_file_blocking, mtime_secs};
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
pub fn plan(
    flat: &Flat,
    target: &Path,
    repair: bool,
    cache: &HashMap<String, String>,
) -> Plan {
    let mut p = Plan::default();

    for e in &flat.entries {
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

                if need_hash {
                    let cached_ok = cache.get(&e.rel).map(|h| h == &e.hash).unwrap_or(false);
                    let hash_ok = if cached_ok {
                        true
                    } else {
                        p.hashed += 1;
                        md5_file_blocking(&path).map(|h| h == e.hash).unwrap_or(false)
                    };
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
                        if mtime != e.date {
                            p.date_fix += 1;
                            p.items.push(Item {
                                entry: e.clone(),
                                action: Action::DateFix,
                                reason: "хэш верный, нужна только дата".into(),
                            });
                        }
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
            }
        }
    }
    p
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
        let p = plan(&flat, dir.path(), false, &HashMap::new());
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
        let p = plan(&flat, dir.path(), false, &HashMap::new());
        assert_eq!(p.hashed, 0);
        assert_eq!(p.date_fix, 1); // mtime новее -> DateFix

        // дата манифеста новее локальной (год 2096) -> считаем md5; кэш от ускоряет
        const FUTURE_DATE: i64 = 4_000_000_000;
        let mut flat2 = Flat::default();
        flat2.entries.push(entry("a.img", 5, &md5, FUTURE_DATE));
        let p2 = plan(&flat2, dir.path(), false, &HashMap::new());
        assert_eq!(p2.hashed, 1);
        assert_eq!(p2.ok, 1);
        assert_eq!(p2.date_fix, 1); // хэш верный, ставим дату

        let p3 = plan(&flat2, dir.path(), false, &HashMap::from([("a.img".to_string(), md5.clone())]));
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
        let p = plan(&flat, dir.path(), false, &HashMap::new());
        assert_eq!(p.download_count, 0);
        let p = plan(&flat, dir.path(), true, &HashMap::new());
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
        let p = plan(&flat, dir.path(), false, &HashMap::new());
        assert_eq!(p.download_count, 1);
    }
}
