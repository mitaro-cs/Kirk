//! Ядро Kirk: всё, что не зависит от окон и платформы, – с тестами.
//!
//! Оболочка (`src-tauri`) только достаёт данные из системы и показывает их;
//! разбор, расчёты и форматы файлов живут здесь.

pub mod campus;
pub mod claude;
pub mod codex;
pub mod geometry;
pub mod limits;
pub mod music;
pub mod paths;

/// Текущее время, мс Unix.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
