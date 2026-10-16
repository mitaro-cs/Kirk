<script lang="ts">
  // Остров. Окно уже нужного размера (его меняет Rust); здесь – форма,
  // анимация между закрытым и открытым видом и страницы модулей.
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { app, start } from '../lib/store.svelte';
  import Icon from '../lib/Icon.svelte';
  import Compact from './Compact.svelte';
  import LimitsPage from './LimitsPage.svelte';
  import MusicPage from './MusicPage.svelte';
  import CampusPage from './CampusPage.svelte';

  type Tab = 'limits' | 'music' | 'campus';
  const TABS: { id: Tab; label: string; icon: 'gauge' | 'music' | 'campus' }[] = [
    { id: 'limits', label: 'Лимиты', icon: 'gauge' },
    { id: 'music', label: 'Музыка', icon: 'music' },
    { id: 'campus', label: 'Campus', icon: 'campus' },
  ];

  onMount(() => {
    start();
  });

  let s = $derived(app.s);
  let shape = $derived(s?.shape ?? null);
  let open = $derived(s?.open ?? false);
  // Последнее всплывшее держим, пока остров сжимается обратно.
  let peek = $derived(open ? null : app.peek);
  let shown = $state(app.peek);
  $effect(() => {
    if (peek) shown = peek;
  });
  let size = $derived(
    !shape
      ? { w: 0, h: 0 }
      : open
        ? { w: shape.openW, h: shape.openH }
        : peek
          ? { w: shape.peekW, h: shape.peekH }
          : { w: shape.compactW, h: shape.compactH },
  );
  let tabs = $derived(s ? TABS.filter((t) => s.config[t.id]) : []);

  function saved(): Tab {
    try {
      const t = localStorage.getItem('kirk-tab');
      if (t === 'limits' || t === 'music' || t === 'campus') return t;
    } catch {
      /* хранилище недоступно – не страшно */
    }
    return 'limits';
  }
  let tab = $state<Tab>(saved());
  let current = $derived(tabs.some((t) => t.id === tab) ? tab : (tabs[0]?.id ?? null));

  function choose(t: Tab) {
    tab = t;
    try {
      localStorage.setItem('kirk-tab', t);
    } catch {
      /* без запоминания */
    }
  }

  const settings = () => invoke('open_settings');
  const expand = () => !open && invoke('island_open', { open: true });
  function openPeek() {
    if (shown?.url) invoke('campus_open', { path: shown.url });
    else {
      choose('campus');
      expand();
    }
  }
</script>

{#if s && shape}
  <div
    class="island"
    class:open
    class:peeking={peek != null}
    role="region"
    aria-label="Kirk"
    style="--w:{size.w}px; --h:{size.h}px;
           --sh:{shape.shoulder}px; --bar:{shape.compactH}px; --ow:{shape.openW}px; --oh:{shape.openH}px"
  >
    <i class="shoulder left"></i>
    <i class="shoulder right"></i>

    <div class="layer compact" aria-hidden={open}>
      <Compact {s} />
      <button class="hit" aria-label="Открыть Kirk" tabindex={open ? -1 : 0} onclick={expand}></button>
    </div>

    <div class="layer banner" aria-hidden={peek == null} aria-live="polite">
      {#if shown}
        <button class="peek" tabindex={peek ? 0 : -1} onclick={openPeek}>
          <span class="badge"><Icon name="campus" size={15} /></span>
          <span class="text">
            <strong>{shown.title}</strong>
            {#if shown.body}<span>{shown.body}</span>{/if}
          </span>
        </button>
      {/if}
    </div>

    <div class="layer full" aria-hidden={!open}>
      <header>
        <nav aria-label="Модули">
          {#each tabs as t (t.id)}
            <button
              class:active={current === t.id}
              onclick={() => choose(t.id)}
              title={t.label}
              aria-label={t.label}
            >
              <Icon name={t.icon} size={15} />
            </button>
          {/each}
        </nav>
        <button class="gear" onclick={settings} title="Настройки" aria-label="Настройки">
          <Icon name="gear" size={15} />
        </button>
      </header>
      <main>
        {#if current === 'limits'}
          <LimitsPage limits={s.limits.filter((p) => s.config[p.provider])} onsettings={settings} />
        {:else if current === 'music'}
          <MusicPage music={s.music} />
        {:else if current === 'campus'}
          <CampusPage campus={s.campus} onsettings={settings} />
        {:else}
          <p class="none">Все модули выключены – включите их в настройках.</p>
        {/if}
      </main>
    </div>
  </div>
{/if}

<style>
  .island {
    position: absolute;
    top: 0;
    left: 50%;
    width: var(--w);
    height: var(--h);
    translate: -50% 0;
    background: #000;
    color: #fff;
    border-radius: 0 0 12px 12px;
    transition:
      width 260ms var(--ease-out),
      height 260ms var(--ease-out),
      border-radius 260ms var(--ease-out),
      box-shadow 260ms;
  }
  .island.open {
    border-radius: 0 0 28px 28px;
    box-shadow: 0 14px 28px rgb(0 0 0 / 0.45);
    transition-duration: 420ms;
  }
  .hit {
    position: absolute;
    inset: 0;
    border: 0;
    padding: 0;
    background: transparent;
    cursor: pointer;
  }

  /* Плечи: вогнутые скругления, которыми остров «вытекает» из края экрана. */
  .shoulder {
    position: absolute;
    top: 0;
    width: var(--sh);
    height: var(--sh);
  }
  .shoulder.left {
    left: calc(-1 * var(--sh));
    background: radial-gradient(circle at 0 100%, transparent calc(var(--sh) - 0.5px), #000 var(--sh));
  }
  .shoulder.right {
    right: calc(-1 * var(--sh));
    background: radial-gradient(circle at 100% 100%, transparent calc(var(--sh) - 0.5px), #000 var(--sh));
  }

  .layer {
    position: absolute;
    top: 0;
    left: 50%;
    translate: -50% 0;
    transition: opacity 180ms;
  }
  .compact {
    width: 100%;
    height: var(--bar);
    opacity: 1;
  }
  .banner {
    top: var(--bar);
    width: 100%;
    padding: 0 12px 10px;
    opacity: 0;
    pointer-events: none;
  }
  .peeking .banner {
    opacity: 1;
    pointer-events: auto;
    transition-delay: 100ms;
  }
  .peeking {
    border-radius: 0 0 22px 22px;
    transition-duration: 380ms;
  }
  .peek {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    border: 0;
    padding: 4px 4px;
    background: transparent;
    text-align: left;
    border-radius: 12px;
  }
  .peek:hover {
    background: rgb(255 255 255 / 0.06);
  }
  .peek .badge {
    width: 30px;
    height: 30px;
    flex: none;
    display: grid;
    place-items: center;
    border-radius: 9px;
    background: linear-gradient(135deg, #7dd3fc, #a78bfa);
    color: #000;
  }
  .peek .text {
    min-width: 0;
    display: grid;
    line-height: 1.25;
  }
  .peek strong,
  .peek .text span {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .peek strong {
    font-size: 13px;
  }
  .peek .text span {
    font-size: 12px;
    color: rgb(255 255 255 / 0.6);
  }
  .open .compact {
    opacity: 0;
    pointer-events: none;
  }
  /* Открытый вид не меняет раскладку во время анимации: он сразу полного
     размера, а остров его обрезает. */
  .full {
    width: var(--ow);
    height: var(--oh);
    opacity: 0;
    pointer-events: none;
    overflow: hidden;
    border-radius: 0 0 28px 28px;
  }
  .open .full {
    opacity: 1;
    pointer-events: auto;
    transition-delay: 120ms;
  }

  header {
    height: var(--bar);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 18px;
  }
  nav {
    display: flex;
    gap: 4px;
  }
  header button {
    width: 28px;
    height: 24px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: rgb(255 255 255 / 0.5);
    display: grid;
    place-items: center;
    transition:
      background 150ms,
      color 150ms;
  }
  header button:hover {
    color: #fff;
    background: rgb(255 255 255 / 0.1);
  }
  header button.active {
    color: #fff;
    background: rgb(255 255 255 / 0.16);
  }
  main {
    height: calc(var(--oh) - var(--bar));
    padding: 6px 16px 16px;
  }
  .none {
    margin: 0;
    height: 100%;
    display: grid;
    place-items: center;
    color: rgb(255 255 255 / 0.55);
    font-size: 13px;
  }
  @media (prefers-reduced-motion: reduce) {
    .island,
    .layer {
      transition-duration: 1ms;
    }
  }
</style>
