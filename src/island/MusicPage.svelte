<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import type { MusicState } from '../lib/types';
  import { duration } from '../lib/format';
  import { clock } from '../lib/store.svelte';
  import Icon from '../lib/Icon.svelte';

  let { music }: { music: MusicState } = $props();

  let t = $derived(music.now);
  // Позиция идёт сама, пока играет: от снятой позиции + прошедшее время.
  let position = $derived.by(() => {
    if (!t || t.elapsed == null) return null;
    const moved = t.playing && t.elapsedAt ? (clock.now - t.elapsedAt) / 1000 : 0;
    const p = t.elapsed + Math.max(0, moved);
    return t.duration ? Math.min(p, t.duration) : p;
  });
  let progress = $derived(t?.duration && position != null ? position / t.duration : null);
  let error = $state('');

  async function send(command: 'toggle' | 'next' | 'previous') {
    error = '';
    try {
      await invoke('music_command', { command });
    } catch (e) {
      error = String(e);
    }
  }
</script>

{#if !music.available}
  <p class="empty">{music.note ?? 'Музыка на этом компьютере недоступна'}</p>
{:else if !t}
  <p class="empty">Сейчас ничего не играет{music.note ? ` · ${music.note}` : ''}</p>
{:else}
  <div class="player">
    <div class="art">
      {#if t.artwork}<img src={t.artwork} alt="" />{:else}<Icon name="music" size={30} />{/if}
    </div>
    <div class="info">
      <strong title={t.title}>{t.title}</strong>
      {#if t.artist || t.album}<span>{[t.artist, t.album].filter(Boolean).join(' – ')}</span>{/if}
      {#if progress != null && position != null && t.duration}
        <div
          class="bar"
          role="progressbar"
          aria-valuenow={Math.round(progress * 100)}
          aria-valuemin={0}
          aria-valuemax={100}
        >
          <i style="transform: scaleX({progress})"></i>
        </div>
        <div class="times">
          <span>{duration(position)}</span><span>−{duration(t.duration - position)}</span>
        </div>
      {/if}
      <div class="controls">
        <button onclick={() => send('previous')} aria-label="Предыдущий трек"
          ><Icon name="prev" size={18} /></button
        >
        <button class="main" onclick={() => send('toggle')} aria-label={t.playing ? 'Пауза' : 'Играть'}>
          <Icon name={t.playing ? 'pause' : 'play'} size={22} />
        </button>
        <button onclick={() => send('next')} aria-label="Следующий трек"
          ><Icon name="next" size={18} /></button
        >
      </div>
      {#if error}<small class="error">{error}</small>{/if}
    </div>
  </div>
{/if}

<style>
  .empty {
    margin: 0;
    height: 100%;
    display: grid;
    place-items: center;
    color: rgb(255 255 255 / 0.55);
    font-size: 13px;
  }
  .player {
    display: flex;
    gap: 18px;
    align-items: center;
    height: 100%;
    padding: 0 6px;
  }
  .art {
    width: 132px;
    height: 132px;
    flex: none;
    border-radius: 18px;
    overflow: hidden;
    background: rgb(255 255 255 / 0.08);
    display: grid;
    place-items: center;
    color: rgb(255 255 255 / 0.4);
    box-shadow: 0 10px 30px rgb(0 0 0 / 0.5);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  strong {
    font-size: 16px;
    font-weight: 650;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .info > span {
    font-size: 13px;
    color: rgb(255 255 255 / 0.6);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .bar {
    margin-top: 10px;
    height: 4px;
    border-radius: 2px;
    background: rgb(255 255 255 / 0.15);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: #fff;
    transform-origin: left;
    transition: transform 1s linear;
  }
  .times {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
    color: rgb(255 255 255 / 0.45);
  }
  .controls {
    display: flex;
    justify-content: center;
    gap: 14px;
    margin-top: 6px;
  }
  .controls button {
    width: 36px;
    height: 36px;
    border: 0;
    border-radius: 50%;
    background: transparent;
    display: grid;
    place-items: center;
    transition:
      background 150ms,
      transform 150ms;
  }
  .controls button:hover {
    background: rgb(255 255 255 / 0.12);
  }
  .controls button:active {
    transform: scale(0.92);
  }
  .controls .main {
    width: 44px;
    height: 44px;
    background: #fff;
    color: #000;
  }
  .controls .main:hover {
    background: #e5e5e5;
  }
  .error {
    color: var(--critical);
    font-size: 11px;
    text-align: center;
  }
</style>
