//! Значок в строке меню (Mac) и в области уведомлений (Windows).

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "Открыть остров", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Настройки…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Выйти из Kirk", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &toggle,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    // Mac: одноцветный шаблон – система сама красит его под светлую и тёмную
    // строку меню. Windows: цветной значок приложения (чёрный не виден на
    // тёмной панели задач).
    let icon = if cfg!(target_os = "macos") {
        tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?
    } else {
        app.default_window_icon()
            .cloned()
            .expect("значок приложения")
    };
    TrayIconBuilder::with_id("kirk")
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("Kirk")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => crate::island::toggle(app),
            "settings" => open_settings(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

/// Окно настроек: одно на приложение, создаётся при первом открытии.
pub fn open_settings(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }
    let built = tauri::WebviewWindowBuilder::new(
        app,
        "settings",
        tauri::WebviewUrl::App("settings.html".into()),
    )
    .title("Kirk – настройки")
    .inner_size(720.0, 560.0)
    .min_inner_size(560.0, 440.0)
    .center()
    .build();
    match built {
        Ok(win) => {
            #[cfg(target_os = "macos")]
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
            let _ = win.set_focus();
            let handle = app.clone();
            win.on_window_event(move |e| {
                if let tauri::WindowEvent::Destroyed = e {
                    // Без открытых окон – снова без значка в Dock.
                    #[cfg(target_os = "macos")]
                    let _ = handle.set_activation_policy(tauri::ActivationPolicy::Accessory);
                    let _ = &handle;
                }
            });
        }
        Err(e) => log::error!("окно настроек: {e}"),
    }
}
