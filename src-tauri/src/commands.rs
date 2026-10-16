//! Команды интерфейса (`invoke`).

use kirk_core::claude::StatusLine;
use kirk_core::music::Command;
use tauri::{AppHandle, Manager};

use crate::config::Config;
use crate::hub::{self, Hub, State};

#[tauri::command]
pub fn get_state(app: AppHandle) -> State {
    app.state::<Hub>().state()
}

#[tauri::command]
pub fn island_open(app: AppHandle, open: bool) {
    crate::island::set_open(&app, open, true);
}

#[tauri::command]
pub fn open_settings(app: AppHandle) {
    crate::island::set_open(&app, false, false);
    crate::tray::open_settings(&app);
}

#[tauri::command]
pub fn music_command(app: AppHandle, command: Command) -> Result<(), String> {
    crate::music::send(&app, command)
}

#[tauri::command]
pub fn set_config(app: AppHandle, config: Config) -> Result<(), String> {
    let before = app.state::<Hub>().config();
    crate::config::save(&config)?;
    if before.hotkey != config.hotkey {
        crate::shortcut::rebind(&app, &before.hotkey, &config.hotkey)?;
    }
    hub::set_config(&app, config);
    crate::claude::refresh(&app);
    crate::campus_wake(&app);
    Ok(())
}

#[tauri::command]
pub fn claude_status() -> Result<StatusLine, String> {
    crate::claude::settings_status()
}

#[tauri::command]
pub fn claude_install(app: AppHandle) -> Result<(), String> {
    crate::claude::install()?;
    crate::claude::refresh(&app);
    Ok(())
}

#[tauri::command]
pub fn claude_uninstall(app: AppHandle) -> Result<(), String> {
    crate::claude::uninstall()?;
    crate::claude::refresh(&app);
    Ok(())
}

#[tauri::command]
pub async fn campus_connect(app: AppHandle, url: String, pin: String) -> Result<(), String> {
    crate::campus::connect(&app, &url, &pin).await
}

#[tauri::command]
pub fn campus_disconnect(app: AppHandle) {
    crate::campus::disconnect(&app);
}

#[tauri::command]
pub async fn campus_done(app: AppHandle, id: i64, value: bool) -> Result<(), String> {
    crate::campus::set_done(&app, id, value).await
}

#[tauri::command]
pub async fn campus_read_all(app: AppHandle) -> Result<(), String> {
    crate::campus::read_all(&app).await
}

/// Открыть страницу Campus в браузере: `/homework/12`, `/schedule/5`, `/notifications`.
#[tauri::command]
pub fn campus_open(app: AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let url = crate::campus::link(&app, &path)?;
    crate::island::set_open(&app, false, false);
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}
