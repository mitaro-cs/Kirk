//! Что играет: общая модель для macOS (MediaRemote через mediaremote-adapter)
//! и Windows (System Media Transport Controls).

use serde::Serialize;
use serde_json::Value;

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct NowPlaying {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// Секунды.
    pub duration: Option<f64>,
    /// Позиция на момент `elapsed_at`, секунды.
    pub elapsed: Option<f64>,
    /// Когда снята позиция, мс Unix: интерфейс сам двигает её дальше.
    pub elapsed_at: Option<i64>,
    pub playing: bool,
    /// Обложка как data: URL.
    pub artwork: Option<String>,
    /// Какое приложение играет (bundle id / AUMID).
    pub app: Option<String>,
}

impl NowPlaying {
    pub fn same_track(&self, other: &NowPlaying) -> bool {
        self.title == other.title && self.artist == other.artist
    }
}

/// Команды плеера.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Command {
    Toggle,
    Next,
    Previous,
}

impl Command {
    /// Номер команды в `mediaremote-adapter send`.
    pub fn adapter_id(self) -> u8 {
        match self {
            Command::Toggle => 2,
            Command::Next => 4,
            Command::Previous => 5,
        }
    }
}

/// Одна строка `mediaremote-adapter stream --no-diff`:
/// `{"type":"data","diff":false,"payload":{...}}`. Пустой `payload` – ничего
/// не играет. `None` – строка не про данные, состояние не меняется.
///
/// Обложка приходит отдельной строкой после трека: если её нет, а трек тот
/// же, оставляем прежнюю – иначе она мигала бы.
pub fn parse_adapter_line(
    line: &str,
    previous: Option<&NowPlaying>,
    now_ms: i64,
) -> Option<Option<NowPlaying>> {
    let v: Value = serde_json::from_str(line).ok()?;
    if v.get("type").and_then(Value::as_str) != Some("data") {
        return None;
    }
    let p = v.get("payload")?;
    let text = |k: &str| {
        p.get(k)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .filter(|s| !s.is_empty())
    };
    let num = |k: &str| p.get(k).and_then(Value::as_f64);
    let Some(title) = text("title") else {
        return Some(None);
    };
    let mut item = NowPlaying {
        title,
        artist: text("artist"),
        album: text("album"),
        duration: num("duration"),
        elapsed: num("elapsedTime"),
        elapsed_at: Some(now_ms),
        playing: p.get("playing").and_then(Value::as_bool).unwrap_or(false),
        artwork: text("artworkData").map(|data| {
            let mime = text("artworkMimeType").unwrap_or_else(|| "image/jpeg".into());
            format!("data:{mime};base64,{data}")
        }),
        app: text("bundleIdentifier"),
    };
    if item.artwork.is_none() {
        if let Some(prev) = previous.filter(|p| p.same_track(&item)) {
            item.artwork = prev.artwork.clone();
        }
    }
    Some(Some(item))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stream_lines() {
        let line = r#"{"type":"data","diff":false,"payload":{"title":"Song","artist":"A",
            "duration":200.5,"elapsedTime":12,"playing":true,"artworkData":"AAA",
            "artworkMimeType":"image/png","bundleIdentifier":"com.spotify.client"}}"#;
        let item = parse_adapter_line(line, None, 7).unwrap().unwrap();
        assert_eq!(item.title, "Song");
        assert_eq!(item.elapsed, Some(12.0));
        assert_eq!(item.elapsed_at, Some(7));
        assert_eq!(item.artwork.as_deref(), Some("data:image/png;base64,AAA"));
        assert!(item.playing);

        let paused = r#"{"type":"data","diff":false,"payload":{"title":"Song","artist":"A","playing":false}}"#;
        let next = parse_adapter_line(paused, Some(&item), 8).unwrap().unwrap();
        assert_eq!(
            next.artwork, item.artwork,
            "обложка того же трека не пропадает"
        );

        let other = r#"{"type":"data","payload":{"title":"Other","artist":"B"}}"#;
        assert!(parse_adapter_line(other, Some(&item), 9)
            .unwrap()
            .unwrap()
            .artwork
            .is_none());

        assert_eq!(
            parse_adapter_line(r#"{"type":"data","payload":{}}"#, None, 0),
            Some(None)
        );
        assert_eq!(parse_adapter_line(r#"{"type":"log"}"#, None, 0), None);
        assert_eq!(parse_adapter_line("oops", None, 0), None);
    }
}
