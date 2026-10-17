//! Движок: манифест -> план -> применение (качание/даты/папки/удаление).
//! Асинхронный (reqwest + tokio), события шлёт в колбэк, отмена — флагом.

use std::fs;use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{bail, Context};

use super::downloader;
use super::manifest;
use super::state;
use super::verify::{self, Action, Plan};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Check,
    Apply,
    Repair,
}

impl Mode {
    pub fn title(self) -> &'static str {
        match self {
            Mode::Check => "Проверка (только чтение)",
            Mode::Apply => "Установка / обновление",
            Mode::Repair => "Починка",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Job {
    pub mode: Mode,
    pub cdn_main: String,
    pub cdn_reserve: String,
    pub target: PathBuf,
    /// true -> variant "future" (новая графика)
    pub future: bool,
    /// Папка Documents для кэша хэшей (game_update_state.json)
    pub documents: PathBuf,
}

#[derive(Debug, Clone)]
pub enum Event {
    Log(String),
    Progress {
        files_done: usize,
        files_total: usize,
        bytes_done: u64,
        bytes_total: u64,
        current: String,
        speed: u64,
    },
    Finished {
        ok: bool,
        text: String,
    },
}

/// CDN по умолчанию — таблица MAP_GAME_TO_CDN_URLS оригинала, игра `arizona`.
pub const DEFAULT_CDN_MAIN: &str = "https://pc.az-ins.com/release";
pub const DEFAULT_CDN_RESERVE: &str = "https://arz-pc.react-group.tech/release";

/// Запас сверх суммы размеров: мета-операции + temp-файлы.
const FREE_SPACE_MARGIN: u64 = 268_435_456; // 256 MB

pub fn gb(b: u64) -> f64 {
    b as f64 / 1_073_741_824.0
}

/// Манифест + план. Общая часть для `check` и выполнения.
pub async fn build_plan(
    job: &Job,
    http: &reqwest::Client,
    on_event: &(dyn Fn(Event) + Send + Sync),
    cancel: &AtomicBool,
) -> anyhow::Result<(manifest::Flat, Plan)> {
    let log = |m: String| on_event(Event::Log(m));

    let root = downloader::fetch_manifest(http, &job.cdn_main, &job.cdn_reserve, &log).await?;
    let flat = manifest::flatten(&root, job.future);
    log(format!(
        "В манифесте: {} файлов ({:.3} GB), variant={}, пропущено variant-узлов: {}, res-записей: {} (учтутся только при починке)",
        flat.files,
        gb(flat.bytes),
        if job.future { "future" } else { "default" },
        flat.skipped_variant,
        flat.res_entries
    ));

    let repair = job.mode == Mode::Repair;
    let cache = state::load_cache(&job.documents);
    log(format!(
        "Кэш хэшей: {} записей",
        cache.len()
    ));
    let plan = verify::plan(&flat, &job.target, repair, &cache);
    log(format!(
        "Проверено: в порядке {}, md5 пересчитано {}",
        plan.ok, plan.hashed
    ));
    log(format!(
        "Нужно: скачать {} файлов ({:.3} GB), создать папок {}, поправить дат {}, удалить {}",
        plan.download_count,
        gb(plan.download_bytes),
        plan.mkdir_count,
        plan.date_fix,
        plan.delete_count
    ));
    let _ = cancel; // в check отмен не бывает — план считается быстро
    Ok((flat, plan))
}

struct Speed {
    at: Instant,
    base: u64,
}

impl Speed {
    fn new() -> Self {
        Speed { at: Instant::now(), base: 0 }
    }
    /// Скорость не чаще раза в 250 мс; None — рано, события не шлём.
    fn tick(&mut self, total: u64) -> Option<u64> {
        let el = self.at.elapsed();
        if el >= Duration::from_millis(250) {
            let delta = total.saturating_sub(self.base);
            let speed = (delta as f64 / el.as_secs_f64()) as u64;
            self.at = Instant::now();
            self.base = total;
            Some(speed)
        } else {
            None
        }
    }
}

/// Полное выполнение Apply/Repair. Возвращает итоговую строку.
pub async fn run(
    job: &Job,
    on_event: &(dyn Fn(Event) + Send + Sync),
    cancel: &AtomicBool,
) -> anyhow::Result<String> {
    let log = |m: String| on_event(Event::Log(m));

    log(format!("Режим: {}", job.mode.title()));
    log(format!("Папка игры: {}", job.target.display()));

    // Права на запись — до долгих операций
    verify::access_test(&job.target).map_err(|e| {
        anyhow::anyhow!("{e:#} (запусти лаунчер от имени администратора или выбери другую папку)")
    })?;

    let http = downloader::client();
    let (_flat, plan) = build_plan(job, &http, on_event, cancel).await?;

    // Свободное место
    if plan.download_bytes > 0 {
        let free = downloader::free_space(&job.target).unwrap_or(u64::MAX);
        let need = plan.download_bytes + FREE_SPACE_MARGIN;
        if free < need {
            bail!(
                "Недостаточно места на диске: нужно {:.3} GB, свободно {:.3} GB",
                gb(need),
                gb(free)
            );
        }
    }

    let files_total = plan.download_count;
    let bytes_total = plan.download_bytes;
    let mut files_done = 0usize;
    let mut bytes_done = 0u64;
    let mut speed = Speed::new();
    let mut cache = state::load_cache(&job.documents);
    let mut mkdirs = 0usize;
    let mut datefixes = 0usize;
    let mut deletes = 0usize;

    for item in &plan.items {
        if cancel.load(Ordering::Relaxed) {
            bail!("отменено пользователем");
        }
        let rel = &item.entry.rel;
        let path = job.target.join(rel);

        match item.action {
            Action::Mkdir => {
                fs::create_dir_all(&path)
                    .with_context(|| format!("создание папки {}", path.display()))?;
                mkdirs += 1;
            }
            Action::Delete => {
                log(format!("удаляю {rel}"));
                fs::remove_file(&path).ok();
                deletes += 1;
            }
            Action::DateFix => {
                downloader::set_mtime(&path, item.entry.date).ok();
                datefixes += 1;
            }
            Action::Download => {
                let base = bytes_done;
                on_event(Event::Progress {
                    files_done,
                    files_total,
                    bytes_done: base,
                    bytes_total,
                    current: rel.clone(),
                    speed: 0,
                });

                // Сначала основной CDN, потом резервный; после успешной качки —
                // MD5-сверка. Битая качка (md5 мимо) НЕ убивает прогон: пробуем
                // другой CDN, и только потом ошибка.
                let mut via: Option<&'static str> = None;
                for (label, cdn) in [
                    ("основном", &job.cdn_main),
                    ("резервном", &job.cdn_reserve),
                ] {
                    let url = downloader::file_url(cdn, rel);
                    let res = downloader::download_to(&http, &url, &path, cancel, |file_bytes| {
                        let total = base + file_bytes;
                        if let Some(s) = speed.tick(total) {
                            on_event(Event::Progress {
                                files_done,
                                files_total,
                                bytes_done: total,
                                bytes_total,
                                current: rel.clone(),
                                speed: s,
                            });
                        }
                    })
                    .await;
                    match res {
                        Err(e) => {
                            if cancel.load(Ordering::Relaxed) {
                                bail!("отменено пользователем");
                            }
                            log(format!("не взялось с {label} CDN ({rel}): {e:#}"));
                        }
                        Ok(_n) => {
                            if item.entry.hash.is_empty() {
                                via = Some(label);
                                break;
                            }
                            let h = downloader::md5_file(path.clone()).await?;
                            if h == item.entry.hash {
                                via = Some(label);
                                break;
                            }
                            log(format!(
                                "md5 не совпал после качки с {label} CDN: {rel} — пробую другой CDN"
                            ));
                            // битый файл убираем: на месте папки verify бы
                            // посчитал его «файлом не того размера»
                            fs::remove_file(&path).ok();
                        }
                    }
                }
                let Some(via) = via else {
                    bail!("не удалось скачать {rel}: оба CDN не дали файл с верным md5");
                };
                let _ = via;

                downloader::set_mtime(&path, item.entry.date).ok();
                cache.insert(rel.clone(), item.entry.hash.clone());

                files_done += 1;
                bytes_done += item.entry.size;
                if files_done <= 40 || files_done == files_total {
                    log(format!(
                        "[{files_done}/{files_total}] {rel} — {:.3} GB",
                        gb(item.entry.size)
                    ));
                } else if files_done == files_total || files_done == 41 {
                    log(format!("... (дальше не логирую каждое), {files_done}/{files_total}"));
                }
                on_event(Event::Progress {
                    files_done,
                    files_total,
                    bytes_done,
                    bytes_total,
                    current: rel.clone(),
                    speed: 0,
                });
            }
        }
    }

    state::save_cache(&job.documents, &cache)
        .with_context(|| "сохранение кэша хэшей")?;
    let summary = format!(
        "Готово: скачано {files_done} файлов ({:.3} GB), создано папок {mkdirs}, поправлено дат {datefixes}, удалено {deletes}",
        gb(bytes_done)
    );
    log(summary.clone());
    Ok(summary)
}
