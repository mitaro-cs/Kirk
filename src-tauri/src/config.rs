//! Настройки Kirk – `config.json` в каталоге данных.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    /// Модули острова.
    pub limits: bool,
    pub music: bool,
    pub campus: bool,
    /// Провайдеры лимитов.
    pub claude: bool,
    pub codex: bool,
    /// Открывать остров наведением (иначе – щелчком или сочетанием).
    pub open_on_hover: bool,
    /// Сочетание клавиш «открыть/закрыть».
    pub hotkey: String,
    /// Новые уведомления Campus коротко всплывают в острове.
    pub peek: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            limits: true,
            music: true,
            campus: true,
            claude: true,
            codex: true,
            open_on_hover: true,
            hotkey: "Control+Alt+K".into(),
            peek: true,
        }
    }
}

fn path() -> std::path::PathBuf {
    kirk_core::paths::data_dir().join("config.json")
}

pub fn load() -> Config {
    std::fs::read(path())
        .ok()
        .and_then(|d| serde_json::from_slice(&d).ok())
        .unwrap_or_default()
}

pub fn save(config: &Config) -> Result<(), String> {
    let data = serde_json::to_vec_pretty(config).map_err(|e| e.to_string())?;
    kirk_core::paths::write_atomic(&path(), &data).map_err(|e| e.to_string())
}
