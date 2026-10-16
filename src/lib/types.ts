// Формы данных из Rust (serde, camelCase).

export type Connection = 'fresh' | 'stale' | 'waiting' | 'signedOut' | 'notInstalled' | 'failed';
export type Pace = 'calm' | 'hot' | 'critical';

export interface QuotaWindow {
  id: string;
  label: string;
  durationMin: number | null;
  used: number;
  resetsAt: number | null;
  pace: Pace;
}

export interface Snapshot {
  provider: 'claude' | 'codex';
  connection: Connection;
  capturedAt: number | null;
  windows: QuotaWindow[];
  note: string | null;
}

export interface NowPlaying {
  title: string;
  artist: string | null;
  album: string | null;
  duration: number | null;
  elapsed: number | null;
  elapsedAt: number | null;
  playing: boolean;
  artwork: string | null;
  app: string | null;
}

export interface MusicState {
  available: boolean;
  note: string | null;
  now: NowPlaying | null;
}

export interface SubjectRef {
  id: number;
  name: string;
  color: string | null;
}

export interface Lesson {
  id: number;
  subject: SubjectRef | null;
  title: string;
  kind: string;
  startsAt: number;
  endsAt: number;
  place: string | null;
  teacher: string | null;
  cancelled: boolean;
}

export interface Homework {
  id: number;
  title: string;
  dueAt: number;
  kind: string;
  done: boolean;
  subject: SubjectRef | null;
  place: string | null;
}

export interface News {
  id: number;
  title: string | null;
  urgent: boolean;
  createdAt: number;
  subject: SubjectRef | null;
}

export interface Summary {
  current: Lesson | null;
  next: Lesson | null;
  leftToday: number;
  deadlines: Homework[];
  overdue: number;
  exams: Homework[];
  news: News[];
}

export interface CampusState {
  connected: boolean;
  site: string | null;
  name: string | null;
  summary: Summary | null;
  updatedAt: number | null;
  error: string | null;
}

export interface Config {
  limits: boolean;
  music: boolean;
  campus: boolean;
  claude: boolean;
  codex: boolean;
  openOnHover: boolean;
  hotkey: string;
}

export interface Shape {
  notch: { width: number; height: number } | null;
  compactW: number;
  compactH: number;
  openW: number;
  openH: number;
  shoulder: number;
}

export interface State {
  version: string;
  platform: string;
  limits: Snapshot[];
  music: MusicState;
  campus: CampusState;
  config: Config;
  shape: Shape | null;
  open: boolean;
}

export type StatusLine = { state: 'missing' } | { state: 'ours' } | { state: 'other'; command: string };
