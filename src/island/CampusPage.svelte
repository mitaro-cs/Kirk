<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import type { CampusState, Lesson } from '../lib/types';
  import { clockTime, lessonKind, taskKind, until, when } from '../lib/format';
  import { clock } from '../lib/store.svelte';
  import Icon from '../lib/Icon.svelte';

  let { campus, onsettings }: { campus: CampusState; onsettings: () => void } = $props();

  let s = $derived(campus.summary);
  let lesson = $derived<Lesson | null>(s?.current ?? s?.next ?? null);
  let now = $derived(s?.current != null && lesson?.id === s.current.id);
  let error = $state('');
  // Отмеченные – прячем сразу, пока сайт подтверждает.
  let pending = $state<number[]>([]);

  const go = (path: string) => invoke('campus_open', { path }).catch((e) => (error = String(e)));

  async function done(id: number) {
    error = '';
    pending = [...pending, id];
    try {
      await invoke('campus_done', { id, value: true });
    } catch (e) {
      error = String(e);
    } finally {
      pending = pending.filter((p) => p !== id);
    }
  }

  let tasks = $derived(
    s ? [...s.exams.slice(0, 1), ...s.deadlines].filter((h) => !pending.includes(h.id)).slice(0, 4) : [],
  );
  let passed = $derived(
    now && lesson ? Math.min(1, (clock.now - lesson.startsAt) / (lesson.endsAt - lesson.startsAt)) : 0,
  );
</script>

{#if !campus.connected}
  <div class="empty">
    <p>{campus.error ?? 'Подключите Campus – пары, дедлайны и новости группы будут здесь.'}</p>
    <button class="pill" onclick={onsettings}>Подключить</button>
  </div>
{:else if !s}
  <p class="empty">{campus.error ?? 'Загружаем…'}</p>
{:else}
  <div class="campus">
    <section class="lesson card">
      {#if lesson}
        <button class="hit" aria-label="Открыть пару на сайте" onclick={() => go(`/schedule/${lesson!.id}`)}
        ></button>
        <small
          >{now
            ? `Сейчас · ещё ${until(lesson.endsAt, clock.now).replace('через ', '')}`
            : `Следующая ${until(lesson.startsAt, clock.now)}`}</small
        >
        <h3 style="--c:{lesson.subject?.color ?? 'var(--calm)'}">{lesson.subject?.name ?? lesson.title}</h3>
        <p class="meta">
          {lessonKind(lesson.kind)} · {clockTime(lesson.startsAt)}–{clockTime(lesson.endsAt)}{lesson.place
            ? ` · ${lesson.place}`
            : ''}
        </p>
        {#if lesson.teacher}<p class="meta">{lesson.teacher}</p>{/if}
        {#if now}<div class="bar"><i style="transform: scaleX({passed})"></i></div>{/if}
        {#if s.leftToday > (now ? 0 : 1)}
          <p class="more">Ещё пар сегодня: {now ? s.leftToday : s.leftToday - 1}</p>
        {/if}
      {:else}
        <small>Расписание</small>
        <h3>Пар сегодня больше нет</h3>
      {/if}
    </section>

    <section class="tasks card">
      <header>
        <small>Дедлайны</small>
        {#if s.overdue}<span class="overdue">просрочено: {s.overdue}</span>{/if}
      </header>
      {#if tasks.length}
        <ul>
          {#each tasks as h (h.id)}
            <li>
              <button
                class="check"
                style="--c:{h.subject?.color ?? 'var(--calm)'}"
                title="Отметить сделанным"
                aria-label="Отметить «{h.title}» сделанным"
                onclick={() => done(h.id)}
              ></button>
              <button class="title" onclick={() => go(`/homework/${h.id}`)} title={h.title}>{h.title}</button>
              <span class="due" class:soon={h.dueAt - clock.now < 86_400_000}>{when(h.dueAt, clock.now)}</span
              >
              <span class="kind">{taskKind(h.kind)}{h.subject ? ` · ${h.subject.name}` : ''}</span>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="meta">Ничего не горит</p>
      {/if}
    </section>
  </div>
  <footer>
    <button class="open" onclick={() => go('/')}>
      {campus.name ?? 'Campus'}
      <Icon name="open" size={12} />
    </button>
    {#if error}
      <span class="error">{error}</span>
    {:else if campus.unread}
      <button class="open bell" onclick={() => go('/notifications')}
        >Новых уведомлений: {campus.unread}</button
      >
      <button class="open" onclick={() => invoke('campus_read_all')}>Прочитать</button>
    {/if}
  </footer>
{/if}

<style>
  .empty {
    margin: 0;
    height: 100%;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 10px;
    text-align: center;
    color: rgb(255 255 255 / 0.6);
    font-size: 13px;
    padding: 0 40px;
  }
  .empty p {
    margin: 0;
  }
  .pill {
    border: 0;
    background: #fff;
    color: #000;
    border-radius: 999px;
    padding: 6px 16px;
    font-size: 12px;
    font-weight: 600;
  }
  .campus {
    display: grid;
    grid-template-columns: 1fr 1.25fr;
    gap: 10px;
    height: calc(100% - 22px);
  }
  .card {
    background: rgb(255 255 255 / 0.06);
    border-radius: 16px;
    padding: 10px 12px;
    min-width: 0;
    overflow: hidden;
  }
  small {
    font-size: 11px;
    color: rgb(255 255 255 / 0.5);
  }
  h3 {
    margin: 4px 0 2px;
    font-size: 16px;
    font-weight: 650;
    line-height: 1.2;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    border-left: 3px solid var(--c, transparent);
    padding-left: 7px;
  }
  .meta,
  .more {
    margin: 2px 0 0;
    font-size: 12px;
    color: rgb(255 255 255 / 0.65);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .more {
    color: rgb(255 255 255 / 0.45);
  }
  .bar {
    margin-top: 8px;
    height: 4px;
    border-radius: 2px;
    background: rgb(255 255 255 / 0.12);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--ok);
    transform-origin: left;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .overdue {
    font-size: 11px;
    color: var(--critical);
  }
  ul {
    list-style: none;
    margin: 6px 0 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }
  .lesson {
    position: relative;
  }
  .hit {
    position: absolute;
    inset: 0;
    border: 0;
    background: transparent;
    border-radius: 16px;
  }
  .hit:hover {
    background: rgb(255 255 255 / 0.04);
  }
  li {
    display: grid;
    grid-template-columns: 14px 1fr auto;
    column-gap: 7px;
    align-items: center;
    font-size: 12.5px;
  }
  .check {
    width: 14px;
    height: 14px;
    padding: 0;
    border-radius: 50%;
    border: 1.6px solid var(--c);
    background: transparent;
    transition: background 150ms;
  }
  .check:hover {
    background: var(--c);
  }
  .title {
    border: 0;
    padding: 0;
    background: none;
    text-align: left;
    font-size: inherit;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .title:hover {
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .due {
    font-size: 11px;
    color: rgb(255 255 255 / 0.55);
  }
  .due.soon {
    color: var(--hot);
  }
  .kind {
    grid-column: 2 / 4;
    font-size: 10.5px;
    color: rgb(255 255 255 / 0.4);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .bell {
    margin-left: auto;
    color: var(--hot) !important;
  }
  .error {
    margin-left: auto;
    font-size: 11px;
    color: var(--critical);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .open {
    border: 0;
    background: none;
    color: rgb(255 255 255 / 0.5);
    font-size: 11px;
    padding: 4px 0 0;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .open:hover {
    color: #fff;
  }
</style>
