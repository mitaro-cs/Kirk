use kirk_core::geometry::Notch;
use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use tauri::WebviewWindow;
use tauri_nspanel::{CollectionBehavior, PanelLevel, StyleMask, WebviewWindowExt};

use panel::IslandPanel;

// Макрос сам импортирует нужное – держим его в отдельном модуле.
mod panel {
    use tauri_nspanel::tauri_panel;

    tauri_panel! {
        panel!(IslandPanel {
            config: {
                can_become_key_window: false,
                can_become_main_window: false,
                is_floating_panel: true
            }
        })
    }
}

pub fn notch() -> Option<Notch> {
    let mtm = MainThreadMarker::new()?;
    let screens = NSScreen::screens(mtm);
    // Первый экран – тот, где строка меню.
    let screen = screens.firstObject()?;
    let top = screen.safeAreaInsets().top;
    if top <= 0.0 {
        return None;
    }
    let frame = screen.frame();
    let left = screen.auxiliaryTopLeftArea();
    let right = screen.auxiliaryTopRightArea();
    let width = frame.size.width - left.size.width - right.size.width;
    (width > 0.0).then_some(Notch { width, height: top })
}

pub fn prepare_island(window: &WebviewWindow) -> tauri::Result<()> {
    let panel = window.to_panel::<IslandPanel>()?;
    // Выше строки меню (24), как системные индикаторы.
    panel.set_level(PanelLevel::Status.value() + 1);
    if let Err(e) = panel.set_style_mask(StyleMask::empty().borderless().nonactivating_panel().value()) {
        log::warn!("остров без nonactivating: {e:?}");
    }
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .can_join_all_spaces()
            .stationary()
            .full_screen_auxiliary()
            .ignores_cycle()
            .value(),
    );
    panel.set_has_shadow(false);
    panel.set_hides_on_deactivate(false);
    Ok(())
}
