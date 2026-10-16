//! Сочетание клавиш «открыть/закрыть остров».

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                crate::island::toggle(app);
            }
        })
        .build()
}

pub fn register(app: &AppHandle, hotkey: &str) {
    if hotkey.trim().is_empty() {
        return;
    }
    if let Err(e) = app.global_shortcut().register(hotkey) {
        // Занято другой программой – Kirk работает и без сочетания.
        log::warn!("сочетание {hotkey}: {e}");
    }
}

pub fn rebind(app: &AppHandle, old: &str, new: &str) -> Result<(), String> {
    if !old.trim().is_empty() {
        let _ = app.global_shortcut().unregister(old);
    }
    if new.trim().is_empty() {
        return Ok(());
    }
    app.global_shortcut()
        .register(new)
        .map_err(|e| format!("Сочетание «{new}» не подходит или занято: {e}"))
}
