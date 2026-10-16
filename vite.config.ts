import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';

// Две страницы: остров (index.html) и настройки (settings.html).
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: {
    target: ['es2022', 'safari16'],
    rollupOptions: { input: { index: 'index.html', settings: 'settings.html' } },
  },
});
