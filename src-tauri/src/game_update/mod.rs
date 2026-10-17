//! Установка / обновление / починка игры по манифесту `game.json`
//! с CDN Arizona. Логика повторяет официальный Arizona Games Launcher
//! (разобран az-proto, см. README того проекта):
//!
//! * правила проверки файлов: нет файла / размер / mtime против
//!   `date_change` / MD5 (с кэшем хэшей);
//! * качка строго по одному файлу, `.part` → переименование;
//! * фоллбэк на резервный CDN на каждый файл;
//! * после качки — MD5-сверка и установка mtime из манифеста.
//!
//! Отличия от оригинала: MD5 после качки сверяется (оригинал верит
//! манифесту), кэш хэшей свой (`game_update_state.json` в Documents).

pub mod downloader;
pub mod engine;
pub mod manifest;
pub mod state;
pub mod verify;
