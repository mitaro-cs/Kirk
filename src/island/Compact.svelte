<script lang="ts">
  // Закрытый остров: крылья по бокам чёлки (или пилюля без чёлки).
  import type { State } from '../lib/types';
  import { clockTime, short } from '../lib/format';
  import { clock } from '../lib/store.svelte';
  import Ring from './Ring.svelte';

  let { s }: { s: State } = $props();

  const COLORS = { claude: 'var(--claude)', codex: 'var(--codex)' };

  let rings = $derived(
    s.config.limits
      ? s.limits
          .filter((p) => p.windows.length > 0)
          .map((p) => {
            const tight = p.windows.reduce((a, b) => (b.used > a.used ? b : a));
            return { provider: p.provider, left: 1 - tight.used, pace: tight.pace };
          })
      : [],
  );
  let track = $derived(s.config.music ? s.music.now : null);
  let summary = $derived(s.config.campus ? s.campus.summary : null);
  let lesson = $derived(summary?.current ?? summary?.next ?? null);
  let notch = $derived(s.shape?.notch ?? null);
  let unread = $derived(s.config.campus ? s.campus.unread : 0);
</script>

<div class="compact" class:plain={!notch} style="--notch:{notch?.width ?? 0}px">
  <div class="wing left">
    {#each rings as r (r.provider)}
      <Ring left={r.left} pace={r.pace} color={COLORS[r.provider]} />
    {/each}
  </div>

  {#if !notch}
    <div class="middle">
      {#if track?.playing}
        {track.title}
      {:else if lesson}
        {lesson.subject?.name ?? lesson.title}
      {/if}
    </div>
  {/if}

  <div class="wing right">
    {#if track?.playing}
      {#if track.artwork}<img src={track.artwork} alt="" />{/if}
      <span class="bars" aria-label="Играет"><i></i><i></i><i></i></span>
    {:else if lesson}
      {#if summary?.current?.id === lesson.id}
        <span class="lesson now">{short(lesson.endsAt, clock.now)}</span>
      {:else}
        <span class="lesson">{clockTime(lesson.startsAt)}</span>
      {/if}
    {/if}
    {#if unread > 0}<i class="unread" title="Новых уведомлений: {unread}"></i>{/if}
  </div>
</div>

<style>
  .compact {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 14px;
    gap: 8px;
    color: #fff;
    font-size: 12px;
  }
  .wing {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .compact:not(.plain) .wing {
    width: calc((100% - var(--notch)) / 2 - 14px);
  }
  .right {
    justify-content: flex-end;
  }
  .middle {
    flex: 1;
    min-width: 0;
    text-align: center;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: rgb(255 255 255 / 0.85);
    font-weight: 500;
  }
  img {
    width: 18px;
    height: 18px;
    border-radius: 5px;
    object-fit: cover;
  }
  .bars {
    display: inline-flex;
    align-items: flex-end;
    gap: 2px;
    height: 12px;
  }
  .bars i {
    width: 2.5px;
    height: 100%;
    border-radius: 2px;
    background: var(--ok);
    transform-origin: bottom;
    animation: bar 900ms ease-in-out infinite alternate;
  }
  .bars i:nth-child(2) {
    animation-delay: -300ms;
  }
  .bars i:nth-child(3) {
    animation-delay: -600ms;
  }
  @keyframes bar {
    from {
      transform: scaleY(0.25);
    }
    to {
      transform: scaleY(1);
    }
  }
  .lesson {
    font-weight: 600;
    color: rgb(255 255 255 / 0.9);
  }
  .lesson.now {
    color: var(--ok);
  }
  .unread {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--critical);
    flex: none;
  }
  @media (prefers-reduced-motion: reduce) {
    .bars i {
      animation: none;
      transform: scaleY(0.6);
    }
  }
</style>
