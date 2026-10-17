//! Остров: размер и место окна, открытие наведением, щелчком и сочетанием.
//!
//! Окно всегда ровно по острову – прозрачное окно на весь верх экрана
//! перехватывало бы щелчки по вкладкам браузера под ним. Открытие: сначала
//! окно становится большим (прозрачно), затем интерфейс плавно растит
//! остров. Закрытие – наоборот: анимация, потом окно сжимается.
//!
//! Наведение ловим положением курсора раз в 100 мс: так одинаково на Mac и
//! Windows и не зависит от того, получает ли неактивное окно события мыши.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use kirk_core::geometry::{self, Rect, Screen, Shape};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::hub::Hub;

const DWELL_OPEN: Duration = Duration::from_millis(150);
const DWELL_CLOSE: Duration = Duration::from_millis(400);
/// Открыли сочетанием и не подвели курсор – закрыть через это время.
const PINNED_IDLE: Duration = Duration::from_secs(8);
/// Сколько длится анимация закрытия в интерфейсе (+ запас).
const CLOSE_ANIMATION: Duration = Duration::from_millis(320);
const TICK: Duration = Duration::from_millis(100);
const REMEASURE: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq)]
struct Monitor {
    x: i32,
    y: i32,
    scale: f64,
}

struct Inner {
    screen: Screen,
    monitor: Monitor,
    open: bool,
    /// Открыт щелчком или сочетанием: не закрывать, пока курсор не побывал внутри.
    pinned: bool,
    entered: bool,
    inside_since: Option<Instant>,
    outside_since: Option<Instant>,
    opened_at: Instant,
    /// Сейчас всплыло уведомление (остров закрыт, но шире).
    peeking: bool,
    generation: u64,
}

pub struct Island(Mutex<Option<Inner>>);

impl Island {
    pub fn new() -> Self {
        Island(Mutex::new(None))
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct IslandEvent {
    open: bool,
    shape: Shape,
}

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window("island")
}

/// Размеры экрана и чёлки. Вызывать в главном потоке (чёлка – AppKit).
fn measure(app: &AppHandle) -> Option<(Screen, Monitor)> {
    let mon = app.primary_monitor().ok().flatten()?;
    let scale = mon.scale_factor();
    let screen = Screen {
        width: mon.size().width as f64 / scale,
        notch: crate::platform::notch(),
    };
    let pos = mon.position();
    Some((
        screen,
        Monitor {
            x: pos.x,
            y: pos.y,
            scale,
        },
    ))
}

fn place(win: &WebviewWindow, m: Monitor, r: Rect) {
    let size = PhysicalSize::new(
        (r.w * m.scale).round() as u32,
        (r.h * m.scale).round() as u32,
    );
    let pos = PhysicalPosition::new(
        m.x + (r.x * m.scale).round() as i32,
        m.y + (r.y * m.scale).round() as i32,
    );
    let _ = win.set_size(size);
    let _ = win.set_position(pos);
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let win = window(app).expect("окно острова в tauri.conf.json");
    crate::platform::prepare_island(&win)?;
    if let Some((screen, monitor)) = measure(app) {
        apply_screen(app, screen, monitor);
    }
    win.show()?;
    start_hover(app.clone());
    Ok(())
}

fn apply_screen(app: &AppHandle, screen: Screen, monitor: Monitor) {
    let island = app.state::<Island>();
    let mut guard = island.0.lock().unwrap_or_else(|e| e.into_inner());
    let open = guard.as_ref().is_some_and(|i| i.open);
    let changed = guard
        .as_ref()
        .is_none_or(|i| i.screen != screen || i.monitor != monitor);
    if !changed {
        return;
    }
    let shape = geometry::shape(&screen);
    *app.state::<Hub>()
        .shape
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(shape);
    match guard.as_mut() {
        Some(i) => {
            i.screen = screen;
            i.monitor = monitor;
        }
        None => {
            *guard = Some(Inner {
                screen,
                monitor,
                open: false,
                pinned: false,
                entered: false,
                inside_since: None,
                outside_since: None,
                opened_at: Instant::now(),
                peeking: false,
                generation: 0,
            })
        }
    }
    if let Some(win) = window(app) {
        let rect = if open {
            geometry::open_rect(&screen)
        } else {
            geometry::compact_rect(&screen)
        };
        place(&win, monitor, rect);
    }
    let _ = app.emit("kirk:island", IslandEvent { open, shape });
}

/// Открыть или закрыть. `pinned` – открыт не наведением.
pub fn set_open(app: &AppHandle, open: bool, pinned: bool) {
    let island = app.state::<Island>();
    let mut guard = island.0.lock().unwrap_or_else(|e| e.into_inner());
    let Some(inner) = guard.as_mut() else { return };
    if inner.open == open {
        inner.pinned = inner.pinned || pinned;
        return;
    }
    inner.open = open;
    inner.pinned = open && pinned;
    inner.entered = false;
    inner.inside_since = None;
    inner.outside_since = None;
    inner.opened_at = Instant::now();
    inner.peeking = false;
    inner.generation += 1;
    let (screen, monitor, generation) = (inner.screen, inner.monitor, inner.generation);
    drop(guard);

    *app.state::<Hub>()
        .open
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = open;
    let shape = geometry::shape(&screen);
    let Some(win) = window(app) else { return };
    if open {
        place(&win, monitor, geometry::open_rect(&screen));
        let _ = app.emit("kirk:island", IslandEvent { open, shape });
        crate::campus_wake(app);
    } else {
        let _ = app.emit("kirk:island", IslandEvent { open, shape });
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(CLOSE_ANIMATION);
            let island = app.state::<Island>();
            let still = island
                .0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
                .is_some_and(|i| !i.open && i.generation == generation);
            if still {
                place(&win, monitor, geometry::compact_rect(&screen));
            }
        });
    }
}

pub fn toggle(app: &AppHandle) {
    let open = app
        .state::<Island>()
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_some_and(|i| i.open);
    set_open(app, !open, true);
}

/// Сколько висит всплывшее уведомление.
const PEEK_FOR: Duration = Duration::from_secs(6);

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Peek {
    pub title: String,
    pub body: String,
    /// Страница Campus, куда ведёт уведомление.
    pub url: Option<String>,
}

/// Коротко показать уведомление в закрытом острове. Открытый не трогаем.
pub fn peek(app: &AppHandle, title: String, body: String, url: Option<String>) {
    if !app.state::<Hub>().config().peek {
        return;
    }
    let island = app.state::<Island>();
    let mut guard = island.0.lock().unwrap_or_else(|e| e.into_inner());
    let Some(inner) = guard.as_mut() else { return };
    if inner.open {
        return;
    }
    inner.peeking = true;
    inner.generation += 1;
    let (screen, monitor, generation) = (inner.screen, inner.monitor, inner.generation);
    drop(guard);

    let Some(win) = window(app) else { return };
    place(&win, monitor, geometry::peek_rect(&screen));
    let _ = app.emit("kirk:peek", Some(Peek { title, body, url }));

    let app = app.clone();
    std::thread::spawn(move || {
        let ours = |app: &AppHandle| {
            app.state::<Island>()
                .0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
                .is_some_and(|i| !i.open && i.generation == generation)
        };
        std::thread::sleep(PEEK_FOR);
        if !ours(&app) {
            return;
        }
        let _ = app.emit("kirk:peek", None::<Peek>);
        std::thread::sleep(CLOSE_ANIMATION);
        if ours(&app) {
            if let Some(i) = app
                .state::<Island>()
                .0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_mut()
            {
                i.peeking = false;
            }
            place(&win, monitor, geometry::compact_rect(&screen));
        }
    });
}

/// Где сейчас остров (без плеч), в точках относительно монитора.
fn hit_rect(i: &Inner) -> Rect {
    let s = geometry::shape(&i.screen);
    let (w, h) = if i.open {
        (s.open_w, s.open_h)
    } else if i.peeking {
        (s.peek_w, s.peek_h)
    } else {
        (s.compact_w, s.compact_h)
    };
    Rect {
        x: (i.screen.width - w) / 2.0,
        y: 0.0,
        w,
        h,
    }
}

fn start_hover(app: AppHandle) {
    std::thread::Builder::new()
        .name("island".into())
        .spawn(move || {
            let mut last_measure = Instant::now();
            loop {
                std::thread::sleep(TICK);
                if last_measure.elapsed() >= REMEASURE {
                    last_measure = Instant::now();
                    let handle = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        if let Some((s, m)) = measure(&handle) {
                            apply_screen(&handle, s, m);
                        }
                    });
                }
                let Ok(cursor) = app.cursor_position() else {
                    continue;
                };
                if let Some(action) = hover_step(&app, cursor) {
                    set_open(&app, action, false);
                }
            }
        })
        .expect("поток острова");
}

/// Один шаг: вернуть Some(открыть/закрыть), если пора.
fn hover_step(app: &AppHandle, cursor: PhysicalPosition<f64>) -> Option<bool> {
    let hover = app.state::<Hub>().config().open_on_hover;
    let island = app.state::<Island>();
    let mut guard = island.0.lock().unwrap_or_else(|e| e.into_inner());
    let i = guard.as_mut()?;
    let m = i.monitor;
    let x = (cursor.x - m.x as f64) / m.scale;
    let y = (cursor.y - m.y as f64) / m.scale;
    let r = hit_rect(i);
    // Сверху – допуск: курсор, упёртый в край экрана, тоже «внутри».
    let margin = if i.open { 8.0 } else { 2.0 };
    let inside = x >= r.x - margin && x <= r.x + r.w + margin && y >= -4.0 && y <= r.h + margin;
    let now = Instant::now();
    if inside {
        i.outside_since = None;
        i.entered = true;
        if !i.open && hover {
            let since = *i.inside_since.get_or_insert(now);
            if now - since >= DWELL_OPEN {
                return Some(true);
            }
        }
    } else {
        i.inside_since = None;
        if i.open && (!i.pinned || i.entered) {
            let since = *i.outside_since.get_or_insert(now);
            if now - since >= DWELL_CLOSE {
                return Some(false);
            }
        } else if i.open && now - i.opened_at >= PINNED_IDLE {
            return Some(false);
        }
    }
    None
}
