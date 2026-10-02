use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout_order: Option<Vec<String>>,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            bg_image: None,
            window_title: None,
            window_size: None,
            layout_order: None,
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
    #[serde(default)]
    pub read_news_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profiles: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_profile_id: Option<String>,
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
            read_news_ids: Vec::new(),
            profiles: None,
            active_profile_id: None,
        }
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
        }
    }

    fn load_config(path: &PathBuf) -> LauncherConfig {
        if let Ok(data) = fs::read_to_string(path) {
            if let Ok(cfg) = serde_json::from_str(&data) {
                return cfg;
            }
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
