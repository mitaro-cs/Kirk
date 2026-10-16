<script lang="ts">
  // Маленькое кольцо для закрытого острова.
  import type { Pace } from '../lib/types';
  let { left, pace, color }: { left: number; pace: Pace; color: string } = $props();
  const C = 2 * Math.PI * 7;
</script>

<svg class="ring {pace}" viewBox="0 0 20 20" width="18" height="18" style="--c:{color}" aria-hidden="true">
  <circle cx="10" cy="10" r="7" class="track" />
  <circle cx="10" cy="10" r="7" class="value" stroke-dasharray="{C * left} {C}" />
</svg>

<style>
  .ring {
    transform: rotate(-90deg);
    flex: none;
  }
  circle {
    fill: none;
    stroke-width: 3;
    stroke-linecap: round;
  }
  .track {
    stroke: rgb(255 255 255 / 0.14);
  }
  .value {
    stroke: var(--c);
    transition: stroke-dasharray 600ms var(--ease-out);
  }
  .hot .value {
    stroke: var(--hot);
  }
  .critical .value {
    stroke: var(--critical);
  }
</style>
