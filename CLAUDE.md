# Kirk – заметки для Claude Code

Остров сверху экрана (как «чёлка» MacBook / Dynamic Island) для **macOS и Windows**: лимиты Claude Code и
Codex, музыка, Campus (сервис группы владельца, репозиторий `mitaro-cs/Campus`). Всё, что видят люди, – по-русски;
тире – короткое «–».

## Стек

Tauri 2 (Rust) + Svelte 5 (руны) + Vite. Без внешних шрифтов и запросов из интерфейса (CSP `default-src 'self'`).

```
crates/kirk-core    чистая логика с тестами: limits, claude (мост и settings.json), codex (JSON-RPC),
                    campus (модели API, summarize, ссылки), music (разбор адаптера), geometry, paths
crates/kirk-bridge  мост строки состояния: stdin → claude::publish, затем прежняя команда (--then base64)
src-tauri/src       hub (состояние + события kirk:*), island (окно, наведение, всплывание), platform/{mac,win},
                    claude, codex, campus, music/{mac,win}, tray, shortcut, commands, config
src/                island/ (Island, Compact, страницы), settings/, lib/ (types, store, format, Icon)
```

## Правила

- Логика, которую можно проверить без окон, – в `kirk-core` с тестом. Оболочка только достаёт данные из
  системы и показывает.
- Окно острова всегда ровно по острову (иначе прозрачное окно перехватывает щелчки под собой). Открытие:
  Rust растит окно → событие `kirk:island` → CSS-анимация. Закрытие – наоборот, окно сжимается через
  `CLOSE_ANIMATION`. Наведение – по положению курсора раз в 100 мс (`island::hover_step`), одинаково на Mac и
  Windows.
- Mac: окно – NSPanel (`tauri-nspanel`), уровень выше строки меню, nonactivating; чёлка – `NSScreen`
  `safeAreaInsets`/`auxiliaryTop*Area`. Windows: `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW`.
- Codex: отправлять только методы из `codex::PERMITTED` – Kirk не трогает вход и токены.
- Мост Claude Code хранит только проценты и время сброса; id сессии – хешем.
- Campus: сессия – в связке ключей (`keyring`), не в файле. Изменения – с `X-CSRF-Token` = cookie `gb_csrf`.
  Ответ «от Campus» – заголовок `X-Groupbase` или JSON (туннель может терять заголовки). Ссылки на сайт – только
  свои пути (`campus::site_link`).
- Событие окну – только если значение изменилось (`hub::replace`): меньше работы интерфейсу.
- Простой ≈ 0 % CPU: никаких таймеров в интерфейсе, когда остров закрыт (часы – раз в 30 с), анимации – CSS.
- Коммиты – Conventional Commits.

## Команды

```
npm ci
npm run tauri dev                 # приложение (мост собирает scripts/prepare.mjs)
cargo test --workspace            # тесты
cargo clippy --workspace --all-targets -- -D warnings
npm run check && npm run lint     # svelte-check, prettier
```

Перед пушем: всё выше + `cargo fmt --all --check`. CI собирает .dmg (Apple Silicon, Intel) и .exe;
тег `v*` – черновик выпуска (`release.yml`).

Кросс-проверка на Linux: `cargo clippy -p kirk --target x86_64-pc-windows-msvc`; для
`aarch64-apple-darwin` нужен подставной C-компилятор (objc2-exception-helper), настоящая проверка – CI на macOS.
