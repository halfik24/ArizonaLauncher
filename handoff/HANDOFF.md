# HANDOFF.md — что и как с лаунчером

Документ для входа в проект «с нуля»: текущее состояние, устройство, как собирать,
что сломано/не доделано. Дублей с `IDEA.md` (карта кода) и `README.md` (публичное
описание) стараемся не плодить — здесь акцент на **статус и порядок действий**.

> Дата составления: 05.11.2026; обновлено 08.11.2026. HEAD: `dbc51e5` (08.11.2026,
> `fix(ui): батч от 08.11 — пилюли тайтлбара, «Моды», каталог, тултипы, дебаг`),
> ветка `main`, рабочее дерево чистое, пуша нет (ahead ~27). Раунд §12 (9 багов)
> и батч §13 (6 фиксов 08.11) закрыты — прогоны PASS.

---

## 1. Что это и где мы сейчас

**Arizona RP Launcher v2** — лаунчер для GTA San Andreas Multiplayer Arizona RP
(СНГ). Написан заново на **Rust + Tauri 2**, заменяет v1 на Python/pywebview.
Только Windows 10/11 x64 (WebView2), установщик NSIS, ставится без прав администратора.

**Текущая версия в коде:** `2.0.0-beta` — синхронизирована в `package.json`,
`src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`; для UI генерируется
`src/version.js` (`v2.0.0-beta`) скриптом `tools/set-version.js`.
⚠️ В `IDEA.md` всё ещё фигурирует `v2.0.0-rc1-new` — это уже не актуально.

**Версии по релизам:**
- `v2.0.0-rc1` — 15.10.2026 (самостоятельный запуск без сторонних программ).
- **бета 1** — 03.11.2026: лаунчер сам обновляет и чинит игру (MD5-верификация
  всех файлов). Текст объявления — в `updatenews.txt` (черновик для ArzLaunchRepo,
  не код).

**Что сделано и подтверждено:**
- Полный аудит бэкенда/фронта/CI (23–24.10.2026), исправления закоммичены по
  фазам: `ee37ae3` … `b052dfd` (критичные баги фронта, single-instance и потоки,
  защита данных, SSRF/лимиты, UX, CI).
- Все 11 заглушек из аудита 18.10 реализованы; бридж полный: каждая команда из
  `main.rs` есть в `src/tauri-bridge.js` (`tools/gen-bridge.js` сверяет и
  генерирует таблицу).
- Бэкенд: **43 теста** (`cargo test` в `src-tauri/`), 0 warnings.
- Выпущена бета 1, CI собирает установщик на каждый push в `main` и на теги.
- Деплой лендинга `site/` на GitHub Pages (`.github/workflows/deploy.yml`).

**Что осталось (подробно — §11):**
1. Живой UI-прогон после правки CSP — по всем экранам (обои/иконки/новости/каталоги).
2. Пункт 5 старого плана: драйверы DX/VCRedist (манифест `drivers/` на CDN).
3. Новости: 3 URL последовательно по 20 с при лежащем GitHub (минор).
4. Режим «Air» с плагином — фактически «выбор без выбора», работает только нативный.
5. Настройки патчера в UI (сами патчи есть, панель настроек пока выключена).

---

## 2. Дерево проекта

```
src/                    фронтенд — ОДИН файл index.html (~15 800 строк: CSS + JS)
  ├─ tauri-bridge.js    мост: позиционные вызовы pywebview.api.* → tauri invoke()
  │                     с именованными camelCase-аргументами (генерируется
  │                     tools/gen-bridge.js из сигнатур launcher.rs)
  ├─ version.js         ГЕНЕРИРУЕТСЯ set-version.js — единственный источник версии для UI
  ├─ lucide.min.js      иконки
  └─ fonts/             Montserrat Alternates (woff2)
src-tauri/src/
  ├─ main.rs (225)      точка входа: логирование, single-instance, трей, ~95 команд
  ├─ launcher.rs (5819) ВСЯ бизнес-логика
  ├─ config.rs (373)    LauncherConfig/LaunchParams/LauncherSettings → config.json
  ├─ util.rs (269)      lock_config, пути, лог, чтение патчей, версии exe
  └─ game_update/       движок установки/обновления/починки игры
      ├─ manifest.rs    game.json: дерево → плоский список (variant future/default)
      ├─ verify.rs      сверка локальных файлов → план (Mkdir/Delete/DateFix/Download)
      ├─ downloader.rs  reqwest: манифест + файлы, фоллбэк main→reserve CDN, .part
      ├─ engine.rs      оркестрация Check/Apply/Repair, события, отмена/пауза
      └─ state.rs       кэш md5 → game_update_state.json
tools/                  gen-bridge.js, build.js, set-version.js, cdp.js, perf-*.js
site/                   лендинг (GitHub Pages)
wallpapers/             встроенные обои — bundle.resources, НЕ во frontendDist
.agent/                 рабочая папка агента (не в git): скриншоты, UI-проверки
dist/                   локальные релизные артефакты (в .gitignore)
.github/workflows/      build.yml (сборка/релизы), deploy.yml (Pages)
```

**Ключевой принцип:** фронтенд — почти нетронутый Python-код из v1, адаптированный
только бриджем. Поэтому в `index.html` всё ещё есть `pywebview.api`, проверки
`if (window.pywebview.api.X)`, позиционные вызовы. **Не «чищить» без нужды** —
сломается бридж и поведение, отлаженное годами.

---

## 3. Бэкенд по подсистемам (`launcher.rs`)

### 3.1 Запуск игры (`start_game`)
- Два метода (`cfg.launch_method`): **`native`** — сами запускаем `gta_sa.exe`,
  **`air`** — через `ArizonaLauncher*.exe` (поддерживается только Air v8).
- Порядок валидаций как в Python: путь к лаунчеру (для air) → путь к игре →
  `quick_game_check` (9 ключевых файлов: наличие + размер > 0, без MD5; по коду
  `FILES_MISSING` фронт сам запускает авто-починку) → ник (`sanitize_nickname`).
- Аргументы (`build_launch_args`): `-c -h <ip> -p <port> -mem <n> -n <nick>
  [-cdn x,x,x] -arizona` + общие флаги (`-x` автологин, `-window`, `-widescreen`,
  `-t`, `-16bpp`, `-allow_hdr`, `-enable_grass`, `-ldo`, `-auth_cef_enable`) +
  флаги v7+ (`-enable_new_grass`, `-use_d3dx9_43`, `-show_dialog_ids`, `-offcef`,
  `-modern_scale`, `-trees_new`, `-old_window`). Позиция `-cdn` разная для
  native/air — покрыта тестами `launch_tests`.
- Перед стартом — `kill_all_launchers()`; в native пишется реестр SA-MP
  (ник, память: `write_samp_registry`).
- Poll 2 сек: Air-стабы завершаются с кодом 0 сразу после спавна дочернего
  процесса — это не краш. Успех → сохранение ник/сервер/параметры в конфиг.
- Ответы — JSON `{success, message, code}`; коды: `LAUNCHER_MISSING`,
  `GAME_MISSING`, `FILES_MISSING`, `NICKNAME_EMPTY`, `NICKNAME_INVALID`,
  `LAUNCHER_CRASHED`, `EXCEPTION`.
- `is_gta_running(consume_exit)` — опрос статус-баром (5 с) и GTA-watcher'ом
  (3 с) со снапшотом ~1.2 с; edge выхода игры разворачивает окно лаунчера.

### 3.2 Авто-поиск игры (`auto_detect_paths`, spawn_blocking)
Реестр (Uninstall-ключи со словом «Arizona») → скан корней дисков (папки с
«arizona», дедлайн 5 с). Ответ может содержать и путь к Air-лаунчеру.
- `select_game_path` возвращает маркер `{code:'EMPTY_FOLDER', folder}` для
  пустой папки (НЕ ошибку) — фронт спрашивает «Установить игру сюда?» и зовёт
  `install_game_api(folder)`. Папка запоминается в
  `AppState.last_picked_empty_folder` — `install_game_to_folder` принимает
  только её (consent живёт на нативной стороне, не в вебвью).

### 3.3 Установка/обновление/починка игры — см. §4.

### 3.4 Патчи (ArizonaPatches)
- Файл патчей: `<Documents>/ArizonaLauncher/` (`patches_json_path`), бэкапы —
  `list/restore/delete/preview_patches_backup` (авто-бэкап перед записью,
  `create_patches_backup`, хранится N последних).
- Каталог/версии качаются с GitHub, без перевыпуска лаунчера:
  - `.../halfik24/ArzLaunchRepo/main/patches/version.txt` и
    `updates/patches.json` (фоллбэк — `worteng/ArizonaLauncher`);
  - zip патчей: `patches/arizonapatches_{version}.zip`;
  - пресеты: `configs.txt`, схема: `patches_schema.json`.
- Версия валидируется (`is_safe_patches_version` — только «безопасный» набор
  символов, анти-injection), качка zip → `extract_patches_zip` (защита от
  path traversal).
- `disable_old_dll` — лаунчер сам вырубает dll старого патчера: патчи вызывают
  краши, поэтому «настройки патчера» в UI пока нет (см. README, раздел Патчи).
- Статус/баннер обновления: `check_patches_status`, `check_patches_files`,
  `check_patches_available(_api)`, `check_patches_update_status`.
- Редактор с диффом (`diff_vs_current_patches`), импорт/экспорт
  (`export_patches`/`import_patches`), установка из файла
  (`install_patches_file` — filename + contentB64).
- Прогресс установки патчей — событие `patches-progress` (stage/progress/message).

### 3.5 Каталоги модов (всё с GitHub, обновляется снаружи)
| Каталог | Источник (фоллбэк — репо worteng/halfik24 ArizonaLauncher) |
| --- | --- |
| Lua-скрипты MoonLoader | `ArzLaunchRepo/main/Lua/lua.txt` |
| Библиотеки (mimgui и др.) | `ArzLaunchRepo/main/libraries.txt` |
| Дополнения (DxVK, Air, прелоады) | `ArzLaunchRepo/main/others.txt` |
| Air-лаунчер/прелоады (для установки лаунчера) | `ArzLaunchRepo/main/others/ArizonaLauncher8.0_byAIR.zip`, `preloading_plugins.zip` |

- Скрипты: `get/toggle/delete_moonloader_script`, `install_moonloader_script`,
  `install_lua_file` (filename + contentB64). Имена файлов санитизируются
  (`safe_script_name`).
- Библиотеки: `list_installed_libraries`, `install_library`, `uninstall_library`
  (zip раскладывается в `moonloader/libs`).
- Дополнения: `fetch_others_catalog`, `install_other_file(url, filename, destination)`,
  `remove_other_file`, `check_other_installed` — destination резолвится
  относительно папки игры (`resolve_dest`, без выхода наружу).
- Парсер каталога — блочный формат `name/description/url/...`
  (см. тесты `parse_catalog_txt`).

### 3.6 Серверы и новости
- `get_servers` / `get_servers_and_news` — stale-while-revalidate: дисковый кэш
  (`servers.json` с ETag/Last-Modified), фоновая ревалидация
  (`spawn_servers_revalidate`/`spawn_news_revalidate`), таймауты.
- Иконки серверов кэшируются на диск (`icon_cache_dir`), отдаются data-URI.
- Онлайн берётся с `https://arizona-ping.react.group/desktop/ping/Arizona/ping.json`.
- Новости лаунчера: `updatenews.txt` из `halfik24/ArzLaunchRepo` — 3 URL
  последовательно (raw, refs/heads, github.com/raw). Прочитанные id —
  `read_news_ids` в конфиге.
- `check_launcher_update` — проверка новой версии лаунчера (для установщика из UI).

### 3.7 UI-инфраструктура
- Рамка окна: `window_frame` = `system` | `custom`. Custom — свой тайтлбар
  (drag/resize/maximize команды), закрытие прячет в трей (`prevent_close`).
  Окно создаётся `visible: false` и показывается после применения рамки —
  чтобы не мигала системная.
- Трей: левый клик — показать окно, меню «Выход». `check_tray_support` для фронта.
- Обои: встроенная коллекция (`wallpapers/` в корне репо, ищется через
  `wallpapers_dirs` в ресурсах) + свои картинки (`select_bg_image` → нативный
  диалог → data-url, `fetch_image_as_base64` для внешних URL с SSRF-фильтром
  `check_image_url`/`is_forbidden_image_host`), превью (`get_wallpaper_thumb`).
  Monet-палитра из обоев — на фронте.
- Профили: `get_profiles`/`save_profiles(profiles, activeId)` — отдельный
  профиль = свой набор настроек, автосохранение.
- Окно/раскладка: `get/set_window_title`, `get_window_size`, `save/get_layout_order`.
- Лог/папки: `open_log_file`, `open_game_folder`, `open_data_folder`, `open_url`.
- `get_debug_info` — версии ОС, WebView, путёвка для отладки (вкладка «Отладка»).

### 3.8 Обновление самого лаунчера
`start_launcher_install(folderPath, version, installType)` — качка zip во
временный файл, `extract_launcher_zip`, `remove_old_launcher_exes`,
`has_preloading_plugins`; прогресс тем же событием `launcher-install-progress`.
Ветка UI — `installOverlay` (шаги warn/dl/inst/done) и `checkLauncherInstallationUI`.

---

## 4. Движок игры: установка/обновление/починка (`game_update/`)

Порт официального Arizona Games Launcher (разобран az-proto), свои отличия —
MD5 после качки сверяется (оригинал верит манифесту), свой кэш хэшей.

**Поток:**
1. `manifest.rs` — `GET {cdn}/game.json` → дерево → плоский список `Entry`
   (rel, kind: dir/file/check/delete, size, hash md5, date_change, res).
   `variant_tag`: при включённой «новой графике» берётся `future`, иначе `default`.
   Секция `res` качается только при починке.
2. `verify.rs` — сверка локальных файлов → `Plan` (Mkdir / Delete / DateFix /
   Download), без записи. Правила: нет файла / размер / mtime против
   `date_change` / MD5 (с кэшем хэшей, параллельный расчёт до 8 воркеров).
3. `engine.rs::run` — права на запись → план → свободное место (+256 МБ запас) →
   применение по порядку: Mkdir, Delete, DateFix, Download.
4. `downloader.rs` — качка потоком в `.part` → rename, докачка с места обрыва
   (Range), таймаут чанка 30 с, фоллбэк **основной CDN → резервный на каждый
   файл**, MD5 после качки (несовпавший файл удаляется и пробуется другой CDN),
   mtime из манифеста, лимит скорости (токен-ведро, `game_update_limit_mb`).

**CDN:** основной `https://pc.az-ins.com/release`, резервный
`https://arz-pc.react-group.tech/release`. Файлы: `{cdn}/game/{rel}`.

**Режимы (`Mode`):** `Check` (только чтение), `Apply` (установка/обновление),
`Repair` (починка, задействует res-секцию).

**Путь назначения:** `game_path` ведёт до `gta_sa.exe`, корень манифеста —
родитель. Все пути конфинятся к корню (`confined_join` — анти path-traversal,
тесты есть).

**API в launcher.rs:**
- `check_update_api(gamePath?)` — синхронный Check, возвращает
  `{needs_update, files, bytes, date_fix, mkdir, deletes}`.
- `download_update_api` / `repair_game_api(future)` / `install_game_api(future)` /
  `install_game_to_folder(folder, future)` — через `spawn_game_job` в фоне.
- `cancel_game_update` / `pause_game_update(paused)` / `game_update_status` —
  флаги `AppState.game_update_cancel` / `game_update_pause` (Arc<AtomicBool>),
  одна активная задача (`claim_game_job` + `GameJobGuard`).
- События фронту — хаб **`launcher-install-progress`**: `{stage: 'dl'|'done'|'error',
  progress, message}`. На стадии проверки `files_total = 0` (счётчик файлов не показываем).
- Задача блокируется, если запущен `gta_sa.exe`.
- Свежая установка: после успеха `game_path` пишется в конфиг, ищется Air-лаунчер —
  есть → `launch_method = air`, нет → `native`.

**Кэш:** `<Documents>/ArizonaLauncher/game_update_state.json` — `{"hashmap": {rel: md5}}`.
Битый файл не роняет загрузку (тест есть).

**Настройки:** «новая графика» (`game_update_future`), лимит скорости
(`game_update_limit`, 0 = без лимита), «проверять обновления при запуске» —
лежит в `launcher_settings.extra` (см. §5).

---

## 5. Конфиг и хранилище пользователя

Всё в `<Documents>/ArizonaLauncher/` (fallback — `%LOCALAPPDATA%`, если Documents
недоступен):

| Файл | Что внутри |
| --- | --- |
| `config.json` | основной конфиг (см. ниже); атомарная запись через `.tmp` + rename |
| `config.json.broken` | копия битого конфига при загрузке (диагностика) |
| `launcher.log` / `launcher.log.old` | лог (ротация при > 2 МБ); в dev — консоль |
| `game_update_state.json` | кэш md5 |
| `servers.json`, новости, кэш иконок | дисковые кэши |

Структура `config.json` (`config.rs`, все ключи camelCase):
```
lastNickname, lastServer, gamePath, launcherPath,
launchMethod ("native"|"air"), readNewsIds[], profiles?, activeProfileId?,
launchParams { memory=4096, widescreen, textureMode, colorDepth16, allowHdr,
               enableGrass, ldo, authCefEnable, windowMode=true, cdn="1,1,1",
               autologin=true, enableNewGrass, oldWindow, useD3dx9_43,
               showDialogIds, offcef, modernScale, treesNew },
launcherSettings { bgImage?, windowTitle?, windowSize?, windowFrame?,
                   layoutOrder?, ...extra (flatten) },
patchesVersion?, patchesLastCheck?, patchesUpdateAvailable?, lastLauncherVersion?
```

**Важно:**
- `launcherSettings.extra` — `#[serde(flatten)]`: незнакомые ключи
  (`hide_profiles`, `status_bar`, `status_bar_pos`, `bg_data`, «проверять
  обновления при запуске») сохраняются целиком и читаются фронтом **в
  snake_case**. Не отбрасывать, не переименовывать.
- `#[serde(default)]` на контейнере: обрезанный/чужой конфиг грузится
  частично, а не падает в default (иначе первое сохранение затирает данные).
- Миграция легаси (Python, snake_case): `migrate_legacy_config` конвертирует
  ключи, а у конфига без `launch_method` выставляет **`air`** (легаси всегда
  запускал через Air) — тест `legacy_snake_case_config_migrates_without_data_loss`.

---

## 6. Фронтенд (`src/index.html`, ~15 800 строк)

Один самодостаточный HTML: стили, разметка, весь JS. Инициализация стартует по
событию `pywebviewready` (его диспатчит бридж) → `init()`.

**Экраны:**
- **Онбординг** (`fl-overlay`, классы `_fl*`) — визард: приветствие → поиск /
  установка игры (авто-поиск, ручной выбор, три кнопки) → фичи → обои → метод
  запуска.
- **Главный** — никнейм, кастомный селект сервера (`csTrigger`/`csDropdown`),
  кнопка Play, `launch-screen` с прогрессом, статус-бар (`statusBar`:
  скрипты, «в игре»), панель прогресса установки (`gpBar`/`gpOverlay`),
  новости (две вкладки: лаунчера/Arizona), профили (`profilesHoverWidget`).
- **Настройки** (`settings-panel`) — вкладки слева:
  `patches` (ArizonaPatches), `launch` (параметры запуска: память, CDN, флаги),
  `import` (импорт/экспорт/бэкапы), `launcher` (кастомизация, рамка окна,
  статус-бар, обои, обновление игры), `about`, `debug` (скрытая — отладка).
  Плюс отдельный пикер обоев (`wallpaperNav`).
- **Модалки** (`mods-modal`, вкладки): MoonLoader-скрипты + библиотеки,
  пресеты патчей, «Другое» (дополнения). Также `folder-modal` (управление
  папками), `script-detail-overlay` (карточка скрипта с галереей),
  `lb-overlay` (лайтбокс), `cre-overlay` (CRE/пресеты), профили
  (`profiles-overlay`, create/edit), `cso-overlay` (CustomStaticObjects),
  `installOverlay` (обновление лаунчера), `gpOverlay` (прогресс обновления игры),
  `drag-overlay` (drag&drop файла), `feedbackPopup` (Google Forms).

**Префиксы классов/функций:** `_fl*` — first-launch, `_cso*` — редактор
серверов/CSO, `_lb*` — лайтбокс, `_mods*` — модалки, `_gp*` — прогресс
установки игры, `_tt*` — тултипы, `_ep*` — редактор профиля.

**Бридж (`src/tauri-bridge.js`):**
- Таблица `CMD_PARAMS`: команда → массив имён параметров в порядке следования.
- `window.pywebview.api.X(...позиционно)` → `invoke('x', {camelCase-имена})`,
  с предупреждением в консоль при несовпадении количества аргументов.
- Генерируется: `node tools/gen-bridge.js` (парсит сигнатуры `launcher.rs`).
  **После добавления новой команды — перегенерировать бридж.**

---

## 7. Dev-цикл

```bash
npm install
npm run dev      # tauri dev: бэкенд пересобирается сам, фронт — Ctrl+R в окне
npm run build    # релиз + NSIS-установщик → src-tauri/target/release/bundle/nsis/
cd src-tauri && cargo test    # 43 теста
node tools/gen-bridge.js      # перегенерация бриджа после новых команд
```

- **CDP-проверки UI:** временно добавить `--remote-debugging-port=9222` в
  `additionalBrowserArgs` в `src-tauri/tauri.conf.json` (**убрать перед
  коммитом!**), затем `node tools/cdp.js "<JS-выражение>"`.
- **Скриншоты:** `powershell -NoProfile -ExecutionPolicy Bypass -File .agent\flcap.ps1 -Out <путь.png>`.
  Окно должно быть в фокусе — иначе WebView2 отдаёт старые кадры
  (SetForegroundWindow уже внутри скрипта).
- `tools/perf-*.js` — замеры производительности (открытие настроек, A/B и т.п.).
- Лог для диагностики: `Документы\ArizonaLauncher\launcher.log` (кнопка
  «Открыть лог» в лаунчере).

---

## 8. Сборка, версии, релизы

- **`npm run build`** = `tools/build.js`:
  1. если версия в `tauri.conf.json` = базовая `2.0.0-beta` → штамп сборки по
     дате/времени (`set-version.js --build`); если версия уже задана (CI ставит
     из тега) → штамп не применяется;
  2. `tauri build`;
  3. в `finally` — **всегда откатывает версию** к исходной, рабочее дерево
     остаётся чистым (иначе CI после сборки увидит бы дифф).
- **CI** (`.github/workflows/build.yml`), windows-latest:
  - push в `main` — контрольная сборка (артефакт `installer`, 90 дней);
  - тег `v*` — версия из тега (`set-version.js --set <tag>`), сборка, **GitHub
    Release с установщиком**;
  - тег `nightly-*` — штамп по времени, только артефакт в Actions.
- Подписи нет → SmartScreen предупреждает (ожидаемо).
- **Pages**: `deploy.yml` публикует `site/` при изменении `site/**`.
- Версии синхронизировать в `package.json`, `tauri.conf.json`, `Cargo.toml`
  — руками не править, только `npm run set-version`.

---

## 9. Известные подводные камни

1. **CSP держим `null`** (`tauri.conf.json`). CSP блокирует инъекцию IPC-бриджа
   Tauri **только в release** (dev не применяет CSP — дефект не виден локально):
   `pywebviewready` не наступает, init не выполняется, UI «разваливается»
   (нативные select'ы поверх кастомных, мусорные панели). Откат был сделан
   коммитом `205c285`. Вернуть hardening — только nonce/hash-подходом
   **обязательным релизным smoke-прогоном** (`cargo check` это не ловит).
2. **Имена процессов:** релизный exe — `Arizona RP Launcher.exe` (с пробелами),
   в dev — `arizona-launcher.exe`. При убийстве процесса проверять оба.
   При single-instance второй экземпляр молча уходит, а показывает окно
   первого — легко перепутать, что тестируешь.
3. **`select_game_path`** возвращает маркер `EMPTY_FOLDER`, а не ошибку (§3.2).
4. **`launcher_settings.extra`** (flatten) — незнакомые ключи сохраняются целиком
   в snake_case; «проверять обновления игры при запуске» живёт там же.
5. **Закрытие окна при custom-рамке = в трей** (prevent_close); выход — через
   меню трея. Второй экземпляр не запускается (tauri-plugin-single-instance).
6. **Обои лежат в `wallpapers/` в КОРНЕ репо** — это `bundle.resources`,
   не `frontendDist`; не переносить в `src/`.
7. **Фронт не чистить от `pywebview.api`** (§2) — это не мёртвый код.
8. **`old_python/` удалён** из репо (коммит `a8a513f`), легаси-кода в дереве нет —
   в IDEA.md упоминание о нём устарело.
9. `.agent/`, `dist/`, `src-tauri/#ArizonaPatches.json` — в `.gitignore`,
   в git не попадают.
10. Рабочая команда git в этой среде: обёртка `snip.exe` падает на `;` —
    вызывать `& "C:\Program Files\Git\cmd\git.exe" ...`.

---

## 10. Конвенции

- Общение — кратко, по-русски; комментарии в коде и UI-строки — тоже на русском.
- Локальные коммиты **без пуша**, историю не переписывать.
- Формат: `тип(область): суть` — `game-update: ...`, `ui: ...`, `feat(onboarding): ...`,
  `fix(front): ...`, `ci: ...`, `chore: ...`, `docs: ...`.
- Секретов/переменных окружения в репе нет; внешние сервисы — только публичные
  CDN и GitHub-репо каталогов.

---

## 11. Очередь задач (что делать дальше)

> ✅ Список из 9 пунктов от 07.11 (§12) **закрыт** — все пункты, прогоны PASS.
> ✅ Батч из 6 фиксов от 08.11 (§13) **закрыт** — прогоны PASS.
> Таблица ниже — актуальная фоновая очередь.

| # | Задача | Заметки |
| --- | --- | --- |
| 1 | **Живой UI-прогон** после CSP-правки | `npm run dev`, пройтись: обои, иконки серверов, новости, каталоги модов, патчи. Подозрение — самые хрупкие места после отката CSP |
| 2 | Драйверы DX/VCRedist | пункт 5 старого плана: манифест `drivers/` на CDN (по образцу game_update) |
| 3 | Новости: 3 URL × 20 с | при лежащем GitHub новость ждёт минуту — сократить таймаут/параллелить (минор) |
| 4 | Режим Air с плагином | сейчас «выбор без выбора»: работает native; air-ветка написана, но не доведена |
| 5 | Настройки патчера в UI | патчи крашат игру → dll вырубается, панель настроек скрыта |
| 6 | Правка IDEA.md | версия `2.0.0-rc1-new` → `2.0.0-beta`, ссылка на HANDOFF.md → `handoff/HANDOFF.md`, убрать `old_python/` |

Приоритет по умолчанию: **№1** — это единственное, что блокирует уверенность
в текущей релизной сборке.

---

## 12. Раунд 07.11.2026 — список из 9 багов/недоделок

Пользователь дал список после ручного теста на home-pc (Win10). HEAD на момент
закрытия раунда: `9f82efe` (main, ahead ~25, пуша нет), рабочее дерево чистое.
**Все 9 пунктов закрыты** (см. статусы), контрольные прогоны на собранном exe
прошли, включая контрольный прогон №7 (`probe-cancel.js` — 12/12 PASS).
exe на home-pc = перенесённая сборка раунда (сверка SHA256).

| # | Баг/недоделка | Разведка (где что) | Статус |
| --- | --- | --- | --- |
| 1 | Кастомная рамка: углы не скруглены на Win10 | `launcher.rs:2154 apply_rounded_corners` — только DWM-атрибут 33 (Win11), на Win10 молча не работает. Фикс `30d4a17` (SetWindowRgn + refresh на Resized) признан халтурной обводкой и **откачен** `715fa2f` — см. §13 №2 | ✅ `30d4a17` → **откат `715fa2f`** |
| 2 | Уведомления позади кнопок окна | `#notificationContainer` и `#toastContainer` лезли под `#titlebar` (z:100000). Фикс: offset top:52px под `body.frame-custom` | ✅ `60a84a2` |
| 3 | Запечённый блюр во вкладке Моды | Отдельный canvas `#modsBaked` (не переиспользовать `#settingsBaked`), гейты `html.anim-full/fade body.mods-anim` (550мс, pop/smooth — живой backdrop), тинт перенесён из stage в выравнивание (`_alignSettingsBaked` 0.7 / `_alignModsBaked` 0.9 — stage общий). Прогон `probe-mods-bake.js` 15/15 на собранном exe | ✅ `9f82efe` |
| 4 | Пилл «Установленные/Каталог» растянут во всю ширину | Прямой inline-ребёнок column-flex `#mlPaneLibraries` — растягивается по align-items:stretch. Фикс: `align-self:flex-start` | ✅ `60a84a2` |
| 5 | Косметика заголовка/плашки версии + компактное меню в дебаг | unslop-аудит хедера: убрана жёсткая тень, clamp от vw → шкала 32/14/12, пилюля `.version-stable` → тихая mono-строка «Launcher · v2.0.0-beta» (класс оставлен — используют визард и шаблон). Компакт-меню: `setCompactMenu` → localStorage `arp_compactMenu` + класс `compact-menu` на `<html>` до первой отрисовки, карточка «Меню» в `#debugTab`; только дебаг, не постоянный режим. Замер: хедер 74px → 33px в одну строку | ✅ `9f82efe` |
| 6 | Попап обратной связи закрывается без анимации | Класс `.closing` + keyframes выхода по образцу `closeGpOverlay` | ✅ `60a84a2` |
| 7 | Отмена обновления игры не отменяет | Корень найден в диагностике (diag7-report): отмена «догоняла» скан только после `verify::plan` — фазы без проверок `cancelled()`. Фикс: отменяемый md5-скан + опрос отмены во всех фазах (`1d1ce31`), фронт `gpCancelUpdate` с guard `_gpCancelling` и «Отменяем…», ветка `/отменено/i` → «Установка отменена» без класса error, гашение 4с (`a4a28fe`). **Контрольный прогон PASS 12/12** (`probe-cancel.js`): состояние «Отменяем…» поймано на +3мс, busy=false через 3мс/108мс, оверлей закрывается движком, полоса гаснет; лог: 4 отмены 07.11 — «отменено пользователем» за 0.7–1.8с (было 136.8с 03.11) | ✅ `1d1ce31` + `a4a28fe` + прогон |
| 8 | Фейковые демо функций в debug | Демо только графически: `_onLauncherInstallProgress` фейковыми событиями, бэкенд не трогает | ✅ `60a84a2` |
| 9 | Панель новостей резко пропадает при закрытии | В transition `.right-panel` добавлены background/border/box-shadow/backdrop — фон гаснет вместе с движением | ✅ `60a84a2` |

**Порядок работ (договорённость):** батчами по зонам, после каждого батча —
`check-syntax.js` → kill exe → `build-cdp.ps1` (cwd `src-tauri`) → запуск →
прогоны (`exp-verify-bugs`, `exp-mods-after-settings`, `probe-mods-bake`,
`check-blur`, `verify-anim-fix` ×4, `exp-align`, `probe-cancel`, №7-контроль),
коммиты группами через `-F`, в конце — `check-default-mode` (сброс на full),
перенос exe на home-pc (`tailscale file cp` + `cleanup-exe.cmd`, сверка
SHA256).

**Инструменты этого раунда** (во временном каталоге сессии, не в git):
`exp-verify-bugs.js` (16 проверок, вкл. повторный цикл open/close),
`exp-mods-after-settings.js` (7), `exp-align.js`, `probe-mods-bake.js` (15,
бэкинг «Модов»), `probe-header.js` (замер геометрии/шрифтов хедера),
`probe-cancel.js` (12, контроль №7: MutationObserver на кнопке отмены),
`cdp-call.js` (вызов функций приложения без кавычек в cmd: `node cdp-call.js
setAnimMode fade`; true/false/null — литералы), `check-syntax.js`,
`build-cdp.ps1`, `list-arizona.ps1`, `tail-cancel.js` (хвост launcher.log).
Починен `.agent/flcap.ps1` (MainWindow процесса — служебное окно WebView2
16×16; теперь EnumWindows + самое крупное окно, снимки в `shots/`).
Прогон в `tools/`: `check-blur.js` (16), `verify-anim-fix.js` (по режимам:
fade 22 / pop 20 / smooth 20 / full 22), `check-default-mode.js`.

---

## 13. Раунд 08.11.2026 — батч из 6 фиксов (текущая работа)

Пользователь дал список после ручного теста. Диагностику делали 3 субагента
explore (read-only), правки применяла основная сессия. HEAD на момент закрытия:
`dbc51e5` (main, ahead ~27, пуша нет), рабочее дерево чистое. **Все 6 пунктов
закрыты**, прогоны на собранном exe PASS.

| # | Баг/недоделка | Разведка и фикс | Статус |
| --- | --- | --- | --- |
| 1 | При открытии настроек в панели названия окна пропадает блюр | Зонтичные правила `body.settings-anim *` / `body.mods-anim *` снимали backdrop-filter у всех внешних слоёв, включая пилюли `#tbTitle`/`#tbButtons`. Фикс: исключить их `:not(#tbTitle):not(#tbButtons)`, зонтик для остальных сохранён | ✅ `dbc51e5` |
| 2 | Халтурная обводка окна вместо скругления как у Windows 7 | Полный revert `30d4a17` (`715fa2f`): убраны `apply_rounded_region`/`clear_window_region`/`refresh_window_region`, `FRAME_CUSTOM`, listener Resized, `ARIZONA_FORCE_REGION`. Итог: Win11 — DWM-атрибут 33 (нативные углы), Win10 — прямые углы | ✅ `715fa2f` |
| 3 | Меню мода: главное меню резко пропадает, закрытие — резко и без блюра | `openModsModal` вместо `display:none` у `.interface`/`.right-panel` ставит `menu-out` + `news-away` (зеркало `openSettings`); закрытие идёт через существующий `_closeWithAnim` → `_showMainInterface` → `reveal-in`/`interfaceReveal` 0.8с fade+blur без подъёма. Контракт `exp-mods-after-settings.js` переписан | ✅ `dbc51e5` |
| 4 | Очень лагает прокрутка каталога скриптов | `.ml-catalog-card`/`.ml-card-install-btn`: `transition: all` → только border-color/background-color, убраны hover `transform`/`box-shadow` (композитинг-слои на карточках под курсором); картинки каталога `loading="lazy" decoding="async"` (до ~8 МБ жадного декода рвало скролл). `.lib-card` не трогали — hover только цвета | ✅ `dbc51e5` |
| 5 | Тайминги вспомогательного текста у кнопок — 3 сек | Тултип: показ через 3000мс вместо 280мс, скрытие мгновенно (как было) | ✅ `dbc51e5` |
| 6 | В дебаг — настройка принудительного показа настроек плагина | Флаг localStorage `arp_forcePluginSettings` + `_dbgForcePluginOn`/`setForcePluginSettings`/`_syncForcePluginUI`; в `renderPatchStatusUI` ветка `if (kind)`: `#checkboxGroup` показан, если флаг включён (баннер с причиной остаётся); карточка «Плагин» (puzzle) в `#debugTab` после «Меню»; синхронизация в `loadDebugInfo` | ✅ `dbc51e5` |

**Прогоны (все PASS):** `probe-fixes.js` 16/16 (№1,4,5,6),
`exp-mods-after-settings.js` 13/13 (№3, новый контракт: menu-out,
interfaceReveal, без подъёма), `exp-verify-bugs.js`, `check-blur.js` 16/16,
`probe-mods-bake.js` 15/15, `verify-anim-fix` fade/pop/smooth/full —
22/20/20/22, `check-default-mode.js` PASS (`anim-full`). `cargo check` чистый,
`check-syntax.js` SYNTAX-OK. Скриншот `fix-window-frame.png` — окно без
самодельной обводки, углы нативные.

**Инструменты этого батча:** `probe-fixes.js` (16 проверок фиксов 1,4,5,6),
переписанный `exp-mods-after-settings.js` (13, контракт «моды как настройки»).
Прогоны — строго последовательно (параллельные на одном CDP дают флейки).

---

## 14. Раунд 08.11.2026 (вечер) — скорость возврата меню + лимит FPS в дебаге

Две жалобы с ручного теста: (1) «после закрытия настроек или модов главное меню
долго появляется», (2) «чувствую фикс FPS до 30/60 в анимациях — сделай в дебаге
настройку, сколько именно разрешить». Оба пункта закрыты, прогоны на собранном
exe PASS.

**1. Возврат меню ускорен ~2 раза** (причина — специально длинные entrance'ы
прошлых фиксов: sink 0.4с + таймер 440мс, вход `interfaceReveal` 0.8с линейно):

| Что | Было | Стало |
| --- | --- | --- |
| `.settings-panel.closing` / `.folder-modal.closing` / `.mods-modal.closing` | 0.4s | 0.26s |
| Таймеры `_closeWithAnim` (настройки/папка/моды) | 440мс | 300мс |
| Вход меню `.interface.reveal-in` (после настроек и модов) | 0.8s linear | 0.46s linear |
| Вход после модов (display:none ветка) `.interface > *` / `.left-panel` | 0.5s | 0.34s |
| Страховка `_clearStart` (снятие инлайн-стилей) | 1000/600мс | 600/450мс |
| `body.settings-anim` / `body.mods-anim` (гашение внешних блюров, запечённый слой) | 550мс | 420мс |

Замер: панель скрыта @309–332мс, меню чёткое @466–474мс (было ~440 + ~800–1200).
Логика «блюр виден, пока панель гаснет» сохранена: 0.46 > 0.3 → к моменту ухода
панели меню ещё размыто (~7px) и проявляется глазами. Открытие настроек/модов
(0.4с rise) не трогали.

**2. Лимит FPS анимаций — вкладка «Отладка», карточка «Анимации»:** строка
«Лимит FPS анимаций» (Без лимита / 120 / 60 / 30) + строка «Замер кадров»
(живой: экран N к/с • анимации M к/с). Хранение `localStorage airAnimFps`
(0 = без лимита, по умолчанию), синхронизация в `loadDebugInfo`, замер
запускается с панелью и останавливается в `closeSettings`.

Механика: веб-анимации привязаны к vsync — **выше частоты экрана не бывают**
(«Без лимита» = частота монитора), ограничение работает только вниз. Губернатор
(`_fpsTick`, rAF) берёт `document.getAnimations()` на паузу и двигает
`currentTime` квантами `1000/N` мс; кванты копятся по РЕАЛЬНОМУ времени, поэтому
длительности в стенах не меняются (300мс-таймеры закрытий синхронны с анимацией),
а визуально анимация обновляется ровно N раз/сек. Новые анимации подхватываются
на ближайшем тике, выбывшие из документа чистятся из `_fpsHeld` (запрет
случайного «отмотывания» fill-forwards). Выключение (`setAnimFps(0)`) — `play()`
всем held, цикл rAF останавливается. Замер: лимит 30 → 32 обновления/сек, без
лимита → 124 (экран ~180Гц: rAF 182/с).

**Важно про пиксельные пробы:** экран сменился на масштаб Windows **125%
(devicePixelRatio 1.25)** — `Page.captureScreenshot` отдаёт device-пиксели, и
старые калиброванные регионы (координаты при dpr=1) уезжали на 25% влево.
`exp-verify-bugs.js` (баг2) теперь умножает регионы на `devicePixelRatio` —
14/14. Латентная та же проблема у `exp-navmatrix.js`, `exp-nav.js`,
`exp-bake2/3/ab.js`, `exp-bugs2.js` — если будешь гонять, домножай координаты.

**Прогоны (все PASS):** `probe-fixes` 16/16, `exp-mods-after-settings` 13/13,
`exp-verify-bugs` 14/14 (после DPR-фикса), `check-blur` 16/16, `probe-mods-bake`
15/15, `verify-anim-fix` full/pop/smooth/fade — 22/20/20/22,
`probe-launcher-grid` 16/16, новый `probe-fps-speed.js` 23/23 (CSS-длительности,
замер закрытия ≤380/≤600мс, лимит FPS: localStorage/подсветка/замер кадров/
освобождение held), `check-default-mode` PASS. Скриншот `debug-tab-fps.png` —
новая строка на вкладке Debug.

---

## 15. Раунд 08.11.2026 (позже вечером) — зимние эффекты в дебаге

Просьба: «эффект падающего снега, виньетка морозная и темная, при наведении
на кнопки их фон как иней — всё в дебаг, т.к. ещё не декабрь». Три
независимых флага на вкладке «Отладка» → новая карточка **«Зима»**
(Выкл/Вкл); **с 1 декабря по 1 марта включаются сами**, вне сезона
выключены:

| Флаг | Хранение | Класс на `<html>` | Как сделано |
| --- | --- | --- | --- |
| Снег | `airSnow` | `winter-snow` | canvas `#winterSnow` (z=9000, pointer-events:none), движок `_snowStart/_snowTick` — 90 снежинок, падение + качка (sin), свой rAF (губернатор `airAnimFps` трогает только `document.getAnimations()` — не мешают друг другу), dt ≤50мс против телепорта после сворачивания, resize пересчитывает канвас ×dpr |
| Виньетка | `airVignette` | `winter-vignette` | `body::after` (z=9001, pointer-events:none): тёмный краевой radial-gradient rgba(3,7,13,.62) + ледяный синий отлив rgba(150,200,255,.10); НАД снегом — у краёв хлопья уходят в темноту |
| Иней на кнопках | `airFrost` | `winter-frost` | CSS `html.winter-frost button:hover`: backdrop кнопки блюрится и освечивается `blur(8px) brightness(1.3) saturate(1.45)` (фон как мёрзлое стекло) + ледяной ободок `outline rgba(214,240,255,.5)` и inset-тень. `#tbButtons` (пилюли окна) исключены — у них свой красный hover |

Классы ставит скрипт в `<head>` (до отрисовки, без мигания), как у
`anim-*`/`compact-menu`. Контроллер `setWinterFx(key, on)` +
`_syncWinterUI()` (вызывается из `loadDebugInfo`). z-уровень 9000/9001 —
под тайтлбаром (100000), тостами (99999) и feedbackFab (9999), но над
всеми панелями/модалками (≤1500).

**Прогон `probe-winter.js` (новый, 26/26 PASS):** дефолт выкл (классов
нет, canvas скрыт, rAF=0, группы «Выкл»), включение (классы+localStorage,
canvas рисует, снежинки падают, ::after с градиентом z=9001), реальный
hover через `Input.dispatchMouseEvent` (backdrop-filter появился),
перезагрузка (head-скрипт ставит классы, снег стартует сам, группы
синхронны), выключение (всё снято, rAF остановлен, иней не применяется),
сезонная матрица `__winterOn` (01.12–28.02 вкл, 01.03/30.11 выкл,
'0'/'1' сильнее сезона) и согласованность классов с
`__winterOn(store, сейчас)`. Проба нормализует localStorage в начале и
восстанавливает состояние пользователя в конце (ручные '1' ломали бы
проверку дефолта). Плюс `check-default-mode` PASS и `probe-fps-speed`
23/23. Ловушка пробы: `p.style.display === 'none'` у свежего приложения
ложно — инлайн-стиля нет (пустая строка), `openSettings()` не вызывался;
вызывать безусловно. Скриншоты `winter-menu.png`, `winter-debug-hover.png`.

**Сезон (договорённость в том же раунде, «1 декабря включайся сам и до
марта работал»)** — логика в head-скрипте, `winterOn(v, d)` висит на
`window.__winterOn` (гонит проба), состояние флага:

| localStorage | Поведение |
| --- | --- |
| `null` (не трогали) | сезон решает: включено 01.12 → 01.03, остальное время выкл |
| `'0'` (явное «Выкл») | выключено всегда — сильнее сезона |
| `'1'` (явное «Вкл») | включено, но **свёртка сезона**: первый запуск после 1 марта (мес. 3–11) снимает такие `'1'` в `null` — 1 марта гаснет, к декабрю включается снова сам |

Свёртка раз в год по маркеру `airWinterYear`; при самом первом запуске
(маркера нет) ничего не снимаем — только ставим маркер, иначе потерялось
бы только что включённое вручную. Проверить гипотезу «а что, если дата
другая» можно только функцией (системную дату CDP не меняет) — так и
сделано в пробе.

---

## 16. Раунд 08.11.2026 (ночь) — два бага обоев

Просьба: «новый файл в папке обоев не подхватывается сразу» и «меню
обоев лагает, потому что картинки показываются в полном разрешении вместо
уменьшенных превью». Оба диагностированы CDP-замерами и исправлены в
`src/index.html` (бэкенд не трогали — `get_wallpapers` сканирует папку
каждый вызов без кэша):

**Баг 1 — устаревший список.** `openWallpaperPicker()` вызывал
`loadWallpapers()` только `if (_wallpapers.length === 0)`, поэтому новый
файл в `wallpapers/` был виден только после рестарта (воспроизведено:
файл положен → переоткрытие → нет; после рестарта — есть). Фикс: пикер
всегда делает свежий скан (`await loadWallpapers()` — рендер внутри
`renderWallpaperGrid → renderWallpaperPicker`), а `loadWallpapers`
**merge-ит** уже загруженные `wp.data_url` по filename — переоткрытие не
перезапрашивает thumb (замер: 0 вызовов `get_wallpaper_thumb` на reopen,
все `data_url` на месте, shimmer не появляется). Мёртвая ветка
`renderWallpaperPicker()+refreshLucideIcons` в открытии убрана — их делает
рендер внутри `loadWallpapers`.

**Баг 2 — «полное разрешение» в меню.** Замеры показали: thumb-пайплайн
здоров — все обои отдают 500px (15–96 КБ), холод 283мс / тёплый 37мс до
всех превью, long tasks пусто, `refreshLucideIcons` дёшев. Единственная
картинка полного разрешения в пикере — карточка **«Своя»**: она рисовала
`launcherSettings.bg_data` (425 КБ, 1500×1000) фоном ячейки 189×106 и
декодировалась заново на каждом открытии. Фикс — `_customBgThumb(src)`:
уменьшенная копия 500px (canvas → jpeg 0.85) кэшируется по src, после
готовности сетка один раз перерисовывается; первый рендер показывает
оригинал (как раньше), `initLauncherSettings` прогревает копию на старте,
при `file:`/tainted canvas — fallback на оригинал. Итог: «Своя» 44 КБ /
500×333. Плюс `paint()` теперь делает `refreshLucideIcons()` только для
активной карточки (там, где добавляется иконка) — 20 → 1 вызов на
открытие.

**Прогон `probe-wp-fixes.js` (новый, 11/11 PASS):** холод (sync 11мс,
все thumb 283мс, lucide 1, long tasks нет, shimmer 0), «Своя» ≤500px,
баг 1 (файл в списке + карточка отрисована при переоткрытии), merge
(reopen: 0 thumb-запросов, все `data_url` на месте). Тестовый файл
`zzz-probe.jpeg` проба кладёт и удаляет сама. Плюс `probe-winter` 26/26,
`check-default-mode` PASS.

Замеры (полезно для будущего): прямой `get_wallpaper_thumb` — 3мс
(кэш), `get_wallpaper_full` для 6.3МБ png — 77мс без long tasks; скрытая
сетка старого стиля (`#wallpaperGrid`, `display:none`) намеренно держит
17 thumb-джобов в `_pendingThumbLoads` до включения «старого стиля» —
это не утечка, их flush делает `toggleWallpaperOldStyle(true)`.

---

## 17. Раунд 09.11.2026 — акцентный цвет: нейтральный вместо синего

Просьба: «в лаунчере дефолт цвет синий? надо белый или какой-то, а то обои
белые а кнопки нет». Причина найдена в Monet (`_pickAccentsFromPixels`):
на белых/серых обоях ВСЕ пиксели отсеиваются порогом `s<0.18`, `bins`
пусты → **fallback на зашитый синий** `0,120,212 / 0,180,255`, и этот
синий закэшировался в `monetTheme:v3:*` (у пользователя в кэше лежали
ровно `0,120,212` — подтверждено замером). Фиксы в `src/index.html`:

| Что | Как |
| --- | --- |
| Fallback вместо синего | нейтральный, контрастный к среднему тону обоев: светлым — графит `28,30,37` / `64,68,77`, тёмным — молочный `234,236,241` / `203,208,217`; порог `lum(avg) > 0.55` |
| `_boostToggleAccent` | нейтральные не красим: `s < 0.20` → только яркость (без подъёма `minS`, иначе серый превращался в случайный оттенок; порог 0.20, а не 0.10 — у «серебра» s≈0.15) |
| Статический дефолт `:root` | `--accent-rgb: 235,238,243`, `--accent2-rgb: 204,210,220` (был синий) — виден до применения обоев и в `_ensureMonetDefaults` |
| Кэш палитры | `monetTheme:v5:` (v3→v4→v5 инвалидации: синий из старого кэша больше не применяется; localStorage не чистили — просто новая версия ключа) |
| Локальные зашитые синие | переведены на переменные: `.install-step.active`, `.cre-bit-badge`, `.bk-btn-restore(+hover)`, `.custom-select` (ring, рамки dropdown, hover/selected `.cs-item`) — теперь подстраиваются под обои |

Статусные цвета (зелёный done/error, красный удаления, фиолетовый
«Устанавливается», бейджи `.cso-item-badge`) — намеренно не трогали:
это семантика, а не акцент. Текст на акценте переключает существующий
`_syncAccentInk` → класс `accent-light` (молочный/графит корректно
меняют ink).

**Прогон `probe-accent2.js` (новый, 10/10 PASS):** текущие vars (молочный
`234,236,241`, серебро `203,208,217`, не синие, кэш v5) + синтетические
ветки `_pickAccentsFromPixels`/`_boostToggleAccent` напрямую: белые обои →
графит, тёмные → молочный, цветные → своя палитра, буст нейтрального не
колоризирует, буст цветного сохраняет оттенок. Плюс `probe-winter` 26/26,
`check-default-mode` PASS.

Замер яркости текущих обоев пользователя (`pinnsaver…jpg`, 1500×1000):
6144 px, все серо-нейтральные, `avgLum 0.27` → ветка «тёмные → молочный»
(правильно: акцент2 молочный на тёмных обоях). Скриншоты
`accent-main.png`, `accent-settings.png`.

---

## 18. Раунд 09.11.2026 — компактность меню и зима переехали в настройки

Просьба: «надо делать настройки компактности главного меню, настройки
лаунчера, также настройки снега и т.п.» — перенёс обе карточки из
дебага (`debugTab`) во вкладку «Лаунчер» (`launcherTab`), сделав их
постоянными настройками:

| Что | Как |
| --- | --- |
| Секция «Компактность меню» | тумблер `.launch-item` → `setCompactMenu` (класс `compact-menu`, `arp_compactMenu`); подсказка про плотную панель и хедер в одну строку |
| Секция «Зимние эффекты» | три тумблера → `setWinterFx('snow'/'vignette'/'frost')` (классы `winter-*`, `airSnow/airVignette/airFrost`) + подсказка про сезон 01.12–01.03 и приоритет явного «Выкл» |
| Синк-функции | `_syncWinterUI`/`_syncCompactUI` теперь ставят `.checked` у новых чекбоксов (`winterSnowCheck` и т.д., `compactMenuCheck`) вместо кнопок `dbg*Group`; вызов при `switchTab('launcher')` (ветка `if (tabName === 'launcher')` — рядом с `applyGameUpdateFuture`), из `loadDebugInfo` убран |
| Дебаг | карточки «Зима» и «Меню» удалены из `debugTab` (перенос, не копия); `setWinterFx`/`setCompactMenu` и движок снега не тронуты — логика одна |
| Комментарии | head/CSS/JS-упоминания «дебаг → карточка „Зима“» → «вкладка Лаунчер» |

Тумблеры — штатный паттерн вкладки (label + `event.preventDefault()` +
`.s-toggle`), состояния читает `switchTab` при каждом открытии вкладки.

**Прогон `probe-winter.js` (переписан под новое место, 30/30 PASS):**
дефолт (4 чекбокса в DOM, зимние сняты, компакт согласован с классом) →
вкл всё (классы/`localStorage`/canvas/rAF/виньетка + чекбоксы отмечены) →
реальный hover (кандидат с `backdrop-filter:none`, `#gameCheckBtn`) →
скриншот `winter-launcher-hover.png` → reload (классы из head, чекбоксы
синхронны через `switchTab('launcher')`) → выкл (обратная цепочка) →
**новое: компактность** — реальный клик по тумблеру вкл/выкл (класс +
`localStorage` + чекбокс) и точное восстановление исходного `null/'0'` →
сезон 01.12–01.03 → хвост/финал с восстановлением состояния пользователя
(`1,1,1` и компакт `0` вернулись). Плюс `check-default-mode` PASS.

Примечание: у пользователя `arp_compactMenu='0'` — компакт выключен;
зима `1,1,1` восстановлена как была.

---

## 19. Раунд 09.11.2026 — снег: 3 плана, отталкивание от мыши, клик-всплеск

Идея дизайнера («снежинки по три плана, реакция на мышь, эффект клику
везде»). Доработан движок снега в `src/index.html` (блок «Движок снега»):

| Что | Как |
| --- | --- |
| Три плана глубины | `SNOW_LAYERS`: дальний (r 0.6–1.2, α 0.15–0.35, 8–18 px/с), средний (1.2–2.2, 0.35–0.6, 20–40), близкий (2.2–4.0, 0.6–0.95, 45–80). Всего 90 = 30×3 (`i % 3`), диапазоны не пересекаются |
| Отталкивание от курсора | `mousemove`/`mouseleave` на окне (только пока снег включён) → в tick сила `(1 − d/90px) × 1600 × repel` (repel 0.25/0.6/1.0 по планам — близкие носятся сильнее, параллакс), пружина `k=40, c=6` возвращает к базовой траектории; смещение `ox/oy` рисуется поверх падения, сброс при респавне |
| Клик-всплеск | `pointerdown` на окне **всегда** (не зависит от снега): 16 белых частиц из точки (разлёт 60–220 px/с, гравитация 300 px/с², жизнь 0.5–0.9 с, увядание alpha), cap 4 всплеска (64 частицы). Холст один на всех: `_snowSyncCanvas` держит его видимым, пока снег жив ИЛИ есть частицы, в простое инлайн сбрасывается → CSS прячет (`display:none`) |
| Общий rAF | tick сам останавливается на «нечего рисовать»; `_snowStart` вешает input-listeners под флагом `inputOn`, `_snowStop` снимает, но даёт дожить живым всплескам |

Слушатели: `pointerdown` вешается один раз на скрипте (всегда);
`mousemove/resize/mouseleave` — при включении снега. Отдельного тумблера
под всплеск нет (YAGNI) — он вместе со снегом, но живёт и без него.

**Прогон `probe-winter.js` — 39/39 PASS:** старые 30 проверок +
новые: планы 30/30/30 с непересекающимися радиусами и α; всплеск при
включённом снеге (16 частиц → умерли, снег не встал); отталкивание
(близкая снежинка уходит >2 px от курсора, пружина возвращает после
увода мыши в противоположный угол); всплеск без снега (реальный клик по
безопасной точке `elementFromPoint` → 16 частиц, холст показан инлайном,
rAF крутится → всё погасло: rAF 0, `display:none`). Плюс
`check-default-mode` PASS, `check-syntax` OK.

Состояние пользователя восстановлено: зима `1,1,1`, компакт `1`
(он его сам включил). Скриншот всплеска — `burst-shot.png`.

---

## 20. Раунд 09.11.2026 — лимит скорости: единицы, 0 = без лимита, фиксы

### 20.1. Выбор единиц скорости (просьба: «там МБ/с, надо выбор кбит… и т.п.»)

`src/index.html` (строка лимита в настройках, вкладка Лаунчер):

| Что | Как |
| --- | --- |
| Селект единиц | Кастомный `s-select` (паттерн `memInit`) рядом с полем: **Кбит/с (×125) · КБ/с (×1024) · Мбит/с (×125000) · МБ/с (×1048576)**, дефолт МБ/с. Нативный `<select id="gameUpdateLimitUnit">` скрыт CSS — он источник пунктов |
| Хранилище | Источник истины — `game_update_limit_bps` (байт/с). Плюс `game_update_limit_value`/`game_update_limit_unit` — только для восстановления UI. Legacy `game_update_limit_mb` синхронизируется: `ceil(bps/1048576)`, min 1 при >0, 0 = без лимита |
| `0 = без ограничения` | bps=0, mb=0; подсказка в UI теперь «0 — без ограничения» |
| Смена юнита | `applyGameUpdateLimitUnit()`: число пересчитывается так, чтобы **реальный лимит не изменился** (5 МБ/с → 41943.04 Кбит/с, bps тот же) — смена юнита не должна молча урезать качку |
| Legacy-фолбэк | `applyGameUpdateFuture()` нормализует старый конфиг (только `mb`) → bps/value/unit при открытии вкладки |
| Бэкенд `launcher.rs` | `game_update_limit()` читает `bps` (f64 → u64 сатурирует), fallback `mb × 1_048_576` |
| **Фикс порога** | `downloader.rs`: `rate_bytes >= 1_048_576` → **`> 0`** — раньше лимиты < 1 МБ/с (Кбит/с, КБ/с) молча игнорировались. Токен-ведро (246–264) корректно для любого rate>0 |

**Регресс-тест Rust** (`downloader.rs`, `cargo test` в vcvars64 — иначе
`cl.exe` не находится): `sub_mb_rate_throttles_download` — локальный
HTTP-сервер отдаёт 300 КБ, лимит 150 КБ/с → качка ≥1.4 c (localhost без
лимита мгновенно — по этому и ловится порог). Прошёл: 2.02 s.

### 20.2. Попап юнита не кликался: две причины (диагностика через elementsFromPoint)

1. **Обрезка `.s-section` (`overflow:hidden`)**: попап вверх от кнопки
   (140 px), а до верха секции ~117 px — обрезанная часть не проходит
   hit-test. Фикс: на время открытия попап выносится в `body`
   (`position:fixed`, координаты от кнопки, flip вниз при нехватке
   места, `min-width:0` — иначе `min-width:100%` от body = всё окно;
   закрытие по scroll/resize/Esc). Память/ep_memory не тронуты.
2. **`#settingsPanel` сам `position:fixed; z-index:1000`** — попап
   (z 45) лежал под панелью. Фикс: в open инлайн `z-index:1100`.

### 20.3. Фикс снега: «снег сбоку окна в самом начале анимации»

Баг переработки из §19: `_snowStart` спавнил снежинки **до** `_snowWake`
→ `_snowResize()`. При первом старте (запуск лаунчера) `w/h = 0` — все
90 снежинок рождались в `(0,0)`, снег шёл полосой у левого края, пока
не респавнился (для дальних планов — минуты). На прогонах не ловился:
`flakes` живут всю сессию. Фикс: `_snowResize()` перед спавном (порядок
из исходного кода). Ловушка в `probe-winter`: после reload (первый старт
в сессии) `max(x) − min(x) > w/2`.

### 20.4. Прогоны

- `probe-speed.js` (новый, 17/17): DOM/юниты/хинт, реальный клик по
  кнопке и пункту попапа, 5 МБ/с → bps, смена юнита без изменения
  лимита, 0 = без лимита, под-МБ лимит (1 Мбит/с = 125000 bps),
  legacy-фолбэк `mb=3`, восстановление настроек юзера.
- `probe-winter.js` **40/40** (+ новая проверка разброса X после reload).
- `check-default-mode` PASS, `cargo test` PASS, `check-syntax` OK.

Состояние пользователя восстановлено: лимит `5 МБ/с` (как и был),
зима `1,1,1`, компакт `1`.

---

## 21. Раунд 09.11.2026 (2) — частицы при клике, компактные строки, количество снега

Три фичи в настройках (просьба: «эффект клика вырубить… компактные
настройки… силу снега — сколько снега будет выпадать»). Дизайн
согласован в чате («д»), реализация — `src/index.html`.

### 21.1. Тумблер «Частицы при клике»

- 4-й тумблер в секции «Зимние эффекты» (`#clickBurstCheck`), паттерн
  зимних тумблеров: `onclick → toggleClickBurst()`.
- Хранилище `airClickBurst`: **дефолт включён** (`!== '0'` = вкл, ключ
  может отсутствовать). Выкл — `_spawnBurst` возвращаетсья на первой
  строке (`if (!clickBurstEnabled()) return`), pointerdown не спавнит
  ни одной частицы. Работает и без снега (как и раньше).
- `_syncBurstUI()` + `_syncSnowAmountUI()` вызываются в
  `switchTab('launcher')` рядом с `_syncWinterUI(); _syncCompactUI();`.

### 21.2. Компактные строки-тумблеры

- «Компактное меню», «Снег», «Морозная виньетка», «Иней на кнопках»,
  «Частицы при клике»: инлайн `width:fit-content; min-width:300px` —
  раньше `.launch-item` (display:flex) тянулся на всю ширину секции.
  Кнопка «Сохранить настройки» остаётся на всю (это сабмит).

### 21.3. Ползунок «Количество снежинок»

- Под тумблером «Снег»: `<input type="range" id="snowAmountRange">`
  **30–180, step 5, дефолт 90** (`SNOW_COUNT`), значение рядом
  (`#snowAmountVal`). Хранилище `airSnowAmount` (кламп при чтении).
- CSS `.s-range`: тонкая дорожка 4px, белый thumb 14px, hover scale,
  focus-visible — дефолтный range в WebView2 синий и крупный.
- Меняется на лету: `_snowSetAmount(n)` приводит `flakes` к n,
  сохраняя баланс планов **как при спавне** — цель по слою
  `floor(n/3) + (L < n%3)` (спавн — `i%3`): 45 → 15/15/15,
  90 → 30/30/30. Лишние — с конца нужного слоя, недостающие —
  спавн случайными точками по высоте (не в углу). При выключенном
  снеге `flakes` пуст — количество применится при следующем включении
  в `_snowStart` (там спавн идёт по `snowAmount()` вместо `SNOW_COUNT`).

### 21.4. Прогоны

- `probe-winter.js` **51/51** (40 старых + 11 новых): DOM тумблера
  клика и ползунка, дефолт «частицы вкл / 90», реальный клик по
  тумблеру выкл → реальный pointerdown даёт `_snow.bursts.length === 0`
  (гард в `_spawnBurst`), клик вкл → снова 16; ползунок 45 → 45
  снежинок 15/15/15, 90 → 30/30/30, кламп JS 5→30 / 500→180;
  все 5 строк-тумблеров <500px (не на всю секцию). Состояние юзера
  (зима/компакт/`airClickBurst`/`airSnowAmount`) восстанавливается.
- `probe-speed.js` **18/18 дважды подряд** (17 старых + новая «клик
  по пункту отработал»). Устойчивость прогона улучшена после сбоев:
  **(1)** нормализация юнита в МБ/с в начале — раньше прогон зависел
  от юнита в конфиге юзера (шаг «5 МБ/с» падал при Кбит/с);
  **(2)** `scrollIntoView` кнопки юнита перед кликом — скролл вкладки
  от предыдущих операций уводил кнопку за вьюпорт (y=-567) и клик
  уходил в пустоту; **(3)** ретрай до 3 раз: открытие попапа (попап
  мог быть открыт до прогона) и клик по пункту (свежие координаты +
  hit-test через `elementsFromPoint`).
  **Урок:** прогоны гонять строго последовательно — параллельный
  `check-default-mode` делает reload и сбивал открытие попапа
  (первый фейл раунда был именно из-за параллельного запуска).
- `check-default-mode` PASS, `check-syntax` OK.
- Конфиг юзера: лимит 7 МБ/с (он сам поменял с 5), юнит МБ/с,
  зима `1,1,1`, компакт `1`, `airClickBurst`/`airSnowAmount` не заданы
  (дефолты вкл / 90).

---

## 22. Ссылки

- `IDEA.md` — карта архитектуры и подводных камней (часть фактов устарела, см. §9.8).
- `README.md` — публичное описание, скриншоты, установка.
- `updatenews.txt` — черновик новости для ArzLaunchRepo (не код).
- `.agent/README.md` — инструменты скриншотов и CDP.
- Внешнее: [ArzLaunchRepo](https://github.com/halfik24/ArzLaunchRepo) (каталоги,
  патчи, новости), CDN `pc.az-ins.com`, релизы проекта.
