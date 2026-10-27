//! User settings, stored as `settings.json` in the app data folder.
//!
//! Launch at login is not stored here: the OS keeps that state, and the
//! autostart plugin reads and writes it directly.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

pub const MIN_ITEMS: usize = 50;
pub const MAX_ITEMS: usize = 10_000;

const THEMES: [&str; 3] = ["system", "light", "dark"];
const ACCENTS: [&str; 6] = ["blue", "purple", "green", "orange", "pink", "graphite"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Unpinned clips kept before the oldest are dropped.
    pub max_items: usize,
    /// "system", "light" or "dark".
    pub theme: String,
    /// Accent colour name; the UI maps it to actual colours.
    pub accent: String,
    /// Global shortcut that opens the history window.
    pub shortcut: String,
    /// Whether copied images are recorded.
    pub save_images: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_items: 500,
            theme: "system".into(),
            accent: "blue".into(),
            shortcut: "CmdOrCtrl+Shift+V".into(),
            save_images: true,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !(MIN_ITEMS..=MAX_ITEMS).contains(&self.max_items) {
            return Err(format!("保存条数需要在 {MIN_ITEMS} 到 {MAX_ITEMS} 之间"));
        }
        if !THEMES.contains(&self.theme.as_str()) {
            return Err(format!("未知的主题：{}", self.theme));
        }
        if !ACCENTS.contains(&self.accent.as_str()) {
            return Err(format!("未知的颜色：{}", self.accent));
        }
        Ok(())
    }
}

pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
}

impl SettingsStore {
    /// Loads saved settings, falling back to defaults for a missing or
    /// unreadable file and for any value that fails validation.
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join("settings.json");
        let current = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
            .filter(|s| s.validate().is_ok())
            .unwrap_or_default();
        Self {
            path,
            current: Mutex::new(current),
        }
    }

    pub fn get(&self) -> Settings {
        self.current.lock().unwrap().clone()
    }

    pub fn save(&self, settings: Settings) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, json).map_err(|e| format!("保存设置失败：{e}"))?;
        *self.current.lock().unwrap() = settings;
        Ok(())
    }
}

/// Turns a shortcut like "CmdOrCtrl+Shift+KeyV" into what the user sees:
/// "⌘⇧V" on macOS, "Ctrl+Shift+V" elsewhere.
pub fn shortcut_label(shortcut: &str, mac: bool) -> String {
    let parts: Vec<String> = shortcut
        .split('+')
        .map(|token| {
            let label = match token.to_ascii_lowercase().as_str() {
                "cmdorctrl" | "commandorcontrol" | "commandorctrl" | "cmdorcontrol" => {
                    if mac { "⌘" } else { "Ctrl" }
                }
                "cmd" | "command" | "super" | "meta" => {
                    if mac { "⌘" } else { "Win" }
                }
                "ctrl" | "control" => {
                    if mac { "⌃" } else { "Ctrl" }
                }
                "alt" | "option" => {
                    if mac { "⌥" } else { "Alt" }
                }
                "shift" => {
                    if mac { "⇧" } else { "Shift" }
                }
                _ => "",
            };
            if !label.is_empty() {
                return label.to_string();
            }
            token
                .strip_prefix("Key")
                .or_else(|| token.strip_prefix("Digit"))
                .unwrap_or(token)
                .to_string()
        })
        .collect();
    parts.join(if mac { "" } else { "+" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_shortcuts_per_platform() {
        assert_eq!(shortcut_label("CmdOrCtrl+Shift+V", true), "⌘⇧V");
        assert_eq!(shortcut_label("CmdOrCtrl+Shift+V", false), "Ctrl+Shift+V");
        assert_eq!(shortcut_label("Alt+Super+KeyC", false), "Alt+Win+C");
        assert_eq!(shortcut_label("Ctrl+Alt+Digit1", true), "⌃⌥1");
    }

    #[test]
    fn rejects_bad_values() {
        let ok = Settings::default();
        assert!(ok.validate().is_ok());
        assert!(Settings { max_items: 10, ..ok.clone() }.validate().is_err());
        assert!(Settings { theme: "neon".into(), ..ok.clone() }.validate().is_err());
        assert!(Settings { accent: "gold".into(), ..ok }.validate().is_err());
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{"maxItems": 1000}"#).unwrap();
        assert_eq!(s.max_items, 1000);
        assert_eq!(s.shortcut, Settings::default().shortcut);
    }
}
