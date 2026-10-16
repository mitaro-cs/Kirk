<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { onMount } from 'svelte';
  import { app, start } from '../lib/store.svelte';
  import type { Config, StatusLine } from '../lib/types';
  import Toggle from './Toggle.svelte';

  let claude = $state<StatusLine | null>(null);
  let claudeError = $state('');
  let autostart = $state(false);
  let saveError = $state('');
  let hotkey = $state('');

  let url = $state('');
  let pin = $state('');
  let busy = $state(false);
  let campusError = $state('');

  onMount(async () => {
    await start();
    hotkey = app.s?.config.hotkey ?? '';
    url = app.s?.campus.site ?? '';
    refreshClaude();
    autostart = await invoke<boolean>('autostart_get');
  });

  async function refreshClaude() {
    try {
      claude = await invoke<StatusLine>('claude_status');
      claudeError = '';
    } catch (e) {
      claudeError = String(e);
    }
  }

  async function setConfig(patch: Partial<Config>) {
    if (!app.s) return;
    saveError = '';
    try {
      await invoke('set_config', { config: { ...app.s.config, ...patch } });
    } catch (e) {
      saveError = String(e);
      if ('hotkey' in patch) hotkey = app.s.config.hotkey;
    }
  }

  async function claudeAction(install: boolean) {
    claudeError = '';
    try {
      await invoke(install ? 'claude_install' : 'claude_uninstall');
    } catch (e) {
      claudeError = String(e);
    }
    refreshClaude();
  }

  async function setAutostart(v: boolean) {
    try {
      await invoke('autostart_set', { value: v });
      autostart = v;
    } catch (e) {
      saveError = String(e);
    }
  }

  async function connect(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    campusError = '';
    try {
      await invoke('campus_connect', { url, pin });
      pin = '';
    } catch (err) {
      campusError = String(err);
    } finally {
      busy = false;
    }
  }

  const CODEX: Record<string, string> = {
    fresh: 'Подключено – лимиты обновляются',
    stale: 'Подключено, но данные устарели',
    waiting: 'Подключаемся…',
    signedOut: 'Codex найден – войдите командой codex login',
    notInstalled: 'Codex не найден на этом компьютере',
    failed: 'Ошибка связи с Codex',
  };

  let s = $derived(app.s);
  let codex = $derived(s?.limits.find((p) => p.provider === 'codex'));
  let isMac = $derived(s?.platform === 'macos');
</script>

{#if s}
  <main>
    <header class="top">
      <h1>Kirk</h1>
      <p>Остров сверху экрана: лимиты ИИ, музыка и Campus.</p>
    </header>

    <section class="card">
      <h2>Модули</h2>
      <Toggle
        label="Лимиты"
        hint="Claude Code и Codex"
        checked={s.config.limits}
        onchange={(v) => setConfig({ limits: v })}
      />
      <Toggle
        label="Музыка"
        hint="Что играет и управление"
        checked={s.config.music}
        onchange={(v) => setConfig({ music: v })}
      />
      <Toggle
        label="Campus"
        hint="Пары, дедлайны, новости группы"
        checked={s.config.campus}
        onchange={(v) => setConfig({ campus: v })}
      />
    </section>

    <section class="card">
      <h2>Остров</h2>
      <Toggle
        label="Открывать наведением"
        hint="Иначе – щелчком или сочетанием клавиш"
        checked={s.config.openOnHover}
        onchange={(v) => setConfig({ openOnHover: v })}
      />
      <Toggle label="Запускать при входе в систему" checked={autostart} onchange={setAutostart} />
      <label class="field">
        <span>Сочетание клавиш</span>
        <input
          bind:value={hotkey}
          placeholder={isMac ? 'Control+Alt+K' : 'Ctrl+Alt+K'}
          onchange={() => setConfig({ hotkey: hotkey.trim() })}
          spellcheck="false"
        />
      </label>
      {#if saveError}<p class="error">{saveError}</p>{/if}
    </section>

    <section class="card">
      <h2>Claude Code</h2>
      <p class="hint">
        Kirk ставит маленький мост в строку состояния Claude Code (<code>~/.claude/settings.json</code>). Он
        сохраняет только проценты лимитов и время сброса – без ключей, подсказок и путей. Ваша прежняя строка
        состояния продолжит работать.
      </p>
      <div class="status">
        {#if claude?.state === 'ours'}
          <span class="badge ok">Подключено</span>
          <button class="ghost" onclick={() => claudeAction(false)}>Отключить</button>
        {:else}
          <span class="badge">{claude?.state === 'other' ? 'Своя строка состояния' : 'Не подключено'}</span>
          <button class="primary" onclick={() => claudeAction(true)}>Подключить</button>
        {/if}
      </div>
      <Toggle
        label="Показывать Claude Code"
        checked={s.config.claude}
        onchange={(v) => setConfig({ claude: v })}
      />
      {#if claudeError}<p class="error">{claudeError}</p>{/if}
    </section>

    <section class="card">
      <h2>Codex</h2>
      <p class="hint">
        Kirk запускает свой <code>codex app-server</code> и только читает лимиты: входить и выходить он не умеет.
      </p>
      <p class="status-line">{codex ? CODEX[codex.connection] : ''}</p>
      <Toggle label="Показывать Codex" checked={s.config.codex} onchange={(v) => setConfig({ codex: v })} />
    </section>

    <section class="card">
      <h2>Campus</h2>
      <Toggle
        label="Всплывающие уведомления"
        hint="Новое уведомление Campus коротко показывается в острове"
        checked={s.config.peek}
        onchange={(v) => setConfig({ peek: v })}
      />
      {#if s.campus.connected}
        <div class="status">
          <span class="badge ok">{s.campus.name ?? 'Подключено'}</span>
          <span class="site">{s.campus.site}</span>
          <button class="ghost" onclick={() => invoke('campus_disconnect')}>Отключить</button>
        </div>
        {#if s.campus.error}<p class="error">{s.campus.error}</p>{/if}
      {:else}
        <p class="hint">
          В Campus откройте «Профиль → Безопасность → Показать код» и введите 6 цифр. Сессия хранится в
          {isMac ? 'связке ключей macOS' : 'диспетчере учётных данных Windows'}.
        </p>
        <form onsubmit={connect}>
          <label class="field">
            <span>Адрес сайта</span>
            <input
              bind:value={url}
              placeholder="campus.example.ru"
              required
              spellcheck="false"
              autocomplete="url"
            />
          </label>
          <label class="field">
            <span>Код</span>
            <input
              bind:value={pin}
              placeholder="123 456"
              inputmode="numeric"
              maxlength="7"
              required
              autocomplete="one-time-code"
            />
          </label>
          <button class="primary" disabled={busy}>{busy ? 'Подключаем…' : 'Подключить'}</button>
        </form>
        {#if campusError || s.campus.error}<p class="error">{campusError || s.campus.error}</p>{/if}
      {/if}
    </section>

    <footer>
      Kirk {s.version} · MIT ·
      <button class="linkish" onclick={() => openUrl('https://github.com/mitaro-cs/Kirk')}>GitHub</button>
      · идея – CapaTheNotch
    </footer>
  </main>
{/if}

<style>
  :global(.settings-body) {
    --bg: #f5f5f7;
    --card: #fff;
    --text: #1d1d1f;
    --muted: #6e6e73;
    --line: rgb(0 0 0 / 0.08);
    --off: #d1d1d6;
    --accent: #6d5dfc;
    --field: #f2f2f5;
    background: var(--bg);
    color: var(--text);
    font-size: 14px;
  }
  @media (prefers-color-scheme: dark) {
    :global(.settings-body) {
      --bg: #161618;
      --card: #222225;
      --text: #f5f5f7;
      --muted: #98989f;
      --line: rgb(255 255 255 / 0.08);
      --off: #48484d;
      --accent: #8b7dff;
      --field: #2c2c30;
    }
  }
  main {
    max-width: 620px;
    margin: 0 auto;
    padding: 24px 20px 32px;
    display: grid;
    gap: 14px;
  }
  .top h1 {
    margin: 0;
    font-size: 26px;
    letter-spacing: -0.02em;
  }
  .top p {
    margin: 4px 0 0;
    color: var(--muted);
  }
  .card {
    background: var(--card);
    border-radius: 16px;
    padding: 14px 18px;
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.04);
  }
  h2 {
    margin: 0 0 4px;
    font-size: 15px;
  }
  .hint {
    margin: 4px 0 10px;
    color: var(--muted);
    font-size: 13px;
    line-height: 1.45;
  }
  code {
    font-family: var(--mono);
    font-size: 12px;
    background: var(--field);
    padding: 1px 5px;
    border-radius: 5px;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 0;
  }
  .status-line {
    margin: 0 0 4px;
  }
  .site {
    color: var(--muted);
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .badge {
    font-size: 12px;
    font-weight: 600;
    padding: 3px 10px;
    border-radius: 999px;
    background: var(--field);
    color: var(--muted);
  }
  .badge.ok {
    background: rgb(52 211 153 / 0.16);
    color: #10a874;
  }
  .status button {
    margin-left: auto;
  }
  button.primary,
  button.ghost {
    border: 0;
    border-radius: 10px;
    padding: 7px 16px;
    font-weight: 600;
    font-size: 13px;
  }
  button.primary {
    background: var(--accent);
    color: #fff;
  }
  button.primary:disabled {
    opacity: 0.6;
  }
  button.ghost {
    background: var(--field);
  }
  form {
    display: grid;
    grid-template-columns: 1fr 140px auto;
    gap: 10px;
    align-items: end;
  }
  .field {
    display: grid;
    gap: 4px;
    padding: 6px 0;
  }
  .field span {
    font-size: 12px;
    color: var(--muted);
  }
  input:not([type='checkbox']) {
    font: inherit;
    color: inherit;
    background: var(--field);
    border: 1px solid transparent;
    border-radius: 10px;
    padding: 7px 10px;
    min-width: 0;
  }
  input:not([type='checkbox']):focus {
    outline: none;
    border-color: var(--accent);
  }
  .error {
    margin: 6px 0 0;
    color: #e5484d;
    font-size: 13px;
  }
  footer {
    text-align: center;
    color: var(--muted);
    font-size: 12px;
  }
  .linkish {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent);
  }
  @media (max-width: 560px) {
    form {
      grid-template-columns: 1fr;
    }
  }
</style>
