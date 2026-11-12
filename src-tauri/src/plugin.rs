//! Дескриптор плагина — `plugin.json` рядом с файлами плагина в
//! `preloading_plugins`.
//!
//! Зачем: настройки плагина должны обновляться вместе с плагином, без
//! пересборки лаунчера. Файл описывает, как называется плагин, какой файл
//! настроек и DLL ему принадлежат и как рисовать его панель (`ui` в формате
//! схемы: `categories` + `settings`).
//!
//! Если `plugin.json` нет или он битый — работают значения v1 по умолчанию,
//! то есть поведение прежних версий лаунчера не меняется.

use std::path::{Path, PathBuf};

/// Имя файла-описателя в папке плагина.
pub const DESCRIPTOR_FILE: &str = "plugin.json";

pub const DEFAULT_ID: &str = "ArizonaPatches";
pub const DEFAULT_NAME: &str = "Arizona Patches";
pub const DEFAULT_SETTINGS_FILE: &str = "#ArizonaPatches.json";
pub const DEFAULT_DLL_FILE: &str = "#ArizonaPatches.dll";

/// Суффикс выключенного плагина: `<dll>.disabled`. Игра не грузит такой файл,
/// вернуть плагин — переименовать обратно.
pub const DLL_DISABLED_SUFFIX: &str = ".disabled";

/// Разобранный `plugin.json` с подставленными значениями по умолчанию.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PluginDescriptor {
    pub id: String,
    pub name: String,
    /// Версия плагина, для которого написан этот дескриптор (например "1.0.0.0").
    /// `None` — дескриптор версию не заявляет, проверка пропускается.
    pub plugin_version: Option<String>,
    pub settings_file: String,
    pub dll_file: String,
    /// Необязательный адрес схемы (если UI хотят обновлять отдельно от плагина).
    pub schema_url: Option<String>,
    /// UI-описание: `{ categories: [...], settings: [...] }` в формате схемы.
    pub ui: Option<serde_json::Value>,
    /// Откуда взяты данные: "plugin.json" или "default".
    pub source: String,
}

impl Default for PluginDescriptor {
    fn default() -> Self {
        Self {
            id: DEFAULT_ID.to_string(),
            name: DEFAULT_NAME.to_string(),
            plugin_version: None,
            settings_file: DEFAULT_SETTINGS_FILE.to_string(),
            dll_file: DEFAULT_DLL_FILE.to_string(),
            schema_url: None,
            ui: None,
            source: "default".to_string(),
        }
    }
}

impl PluginDescriptor {
    /// Путь к файлу настроек внутри папки плагина.
    pub fn settings_path(&self, plugins_dir: &Path) -> PathBuf {
        plugins_dir.join(&self.settings_file)
    }

    /// Путь к DLL внутри папки плагина.
    pub fn dll_path(&self, plugins_dir: &Path) -> PathBuf {
        plugins_dir.join(&self.dll_file)
    }

    /// Путь к выключенному DLL (`<файл>.disabled`).
    pub fn dll_disabled_path(&self, plugins_dir: &Path) -> PathBuf {
        plugins_dir.join(format!("{}{}", self.dll_file, DLL_DISABLED_SUFFIX))
    }

    /// Выключен ли плагин: есть `.disabled`-файл.
    pub fn is_disabled(&self, plugins_dir: &Path) -> bool {
        self.dll_disabled_path(plugins_dir).is_file()
    }
}

/// Читает дескриптор из папки плагина.
///
/// Возвращает дескриптор и необязательное предупреждение для лога/UI.
/// Любая проблема с файлом не роняет лаунчер: берутся значения по умолчанию.
pub fn load(plugins_dir: &Path) -> (PluginDescriptor, Option<String>) {
    let path = plugins_dir.join(DESCRIPTOR_FILE);
    if !path.is_file() {
        return (PluginDescriptor::default(), None);
    }
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(e) => {
            return (
                PluginDescriptor::default(),
                Some(format!("plugin.json не прочитан ({e}), работают настройки v1")),
            );
        }
    };
    parse(&raw)
}

/// Разбор текста `plugin.json` (вынесен отдельно ради тестов).
pub fn parse(raw: &str) -> (PluginDescriptor, Option<String>) {
    let mut desc = PluginDescriptor::default();
    let value: serde_json::Value = match serde_json::from_str(raw.trim()) {
        Ok(v) => v,
        Err(e) => {
            return (
                desc,
                Some(format!("plugin.json некорректный ({e}), работают настройки v1")),
            );
        }
    };
    if !value.is_object() {
        return (
            desc,
            Some("plugin.json должен быть объектом, работают настройки v1".to_string()),
        );
    }

    let mut warnings: Vec<String> = Vec::new();

    if let Some(id) = value.get("id").and_then(|v| v.as_str()) {
        if is_safe_id(id) {
            desc.id = id.to_string();
        } else {
            warnings.push(format!("plugin.json: id «{id}» отброшен"));
        }
    }
    if let Some(name) = value.get("name").and_then(|v| v.as_str()) {
        let name = name.trim();
        if !name.is_empty() && name.len() <= 120 {
            desc.name = name.to_string();
        } else {
            warnings.push("plugin.json: пустое имя плагина отброшено".to_string());
        }
    }
    if let Some(ver) = value.get("plugin_version").and_then(|v| v.as_str()) {
        if parse_version(ver).is_some() {
            desc.plugin_version = Some(ver.trim().to_string());
        } else {
            warnings.push(format!("plugin.json: версия «{ver}» отброшена"));
        }
    }
    if let Some(f) = value.get("settings_file").and_then(|v| v.as_str()) {
        if is_safe_file_name(f) {
            desc.settings_file = f.to_string();
        } else {
            warnings.push(format!("plugin.json: settings_file «{f}» отброшен"));
        }
    }
    if let Some(f) = value.get("dll_file").and_then(|v| v.as_str()) {
        if is_safe_file_name(f) {
            desc.dll_file = f.to_string();
        } else {
            warnings.push(format!("plugin.json: dll_file «{f}» отброшен"));
        }
    }
    if let Some(url) = value.get("schema_url").and_then(|v| v.as_str()) {
        let url = url.trim();
        if url.starts_with("https://") && url.len() <= 2048 {
            desc.schema_url = Some(url.to_string());
        } else {
            warnings.push("plugin.json: schema_url должен начинаться с https://".to_string());
        }
    }
    match value.get("ui") {
        Some(ui) if ui.is_object() => {
            let has_categories = ui.get("categories").map(|c| c.is_array()).unwrap_or(false);
            let has_settings = ui.get("settings").map(|s| s.is_array()).unwrap_or(false);
            if has_categories && has_settings {
                desc.ui = Some(ui.clone());
            } else {
                warnings.push("plugin.json: ui без массивов categories/settings отброшен".to_string());
            }
        }
        Some(_) => warnings.push("plugin.json: ui должен быть объектом".to_string()),
        None => {}
    }

    desc.source = "plugin.json".to_string();
    let warning = if warnings.is_empty() {
        None
    } else {
        Some(warnings.join("; "))
    };
    (desc, warning)
}

/// Сравнивает версию из дескриптора с версией DLL (`1.0.0.0`).
///
/// Сравнение по компонентам с добивкой нулями: "1.0" == "1.0.0.0".
/// Если дескриптор версию не заявляет — проверять нечего, совпадает.
/// Если заявляет, а DLL нет или у неё нет версии — НЕ доверяем дескриптору:
/// иначе панель v2 показалась бы поверх отсутствующего плагина.
pub fn version_matches(declared: Option<&str>, actual: Option<&str>) -> bool {
    let Some(declared) = declared else {
        return true;
    };
    let Some(actual) = actual else {
        return false;
    };
    let (Some(a), Some(b)) = (parse_version(declared), parse_version(actual)) else {
        return false;
    };
    a == b
}

/// "1.2.3.4" → [1, 2, 3, 4]; допускает 1–4 компоненты, суффиксы не поддерживаются.
fn parse_version(s: &str) -> Option<[u64; 4]> {
    let s = s.trim();
    if s.is_empty() || s.len() > 40 {
        return None;
    }
    let mut out = [0u64; 4];
    let mut count = 0;
    for part in s.split('.') {
        if count == 4 {
            return None;
        }
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        out[count] = part.parse().ok()?;
        count += 1;
    }
    if count == 0 {
        None
    } else {
        Some(out)
    }
}

fn is_safe_id(id: &str) -> bool {
    let id = id.trim();
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
}

/// Имя файла без путей: не пустое, без разделителей, без ".." и NUL.
pub fn is_safe_file_name(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty()
        && name.len() <= 260
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\0')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let (d, warn) = load(dir.path());
        assert_eq!(d.id, DEFAULT_ID);
        assert_eq!(d.settings_file, DEFAULT_SETTINGS_FILE);
        assert_eq!(d.dll_file, DEFAULT_DLL_FILE);
        assert_eq!(d.source, "default");
        assert!(warn.is_none());
    }

    #[test]
    fn broken_json_gives_defaults_with_warning() {
        let (d, warn) = parse("{ not json");
        assert_eq!(d.source, "default");
        assert_eq!(d.settings_file, DEFAULT_SETTINGS_FILE);
        assert!(warn.unwrap_or_default().contains("некорректный"));
    }

    #[test]
    fn full_descriptor_is_parsed() {
        let raw = r##"{
            "id": "ArizonaPatches",
            "name": "Arizona Patches",
            "plugin_version": "2.0.1",
            "settings_file": "#ArizonaPatches2.json",
            "dll_file": "#ArizonaPatches2.dll",
            "schema_url": "https://example.com/schema.json",
            "ui": { "categories": [], "settings": [] }
        }"##;
        let (d, warn) = parse(raw);
        assert!(warn.is_none(), "{warn:?}");
        assert_eq!(d.plugin_version.as_deref(), Some("2.0.1"));
        assert_eq!(d.settings_file, "#ArizonaPatches2.json");
        assert_eq!(d.dll_file, "#ArizonaPatches2.dll");
        assert!(d.ui.is_some());
        assert_eq!(d.source, "plugin.json");
    }

    #[test]
    fn path_traversal_falls_back_to_defaults() {
        let raw = r#"{
            "settings_file": "../../config.json",
            "dll_file": "C:\\Windows\\evil.dll",
            "ui": { "categories": [], "settings": [] }
        }"#;
        let (d, warn) = parse(raw);
        assert_eq!(d.settings_file, DEFAULT_SETTINGS_FILE);
        assert_eq!(d.dll_file, DEFAULT_DLL_FILE);
        assert!(warn.unwrap_or_default().contains("settings_file"));
    }

    #[test]
    fn ui_without_arrays_is_dropped() {
        let (d, warn) = parse(r#"{"ui": {"categories": []}}"#);
        assert!(d.ui.is_none());
        assert!(warn.unwrap_or_default().contains("ui"));
    }

    #[test]
    fn version_matching_pads_components() {
        assert!(version_matches(Some("1.0"), Some("1.0.0.0")));
        assert!(version_matches(Some("1.0.0.0"), Some("1.0.0.0")));
        assert!(!version_matches(Some("2.0.0.0"), Some("1.0.0.0")));
        assert!(version_matches(None, Some("1.0.0.0")));
        // Дескриптор заявляет версию, а DLL нет — не доверяем.
        assert!(!version_matches(Some("1.0"), None));
        assert!(!version_matches(Some("beta"), Some("1.0.0.0")));
        assert!(!version_matches(Some("1.2.3.4.5"), Some("1.2.3.4")));
    }

    #[test]
    fn settings_and_dll_paths_stay_inside_plugins_dir() {
        let dir = Path::new("C:/game/preloading_plugins");
        let d = PluginDescriptor {
            settings_file: "s.json".into(),
            dll_file: "p.dll".into(),
            ..Default::default()
        };
        assert_eq!(d.settings_path(dir), dir.join("s.json"));
        assert_eq!(d.dll_path(dir), dir.join("p.dll"));
        assert_eq!(d.dll_disabled_path(dir), dir.join("p.dll.disabled"));
    }

    #[test]
    fn safe_file_name_rules() {
        assert!(is_safe_file_name("#ArizonaPatches.json"));
        assert!(!is_safe_file_name(""));
        assert!(!is_safe_file_name(".."));
        assert!(!is_safe_file_name("a/b.json"));
        assert!(!is_safe_file_name("a\\b.json"));
    }
}
