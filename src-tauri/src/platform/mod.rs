//! То, что на Mac и Windows делается по-разному: окно поверх всего, не
//! забирающее фокус, и размеры чёлки.

use kirk_core::geometry::Notch;
use tauri::WebviewWindow;

#[cfg(target_os = "macos")]
mod mac;
#[cfg(windows)]
mod win;

/// Чёлка основного экрана (только MacBook с чёлкой).
pub fn notch() -> Option<Notch> {
    #[cfg(target_os = "macos")]
    return mac::notch();
    #[cfg(not(target_os = "macos"))]
    None
}

/// Делает окно островом: поверх строки меню и полноэкранных окон, на всех
/// рабочих столах, без фокуса при щелчке, без кнопки на панели задач.
pub fn prepare_island(window: &WebviewWindow) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    return mac::prepare_island(window);
    #[cfg(windows)]
    return win::prepare_island(window);
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = window;
        Ok(())
    }
}
