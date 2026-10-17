//! Кэш хэшей: rel -> md5 из прошлых прогонов.
//! Хранится отдельно от конфига лаунчера: `<Documents>/ArizonaLauncher/game_update_state.json`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

fn state_path(documents: &Path) -> PathBuf {
    documents.join("game_update_state.json")
}

pub fn load_cache(documents: &Path) -> HashMap<String, String> {
    let Ok(s) = fs::read_to_string(state_path(documents)) else {
        return HashMap::new();
    };
    serde_json::from_str::<serde_json::Value>(&s)
        .ok()
        .and_then(|v| v.get("hashmap").cloned())
        .and_then(|v| serde_json::from_value::<HashMap<String, String>>(v).ok())
        .unwrap_or_default()
}

pub fn save_cache(documents: &Path, map: &HashMap<String, String>) -> anyhow::Result<()> {
    let path = state_path(documents);
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?;
    }
    let json = serde_json::json!({ "hashmap": map });
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(&json)?)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_roundtrip_and_corrupt_tolerance() {
        let dir = tempfile::tempdir().unwrap();
        let empty = load_cache(dir.path());
        assert!(empty.is_empty());

        let mut map = HashMap::new();
        map.insert("a.img".to_string(), "deadbeef".to_string());
        save_cache(dir.path(), &map).unwrap();
        let loaded = load_cache(dir.path());
        assert_eq!(loaded.get("a.img").map(String::as_str), Some("deadbeef"));

        // Битый файл не роняет загрузку
        std::fs::write(dir.path().join("game_update_state.json"), "{oops").unwrap();
        assert!(load_cache(dir.path()).is_empty());
    }
}
