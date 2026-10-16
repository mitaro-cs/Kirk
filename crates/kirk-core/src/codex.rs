//! Лимиты Codex через `codex app-server` (JSON-RPC построчно в stdin/stdout).
//!
//! Kirk только читает: `initialize`, `account/read`, `account/rateLimits/read`
//! и слушает `account/rateLimits/updated`. Методов входа, выхода и токенов
//! в списке нет – учётные данные Codex Kirk не трогает.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{json, Value};

use crate::limits::{Connection, Provider, QuotaWindow, Snapshot};

pub const INITIALIZE: &str = "initialize";
pub const INITIALIZED: &str = "initialized";
pub const READ_ACCOUNT: &str = "account/read";
pub const READ_LIMITS: &str = "account/rateLimits/read";
pub const LIMITS_UPDATED: &str = "account/rateLimits/updated";

/// Всё, что Kirk вправе отправить.
pub const PERMITTED: [&str; 4] = [INITIALIZE, INITIALIZED, READ_ACCOUNT, READ_LIMITS];

/// Строка запроса. Метод вне `PERMITTED` – ошибка программы.
pub fn request(id: u64, method: &str, params: Option<Value>) -> String {
    assert!(PERMITTED.contains(&method), "метод {method} не разрешён");
    let mut msg = json!({ "method": method, "id": id });
    if let Some(p) = params {
        msg["params"] = p;
    }
    msg.to_string()
}

pub fn notification(method: &str) -> String {
    assert!(PERMITTED.contains(&method), "метод {method} не разрешён");
    json!({ "method": method }).to_string()
}

pub fn initialize_params(version: &str) -> Value {
    json!({ "clientInfo": { "name": "kirk", "version": version } })
}

#[derive(Debug, PartialEq)]
pub enum Inbound {
    Response { id: u64, result: Value },
    Failure { id: u64, message: String },
    Notification { method: String, params: Value },
}

pub fn parse(line: &str) -> Option<Inbound> {
    let v: Value = serde_json::from_str(line).ok()?;
    let id = v.get("id").and_then(Value::as_u64);
    match (id, v.get("method").and_then(Value::as_str)) {
        (Some(id), None) => {
            if let Some(err) = v.get("error") {
                let message = err
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("ошибка Codex")
                    .to_owned();
                Some(Inbound::Failure { id, message })
            } else {
                Some(Inbound::Response {
                    id,
                    result: v.get("result").cloned().unwrap_or(Value::Null),
                })
            }
        }
        (None, Some(method)) => Some(Inbound::Notification {
            method: method.to_owned(),
            params: v.get("params").cloned().unwrap_or(Value::Null),
        }),
        // Запрос от сервера к клиенту – Kirk на них не отвечает.
        _ => None,
    }
}

/// Вошёл ли человек в Codex (`account` есть).
pub fn signed_in(account_result: &Value) -> bool {
    account_result.get("account").is_some_and(|a| !a.is_null())
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RateWindow {
    pub used_percent: f64,
    pub window_duration_mins: Option<u32>,
    /// Секунды Unix.
    pub resets_at: Option<i64>,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Default)]
pub struct RateLimits {
    pub primary: Option<RateWindow>,
    pub secondary: Option<RateWindow>,
}

/// `rateLimits` из ответа `account/rateLimits/read` или уведомления.
pub fn rate_limits(result: &Value) -> Option<RateLimits> {
    serde_json::from_value(result.get("rateLimits")?.clone()).ok()
}

/// Уведомление бывает неполным: чего в нём нет – остаётся прежним.
pub fn merge(update: RateLimits, previous: &RateLimits) -> RateLimits {
    RateLimits {
        primary: update.primary.or_else(|| previous.primary.clone()),
        secondary: update.secondary.or_else(|| previous.secondary.clone()),
    }
}

pub fn snapshot(limits: &RateLimits, now_ms: i64) -> Snapshot {
    let windows = [("codex-primary", &limits.primary), ("codex-secondary", &limits.secondary)]
        .into_iter()
        .filter_map(|(id, w)| {
            w.as_ref().map(|w| {
                QuotaWindow::new(
                    id,
                    w.window_duration_mins,
                    w.used_percent / 100.0,
                    w.resets_at.map(|s| s * 1000),
                    now_ms,
                )
            })
        })
        .collect();
    Snapshot {
        provider: Provider::Codex,
        connection: Connection::Fresh,
        captured_at: Some(now_ms),
        windows,
        note: None,
    }
}

/// Где искать `codex`: известные места, затем PATH.
pub fn candidates(home: &Path, path_var: Option<&std::ffi::OsStr>) -> Vec<PathBuf> {
    let mut list: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        list.extend(
            [
                "/Applications/ChatGPT.app/Contents/Resources/codex",
                "/Applications/ChatGPT.app/Contents/Resources/codex-cli/bin/codex",
                "/opt/homebrew/bin/codex",
                "/usr/local/bin/codex",
            ]
            .map(PathBuf::from),
        );
    }
    if cfg!(windows) {
        if let Some(appdata) = dirs::data_dir() {
            list.push(appdata.join("npm").join("codex.cmd"));
        }
        if let Some(local) = dirs::data_local_dir() {
            list.push(local.join("Programs").join("codex").join("codex.exe"));
        }
    }
    list.push(home.join(".local").join("bin").join(exe("codex")));
    list.push(home.join(".codex").join("bin").join(exe("codex")));
    if let Some(p) = path_var {
        for dir in std::env::split_paths(p) {
            list.push(dir.join(exe("codex")));
            if cfg!(windows) {
                list.push(dir.join("codex.cmd"));
            }
        }
    }
    list
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

pub fn locate() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    candidates(&home, std::env::var_os("PATH").as_deref())
        .into_iter()
        .find(|p| p.is_file())
}

/// PATH для запуска Codex: приложение из Finder видит только системные
/// каталоги, а Codex из npm – это скрипт, которому нужен `node`.
pub fn augmented_path(base: Option<&std::ffi::OsStr>, home: &Path) -> std::ffi::OsString {
    let mut dirs_list: Vec<PathBuf> = base.map(|b| std::env::split_paths(b).collect()).unwrap_or_default();
    let extra: Vec<PathBuf> = if cfg!(windows) {
        dirs::data_dir().map(|d| vec![d.join("npm")]).unwrap_or_default()
    } else {
        vec![
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
            home.join(".local/bin"),
        ]
    };
    for e in extra {
        if !dirs_list.contains(&e) {
            dirs_list.push(e);
        }
    }
    std::env::join_paths(dirs_list).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_requests() {
        let line = request(1, INITIALIZE, Some(initialize_params("0.1.0")));
        let v: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["id"], 1);
        assert_eq!(v["params"]["clientInfo"]["name"], "kirk");
        assert_eq!(notification(INITIALIZED), r#"{"method":"initialized"}"#);
    }

    #[test]
    #[should_panic]
    fn refuses_unlisted_methods() {
        request(1, "account/login/start", None);
    }

    #[test]
    fn parses_messages() {
        assert_eq!(
            parse(r#"{"id":2,"result":{"account":null,"requiresOpenaiAuth":true}}"#),
            Some(Inbound::Response {
                id: 2,
                result: json!({"account":null,"requiresOpenaiAuth":true})
            })
        );
        assert!(matches!(
            parse(r#"{"id":3,"error":{"code":-1,"message":"нет"}}"#),
            Some(Inbound::Failure { id: 3, .. })
        ));
        assert!(matches!(
            parse(r#"{"method":"account/rateLimits/updated","params":{}}"#),
            Some(Inbound::Notification { .. })
        ));
        assert_eq!(parse("garbage"), None);
        assert!(!signed_in(&json!({"account":null})));
        assert!(signed_in(&json!({"account":{"type":"chatgpt"}})));
    }

    #[test]
    fn maps_limits_and_merges_updates() {
        let result = json!({"rateLimits":{
            "primary":{"usedPercent":25,"windowDurationMins":300,"resetsAt":1000},
            "secondary":{"usedPercent":60,"windowDurationMins":10080,"resetsAt":5000}}});
        let full = rate_limits(&result).unwrap();
        let update = rate_limits(&json!({"rateLimits":{
            "primary":{"usedPercent":30,"windowDurationMins":300,"resetsAt":1000}}}))
        .unwrap();
        let merged = merge(update, &full);
        let snap = snapshot(&merged, 0);
        assert_eq!(snap.windows.len(), 2);
        assert_eq!(snap.windows[0].label, "5 ч");
        assert!((snap.windows[0].used - 0.30).abs() < 1e-9);
        assert_eq!(snap.windows[0].resets_at, Some(1_000_000));
        assert!((snap.windows[1].used - 0.60).abs() < 1e-9);
    }

    #[test]
    fn path_gets_common_dirs_once() {
        let home = Path::new("/Users/me");
        let p = augmented_path(Some(std::ffi::OsStr::new("/usr/bin")), home);
        let again = augmented_path(Some(&p), home);
        assert_eq!(p, again);
    }
}
