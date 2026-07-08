//! Non-sensitive app settings, persisted as JSON in the app data dir. Changing
//! them re-applies the global hotkey and autostart at runtime (handled in lib.rs).

use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub auto_lock_minutes: u32,
    pub clipboard_clear_seconds: u32,
    pub launch_at_login: bool,
    pub global_hotkey: String,
    /// "light" | "dark" | "system"
    pub theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            auto_lock_minutes: 15,
            clipboard_clear_seconds: 30,
            launch_at_login: false,
            global_hotkey: "Alt+Space".into(),
            theme: "system".into(),
        }
    }
}

impl Settings {
    pub fn load(dir: &Path) -> Settings {
        let path = dir.join("settings.json");
        std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join("settings.json");
        std::fs::write(path, serde_json::to_vec_pretty(self).unwrap_or_default())
    }
}
