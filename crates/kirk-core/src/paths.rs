//! Где Kirk хранит свои файлы. Тот же каталог, что `app_data_dir()` у Tauri
//! (каталог данных системы + идентификатор), – его знает и мост Claude Code,
//! который запускается без Tauri.

use std::path::{Path, PathBuf};

/// Идентификатор приложения (`tauri.conf.json → identifier`).
pub const IDENTIFIER: &str = "app.kirk";

/// macOS: `~/Library/Application Support/app.kirk`, Windows: `%APPDATA%\app.kirk`.
pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(IDENTIFIER)
}

/// Записывает файл целиком или не трогает его: временный файл рядом и
/// переименование. Читатель никогда не увидит половину.
pub fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_ends_with_identifier() {
        assert!(data_dir().ends_with(IDENTIFIER));
    }

    #[test]
    fn atomic_write_replaces_content() {
        let dir = std::env::temp_dir().join(format!("kirk-paths-{}", std::process::id()));
        let file = dir.join("a.json");
        write_atomic(&file, b"1").unwrap();
        write_atomic(&file, b"22").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"22");
        let _ = std::fs::remove_dir_all(dir);
    }
}
