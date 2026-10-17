//! Общее состояние приложения и рассылка его окнам.
//!
//! Службы (лимиты, музыка, Campus) кладут сюда свежие данные; окно получает
//! событие только если что-то действительно изменилось.

use std::sync::Mutex;

use kirk_core::campus::{Notification, Summary};
use kirk_core::geometry::Shape;
use kirk_core::limits::{Connection, Provider, Snapshot};
use kirk_core::music::NowPlaying;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::config::Config;

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct MusicState {
    /// Можно ли узнать, что играет, на этом компьютере.
    pub available: bool,
    pub note: Option<String>,
    pub now: Option<NowPlaying>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CampusState {
    pub connected: bool,
    pub site: Option<String>,
    pub name: Option<String>,
    pub summary: Option<Summary>,
    pub updated_at: Option<i64>,
    pub error: Option<String>,
    /// Непрочитанных уведомлений.
    pub unread: u32,
    /// Последние уведомления.
    pub notifications: Vec<Notification>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub version: String,
    pub platform: &'static str,
    pub limits: Vec<Snapshot>,
    pub music: MusicState,
    pub campus: CampusState,
    pub config: Config,
    pub shape: Option<Shape>,
    pub open: bool,
}

pub struct Hub {
    claude: Mutex<Snapshot>,
    codex: Mutex<Snapshot>,
    music: Mutex<MusicState>,
    campus: Mutex<CampusState>,
    pub config: Mutex<Config>,
    pub shape: Mutex<Option<Shape>>,
    pub open: Mutex<bool>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    // Паника в одной службе не должна ронять остальные.
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Hub {
    pub fn new(config: Config) -> Self {
        Hub {
            claude: Mutex::new(Snapshot::empty(Provider::Claude, Connection::Waiting, None)),
            codex: Mutex::new(Snapshot::empty(Provider::Codex, Connection::Waiting, None)),
            music: Mutex::new(MusicState::default()),
            campus: Mutex::new(CampusState::default()),
            config: Mutex::new(config),
            shape: Mutex::new(None),
            open: Mutex::new(false),
        }
    }

    pub fn config(&self) -> Config {
        lock(&self.config).clone()
    }

    pub fn limits(&self) -> Vec<Snapshot> {
        vec![lock(&self.claude).clone(), lock(&self.codex).clone()]
    }

    pub fn campus(&self) -> CampusState {
        lock(&self.campus).clone()
    }

    pub fn music(&self) -> MusicState {
        lock(&self.music).clone()
    }

    pub fn state(&self) -> State {
        State {
            version: env!("CARGO_PKG_VERSION").into(),
            platform: std::env::consts::OS,
            limits: self.limits(),
            music: self.music(),
            campus: self.campus(),
            config: self.config(),
            shape: *lock(&self.shape),
            open: *lock(&self.open),
        }
    }
}

/// Кладёт значение и рассылает событие, если оно изменилось.
fn replace<T: PartialEq + Clone>(slot: &Mutex<T>, value: T) -> bool {
    let mut cur = lock(slot);
    if *cur == value {
        return false;
    }
    *cur = value;
    true
}

pub fn set_limits(app: &AppHandle, snap: Snapshot) {
    let hub = app.state::<Hub>();
    let slot = match snap.provider {
        Provider::Claude => &hub.claude,
        Provider::Codex => &hub.codex,
    };
    if replace(slot, snap) {
        let _ = app.emit("kirk:limits", hub.limits());
    }
}

pub fn set_music(app: &AppHandle, music: MusicState) {
    let hub = app.state::<Hub>();
    if replace(&hub.music, music) {
        let _ = app.emit("kirk:music", hub.music());
    }
}

pub fn update_campus(app: &AppHandle, f: impl FnOnce(&mut CampusState)) {
    let hub = app.state::<Hub>();
    let mut next = hub.campus();
    f(&mut next);
    if replace(&hub.campus, next) {
        let _ = app.emit("kirk:campus", hub.campus());
    }
}

pub fn set_config(app: &AppHandle, config: Config) {
    let hub = app.state::<Hub>();
    if replace(&hub.config, config) {
        let _ = app.emit("kirk:config", hub.config());
    }
}
