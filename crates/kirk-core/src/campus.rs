//! Campus: ближайшая пара, дедлайны и новости группы из `GET /api/today`.
//!
//! Вход – кодом «Показать код» из Campus (Профиль → Безопасность):
//! `POST /api/auth/link/redeem {pin}` с CSRF из `/api/health`.
//! Время в API – мс Unix, UTC.

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubjectRef {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Lesson {
    pub id: i64,
    #[serde(default)]
    pub subject: Option<SubjectRef>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub kind: String,
    pub starts_at: i64,
    pub ends_at: i64,
    #[serde(default)]
    pub place: Option<String>,
    #[serde(default)]
    pub teacher: Option<String>,
    #[serde(default)]
    pub cancelled: bool,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Homework {
    pub id: i64,
    #[serde(default)]
    pub title: String,
    pub due_at: i64,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub subject: Option<SubjectRef>,
    #[serde(default)]
    pub place: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct News {
    pub id: i64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub urgent: bool,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub subject: Option<SubjectRef>,
}

#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Today {
    #[serde(default)]
    pub upcoming: Vec<Homework>,
    #[serde(default)]
    pub overdue: Vec<Homework>,
    #[serde(default)]
    pub pinned: Vec<News>,
    #[serde(default)]
    pub news: Vec<News>,
    #[serde(default)]
    pub exams: Vec<Homework>,
    #[serde(default)]
    pub lessons: Vec<Lesson>,
}

/// Ответ `/api/me` – нужно только имя.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Me {
    pub user: MeUser,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MeUser {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
}

impl Me {
    pub fn name(&self) -> String {
        self.user
            .display_name
            .clone()
            .filter(|n| !n.trim().is_empty())
            .or_else(|| self.user.username.clone())
            .unwrap_or_default()
    }
}

/// Что показывает остров.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// Пара идёт сейчас.
    pub current: Option<Lesson>,
    /// Следующая пара сегодня.
    pub next: Option<Lesson>,
    /// Сколько пар сегодня ещё впереди (не считая текущей).
    pub left_today: usize,
    /// Ближайшие невыполненные задания, по сроку.
    pub deadlines: Vec<Homework>,
    pub overdue: usize,
    /// Ближайшие зачёты и экзамены.
    pub exams: Vec<Homework>,
    /// Закреплённые и свежие новости.
    pub news: Vec<News>,
}

pub fn summarize(today: &Today, now_ms: i64) -> Summary {
    let mut lessons: Vec<&Lesson> = today.lessons.iter().filter(|l| !l.cancelled).collect();
    lessons.sort_by_key(|l| l.starts_at);
    let current = lessons
        .iter()
        .find(|l| l.starts_at <= now_ms && now_ms < l.ends_at)
        .map(|l| (*l).clone());
    let ahead: Vec<&&Lesson> = lessons.iter().filter(|l| l.starts_at > now_ms).collect();

    let mut deadlines: Vec<Homework> = today
        .upcoming
        .iter()
        .filter(|h| !h.done && h.due_at >= now_ms)
        .cloned()
        .collect();
    deadlines.sort_by_key(|h| h.due_at);
    deadlines.truncate(5);

    let mut exams: Vec<Homework> = today.exams.iter().filter(|h| h.due_at >= now_ms).cloned().collect();
    exams.sort_by_key(|h| h.due_at);
    exams.truncate(3);

    let mut news: Vec<News> = today.pinned.clone();
    for n in &today.news {
        if !news.iter().any(|m| m.id == n.id) {
            news.push(n.clone());
        }
    }
    news.truncate(3);

    Summary {
        current,
        next: ahead.first().map(|l| (**l).clone()),
        left_today: ahead.len(),
        deadlines,
        overdue: today.overdue.iter().filter(|h| !h.done).count(),
        exams,
        news,
    }
}

/// Адрес сайта: схема по умолчанию https, без хвостового «/».
pub fn normalize_url(input: &str) -> Option<String> {
    let s = input.trim();
    let (scheme, rest) = match s.split_once("://") {
        Some((scheme @ ("http" | "https"), rest)) => (scheme, rest),
        Some(_) => return None,
        None => ("https", s),
    };
    let rest = rest.trim_end_matches('/');
    let host = rest.split('/').next().unwrap_or("");
    if host.is_empty() || rest.contains(char::is_whitespace) {
        return None;
    }
    Some(format!("{scheme}://{rest}"))
}

/// Код входа: 6 цифр, пробелы и дефисы не мешают.
pub fn normalize_pin(input: &str) -> Option<String> {
    let digits: String = input.chars().filter(|c| c.is_ascii_digit()).collect();
    let rest = input.chars().all(|c| c.is_ascii_digit() || c == ' ' || c == '-');
    (digits.len() == 6 && rest).then_some(digits)
}

/// Значение cookie по окончанию имени (`gb_session`, `__Host-gb_session`)
/// из заголовков `Set-Cookie`. Возвращает (имя, значение).
pub fn cookie<'a>(set_cookies: impl IntoIterator<Item = &'a str>, suffix: &str) -> Option<(String, String)> {
    set_cookies.into_iter().find_map(|h| {
        let pair = h.split(';').next()?.trim();
        let (name, value) = pair.split_once('=')?;
        (name.ends_with(suffix) && !value.is_empty()).then(|| (name.to_owned(), value.to_owned()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: i64 = 3_600_000;

    fn lesson(id: i64, start: i64, end: i64) -> Lesson {
        Lesson {
            id,
            subject: None,
            title: format!("Пара {id}"),
            kind: "lecture".into(),
            starts_at: start,
            ends_at: end,
            place: Some("А-101".into()),
            teacher: None,
            cancelled: false,
        }
    }

    fn hw(id: i64, due: i64, done: bool) -> Homework {
        Homework { id, title: format!("ДЗ {id}"), due_at: due, kind: "homework".into(), done, subject: None, place: None }
    }

    #[test]
    fn parses_today_from_campus() {
        let json = r##"{"upcoming":[{"id":1,"title":"Лаба","dueAt":10,"kind":"lab","done":false,
            "subject":{"id":3,"name":"Сети","color":"#08f"},"bodyMd":"x","groups":[],"can":{}}],
            "overdue":[],"pinned":[],"news":[{"id":5,"title":null,"urgent":true,"createdAt":1}],
            "exams":[],"lessons":[{"id":9,"groupId":1,"subject":null,"title":"Сети","kind":"lab",
            "startsAt":1,"endsAt":2,"place":null,"teacher":"Иванов","note":null,"homework":0,
            "materials":0,"cancelled":false,"can":{}}]}"##;
        let t: Today = serde_json::from_str(json).unwrap();
        assert_eq!(t.upcoming[0].subject.as_ref().unwrap().name, "Сети");
        assert_eq!(t.lessons[0].teacher.as_deref(), Some("Иванов"));
        assert!(t.news[0].urgent);
    }

    #[test]
    fn current_and_next_lesson() {
        let mut cancelled = lesson(4, 5 * H, 6 * H);
        cancelled.cancelled = true;
        let t = Today {
            lessons: vec![lesson(3, 4 * H, 5 * H), lesson(1, H, 2 * H), lesson(2, 2 * H + 1, 3 * H), cancelled],
            ..Default::default()
        };
        let s = summarize(&t, H + 10);
        assert_eq!(s.current.unwrap().id, 1);
        assert_eq!(s.next.unwrap().id, 2);
        assert_eq!(s.left_today, 2);
        let s = summarize(&t, 6 * H);
        assert!(s.current.is_none() && s.next.is_none());
    }

    #[test]
    fn deadlines_sorted_without_done() {
        let t = Today {
            upcoming: vec![hw(1, 30, false), hw(2, 10, true), hw(3, 20, false), hw(4, 1, false)],
            overdue: vec![hw(5, 0, false), hw(6, 0, true)],
            ..Default::default()
        };
        let s = summarize(&t, 5);
        assert_eq!(s.deadlines.iter().map(|h| h.id).collect::<Vec<_>>(), vec![3, 1]);
        assert_eq!(s.overdue, 1);
    }

    #[test]
    fn urls_and_pins() {
        assert_eq!(normalize_url("campus.example.ru/").as_deref(), Some("https://campus.example.ru"));
        assert_eq!(normalize_url("http://192.168.1.5:8080").as_deref(), Some("http://192.168.1.5:8080"));
        assert_eq!(normalize_url("  "), None);
        assert_eq!(normalize_url("https://"), None);
        assert_eq!(normalize_pin("123 456").as_deref(), Some("123456"));
        assert_eq!(normalize_pin("12345"), None);
        assert_eq!(normalize_pin("12a456"), None);
    }

    #[test]
    fn finds_cookies_with_host_prefix() {
        let headers = [
            "__Host-gb_csrf=abc; Path=/; Secure; SameSite=Strict",
            "__Host-gb_session=tok; Path=/; HttpOnly; Secure",
        ];
        assert_eq!(
            cookie(headers, "gb_session"),
            Some(("__Host-gb_session".into(), "tok".into()))
        );
        assert_eq!(cookie(["gb_csrf=; Max-Age=0"], "gb_csrf"), None);
    }
}
