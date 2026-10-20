use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LaunchParams {
    pub memory: u32,
    pub widescreen: bool,
    pub texture_mode: bool,
    pub color_depth_16: bool,
    pub allow_hdr: bool,
    pub enable_grass: bool,
    pub ldo: bool,
    pub auth_cef_enable: bool,
    pub window_mode: bool,
    pub cdn: String,
    pub autologin: bool,
    pub enable_new_grass: bool,
    pub old_window: bool,
    pub use_d3dx9_43: bool,
    pub show_dialog_ids: bool,
    pub offcef: bool,
    pub modern_scale: bool,
    pub trees_new: bool,
}

impl Default for LaunchParams {
    fn default() -> Self {
        Self {
            memory: 4096,
            widescreen: false,
            texture_mode: false,
            color_depth_16: false,
            allow_hdr: false,
            enable_grass: false,
            ldo: false,
            auth_cef_enable: false,
            window_mode: true,
            cdn: "1,1,1".to_string(),
            autologin: true,
            enable_new_grass: false,
            old_window: false,
            use_d3dx9_43: false,
            show_dialog_ids: false,
            offcef: false,
            modern_scale: false,
            trees_new: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_size: Option<Vec<i32>>,
    /// Рамка окна: "system" (стандартная Windows) или "custom" (наша,
    /// с собственным тайтлбаром). None => "system".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_frame: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout_order: Option<Vec<String>>,
    /// Поля, не описанные структурой (hide_profiles, status_bar,
    /// status_bar_pos, bg_data, …), — как в легаси: словарь целиком,
    /// иначе save_launcher_settings молча их отбрасывал.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            bg_image: None,
            window_title: None,
            window_size: None,
            window_frame: None,
            layout_order: None,
            extra: serde_json::Map::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherConfig {
    pub last_nickname: String,
    pub last_server: u32,
    pub game_path: String,
    pub launcher_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_launcher_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patches_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patches_last_check: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patches_update_available: Option<String>,
    pub launch_params: LaunchParams,
    pub launcher_settings: LauncherSettings,
    /// Способ запуска игры: "native" (запускаем gta_sa.exe напрямую)
    /// или "air" (через ArizonaLauncher*.exe). Старые конфиги без
    /// поля получают "native".
    #[serde(default = "default_launch_method")]
    pub launch_method: String,
    #[serde(default)]
    pub read_news_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profiles: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_profile_id: Option<String>,
}

fn default_launch_method() -> String {
    "native".to_string()
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            last_nickname: String::new(),
            last_server: 15,
            game_path: String::new(),
            launcher_path: String::new(),
            last_launcher_version: None,
            patches_version: None,
            patches_last_check: None,
            patches_update_available: None,
            launch_params: LaunchParams::default(),
            launcher_settings: LauncherSettings::default(),
            launch_method: default_launch_method(),
            read_news_ids: Vec::new(),
            profiles: None,
            active_profile_id: None,
        }
    }
}

/// `snake_case` → `camelCase` для ключей JSON-объекта (рекурсивно).
/// Используется только для миграции конфига легаси-версии.
fn snake_key_to_camel(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    let mut upper = false;
    for c in key.chars() {
        if c == '_' {
            upper = true;
            continue;
        }
        if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn keys_snake_to_camel(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(k, v)| (snake_key_to_camel(&k), keys_snake_to_camel(v)))
                .collect(),
        ),
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(keys_snake_to_camel).collect())
        }
        other => other,
    }
}

/// Миграция легаси-конфига (Python, snake_case) → наш формат.
///
/// Особый случаи:
/// * `launcher_settings`: известные поля структуры переводим в camelCase,
///   а extra-поля (`hide_profiles`, `status_bar`, `bg_data`, …) оставляем
///   в snake_case — фронт читает их именно в таком виде;
/// * `launch_method`: легаси всегда запускал игру через Air-лаунчер
///   (с preloading_plugins/патчами) — дефолт "native" сломал бы это.
fn migrate_legacy_config(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(mut map) => {
            // Апгрейд с легаси: поле launch_method в том конфиге не существовало
            if !map.contains_key("launch_method") && !map.contains_key("launchMethod") {
                map.insert(
                    "launchMethod".to_string(),
                    serde_json::Value::String("air".to_string()),
                );
            }
            serde_json::Value::Object(
                map.into_iter()
                    .map(|(k, v)| {
                        let key = snake_key_to_camel(&k);
                        let val = match key.as_str() {
                            "launchParams" => keys_snake_to_camel(v),
                            "launcherSettings" => migrate_legacy_settings(v),
                            _ => v,
                        };
                        (key, val)
                    })
                    .collect(),
            )
        }
        other => other,
    }
}

fn migrate_legacy_settings(value: serde_json::Value) -> serde_json::Value {
    const KNOWN: &[(&str, &str)] = &[
        ("bg_image", "bgImage"),
        ("window_title", "windowTitle"),
        ("window_size", "windowSize"),
        ("window_frame", "windowFrame"),
        ("layout_order", "layoutOrder"),
    ];
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(k, v)| {
                    let renamed = KNOWN
                        .iter()
                        .find(|(snake, _)| *snake == k.as_str())
                        .map(|(_, camel)| (*camel).to_string())
                        .unwrap_or(k);
                    (renamed, v)
                })
                .collect(),
        ),
        other => other,
    }
}

pub struct ServersCache {
    pub ts: std::time::Instant,
    pub data: Vec<serde_json::Value>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

pub struct AppState {
    pub config: Mutex<LauncherConfig>,
    pub config_path: PathBuf,
    pub documents_path: PathBuf,
    pub servers_cache: Mutex<Option<ServersCache>>,
    /// Фоновое обновление серверов уже запущено (stale-while-revalidate).
    pub servers_revalidating: std::sync::atomic::AtomicBool,
    /// Фоновое обновление новостей уже запущено (stale-while-revalidate).
    pub news_revalidating: std::sync::atomic::AtomicBool,
    /// Активная задача установки/обновления/починки игры: флаг отмены.
    /// None — задач нет; Some — задача идёт (флаг=true => отмена).
    pub game_update_cancel: Mutex<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>>,
    /// Флаг паузы активной задачи: true — качка на паузе (задача ждёт).
    /// Сбрасывается при старте каждой новой задачи.
    pub game_update_pause: Mutex<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>>,
    /// Последняя папка, выбранная пользователем в нативном диалоге
    /// select_game_path без игры (код EMPTY_FOLDER): install_game_to_folder
    /// принимает только её — consent живёт на нативной стороне, а не в вебвью.
    pub last_picked_empty_folder: Mutex<Option<PathBuf>>,
}

impl AppState {
    pub fn new() -> Self {
        let documents_path = dirs::document_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ArizonaLauncher");
        fs::create_dir_all(&documents_path).ok();

        let config_path = documents_path.join("config.json");
        let config = Self::load_config(&config_path);

        Self {
            config: Mutex::new(config),
            config_path,
            documents_path,
            servers_cache: Mutex::new(None),
            servers_revalidating: std::sync::atomic::AtomicBool::new(false),
            news_revalidating: std::sync::atomic::AtomicBool::new(false),
            game_update_cancel: Mutex::new(None),
            game_update_pause: Mutex::new(None),
            last_picked_empty_folder: Mutex::new(None),
        }
    }

    fn load_config(path: &PathBuf) -> LauncherConfig {
        if let Ok(data) = fs::read_to_string(path) {
            // 1) Наш формат (camelCase) — быстрый путь.
            if let Ok(cfg) = serde_json::from_str(&data) {
                return cfg;
            }
            // 2) Легаси-конфиг от Python-лаунчера (snake_case ключи):
            //    конвертируем ключи и пробуем снова, чтобы не обнулить
            //    game_path/ник/параметры у апгрейдящихся пользователей.
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&data) {
                let migrated = migrate_legacy_config(value);
                if let Ok(cfg) = serde_json::from_value(migrated) {
                    return cfg;
                }
            }
            // 3) Совсем битый файл — не перезаписываем молча: оставляем
            //    копию для диагностики (первое сохранение затрёт оригинал).
            fs::copy(path, path.with_extension("json.broken")).ok();
        }
        LauncherConfig::default()
    }

    pub fn save_config(&self) -> Result<(), String> {
        let config = self.config.lock().map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(&*config).map_err(|e| e.to_string())?;
        let tmp = self.config_path.with_extension("json.tmp");
        fs::write(&tmp, &json).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &self.config_path).map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Конфиг, который пишет легаси-лаунчер на Python (main.py:129–141).
    const LEGACY: &str = r#"{
        "last_nickname": "TestPlayer",
        "last_server": 15,
        "game_path": "H:\\Arizona Games Launcher\\bin\\arizona\\gta_sa.exe",
        "launcher_path": "H:\\Arizona Games Launcher\\bin\\ArizonaLauncher8.0_byAIR.exe",
        "patches_version": "1.2.3",
        "patches_last_check": 1700000000,
        "launch_params": {
            "memory": 6144, "widescreen": true, "texture_mode": true,
            "color_depth_16": false, "allow_hdr": true, "enable_grass": true,
            "ldo": false, "auth_cef_enable": true, "window_mode": false,
            "cdn": "2,2,2", "autologin": true, "enable_new_grass": true,
            "old_window": false, "use_d3dx9_43": true, "show_dialog_ids": true,
            "offcef": false, "modern_scale": true, "trees_new": true
        },
        "launcher_settings": {
            "bg_image": "10.jpeg",
            "bg_data": "data:image/jpeg;base64,xxx",
            "hide_profiles": true,
            "status_bar": false,
            "status_bar_pos": "top",
            "window_title": "Мой тайтл"
        }
    }"#;

    #[test]
    fn legacy_snake_case_config_migrates_without_data_loss() {
        let value: serde_json::Value = serde_json::from_str(LEGACY).unwrap();
        let migrated = migrate_legacy_config(value);
        let cfg: LauncherConfig =
            serde_json::from_value(migrated).expect("легаси-конфиг должен мигрировать");

        assert_eq!(cfg.last_nickname, "TestPlayer");
        assert_eq!(cfg.last_server, 15);
        assert!(cfg.game_path.ends_with("gta_sa.exe"));
        assert_eq!(cfg.patches_version.as_deref(), Some("1.2.3"));
        // Легаси всегда запускал через Air-лаунчер — миграция сохраняет это
        assert_eq!(cfg.launch_method, "air");

        assert_eq!(cfg.launch_params.memory, 6144);
        assert!(cfg.launch_params.texture_mode);
        assert!(cfg.launch_params.allow_hdr);
        assert_eq!(cfg.launch_params.cdn, "2,2,2");
        assert!(cfg.launch_params.use_d3dx9_43);

        let ls = &cfg.launcher_settings;
        assert_eq!(ls.bg_image.as_deref(), Some("10.jpeg"));
        assert_eq!(ls.window_title.as_deref(), Some("Мой тайтл"));
        // extra-поля остаются в snake_case — фронт читает их так
        assert_eq!(ls.extra.get("hide_profiles").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(ls.extra.get("status_bar").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(ls.extra.get("status_bar_pos").and_then(|v| v.as_str()), Some("top"));
        assert!(ls.extra.get("bg_data").is_some());
    }

    #[test]
    fn our_camel_case_config_loads_as_is() {
        let cfg = LauncherConfig::default();
        let value = serde_json::to_value(&cfg).unwrap();
        let loaded: LauncherConfig = serde_json::from_value(value).unwrap();
        assert_eq!(loaded.last_server, cfg.last_server);
        assert_eq!(loaded.launch_params.cdn, cfg.launch_params.cdn);
    }
}
