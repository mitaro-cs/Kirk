//! macOS: что играет.
//!
//! 1. mediaremote-adapter (если собран в приложение, см. `scripts/prepare.mjs`):
//!    `/usr/bin/perl mediaremote-adapter.pl <framework> stream` – любой плеер
//!    из «Пункта управления», с обложкой.
//! 2. Иначе – распределённые уведомления Music и Spotify (без опроса,
//!    без разрешений): название, исполнитель, пауза. Без обложки.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command as Proc, Stdio};
use std::ptr::NonNull;
use std::sync::Mutex;
use std::time::Duration;

use block2::RcBlock;
use kirk_core::music::{self, Command, NowPlaying};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSDictionary, NSDistributedNotificationCenter, NSNotification, NSNumber, NSOperationQueue, NSString};
use tauri::{AppHandle, Manager};

use crate::hub::{self, MusicState};

const PERL: &str = "/usr/bin/perl";

struct Adapter {
    script: PathBuf,
    framework: PathBuf,
}

impl Adapter {
    fn find(app: &AppHandle) -> Option<Adapter> {
        let script = app.path().resource_dir().ok()?.join("mediaremote-adapter.pl");
        let exe = std::env::current_exe().ok()?;
        let framework = exe.parent()?.parent()?.join("Frameworks/MediaRemoteAdapter.framework");
        (script.is_file() && framework.is_dir()).then_some(Adapter { script, framework })
    }

    fn command(&self, args: &[&str]) -> Proc {
        let mut c = Proc::new(PERL);
        c.arg(&self.script).arg(&self.framework).args(args);
        c
    }
}

/// Какое приложение последним сообщило о треке (для запасного пути).
static PLAYER: Mutex<Option<&'static str>> = Mutex::new(None);

pub fn start(app: AppHandle) {
    match Adapter::find(&app) {
        Some(adapter) => {
            std::thread::Builder::new()
                .name("music".into())
                .spawn(move || stream(app, adapter))
                .expect("поток музыки");
        }
        None => notifications(app),
    }
}

fn stream(app: AppHandle, adapter: Adapter) {
    let mut backoff = Duration::from_secs(2);
    loop {
        let child = adapter
            .command(&["stream", "--no-diff", "--debounce=100"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(e) => {
                hub::set_music(&app, MusicState { available: false, note: Some(format!("Адаптер музыки: {e}")), now: None });
                return;
            }
        };
        hub::set_music(&app, MusicState { available: true, note: None, now: None });
        let mut current: Option<NowPlaying> = None;
        if let Some(out) = child.stdout.take() {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                if let Some(next) = music::parse_adapter_line(&line, current.as_ref(), kirk_core::now_ms()) {
                    current = next;
                    hub::set_music(&app, MusicState { available: true, note: None, now: current.clone() });
                    backoff = Duration::from_secs(2);
                }
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        std::thread::sleep(backoff);
        backoff = (backoff * 2).min(Duration::from_secs(60));
    }
}

fn text(info: &NSDictionary<AnyObject, AnyObject>, key: &str) -> Option<String> {
    let value = info.objectForKey(&NSString::from_str(key))?;
    let s = value.downcast::<NSString>().ok()?.to_string();
    (!s.is_empty()).then_some(s)
}

fn number(info: &NSDictionary<AnyObject, AnyObject>, key: &str) -> Option<f64> {
    let value = info.objectForKey(&NSString::from_str(key))?;
    Some(value.downcast::<NSNumber>().ok()?.as_f64())
}

/// userInfo уведомления Music/Spotify → трек. «Total Time» и «Duration» – мс.
fn parse(info: &NSDictionary<AnyObject, AnyObject>, player: &'static str) -> Option<NowPlaying> {
    let state = text(info, "Player State").unwrap_or_default();
    if state == "Stopped" {
        return None;
    }
    let title = text(info, "Name")?;
    let ms = number(info, "Total Time").or_else(|| number(info, "Duration"));
    Some(NowPlaying {
        title,
        artist: text(info, "Artist"),
        album: text(info, "Album"),
        duration: ms.map(|m| m / 1000.0).filter(|d| *d > 0.0),
        elapsed: number(info, "Playback Position"),
        elapsed_at: Some(kirk_core::now_ms()),
        playing: state == "Playing",
        artwork: None,
        app: Some(player.into()),
    })
}

fn notifications(app: AppHandle) {
    hub::set_music(&app, MusicState { available: true, note: Some("Music и Spotify".into()), now: None });
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let center = NSDistributedNotificationCenter::defaultCenter();
        let queue = NSOperationQueue::mainQueue();
        for (name, player) in [("com.apple.Music.playerInfo", "Music"), ("com.spotify.client.PlaybackStateChanged", "Spotify")] {
            let app = handle.clone();
            let block = RcBlock::new(move |n: NonNull<NSNotification>| {
                let n = unsafe { n.as_ref() };
                let info: Option<Retained<NSDictionary<AnyObject, AnyObject>>> = n.userInfo();
                let now = info.as_deref().and_then(|i| parse(i, player));
                if now.is_some() {
                    *PLAYER.lock().unwrap_or_else(|e| e.into_inner()) = Some(player);
                }
                hub::set_music(&app, MusicState { available: true, note: None, now });
            });
            let token = unsafe {
                center.addObserverForName_object_queue_usingBlock(Some(&NSString::from_str(name)), None, Some(&queue), &block)
            };
            // Наблюдатель живёт всё время работы приложения.
            std::mem::forget(token);
        }
    });
}

pub fn send(app: &AppHandle, command: Command) -> Result<(), String> {
    if let Some(adapter) = Adapter::find(app) {
        let id = command.adapter_id().to_string();
        let status = adapter.command(&["send", &id]).stdout(Stdio::null()).stderr(Stdio::null()).status();
        return status.map(|_| ()).map_err(|e| e.to_string());
    }
    let player = PLAYER.lock().unwrap_or_else(|e| e.into_inner()).ok_or("ничего не играет")?;
    let verb = match command {
        Command::Toggle => "playpause",
        Command::Next => "next track",
        Command::Previous => "previous track",
    };
    Proc::new("/usr/bin/osascript")
        .args(["-e", &format!("tell application \"{player}\" to {verb}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
