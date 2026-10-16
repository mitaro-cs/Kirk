//! Музыка: что играет и управление плеером.
//!
//! - Windows – System Media Transport Controls: любой плеер, который
//!   показывает себя в системном меню громкости (Spotify, браузеры, …).
//! - macOS – mediaremote-adapter, если он собран в приложение (любой плеер
//!   из «Пункта управления»); иначе – уведомления Music и Spotify.

use kirk_core::music::Command;
use tauri::AppHandle;

#[cfg(target_os = "macos")]
mod mac;
#[cfg(windows)]
mod win;

pub fn start(app: AppHandle) {
    #[cfg(target_os = "macos")]
    mac::start(app);
    #[cfg(windows)]
    win::start(app);
    #[cfg(not(any(target_os = "macos", windows)))]
    crate::hub::set_music(
        &app,
        crate::hub::MusicState {
            available: false,
            note: Some("На этой системе музыка пока не поддерживается".into()),
            now: None,
        },
    );
}

pub fn send(app: &AppHandle, command: Command) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return mac::send(app, command);
    #[cfg(windows)]
    return win::send(app, command);
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = (app, command);
        Err("не поддерживается".into())
    }
}
