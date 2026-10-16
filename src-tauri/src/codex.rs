//! Служба лимитов Codex: свой `codex app-server`, только чтение.

use std::process::Stdio;
use std::time::Duration;

use kirk_core::codex::{self, Inbound, RateLimits};
use kirk_core::limits::{Connection, Provider, Snapshot};
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{ChildStdin, ChildStdout, Command};

use crate::hub::{self, Hub};

const REFRESH: Duration = Duration::from_secs(5 * 60);
const ANSWER: Duration = Duration::from_secs(30);

fn emit(app: &AppHandle, connection: Connection, note: &str) {
    hub::set_limits(app, Snapshot::empty(Provider::Codex, connection, Some(note)));
}

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut backoff = Duration::from_secs(15);
        loop {
            if !app.state::<Hub>().config().codex {
                emit(&app, Connection::Waiting, "Выключено в настройках");
                tokio::time::sleep(Duration::from_secs(30)).await;
                continue;
            }
            let Some(path) = codex::locate() else {
                emit(&app, Connection::NotInstalled, "Codex не найден");
                tokio::time::sleep(Duration::from_secs(10 * 60)).await;
                continue;
            };
            match session(&app, &path).await {
                Ok(Ended::SignedOut) => {
                    emit(&app, Connection::SignedOut, "Войдите: codex login");
                    tokio::time::sleep(Duration::from_secs(5 * 60)).await;
                    backoff = Duration::from_secs(15);
                }
                Ok(Ended::Disabled) => {}
                Err(e) => {
                    log::warn!("Codex: {e}");
                    let mut snap = app.state::<Hub>().limits()[1].clone();
                    snap.connection = if snap.windows.is_empty() { Connection::Failed } else { Connection::Stale };
                    snap.note = Some(format!("Codex: {e}"));
                    hub::set_limits(&app, snap);
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(Duration::from_secs(10 * 60));
                }
            }
        }
    });
}

enum Ended {
    SignedOut,
    Disabled,
}

struct Rpc {
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    next: u64,
    limits: RateLimits,
}

impl Rpc {
    async fn send(&mut self, line: String) -> Result<(), String> {
        self.stdin.write_all(line.as_bytes()).await.map_err(|e| e.to_string())?;
        self.stdin.write_all(b"\n").await.map_err(|e| e.to_string())?;
        self.stdin.flush().await.map_err(|e| e.to_string())
    }

    /// Запрос и ожидание ответа; уведомления по пути тоже разбираются.
    async fn call(&mut self, app: &AppHandle, method: &str, params: Option<serde_json::Value>) -> Result<serde_json::Value, String> {
        self.next += 1;
        let id = self.next;
        self.send(codex::request(id, method, params)).await?;
        let wait = async {
            loop {
                let line = self.lines.next_line().await.map_err(|e| e.to_string())?;
                let Some(line) = line else { return Err("Codex закрылся".to_string()) };
                match codex::parse(&line) {
                    Some(Inbound::Response { id: got, result }) if got == id => return Ok(result),
                    Some(Inbound::Failure { id: got, message }) if got == id => return Err(message),
                    Some(Inbound::Notification { method, params }) => self.notified(app, &method, &params),
                    _ => {}
                }
            }
        };
        tokio::time::timeout(ANSWER, wait).await.map_err(|_| "Codex не ответил".to_string())?
    }

    fn notified(&mut self, app: &AppHandle, method: &str, params: &serde_json::Value) {
        if method == codex::LIMITS_UPDATED {
            if let Some(update) = codex::rate_limits(params) {
                self.limits = codex::merge(update, &self.limits);
                hub::set_limits(app, codex::snapshot(&self.limits, kirk_core::now_ms()));
            }
        }
    }
}

async fn session(app: &AppHandle, path: &std::path::Path) -> Result<Ended, String> {
    let home = dirs_home();
    let mut cmd = Command::new(path);
    cmd.arg("app-server")
        .env("PATH", codex::augmented_path(std::env::var_os("PATH").as_deref(), &home))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let mut child = cmd.spawn().map_err(|e| format!("запуск: {e}"))?;
    let mut rpc = Rpc {
        stdin: child.stdin.take().ok_or("нет stdin")?,
        lines: BufReader::new(child.stdout.take().ok_or("нет stdout")?).lines(),
        next: 0,
        limits: RateLimits::default(),
    };

    rpc.call(app, codex::INITIALIZE, Some(codex::initialize_params(env!("CARGO_PKG_VERSION")))).await?;
    rpc.send(codex::notification(codex::INITIALIZED)).await?;
    let account = rpc.call(app, codex::READ_ACCOUNT, None).await?;
    if !codex::signed_in(&account) {
        return Ok(Ended::SignedOut);
    }

    let mut tick = tokio::time::interval(REFRESH);
    loop {
        tokio::select! {
            _ = tick.tick() => {
                if !app.state::<Hub>().config().codex {
                    return Ok(Ended::Disabled);
                }
                let result = rpc.call(app, codex::READ_LIMITS, None).await?;
                rpc.limits = codex::rate_limits(&result).ok_or("ответ без rateLimits")?;
                hub::set_limits(app, codex::snapshot(&rpc.limits, kirk_core::now_ms()));
            }
            line = rpc.lines.next_line() => {
                match line.map_err(|e| e.to_string())? {
                    Some(line) => {
                        if let Some(Inbound::Notification { method, params }) = codex::parse(&line) {
                            rpc.notified(app, &method, &params);
                        }
                    }
                    None => return Err("Codex закрылся".into()),
                }
            }
        }
    }
}

fn dirs_home() -> std::path::PathBuf {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(Into::into)
        .unwrap_or_default()
}
