use tauri::WebviewWindow;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

pub fn prepare_island(window: &WebviewWindow) -> tauri::Result<()> {
    // HWND из версии windows, что у Tauri, → наша версия.
    let hwnd = HWND(window.hwnd()?.0);
    unsafe {
        // Щелчок по острову не уводит фокус из окна, где печатают;
        // в Alt+Tab и на панели задач острова нет.
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | (WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0) as isize);
    }
    Ok(())
}
