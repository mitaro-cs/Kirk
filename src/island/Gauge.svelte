<script lang="ts">
  // Дуга остатка окна квоты: сколько осталось, цвет – темп расхода.
  import type { QuotaWindow } from '../lib/types';
  import { percent, when } from '../lib/format';
  import { clock } from '../lib/store.svelte';

  let { w, size = 104 }: { w: QuotaWindow; size?: number } = $props();

  const R = 42;
  const SWEEP = 0.75; // дуга на 270°
  const full = 2 * Math.PI * R * SWEEP;
  let left = $derived(1 - w.used);
  let dash = $derived(`${full * left} ${2 * Math.PI * R}`);
</script>

<figure class="gauge" style="--size:{size}px">
  <svg viewBox="0 0 100 100" aria-hidden="true">
    <circle class="track" cx="50" cy="50" r={R} stroke-dasharray="{full} 999" />
    <circle class="value {w.pace}" cx="50" cy="50" r={R} stroke-dasharray={dash} />
  </svg>
  <div class="center">
    <strong>{percent(left)}</strong>
    <span>{w.label}</span>
  </div>
  <figcaption>
    {#if w.resetsAt}сброс {when(w.resetsAt, clock.now)}{:else}полный{/if}
  </figcaption>
</figure>

<style>
  .gauge {
    margin: 0;
    width: var(--size);
    display: grid;
    justify-items: center;
    position: relative;
  }
  svg {
    width: var(--size);
    height: var(--size);
    transform: rotate(135deg);
  }
  circle {
    fill: none;
    stroke-width: 6;
    stroke-linecap: round;
  }
  .track {
    stroke: rgb(255 255 255 / 0.1);
  }
  .value {
    stroke: var(--calm);
    transition:
      stroke-dasharray 600ms var(--ease-out),
      stroke 300ms;
  }
  .value.hot {
    stroke: var(--hot);
  }
  .value.critical {
    stroke: var(--critical);
  }
  .center {
    position: absolute;
    top: 0;
    height: var(--size);
    display: grid;
    place-content: center;
    text-align: center;
    line-height: 1.1;
  }
  strong {
    font-size: 17px;
    font-weight: 650;
    letter-spacing: -0.02em;
  }
  span,
  figcaption {
    font-size: 10.5px;
    color: rgb(255 255 255 / 0.55);
  }
  figcaption {
    margin-top: -6px;
    white-space: nowrap;
  }
</style>
