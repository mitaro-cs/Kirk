//! Лимиты ИИ-сервисов: окна квот, их подписи и «темп» расхода.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Claude,
    Codex,
}

/// Что известно о связи с сервисом.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Connection {
    /// Данные свежие.
    Fresh,
    /// Данные есть, но давно не обновлялись.
    Stale,
    /// Ждём первых данных (Claude Code пришлёт после следующего ответа).
    Waiting,
    /// Сервис установлен, но вход не выполнен.
    SignedOut,
    /// Сервис не найден на компьютере.
    NotInstalled,
    /// Ошибка, подробности – в `note`.
    Failed,
}

/// Насколько быстро тратится окно относительно прошедшего времени.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Pace {
    /// Тратится не быстрее времени.
    Calm,
    /// Тратится заметно быстрее – к сбросу может не хватить.
    Hot,
    /// Осталось меньше 10 %.
    Critical,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    pub id: String,
    pub label: String,
    pub duration_min: Option<u32>,
    /// Израсходовано, 0…1.
    pub used: f64,
    /// Когда окно обнулится, мс Unix.
    pub resets_at: Option<i64>,
    pub pace: Pace,
}

impl QuotaWindow {
    pub fn new(
        id: impl Into<String>,
        duration_min: Option<u32>,
        used: f64,
        resets_at: Option<i64>,
        now_ms: i64,
    ) -> Self {
        let used = if used.is_finite() { used.clamp(0.0, 1.0) } else { 0.0 };
        QuotaWindow {
            id: id.into(),
            label: label(duration_min),
            duration_min,
            used,
            resets_at,
            pace: pace(used, duration_min, resets_at, now_ms),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub provider: Provider,
    pub connection: Connection,
    /// Когда сняты данные, мс Unix.
    pub captured_at: Option<i64>,
    pub windows: Vec<QuotaWindow>,
    /// Пояснение для человека, по-русски.
    pub note: Option<String>,
}

impl Snapshot {
    pub fn empty(provider: Provider, connection: Connection, note: Option<&str>) -> Self {
        Snapshot {
            provider,
            connection,
            captured_at: None,
            windows: Vec::new(),
            note: note.map(str::to_owned),
        }
    }
}

/// Подпись окна по длительности: «5 ч», «Неделя», «День», «30 мин».
pub fn label(duration_min: Option<u32>) -> String {
    match duration_min {
        None | Some(0) => "Лимит".into(),
        Some(10_080) => "Неделя".into(),
        Some(1_440) => "День".into(),
        Some(m) if m < 60 => format!("{m} мин"),
        Some(m) if m % 1_440 == 0 => format!("{} дн", m / 1_440),
        Some(m) if m % 60 == 0 => format!("{} ч", m / 60),
        Some(m) => format!("{m} мин"),
    }
}

/// Темп: сравниваем долю израсходованного с долей прошедшего времени окна.
pub fn pace(used: f64, duration_min: Option<u32>, resets_at: Option<i64>, now_ms: i64) -> Pace {
    if used >= 0.9 {
        return Pace::Critical;
    }
    let (Some(minutes), Some(resets)) = (duration_min, resets_at) else {
        return Pace::Calm;
    };
    let total = minutes as f64 * 60_000.0;
    if total <= 0.0 {
        return Pace::Calm;
    }
    let left = ((resets - now_ms) as f64).clamp(0.0, total);
    let elapsed = 1.0 - left / total;
    if used > elapsed + 0.15 {
        Pace::Hot
    } else {
        Pace::Calm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(label(Some(300)), "5 ч");
        assert_eq!(label(Some(10_080)), "Неделя");
        assert_eq!(label(Some(1_440)), "День");
        assert_eq!(label(Some(2_880)), "2 дн");
        assert_eq!(label(Some(45)), "45 мин");
        assert_eq!(label(Some(90)), "90 мин");
        assert_eq!(label(None), "Лимит");
    }

    #[test]
    fn pace_compares_spend_with_time() {
        let hour = 60 * 60_000;
        // Окно 5 ч, прошёл 1 ч (осталось 4) – 20 % времени.
        let resets = 4 * hour;
        assert_eq!(pace(0.2, Some(300), Some(resets), 0), Pace::Calm);
        assert_eq!(pace(0.5, Some(300), Some(resets), 0), Pace::Hot);
        assert_eq!(pace(0.95, Some(300), Some(resets), 0), Pace::Critical);
        assert_eq!(pace(0.5, None, None, 0), Pace::Calm);
    }

    #[test]
    fn window_clamps_used() {
        let w = QuotaWindow::new("x", Some(300), 1.7, None, 0);
        assert_eq!(w.used, 1.0);
        let w = QuotaWindow::new("x", Some(300), f64::NAN, None, 0);
        assert_eq!(w.used, 0.0);
    }
}
