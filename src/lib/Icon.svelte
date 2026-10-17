<script lang="ts">
  // Свои простые значки: ничего не грузим извне.
  type Name = 'gauge' | 'music' | 'campus' | 'gear' | 'play' | 'pause' | 'next' | 'prev' | 'open' | 'clock';
  let { name, size = 16 }: { name: Name; size?: number } = $props();

  const paths: Record<Name, string> = {
    gauge: 'M4 15a8 8 0 1 1 16 0M12 15l4-5',
    music:
      'M9 18V6l10-2v12M9 18a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0Zm10-2a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0Z',
    campus: 'M2 9l10-5 10 5-10 5L2 9Zm4 2.5V16c0 1.5 2.7 3 6 3s6-1.5 6-3v-4.5M22 9v5',
    gear: 'M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Zm7.4-3a7.4 7.4 0 0 0-.1-1.2l2-1.6-2-3.4-2.4 1a7.6 7.6 0 0 0-2-1.2L14.5 3h-4l-.4 2.6a7.6 7.6 0 0 0-2 1.2l-2.4-1-2 3.4 2 1.6a7.4 7.4 0 0 0 0 2.4l-2 1.6 2 3.4 2.4-1a7.6 7.6 0 0 0 2 1.2l.4 2.6h4l.4-2.6a7.6 7.6 0 0 0 2-1.2l2.4 1 2-3.4-2-1.6c.1-.4.1-.8.1-1.2Z',
    play: 'M7 4.5v15l12-7.5L7 4.5Z',
    pause: 'M7 4h3.5v16H7zM13.5 4H17v16h-3.5z',
    next: 'M5 5v14l9-7-9-7ZM16 5h2.5v14H16z',
    prev: 'M19 5v14l-9-7 9-7ZM5.5 5H8v14H5.5z',
    open: 'M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5',
    clock: 'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Zm0-13v4l3 2',
  };
  const filled = (n: Name) => n === 'play' || n === 'pause' || n === 'next' || n === 'prev';
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill={filled(name) ? 'currentColor' : 'none'}
  stroke="currentColor"
  stroke-width={filled(name) ? 0 : 1.8}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d={paths[name]} />
</svg>
