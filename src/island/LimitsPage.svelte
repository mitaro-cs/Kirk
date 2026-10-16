<script lang="ts">
  import type { Snapshot } from '../lib/types';
  import { PROVIDERS, when } from '../lib/format';
  import { clock } from '../lib/store.svelte';
  import Gauge from './Gauge.svelte';

  let { limits, onsettings }: { limits: Snapshot[]; onsettings: () => void } = $props();

  const STATUS: Record<string, string> = {
    fresh: 'на связи',
    stale: 'давно не обновлялось',
    waiting: 'ждём данных',
    signedOut: 'не подключено',
    notInstalled: 'не установлено',
    failed: 'ошибка',
  };
</script>

<div class="limits">
  {#each limits as p (p.provider)}
    <section class="card {p.provider}">
      <header>
        <span class="dot {p.connection}"></span>
        <h3>{PROVIDERS[p.provider]}</h3>
        <small>{STATUS[p.connection]}</small>
      </header>
      {#if p.windows.length}
        <div class="gauges">
          {#each p.windows as w (w.id)}
            <Gauge {w} />
          {/each}
        </div>
        {#if p.capturedAt && p.connection === 'stale'}
          <p class="note">Данные от {when(p.capturedAt, clock.now)}</p>
        {/if}
      {:else}
        <p class="note">{p.note ?? 'Нет данных'}</p>
        {#if p.connection === 'signedOut' || p.connection === 'notInstalled'}
          <button class="link" onclick={onsettings}>Настроить</button>
        {/if}
      {/if}
    </section>
  {/each}
</div>

<style>
  .limits {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    height: 100%;
  }
  .card {
    background: rgb(255 255 255 / 0.06);
    border-radius: 16px;
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  header {
    display: flex;
    align-items: center;
    gap: 7px;
  }
  h3 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
  }
  .claude h3 {
    color: var(--claude);
  }
  .codex h3 {
    color: var(--codex);
  }
  small {
    margin-left: auto;
    font-size: 11px;
    color: rgb(255 255 255 / 0.45);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: rgb(255 255 255 / 0.25);
  }
  .dot.fresh {
    background: var(--ok);
  }
  .dot.stale {
    background: var(--hot);
  }
  .dot.failed {
    background: var(--critical);
  }
  .gauges {
    flex: 1;
    display: flex;
    justify-content: space-around;
    align-items: center;
  }
  .note {
    margin: auto 0;
    font-size: 12px;
    color: rgb(255 255 255 / 0.6);
    line-height: 1.4;
  }
  .gauges + .note {
    margin: 0;
    font-size: 11px;
    text-align: center;
  }
  .link {
    align-self: flex-start;
    border: 0;
    background: rgb(255 255 255 / 0.12);
    border-radius: 999px;
    padding: 5px 12px;
    font-size: 12px;
  }
  .link:hover {
    background: rgb(255 255 255 / 0.2);
  }
</style>
