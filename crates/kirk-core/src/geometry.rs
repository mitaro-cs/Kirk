//! Размеры и место острова. Всё в логических точках относительно монитора.
//!
//! На Mac с чёлкой остров обнимает чёлку: по бокам «крылья» для значков,
//! высота – как у строки меню. Без чёлки (Windows, внешний монитор) – та же
//! форма по центру, чуть ниже и уже. Окно шире острова на «плечи» – вогнутые
//! скругления, которыми остров переходит в верхний край экрана.

use serde::Serialize;

/// Ширина крыла по бокам от чёлки.
pub const WING: f64 = 76.0;
/// Плечо с каждой стороны (рисуется внутри окна).
pub const SHOULDER: f64 = 10.0;
/// Высота острова без чёлки.
pub const PLAIN_HEIGHT: f64 = 30.0;
/// Ширина острова без чёлки.
pub const PLAIN_WIDTH: f64 = 236.0;
/// Открытый остров.
pub const OPEN_WIDTH: f64 = 600.0;
pub const OPEN_HEIGHT: f64 = 250.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notch {
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Screen {
    /// Ширина монитора, точки.
    pub width: f64,
    pub notch: Option<Notch>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Что знает интерфейс о форме острова.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shape {
    pub notch: Option<Notch>,
    /// Закрытый остров (без плеч).
    pub compact_w: f64,
    pub compact_h: f64,
    /// Открытый остров (без плеч).
    pub open_w: f64,
    pub open_h: f64,
    pub shoulder: f64,
}

pub fn shape(screen: &Screen) -> Shape {
    let (compact_w, compact_h) = match screen.notch {
        Some(n) => (n.width + 2.0 * WING, n.height),
        None => (PLAIN_WIDTH, PLAIN_HEIGHT),
    };
    let max_w = (screen.width - 2.0 * SHOULDER - 24.0).max(compact_w);
    Shape {
        notch: screen.notch,
        compact_w,
        compact_h,
        open_w: OPEN_WIDTH.max(compact_w + 120.0).min(max_w),
        open_h: OPEN_HEIGHT + compact_h,
        shoulder: SHOULDER,
    }
}

/// Окно закрытого острова: остров + плечи, по центру сверху.
pub fn compact_rect(screen: &Screen) -> Rect {
    let s = shape(screen);
    centered(screen, s.compact_w + 2.0 * s.shoulder, s.compact_h)
}

/// Окно открытого острова. Снизу запас под тень.
pub fn open_rect(screen: &Screen) -> Rect {
    let s = shape(screen);
    centered(screen, s.open_w + 2.0 * s.shoulder, s.open_h + 24.0)
}

fn centered(screen: &Screen, w: f64, h: f64) -> Rect {
    Rect { x: ((screen.width - w) / 2.0).round(), y: 0.0, w: w.round(), h: h.round() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notch_island_hugs_the_notch() {
        let mbp = Screen { width: 1512.0, notch: Some(Notch { width: 185.0, height: 32.0 }) };
        let r = compact_rect(&mbp);
        assert_eq!(r.w, 185.0 + 2.0 * WING + 2.0 * SHOULDER);
        assert_eq!(r.h, 32.0);
        assert_eq!(r.x, ((1512.0 - r.w) / 2.0).round());
        assert_eq!(r.y, 0.0);
    }

    #[test]
    fn plain_screen_gets_a_pill() {
        let win = Screen { width: 1920.0, notch: None };
        assert_eq!(compact_rect(&win).h, PLAIN_HEIGHT);
        let open = open_rect(&win);
        assert_eq!(open.w, OPEN_WIDTH + 2.0 * SHOULDER);
    }

    #[test]
    fn open_island_fits_small_screens() {
        let tiny = Screen { width: 500.0, notch: None };
        assert!(open_rect(&tiny).w <= 500.0);
    }
}
