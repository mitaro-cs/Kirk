// Состояние приложения в окне: начальный снимок из Rust и события по изменениям.

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { CampusState, Config, MusicState, Peek, Shape, Snapshot, State } from './types';

export const app = $state<{ ready: boolean; s: State | null; peek: Peek | null }>({
  ready: false,
  s: null,
  peek: null,
});

/** Текущее время: тикает раз в секунду, пока остров открыт, иначе раз в 30 с. */
export const clock = $state({ now: Date.now() });

let timer: ReturnType<typeof setInterval> | undefined;
function tick(fast: boolean) {
  clearInterval(timer);
  clock.now = Date.now();
  timer = setInterval(() => (clock.now = Date.now()), fast ? 1000 : 30_000);
}

export async function start() {
  app.s = await invoke<State>('get_state');
  app.ready = true;
  tick(app.s.open);
  await Promise.all([
    listen<Snapshot[]>('kirk:limits', (e) => app.s && (app.s.limits = e.payload)),
    listen<MusicState>('kirk:music', (e) => app.s && (app.s.music = e.payload)),
    listen<CampusState>('kirk:campus', (e) => app.s && (app.s.campus = e.payload)),
    listen<Config>('kirk:config', (e) => app.s && (app.s.config = e.payload)),
    listen<Peek | null>('kirk:peek', (e) => (app.peek = e.payload)),
    listen<{ open: boolean; shape: Shape }>('kirk:island', (e) => {
      if (!app.s) return;
      app.s.open = e.payload.open;
      if (e.payload.open) app.peek = null;
      app.s.shape = e.payload.shape;
      tick(e.payload.open);
    }),
  ]);
}
