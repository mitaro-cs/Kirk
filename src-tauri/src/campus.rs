//! Campus: вход кодом «Показать код», затем «Сегодня» раз в несколько минут
//! и сразу, когда на сайте что-то изменилось (`/api/live/seq`).
//!
//! Сессия хранится в связке ключей системы (Keychain, диспетчер учётных
//! данных Windows), не в файле.

use std::time::Duration;

use kirk_core::campus::{self, Me, Today};
use reqwest::header::{HeaderMap, CONTENT_TYPE, COOKIE, SET_COOKIE};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tokio::sync::Notify;

use crate::hub::{self, Hub};

const KEYRING_SERVICE: &str = kirk_core::paths::IDENTIFIER;
const KEYRING_USER: &str = "campus";
const TODAY_EVERY: Duration = Duration::from_secs(3 * 60);
const SEQ_EVERY: Duration = Duration::from_secs(20);

#[derive(Serialize, Deserialize, Clone)]
struct Session {
    url: String,
    cookie_name: String,
    cookie: String,
}

/// Будит опрос: вход, выход, открытие острова.
pub struct Wake(pub Notify);

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("Kirk/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(15))
        .build()
        .expect("HTTP-клиент")
}

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|e| e.to_string())
}

fn load() -> Option<Session> {
    let secret = entry().ok()?.get_password().ok()?;
    serde_json::from_str(&secret).ok()
}

fn store(s: &Session) -> Result<(), String> {
    let text = serde_json::to_string(s).map_err(|e| e.to_string())?;
    entry()?.set_password(&text).map_err(|e| format!("связка ключей: {e}"))
}

fn forget() {
    if let Ok(e) = entry() {
        let _ = e.delete_credential();
    }
}

fn set_cookies(headers: &HeaderMap) -> Vec<&str> {
    headers.get_all(SET_COOKIE).iter().filter_map(|v| v.to_str().ok()).collect()
}

/// Ответ от сервера Campus, а не от страницы туннеля: метка или JSON.
fn from_campus(res: &reqwest::Response) -> bool {
    res.headers().contains_key("x-groupbase")
        || res
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|t| t.starts_with("application/json"))
}

#[derive(Deserialize)]
struct ApiError {
    message: Option<String>,
}

async fn error_text(res: reqwest::Response) -> String {
    let status = res.status();
    match res.json::<ApiError>().await {
        Ok(ApiError { message: Some(m) }) => m,
        _ => format!("сервер ответил {status}"),
    }
}

/// Вход по 6 цифрам из Campus (Профиль → Безопасность → «Показать код»).
pub async fn connect(app: &AppHandle, url: &str, pin: &str) -> Result<(), String> {
    let url = campus::normalize_url(url).ok_or("Проверьте адрес сайта")?;
    let pin = campus::normalize_pin(pin).ok_or("Код – 6 цифр")?;
    let http = client();

    let health = http
        .get(format!("{url}/api/health"))
        .send()
        .await
        .map_err(|e| format!("Сайт не отвечает: {e}"))?;
    if !from_campus(&health) {
        return Err("По этому адресу не Campus или сайт сейчас выключен".into());
    }
    let (csrf_name, csrf) = campus::cookie(set_cookies(health.headers()), "gb_csrf")
        .ok_or("Сайт не выдал защитный ключ (CSRF)")?;

    let device = format!("Kirk · {}", if cfg!(target_os = "macos") { "Mac" } else if cfg!(windows) { "Windows" } else { "Linux" });
    let res = http
        .post(format!("{url}/api/auth/link/redeem"))
        .header("X-CSRF-Token", &csrf)
        .header(COOKIE, format!("{csrf_name}={csrf}"))
        .json(&serde_json::json!({ "pin": pin, "device": device }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(error_text(res).await);
    }
    let (cookie_name, cookie) = campus::cookie(set_cookies(res.headers()), "gb_session")
        .ok_or("Сайт не открыл сессию")?;
    store(&Session { url: url.clone(), cookie_name, cookie })?;
    hub::update_campus(app, |c| {
        *c = Default::default();
        c.connected = true;
        c.site = Some(url);
    });
    app.state::<Wake>().0.notify_one();
    Ok(())
}

pub fn disconnect(app: &AppHandle) {
    forget();
    hub::update_campus(app, |c| *c = Default::default());
    app.state::<Wake>().0.notify_one();
}

enum Failure {
    /// Сессия больше не действует.
    SignedOut,
    Offline(String),
}

async fn get<T: serde::de::DeserializeOwned>(http: &reqwest::Client, s: &mut Session, path: &str) -> Result<T, Failure> {
    let res = http
        .get(format!("{}{path}", s.url))
        .header(COOKIE, format!("{}={}", s.cookie_name, s.cookie))
        .send()
        .await
        .map_err(|e| Failure::Offline(format!("нет связи: {e}")))?;
    if res.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(Failure::SignedOut);
    }
    if !res.status().is_success() || !from_campus(&res) {
        return Err(Failure::Offline(format!("сайт недоступен ({})", res.status())));
    }
    // Сервер продлевает сессию – запоминаем новое значение.
    if let Some((name, value)) = campus::cookie(set_cookies(res.headers()), "gb_session") {
        if value != s.cookie {
            s.cookie_name = name;
            s.cookie = value;
            let _ = store(s);
        }
    }
    res.json::<T>().await.map_err(|e| Failure::Offline(format!("ответ не разобрать: {e}")))
}

#[derive(Deserialize, PartialEq)]
struct Seq {
    seq: serde_json::Value,
}

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let http = client();
        let mut last_seq: Option<serde_json::Value> = None;
        let mut last_today = 0i64;
        // Последний «Сегодня»: текущая и следующая пара пересчитываются по
        // времени без запроса к сайту.
        let mut today: Option<Today> = None;
        loop {
            let wake = app.state::<Wake>();
            let enabled = app.state::<Hub>().config().campus;
            let session = if enabled { load() } else { None };
            let Some(mut s) = session else {
                today = None;
                hub::update_campus(&app, |c| {
                    if c.connected {
                        *c = Default::default();
                    }
                });
                tokio::select! {
                    _ = wake.0.notified() => {}
                    _ = tokio::time::sleep(Duration::from_secs(60)) => {}
                }
                continue;
            };

            let now = kirk_core::now_ms();
            let mut due = now - last_today >= TODAY_EVERY.as_millis() as i64;
            match get::<Seq>(&http, &mut s, "/api/live/seq").await {
                Ok(seq) => {
                    if last_seq.as_ref() != Some(&seq.seq) {
                        due = true;
                        last_seq = Some(seq.seq);
                    }
                }
                Err(Failure::SignedOut) => {
                    forget();
                    today = None;
                    hub::update_campus(&app, |c| {
                        *c = Default::default();
                        c.error = Some("Сессия Campus закончилась – войдите снова".into());
                    });
                    continue;
                }
                Err(Failure::Offline(_)) => {}
            }

            let need_name = app.state::<Hub>().campus().name.is_none();
            if due || need_name {
                let me = if need_name { get::<Me>(&http, &mut s, "/api/me").await.ok() } else { None };
                let fetched = get::<Today>(&http, &mut s, "/api/today").await;
                let url = s.url.clone();
                hub::update_campus(&app, |c| {
                    c.connected = true;
                    c.site = Some(url);
                    if let Some(me) = me {
                        c.name = Some(me.name());
                    }
                    match fetched {
                        Ok(t) => {
                            today = Some(t);
                            c.updated_at = Some(kirk_core::now_ms());
                            c.error = None;
                        }
                        Err(Failure::Offline(e)) => c.error = Some(e),
                        Err(Failure::SignedOut) => c.error = Some("Нужно войти снова".into()),
                    }
                });
                last_today = kirk_core::now_ms();
            }
            if let Some(t) = &today {
                let summary = campus::summarize(t, kirk_core::now_ms());
                hub::update_campus(&app, |c| c.summary = Some(summary));
            }

            tokio::select! {
                _ = wake.0.notified() => { last_today = 0; }
                _ = tokio::time::sleep(SEQ_EVERY) => {}
            }
        }
    });
}
