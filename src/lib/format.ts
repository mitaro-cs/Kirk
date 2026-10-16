// Время и подписи по-русски.

const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;

const time = new Intl.DateTimeFormat('ru-RU', { hour: '2-digit', minute: '2-digit' });
const weekday = new Intl.DateTimeFormat('ru-RU', { weekday: 'short' });
const date = new Intl.DateTimeFormat('ru-RU', { day: 'numeric', month: 'short' });

export const clockTime = (ms: number) => time.format(ms);

function startOfDay(ms: number) {
  const d = new Date(ms);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

/** Коротко: «14:20», «завтра 09:00», «пт 23:59» (эта неделя), «18 окт.». */
export function when(ms: number, now: number) {
  const days = Math.round((startOfDay(ms) - startOfDay(now)) / DAY);
  if (days === 0) return clockTime(ms);
  if (days === 1) return `завтра ${clockTime(ms)}`;
  if (days > 1 && days < 7) return `${weekday.format(ms)} ${clockTime(ms)}`;
  return date.format(ms);
}

/** «через 5 мин», «через 2 ч 10 мин», «через 3 дн». */
export function until(ms: number, now: number) {
  const left = ms - now;
  if (left < MIN) return 'сейчас';
  if (left < HOUR) return `через ${Math.round(left / MIN)} мин`;
  if (left < DAY) {
    const h = Math.floor(left / HOUR);
    const m = Math.round((left % HOUR) / MIN);
    return m ? `через ${h} ч ${m} мин` : `через ${h} ч`;
  }
  return `через ${Math.round(left / DAY)} дн`;
}

/** Коротко для крыла острова: «12м», «2ч», «3д». */
export function short(ms: number, now: number) {
  const left = Math.max(0, ms - now);
  if (left < HOUR) return `${Math.max(1, Math.round(left / MIN))}м`;
  if (left < DAY) return `${Math.round(left / HOUR)}ч`;
  return `${Math.round(left / DAY)}д`;
}

/** «3:07». */
export function duration(sec: number) {
  const s = Math.max(0, Math.floor(sec));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}

export const percent = (fraction: number) => `${Math.round(fraction * 100)}%`;

const LESSON_KINDS: Record<string, string> = {
  lecture: 'Лекция',
  practice: 'Практика',
  seminar: 'Семинар',
  lab: 'Лабораторная',
  consult: 'Консультация',
  credit: 'Зачёт',
  exam: 'Экзамен',
  retake: 'Пересдача',
  other: 'Занятие',
};

const TASK_KINDS: Record<string, string> = {
  homework: 'ДЗ',
  lab: 'Лаба',
  test: 'Контрольная',
  credit: 'Зачёт',
  exam: 'Экзамен',
};

export const lessonKind = (k: string) => LESSON_KINDS[k] ?? 'Занятие';
export const taskKind = (k: string) => TASK_KINDS[k] ?? 'Задание';

export const PROVIDERS = { claude: 'Claude Code', codex: 'Codex' } as const;
