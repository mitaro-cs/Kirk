//! Campus: вход кодом «Показать код», затем «Сегодня» и уведомления – сразу,
//! как на сайте что-то изменилось (`/api/live/seq`: раз в 5 с, пока остров
//! открыт, иначе раз в 15 с), и не реже раза в 3 минуты.
//!
//! Из острова можно отметить задание сделанным, открыть задание, пару или
//! уведомление на сайте. Новое уведомление коротко всплывает в острове.
//!
//! Сессия хранится в связке ключей системы (Keychain, диспетчер учётных
//! данных Windows), не в файле.

use std::sync::Mutex;
use std::time::Duration;

use kirk_core::campus::{self, Me, NotificationPage, Today};
use reqwest::header::{HeaderMap, CONTENT_TYPE, COOKIE, SET_COOKIE};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tokio::sync::Notify;

use crate::hub::{self, Hub};

const KEYRING_SERVICE: &str = kirk_core::paths::IDENTIFIER;
const KEYRING_USER: &str = "campus";
const TODAY_EVERY: Duration = Duration::from_secs(3 * 60);
const SEQ_OPEN: Duration = Duration::from_secs(5);
const SEQ_CLOSED: Duration = Duration::from_secs(15);

#[derive(Serialize, Deserialize, Clone)]
struct Session {
    url: String,
    cookie_name: String,
    cookie: String,
    /// Ключ CSRF (двойная отправка: cookie и заголовок) – для изменений.
    #[serde(default)]
    csrf_name: String,
    #[serde(default)]
    csrf: String,
}

/// Управление опросом из команд: разбудить, отметить сделанным сразу.
pub struct Campus {
    wake: Notify,
    today: Mutex<Option<Today>>,
}

impl Campus {
    pub fn new() -> Self {
        Campus {
            wake: Notify::new(),
            today: Mutex::new(None),
        }
    }

    pub fn wake(&self) {
        self.wake.notify_one();
    }

    fn today(&self) -> std::sync::MutexGuard<'_, Option<Today>> {
        self.today.lock().unwrap_or_else(|e| e.into_inner())
    }
}

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
    entry()?
        .set_password(&text)
        .map_err(|e| format!("связка ключей: {e}"))
}

fn forget() {
    if let Ok(e) = entry() {
        let _ = e.delete_credential();
    }
}

fn set_cookies(headers: &HeaderMap) -> Vec<&str> {
    headers
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect()
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

/// CSRF-ключ сайта: его выдаёт любой ответ, проще всего `/api/health`.
async fn fetch_csrf(http: &reqwest::Client, url: &str) -> Result<(String, String), String> {
    let health = http
        .get(format!("{url}/api/health"))
        .send()
        .await
        .map_err(|e| format!("Сайт не отвечает: {e}"))?;
    if !from_campus(&health) {
        return Err("По этому адресу не Campus или сайт сейчас выключен".into());
    }
    campus::cookie(set_cookies(health.headers()), "gb_csrf")
        .ok_or_else(|| "Сайт не выдал защитный ключ (CSRF)".into())
}

/// Вход по 6 цифрам из Campus (Профиль → Безопасность → «Показать код»).
pub async fn connect(app: &AppHandle, url: &str, pin: &str) -> Result<(), String> {
    let url = campus::normalize_url(url).ok_or("Проверьте адрес сайта")?;
    let pin = campus::normalize_pin(pin).ok_or("Код – 6 цифр")?;
    let http = client();
    let (csrf_name, csrf) = fetch_csrf(&http, &url).await?;

    let device = format!(
        "Kirk · {}",
        if cfg!(target_os = "macos") {
            "Mac"
        } else if cfg!(windows) {
            "Windows"
        } else {
            "Linux"
        }
    );
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
    let (cookie_name, cookie) =
        campus::cookie(set_cookies(res.headers()), "gb_session").ok_or("Сайт не открыл сессию")?;
    store(&Session {
        url: url.clone(),
        cookie_name,
        cookie,
        csrf_name,
        csrf,
    })?;
    *app.state::<Campus>().today() = None;
    hub::update_campus(app, |c| {
        *c = Default::default();
        c.connected = true;
        c.site = Some(url);
    });
    app.state::<Campus>().wake();
    Ok(())
}

pub fn disconnect(app: &AppHandle) {
    forget();
    *app.state::<Campus>().today() = None;
    hub::update_campus(app, |c| *c = Default::default());
    app.state::<Campus>().wake();
}

enum Failure {
    /// Сессия больше не действует.
    SignedOut,
    Offline(String),
}

impl Failure {
    fn text(self) -> String {
        match self {
            Failure::SignedOut => "Сессия Campus закончилась – войдите снова".into(),
            Failure::Offline(e) => e,
        }
    }
}

fn cookies(s: &Session) -> String {
    if s.csrf.is_empty() {
        format!("{}={}", s.cookie_name, s.cookie)
    } else {
        format!("{}={}; {}={}", s.cookie_name, s.cookie, s.csrf_name, s.csrf)
    }
}

/// Запрос к API с сессией. Изменения (не GET) – с CSRF.
async fn call(
    http: &reqwest::Client,
    s: &mut Session,
    method: reqwest::Method,
    path: &str,
    body: Option<serde_json::Value>,
) -> Result<reqwest::Response, Failure> {
    let mutating = method != reqwest::Method::GET;
    if mutating && s.csrf.is_empty() {
        let (name, value) = fetch_csrf(http, &s.url).await.map_err(Failure::Offline)?;
        s.csrf_name = name;
        s.csrf = value;
        let _ = store(s);
    }
    let mut req = http
        .request(method, format!("{}{path}", s.url))
        .header(COOKIE, cookies(s));
    if mutating {
        req = req.header("X-CSRF-Token", &s.csrf);
    }
    if let Some(b) = body {
        req = req.json(&b);
    }
    let res = req
        .send()
        .await
        .map_err(|e| Failure::Offline(format!("нет связи: {e}")))?;
    if res.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(Failure::SignedOut);
    }
    if !from_campus(&res) {
        return Err(Failure::Offline(format!(
            "сайт недоступен ({})",
            res.status()
        )));
    }
    // Сервер продлевает сессию и может сменить CSRF – запоминаем.
    let mut changed = false;
    if let Some((name, value)) = campus::cookie(set_cookies(res.headers()), "gb_session") {
        if value != s.cookie {
            (s.cookie_name, s.cookie, changed) = (name, value, true);
        }
    }
    if let Some((name, value)) = campus::cookie(set_cookies(res.headers()), "gb_csrf") {
        if value != s.csrf {
            (s.csrf_name, s.csrf, changed) = (name, value, true);
        }
    }
    if changed {
        let _ = store(s);
    }
    if !res.status().is_success() {
        return Err(Failure::Offline(error_text(res).await));
    }
    Ok(res)
}

async fn get<T: serde::de::DeserializeOwned>(
    http: &reqwest::Client,
    s: &mut Session,
    path: &str,
) -> Result<T, Failure> {
    call(http, s, reqwest::Method::GET, path, None)
        .await?
        .json::<T>()
        .await
        .map_err(|e| Failure::Offline(format!("ответ не разобрать: {e}")))
}

/// Отметить задание сделанным (или вернуть). В острове – сразу, на сайте – следом.
pub async fn set_done(app: &AppHandle, id: i64, value: bool) -> Result<(), String> {
    let mut s = load().ok_or("Campus не подключён")?;
    let ctl = app.state::<Campus>();
    if let Some(t) = ctl.today().as_mut() {
        campus::mark_done(t, id, value);
    }
    refresh_summary(app);
    let res = call(
        &client(),
        &mut s,
        reqwest::Method::PUT,
        &format!("/api/homework/{id}/done"),
        Some(serde_json::json!({ "value": value })),
    )
    .await;
    if let Err(e) = res {
        // Не вышло – вернуть как было.
        if let Some(t) = ctl.today().as_mut() {
            campus::mark_done(t, id, !value);
        }
        refresh_summary(app);
        return Err(e.text());
    }
    ctl.wake();
    Ok(())
}

pub async fn read_all(app: &AppHandle) -> Result<(), String> {
    let mut s = load().ok_or("Campus не подключён")?;
    call(
        &client(),
        &mut s,
        reqwest::Method::POST,
        "/api/notifications/read",
        Some(serde_json::json!({ "all": true })),
    )
    .await
    .map_err(Failure::text)?;
    hub::update_campus(app, |c| {
        c.unread = 0;
        c.notifications.iter_mut().for_each(|n| n.read = true);
    });
    app.state::<Campus>().wake();
    Ok(())
}

/// Ссылка на страницу сайта (только свой путь).
pub fn link(app: &AppHandle, path: &str) -> Result<String, String> {
    let site = app
        .state::<Hub>()
        .campus()
        .site
        .ok_or("Campus не подключён")?;
    campus::site_link(&site, path).ok_or_else(|| "Неверный адрес".into())
}

fn refresh_summary(app: &AppHandle) {
    let summary = app
        .state::<Campus>()
        .today()
        .as_ref()
        .map(|t| campus::summarize(t, kirk_core::now_ms()));
    if let Some(summary) = summary {
        hub::update_campus(app, |c| c.summary = Some(summary));
    }
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
        // Уведомления, что уже видели: новое среди них не всплывёт второй раз.
        let mut seen: Option<Vec<i64>> = None;
        loop {
            let ctl = app.state::<Campus>();
            let enabled = app.state::<Hub>().config().campus;
            let session = if enabled { load() } else { None };
            let Some(mut s) = session else {
                *ctl.today() = None;
                seen = None;
                hub::update_campus(&app, |c| {
                    if c.connected {
                        *c = Default::default();
                    }
                });
                tokio::select! {
                    _ = ctl.wake.notified() => {}
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
                    *ctl.today() = None;
                    hub::update_campus(&app, |c| {
                        *c = Default::default();
                        c.error = Some(Failure::SignedOut.text());
                    });
                    continue;
                }
                Err(Failure::Offline(_)) => {}
            }

            let need_name = app.state::<Hub>().campus().name.is_none();
            if due || need_name {
                let me = if need_name {
                    get::<Me>(&http, &mut s, "/api/me").await.ok()
                } else {
                    None
                };
                let fetched = get::<Today>(&http, &mut s, "/api/today").await;
                let notes = get::<NotificationPage>(&http, &mut s, "/api/notifications?limit=8")
                    .await
                    .ok();

                if let Some(page) = &notes {
                    for n in campus::fresh_notifications(seen.as_deref(), &page.items)
                        .into_iter()
                        .take(1)
                    {
                        crate::island::peek(
                            &app,
                            n.title.clone(),
                            n.body.clone().unwrap_or_default(),
                            n.url.clone(),
                        );
                    }
                    let mut ids: Vec<i64> = page.items.iter().map(|n| n.id).collect();
                    ids.extend(seen.take().unwrap_or_default());
                    ids.sort_unstable();
                    ids.dedup();
                    seen = Some(ids);
                }

                let ok = fetched.is_ok();
                let error = match fetched {
                    Ok(t) => {
                        *ctl.today() = Some(t);
                        None
                    }
                    Err(e) => Some(e.text()),
                };
                let url = s.url.clone();
                hub::update_campus(&app, |c| {
                    c.connected = true;
                    c.site = Some(url);
                    if let Some(me) = me {
                        c.name = Some(me.name());
                    }
                    if let Some(page) = notes {
                        c.unread = page.unread;
                        c.notifications = page.items;
                    }
                    if ok {
                        c.updated_at = Some(kirk_core::now_ms());
                    }
                    c.error = error;
                });
                last_today = kirk_core::now_ms();
            }
            // Текущая и следующая пара – по времени, без запроса к сайту.
            refresh_summary(&app);

            let open = *app
                .state::<Hub>()
                .open
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            tokio::select! {
                _ = ctl.wake.notified() => { last_today = 0; }
                _ = tokio::time::sleep(if open { SEQ_OPEN } else { SEQ_CLOSED }) => {}
            }
        }
    });
}
