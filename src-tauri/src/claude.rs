//! Служба лимитов Claude Code: следит за файлом моста и ставит мост в
//! `~/.claude/settings.json` по просьбе человека.

use std::path::PathBuf;
use std::time::Duration;

use kirk_core::claude::{self, StatusLine};
use kirk_core::limits::{Connection, Provider, Snapshot};
use notify::{RecursiveMode, Watcher};
use tauri::{AppHandle, Manager};

use crate::hub::{self, Hub};

/// Мост лежит рядом с приложением (sidecar Tauri).
pub fn bridge_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let name = if cfg!(windows) { "kirk-bridge.exe" } else { "kirk-bridge" };
    let path = exe.parent().ok_or("нет каталога приложения")?.join(name);
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!("мост не найден: {}", path.display()))
    }
}

fn snapshot(app: &AppHandle) -> Snapshot {
    if !app.state::<Hub>().config().claude {
        return Snapshot::empty(Provider::Claude, Connection::Waiting, Some("Выключено в настройках"));
    }
    let snap = claude::read(&claude::snapshot_path(), kirk_core::now_ms());
    if snap.connection != Connection::Waiting {
        return snap;
    }
    let installed = settings_status().is_ok_and(|s| s == StatusLine::Ours);
    if installed {
        snap
    } else {
        Snapshot::empty(
            Provider::Claude,
            Connection::SignedOut,
            Some("Подключите Claude Code в настройках Kirk"),
        )
    }
}

pub fn refresh(app: &AppHandle) {
    hub::set_limits(app, snapshot(app));
}

/// Следит за файлом моста (без опроса) и раз в минуту пересчитывает –
/// сбросы окон и «устарело» зависят от времени.
pub fn start(app: AppHandle) {
    let file = claude::snapshot_path();
    let dir = file.parent().map(PathBuf::from).unwrap_or_default();
    let _ = std::fs::create_dir_all(&dir);

    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let watched = file.clone();
    let watcher = notify::recommended_watcher(move |ev: notify::Result<notify::Event>| {
        if ev.is_ok_and(|e| e.paths.iter().any(|p| p.file_name() == watched.file_name())) {
            let _ = tx.send(());
        }
    });
    let mut watcher = match watcher {
        Ok(w) => Some(w),
        Err(e) => {
            log::warn!("наблюдение за файлом Claude недоступно: {e}");
            None
        }
    };
    if let Some(w) = watcher.as_mut() {
        if let Err(e) = w.watch(&dir, RecursiveMode::NonRecursive) {
            log::warn!("наблюдение за каталогом данных: {e}");
        }
    }

    std::thread::Builder::new()
        .name("claude".into())
        .spawn(move || {
            let _keep = watcher;
            refresh(&app);
            loop {
                match rx.recv_timeout(Duration::from_secs(60)) {
                    // Несколько событий подряд от одной записи – одна перечитка.
                    Ok(()) => {
                        std::thread::sleep(Duration::from_millis(50));
                        while rx.try_recv().is_ok() {}
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        std::thread::sleep(Duration::from_secs(60));
                    }
                }
                refresh(&app);
            }
        })
        .expect("поток Claude");
}

fn read_settings() -> Result<(PathBuf, String), String> {
    let path = claude::settings_path().ok_or("не найден домашний каталог")?;
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    Ok((path, text))
}

pub fn settings_status() -> Result<StatusLine, String> {
    let (_, text) = read_settings()?;
    claude::status(&text)
}

/// Ставит мост. Прежний файл сохраняется рядом (`settings.json.kirk-backup`).
pub fn install() -> Result<(), String> {
    let bridge = bridge_path()?;
    let (path, text) = read_settings()?;
    let next = claude::install(&text, &bridge)?;
    if !text.is_empty() {
        std::fs::write(path.with_extension("json.kirk-backup"), &text).map_err(|e| e.to_string())?;
    }
    kirk_core::paths::write_atomic(&path, next.as_bytes()).map_err(|e| e.to_string())
}

pub fn uninstall() -> Result<(), String> {
    let (path, text) = read_settings()?;
    if text.is_empty() {
        return Ok(());
    }
    let next = claude::uninstall(&text)?;
    kirk_core::paths::write_atomic(&path, next.as_bytes()).map_err(|e| e.to_string())
}
