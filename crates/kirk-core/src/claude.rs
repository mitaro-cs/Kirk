//! Лимиты Claude Code через строку состояния.
//!
//! Claude Code после каждого ответа запускает команду строки состояния
//! (`statusLine` в `~/.claude/settings.json`) и передаёт ей JSON, где есть
//! `rate_limits.five_hour` и `rate_limits.seven_day`. Мост Kirk (`kirk-bridge`)
//! берёт из него **только** проценты и время сброса и пишет их в файл,
//! приложение этот файл читает. Ни ключей, ни подсказок, ни путей.
//!
//! Открыто несколько сессий – каждая шлёт лимиты, как их видел её последний
//! запрос. Чтобы простаивающая сессия не затирала свежие цифры активной,
//! для каждого окна верим той сессии, у которой оно менялось последним.

use std::path::{Path, PathBuf};

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::limits::{Connection, Provider, QuotaWindow, Snapshot};

/// Окна, которые сообщает Claude Code: id, длительность в минутах.
const WINDOWS: [(&str, u32); 2] = [("five_hour", 300), ("seven_day", 10_080)];

/// Сколько данные считаются свежими после записи.
pub const FRESH_MS: i64 = 15 * 60_000;

/// Сессий в файле не больше – и забываем тех, кого не слышно неделю.
const MAX_SESSIONS: usize = 32;
const FORGET_S: f64 = 7.0 * 86_400.0;

#[derive(Debug, PartialEq, Eq)]
pub enum BridgeError {
    /// Во входе нет лимитов (до первого ответа API их нет).
    NoRateLimits,
    /// Вход – не JSON строки состояния.
    Malformed,
    Io(String),
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BridgeError::NoRateLimits => write!(f, "нет rate_limits"),
            BridgeError::Malformed => write!(f, "неразборчивый вход"),
            BridgeError::Io(e) => write!(f, "запись: {e}"),
        }
    }
}

pub fn snapshot_path() -> PathBuf {
    crate::paths::data_dir().join("claude-capacity.json")
}

// ---------- вход строки состояния ----------

#[derive(Deserialize)]
struct StatusLineInput {
    session_id: Option<String>,
    rate_limits: Option<RateLimits>,
}

#[derive(Deserialize)]
struct RateLimits {
    five_hour: Option<SourceWindow>,
    seven_day: Option<SourceWindow>,
}

#[derive(Deserialize)]
struct SourceWindow {
    used_percentage: f64,
    /// Секунды Unix.
    resets_at: f64,
}

// ---------- наш файл ----------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct Stored {
    schema_version: u32,
    captured_at: f64,
    windows: Vec<StoredWindow>,
    #[serde(default)]
    sessions: Vec<StoredSession>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct StoredWindow {
    id: String,
    used_percentage: f64,
    resets_at: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct StoredSession {
    /// Хеш id сессии – сам id из него не восстановить.
    id: String,
    seen_at: f64,
    windows: Vec<SessionWindow>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct SessionWindow {
    id: String,
    used_percentage: f64,
    resets_at: f64,
    changed_at: f64,
}

fn session_key(id: Option<&str>) -> String {
    let digest = Sha256::digest(id.unwrap_or("").as_bytes());
    digest[..6].iter().map(|b| format!("{b:02x}")).collect()
}

/// Разбирает вход строки состояния и обновляет файл `dest`.
pub fn publish(input: &[u8], now_s: f64, dest: &Path) -> Result<(), BridgeError> {
    let input: StatusLineInput =
        serde_json::from_slice(input).map_err(|_| BridgeError::Malformed)?;
    let limits = input.rate_limits.ok_or(BridgeError::NoRateLimits)?;
    let incoming: Vec<StoredWindow> = [
        ("five_hour", limits.five_hour),
        ("seven_day", limits.seven_day),
    ]
    .into_iter()
    .filter_map(|(id, w)| {
        w.map(|w| StoredWindow {
            id: id.into(),
            used_percentage: w.used_percentage,
            resets_at: w.resets_at,
        })
    })
    .collect();
    if incoming.is_empty() {
        return Err(BridgeError::NoRateLimits);
    }

    let held = load(dest).map(|s| s.sessions).unwrap_or_default();
    let sessions = record(
        &incoming,
        &session_key(input.session_id.as_deref()),
        held,
        now_s,
    );
    let stored = Stored {
        schema_version: 1,
        captured_at: now_s,
        windows: believed(&sessions, now_s),
        sessions,
    };
    let data = serde_json::to_vec(&stored).map_err(|e| BridgeError::Io(e.to_string()))?;
    crate::paths::write_atomic(dest, &data).map_err(|e| BridgeError::Io(e.to_string()))
}

fn record(
    incoming: &[StoredWindow],
    id: &str,
    held: Vec<StoredSession>,
    now: f64,
) -> Vec<StoredSession> {
    let before = held.iter().find(|s| s.id == id);
    let mut windows: Vec<SessionWindow> = incoming
        .iter()
        .map(|new| {
            let old = before.and_then(|b| b.windows.iter().find(|w| w.id == new.id));
            let same = old.is_some_and(|o| {
                o.used_percentage == new.used_percentage && o.resets_at == new.resets_at
            });
            SessionWindow {
                id: new.id.clone(),
                used_percentage: new.used_percentage,
                resets_at: new.resets_at,
                changed_at: if same { old.unwrap().changed_at } else { now },
            }
        })
        .collect();
    // Окно, которое Claude Code перестал присылать (сброс прошёл), храним.
    if let Some(b) = before {
        for old in &b.windows {
            if !windows.iter().any(|w| w.id == old.id) {
                windows.push(old.clone());
            }
        }
    }
    let mut sessions: Vec<StoredSession> = held
        .into_iter()
        .filter(|s| s.id != id && now - s.seen_at < FORGET_S)
        .collect();
    sessions.push(StoredSession {
        id: id.into(),
        seen_at: now,
        windows,
    });
    sessions.sort_by(|a, b| b.seen_at.total_cmp(&a.seen_at));
    sessions.truncate(MAX_SESSIONS);
    sessions
}

fn believed(sessions: &[StoredSession], now: f64) -> Vec<StoredWindow> {
    WINDOWS
        .iter()
        .filter_map(|(id, _)| {
            sessions
                .iter()
                .filter_map(|s| s.windows.iter().find(|w| w.id == *id && w.resets_at > now))
                .max_by(|a, b| a.changed_at.total_cmp(&b.changed_at))
                .map(|w| StoredWindow {
                    id: w.id.clone(),
                    used_percentage: w.used_percentage,
                    resets_at: w.resets_at,
                })
        })
        .collect()
}

fn load(path: &Path) -> Option<Stored> {
    let data = std::fs::read(path).ok()?;
    let stored: Stored = serde_json::from_slice(&data).ok()?;
    (stored.schema_version == 1).then_some(stored)
}

/// Что показать: лимиты из файла моста. Окно, чей сброс уже прошёл,
/// считается целым (0 %), пока Claude Code не пришлёт новое.
pub fn read(path: &Path, now_ms: i64) -> Snapshot {
    let Some(stored) = load(path) else {
        return Snapshot::empty(
            Provider::Claude,
            Connection::Waiting,
            Some("Данные придут после следующего ответа Claude Code"),
        );
    };
    let now_s = now_ms as f64 / 1000.0;
    let windows: Vec<QuotaWindow> = WINDOWS
        .iter()
        .filter_map(|(id, minutes)| {
            let seen_ever = stored
                .sessions
                .iter()
                .any(|s| s.windows.iter().any(|w| w.id == *id))
                || stored.windows.iter().any(|w| w.id == *id);
            match stored
                .windows
                .iter()
                .find(|w| w.id == *id && w.resets_at > now_s)
            {
                Some(w) => Some(QuotaWindow::new(
                    *id,
                    Some(*minutes),
                    w.used_percentage / 100.0,
                    Some((w.resets_at * 1000.0) as i64),
                    now_ms,
                )),
                None if seen_ever => Some(QuotaWindow::new(*id, Some(*minutes), 0.0, None, now_ms)),
                None => None,
            }
        })
        .collect();
    let captured = (stored.captured_at * 1000.0) as i64;
    Snapshot {
        provider: Provider::Claude,
        connection: if now_ms - captured <= FRESH_MS {
            Connection::Fresh
        } else {
            Connection::Stale
        },
        captured_at: Some(captured),
        windows,
        note: None,
    }
}

// ---------- ~/.claude/settings.json ----------

/// Каталог настроек Claude Code (учитывает `CLAUDE_CONFIG_DIR`).
pub fn settings_path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        return Some(PathBuf::from(dir).join("settings.json"));
    }
    dirs::home_dir().map(|h| h.join(".claude").join("settings.json"))
}

/// Признак нашей команды в `statusLine.command`.
const MARK: &str = "kirk-bridge";
const THEN: &str = "--then";

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", content = "command", rename_all = "camelCase")]
pub enum StatusLine {
    /// Строки состояния нет.
    Missing,
    /// Стоит мост Kirk.
    Ours,
    /// Стоит чужая команда – при установке она продолжит работать через мост.
    Other(String),
}

fn parse_settings(text: &str) -> Result<Map<String, Value>, String> {
    if text.trim().is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(m)) => Ok(m),
        Ok(_) => Err("settings.json – не объект".into()),
        Err(e) => Err(format!("settings.json не разобрать: {e}")),
    }
}

pub fn status(text: &str) -> Result<StatusLine, String> {
    let map = parse_settings(text)?;
    let command = map
        .get("statusLine")
        .and_then(|v| v.get("command"))
        .and_then(Value::as_str)
        .unwrap_or("");
    Ok(if command.trim().is_empty() {
        StatusLine::Missing
    } else if command.contains(MARK) {
        StatusLine::Ours
    } else {
        StatusLine::Other(command.to_owned())
    })
}

/// Команда моста; путь – с прямыми слешами и в кавычках: её запускает
/// оболочка (bash на Mac, Git Bash или cmd на Windows).
pub fn bridge_command(bridge: &Path, previous: Option<&str>) -> String {
    let path = bridge.to_string_lossy().replace('\\', "/");
    let mut cmd = format!("\"{path}\"");
    if let Some(prev) = previous.filter(|p| !p.trim().is_empty()) {
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(prev);
        cmd.push_str(&format!(" {THEN} {encoded}"));
    }
    cmd
}

/// Прежняя команда, сохранённая в нашей (`--then <base64>`).
pub fn previous_command(ours: &str) -> Option<String> {
    let rest = ours.split_once(THEN)?.1.trim();
    let encoded = rest.split_whitespace().next()?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .ok()?;
    String::from_utf8(bytes).ok()
}

/// Новый текст settings.json с мостом. Прочие ключи остаются как были.
pub fn install(text: &str, bridge: &Path) -> Result<String, String> {
    let mut map = parse_settings(text)?;
    let mut line = match map.remove("statusLine") {
        Some(Value::Object(o)) => o,
        _ => Map::new(),
    };
    let current = line
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let previous = if current.contains(MARK) {
        previous_command(&current)
    } else {
        Some(current).filter(|c| !c.trim().is_empty())
    };
    line.insert("type".into(), Value::String("command".into()));
    line.insert(
        "command".into(),
        Value::String(bridge_command(bridge, previous.as_deref())),
    );
    map.insert("statusLine".into(), Value::Object(line));
    serde_json::to_string_pretty(&Value::Object(map))
        .map(|s| s + "\n")
        .map_err(|e| e.to_string())
}

/// Убирает мост: возвращает прежнюю команду или удаляет `statusLine`.
pub fn uninstall(text: &str) -> Result<String, String> {
    let mut map = parse_settings(text)?;
    let current = map
        .get("statusLine")
        .and_then(|v| v.get("command"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    if !current.contains(MARK) {
        return Ok(text.to_owned());
    }
    match previous_command(&current) {
        Some(prev) => {
            if let Some(Value::Object(line)) = map.get_mut("statusLine") {
                line.insert("command".into(), Value::String(prev));
            }
        }
        None => {
            map.remove("statusLine");
        }
    }
    serde_json::to_string_pretty(&Value::Object(map))
        .map(|s| s + "\n")
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kirk-claude-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("claude-capacity.json")
    }

    fn input(session: &str, five: f64, week: f64, resets: f64) -> Vec<u8> {
        format!(
            r#"{{"session_id":"{session}","model":{{"id":"x"}},"rate_limits":{{
            "five_hour":{{"used_percentage":{five},"resets_at":{resets}}},
            "seven_day":{{"used_percentage":{week},"resets_at":{}}}}}}}"#,
            resets + 86_400.0
        )
        .into_bytes()
    }

    #[test]
    fn publishes_and_reads_windows() {
        let file = tmp("basic");
        publish(&input("a", 42.0, 10.0, 10_000.0), 1_000.0, &file).unwrap();
        let snap = read(&file, 1_000_000);
        assert_eq!(snap.connection, Connection::Fresh);
        assert_eq!(snap.windows.len(), 2);
        assert_eq!(snap.windows[0].label, "5 ч");
        assert!((snap.windows[0].used - 0.42).abs() < 1e-9);
        assert_eq!(snap.windows[0].resets_at, Some(10_000_000));
        assert_eq!(snap.windows[1].label, "Неделя");
    }

    #[test]
    fn stores_no_session_ids_or_extra_fields() {
        let file = tmp("private");
        publish(&input("secret-session", 1.0, 1.0, 10_000.0), 1_000.0, &file).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(!text.contains("secret-session"));
        assert!(!text.contains("model"));
    }

    #[test]
    fn idle_session_does_not_override_active_one() {
        let file = tmp("sessions");
        publish(&input("busy", 10.0, 5.0, 10_000.0), 100.0, &file).unwrap();
        publish(&input("idle", 10.0, 5.0, 10_000.0), 101.0, &file).unwrap();
        publish(&input("busy", 30.0, 7.0, 10_000.0), 200.0, &file).unwrap();
        // Простаивающая повторяет старое – ей не верим.
        publish(&input("idle", 10.0, 5.0, 10_000.0), 300.0, &file).unwrap();
        let snap = read(&file, 300_000);
        assert!((snap.windows[0].used - 0.30).abs() < 1e-9);
    }

    #[test]
    fn passed_reset_reads_as_whole() {
        let file = tmp("reset");
        publish(&input("a", 80.0, 10.0, 500.0), 100.0, &file).unwrap();
        let snap = read(&file, 600_000);
        assert_eq!(snap.windows[0].used, 0.0);
        assert_eq!(snap.windows[0].resets_at, None);
    }

    #[test]
    fn missing_rate_limits_is_an_error_and_keeps_file() {
        let file = tmp("missing");
        publish(&input("a", 50.0, 10.0, 10_000.0), 100.0, &file).unwrap();
        let before = std::fs::read(&file).unwrap();
        assert_eq!(
            publish(br#"{"session_id":"a"}"#, 200.0, &file),
            Err(BridgeError::NoRateLimits)
        );
        assert_eq!(
            publish(b"not json", 200.0, &file),
            Err(BridgeError::Malformed)
        );
        assert_eq!(std::fs::read(&file).unwrap(), before);
    }

    #[test]
    fn no_file_means_waiting() {
        assert_eq!(read(&tmp("none"), 0).connection, Connection::Waiting);
    }

    #[test]
    fn install_keeps_previous_command_and_other_keys() {
        let text =
            r#"{"model":"opus","statusLine":{"type":"command","command":"echo 'hi'","padding":0}}"#;
        assert_eq!(status(text).unwrap(), StatusLine::Other("echo 'hi'".into()));
        let bridge = Path::new("/Applications/Kirk.app/Contents/MacOS/kirk-bridge");
        let installed = install(text, bridge).unwrap();
        assert_eq!(status(&installed).unwrap(), StatusLine::Ours);
        let v: Value = serde_json::from_str(&installed).unwrap();
        assert_eq!(v["model"], "opus");
        assert_eq!(v["statusLine"]["padding"], 0);
        let cmd = v["statusLine"]["command"].as_str().unwrap();
        assert_eq!(previous_command(cmd).as_deref(), Some("echo 'hi'"));
        // Повторная установка не вкладывает мост в мост.
        let again = install(&installed, bridge).unwrap();
        assert_eq!(again, installed);
        let removed = uninstall(&installed).unwrap();
        assert_eq!(
            status(&removed).unwrap(),
            StatusLine::Other("echo 'hi'".into())
        );
    }

    #[test]
    fn install_into_empty_and_remove() {
        let bridge = Path::new(r"C:\Program Files\Kirk\kirk-bridge.exe");
        let installed = install("", bridge).unwrap();
        let v: Value = serde_json::from_str(&installed).unwrap();
        assert_eq!(
            v["statusLine"]["command"],
            "\"C:/Program Files/Kirk/kirk-bridge.exe\""
        );
        let removed = uninstall(&installed).unwrap();
        assert_eq!(status(&removed).unwrap(), StatusLine::Missing);
    }

    #[test]
    fn broken_settings_are_not_overwritten() {
        assert!(install("{ not json", Path::new("/x/kirk-bridge")).is_err());
    }
}
