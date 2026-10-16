//! Kirk – остров сверху экрана: лимиты Claude Code и Codex, музыка, Campus.

mod campus;
mod claude;
mod codex;
mod commands;
mod config;
mod hub;
mod island;
mod music;
mod platform;
mod shortcut;
mod tray;

use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

pub(crate) fn campus_wake(app: &AppHandle) {
    app.state::<campus::Wake>().0.notify_one();
}

#[tauri::command]
fn autostart_get(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn autostart_set(app: AppHandle, value: bool) -> Result<(), String> {
    let launcher = app.autolaunch();
    if value { launcher.enable() } else { launcher.disable() }.map_err(|e| e.to_string())
}

pub fn run() {
    let config = config::load();
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::open_settings(app)))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_opener::init())
        .plugin(shortcut::plugin());
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());

    builder
        .manage(hub::Hub::new(config))
        .manage(island::Island::new())
        .manage(campus::Wake(tokio::sync::Notify::new()))
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::island_open,
            commands::open_settings,
            commands::music_command,
            commands::set_config,
            commands::claude_status,
            commands::claude_install,
            commands::claude_uninstall,
            commands::campus_connect,
            commands::campus_disconnect,
            autostart_get,
            autostart_set,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let handle = app.handle().clone();
            island::setup(&handle)?;
            tray::setup(&handle)?;
            shortcut::register(&handle, &handle.state::<hub::Hub>().config().hotkey);
            claude::start(handle.clone());
            codex::start(handle.clone());
            campus::start(handle.clone());
            music::start(handle);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Kirk не запустился");
}
