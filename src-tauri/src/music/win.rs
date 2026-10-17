//! Windows: GlobalSystemMediaTransportControlsSessionManager.
//!
//! События сессии лишь будят поток; он заново читает состояние целиком –
//! так не бывает рассинхрона между частями (трек, пауза, позиция).

use std::sync::mpsc;
use std::sync::OnceLock;
use std::time::Duration;

use base64::Engine;
use kirk_core::music::{Command, NowPlaying};
use tauri::AppHandle;
use windows::core::HSTRING;
use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Storage::Streams::DataReader;

use crate::hub::{self, MusicState};

static MANAGER: OnceLock<Manager> = OnceLock::new();

pub fn start(app: AppHandle) {
    std::thread::Builder::new()
        .name("music".into())
        .spawn(move || run(app))
        .expect("поток музыки");
}

fn run(app: AppHandle) {
    let manager = match Manager::RequestAsync().and_then(|op| op.join()) {
        Ok(m) => m,
        Err(e) => {
            hub::set_music(
                &app,
                MusicState {
                    available: false,
                    note: Some(format!("Медиаэлементы Windows недоступны: {e}")),
                    now: None,
                },
            );
            return;
        }
    };
    let _ = MANAGER.set(manager.clone());
    let (tx, rx) = mpsc::channel::<()>();

    let wake = tx.clone();
    let _ = manager.CurrentSessionChanged(&TypedEventHandler::new(move |_, _| {
        let _ = wake.send(());
        Ok(())
    }));

    let mut subscribed: Option<HSTRING> = None;
    let mut artwork: Option<(String, Option<String>)> = None; // (ключ трека, data URL)
    loop {
        let session = manager.GetCurrentSession().ok();
        let id = session.as_ref().and_then(|s| s.SourceAppUserModelId().ok());
        if id != subscribed {
            if let Some(s) = &session {
                subscribe(s, &tx);
            }
            subscribed = id;
        }
        let now = session
            .as_ref()
            .and_then(|s| read(s, &mut artwork).ok())
            .flatten();
        hub::set_music(
            &app,
            MusicState {
                available: true,
                note: None,
                now,
            },
        );

        // Ждём события; позицию раз в 5 с перечитываем и без него.
        let _ = rx.recv_timeout(Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(80));
        while rx.try_recv().is_ok() {}
    }
}

fn subscribe(session: &Session, tx: &mpsc::Sender<()>) {
    let a = tx.clone();
    let _ = session.MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| {
        let _ = a.send(());
        Ok(())
    }));
    let b = tx.clone();
    let _ = session.PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| {
        let _ = b.send(());
        Ok(())
    }));
    let c = tx.clone();
    let _ = session.TimelinePropertiesChanged(&TypedEventHandler::new(move |_, _| {
        let _ = c.send(());
        Ok(())
    }));
}

fn read(
    session: &Session,
    artwork: &mut Option<(String, Option<String>)>,
) -> windows::core::Result<Option<NowPlaying>> {
    let props = session.TryGetMediaPropertiesAsync()?.join()?;
    let title = props.Title()?.to_string();
    if title.is_empty() {
        return Ok(None);
    }
    let artist = Some(props.Artist()?.to_string()).filter(|s| !s.is_empty());
    let album = Some(props.AlbumTitle()?.to_string()).filter(|s| !s.is_empty());
    let playing = session.GetPlaybackInfo()?.PlaybackStatus()? == Status::Playing;
    let timeline = session.GetTimelineProperties()?;
    // TimeSpan – сотни наносекунд.
    let ticks = |t: windows::Foundation::TimeSpan| t.Duration as f64 / 10_000_000.0;
    let duration =
        Some(ticks(timeline.EndTime()?) - ticks(timeline.StartTime()?)).filter(|d| *d > 0.0);
    let elapsed = Some(ticks(timeline.Position()?));
    let updated = timeline.LastUpdatedTime()?.UniversalTime; // 100 нс с 1601 года
    let elapsed_at = Some((updated - 116_444_736_000_000_000) / 10_000).filter(|t| *t > 0);

    let key = format!("{title}\u{1}{}", artist.clone().unwrap_or_default());
    let art = match artwork {
        Some((k, art)) if *k == key => art.clone(),
        _ => {
            let art = thumbnail(&props).ok().flatten();
            *artwork = Some((key, art.clone()));
            art
        }
    };

    Ok(Some(NowPlaying {
        title,
        artist,
        album,
        duration,
        elapsed,
        elapsed_at: elapsed_at.or(Some(kirk_core::now_ms())),
        playing,
        artwork: art,
        app: session.SourceAppUserModelId().ok().map(|s| s.to_string()),
    }))
}

fn thumbnail(
    props: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties,
) -> windows::core::Result<Option<String>> {
    let Ok(reference) = props.Thumbnail() else {
        return Ok(None);
    };
    let stream = reference.OpenReadAsync()?.join()?;
    let size = stream.Size()? as u32;
    if size == 0 || size > 4_000_000 {
        return Ok(None);
    }
    let reader = DataReader::CreateDataReader(&stream)?;
    reader.LoadAsync(size)?.join()?;
    let mut buf = vec![0u8; size as usize];
    reader.ReadBytes(&mut buf)?;
    let mime = stream
        .ContentType()
        .map(|s| s.to_string())
        .unwrap_or_default();
    let mime = if mime.starts_with("image/") {
        mime
    } else {
        "image/jpeg".into()
    };
    Ok(Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(buf)
    )))
}

pub fn send(_app: &AppHandle, command: Command) -> Result<(), String> {
    let manager = MANAGER.get().ok_or("музыка не запущена")?;
    let session = manager
        .GetCurrentSession()
        .map_err(|_| "ничего не играет".to_string())?;
    let op = match command {
        Command::Toggle => session.TryTogglePlayPauseAsync(),
        Command::Next => session.TrySkipNextAsync(),
        Command::Previous => session.TrySkipPreviousAsync(),
    };
    op.and_then(|o| o.join())
        .map(|_| ())
        .map_err(|e| e.to_string())
}
