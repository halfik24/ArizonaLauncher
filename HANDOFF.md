# HANDOFF — ArizonaLauncher (ветка rust-rewrite)

## Состояние на момент передачи

### Сделано и закоммичено (последний коммит — см. `git log`, тема: порт 10 заглушек)
- **Порт всех 10 заглушек `launcher.rs` из `old_python/main.py`:**
  - `download_and_install_patches` — скачивание zip с GitHub (0..80% прогресса), распаковка только `#ArizonaPatches.json`/`#ArizonaPatches.dll` в `preloading_plugins` (защита от zip-slip, лимит файлов), удаление старой `.dll.1`, запись `patches_version`/`patches_last_check`, стадии `download→extract→done/error` через новое событие `patches-install-progress`.
  - `install_patches_file` (b64 → plugins + бэкап `before_drop`), `install_lua_file` (b64 → moonloader), `fetch_patch_presets`/`fetch_others_catalog` (primary `halfik24/ArzLaunchRepo` → fallback `worteng`), `install_patch_preset` (скачивание, вычистка // комментариев, вложенный формат `{"patches":{}}`, `write_patches_inner`), `fetch_patches_schema` (валидация categories+settings, кэш `static Mutex`), `install_other_file`/`remove_other_file`/`check_other_installed` (+ helper `resolve_dest`: root/asi/cleo/plugins/moonloader), `select_bg_image` (blocking_pick_file → data URL).
- **Хардкоды исправлены:** `check_launcher_update` (реальная логика через `install_action_for_folder`, включая ветку `error`), `check_tray_support` (`app.tray_by_id("main").is_some()`), `get_debug_info` (все поля фронта: patches_path, backups_count, profiles_count, active_profile_name, launch_params_count/list, moonloader_status/scripts, os_version через `RtlGetVersion`, python_version; `launcher_version` через `detect_launcher_version` вместо хардкода), вкладка Debug label «Python» → «Рантайм».
- **`get_wallpaper_thumb`** — реальный ресайз до 500px (crate `image 0.25`, jpeg/png/webp/bmp; PNG с альфой, иначе JPEG q85), fallback на оригинал при ошибке декодирования.
- **Ложный успех убит:** helper `demoGuard(cmd)` в index.html — в Tauri-приложении (`window.__TAURI__`) демо-ветка бросает ошибку вместо фейкового «Готово»; в обычном браузере демо работает как раньше. Применён в ~10 местах (start_game, install/delete moonloader, install_library, install_patch_preset, save_launcher_settings, install_patches_file, install_lua_file, install/remove others, quick-install).
- **Бридж:** `install_game_to_folder` добавлен в CMD_PARAMS (`["folder","future"]`) и KNOWN (починка живого бага: команда была в main.rs, но не в бридже → `undefined` → фронт молчал); подписка на события обобщена в `listenEvent` → теперь слушает и `patches-install-progress` → `window._onPatchesInstallProgress`.
- **Warnings:** `cargo check` — **0 предупреждений** (было ~18): убраны unused import/hash_total/current_flat, `reason` теперь логируется в Delete, `files_done/files_total` показываются в сообщении прогресса (`файлы X/Y, ...`).
- **Версия выровнена:** `Cargo.toml` `2.0.0-rc1` → `2.0.0-rc1-new` (согласовано с tauri.conf.json/package.json); NSIS; `additionalBrowserArgs` без 9222.
- Проверки: `cargo check` ok (0 warnings), JS-синтаксис inline-блоков index.html ok (node `new Function`).
- **Косметика:** ~14 пустых catch при вызовах бэкенд-API заменены на `console.warn` (localStorage/тикерные оставлены пустыми — безобидны); `let _ =` у emit-прогресса и `update_config(game_path)` заменены на логирование (раньше молчаливая невозможность записать game_path при логе «записан»).

### Живой прогон через CDP — прогнан (18.10, `node tools/cdp.js` + `tools/smoke-expr.js`)
- Обе подписки событий установились (`launcher-install-progress`, `patches-install-progress`).
- Команды живые: `fetch_patch_presets` (1 пресет), `fetch_patches_schema`, `fetch_others_catalog` (1 запись DxVk), `get_debug_info` (все поля: v2.0.0-rc1-new, Windows 10.0 build 26300, v8, moonloader 13 скриптов), `check_tray_support` (true), `check_launcher_update`, `check_other_installed`, `get_wallpapers`, `get_wallpaper_thumb` (JPEG-превью реально сжато), `get_launch_method`/`get_saved_data`.
- **Пресет установлен и откачен:** `install_patch_preset(Dapo)` → `Установлено 24 настроек` + бэкап; `restore_patches_backup` вернул исходные 10 ключей. Связка install→backup→restore работает.
- **«Другое» круговым тестом:** `install_other_file(d3d9.dll)` → `installed:true` → `remove_other_file` → `installed:false` (файл в `G:\Arizona Games Launcher` удалён, не остался).
- **Путь ошибки патчей честный:** zip сейчас не выпущен (404 в обоих зеркалах, `updates/patches.json` → `released:false`) → `download_and_install_patches` вернул `success:false` + события `download→Пробую зеркало...→error`, фронт получил понятный текст.
- **Исправлено найденное в живом прогоне:**
  1. **BOM-баг:** primary `others.txt` отдаётся с UTF-8 BOM → первая строка `[file]` не парсилась, каталог пустой и **fallback не пробовался**. Фикс: `decode_text_bytes` срезает BOM + пустой парс зеркала не принимается (в `fetch_others_catalog`).
  2. **Даты бэкапов:** `fs::copy`/`shutil.copy2` сохраняют mtime источника → у свежего бэкапа дата в листинге была старой (в Python та же особенность). Фикс: `backup_display_date` читает timestamp из имени файла; сортировка и лимит 15 штук — по имени (по mtime свежий бэкап мог бы первым удаляться).
  3. **Зеркала патчей:** `patches_download_urls` = ArzLaunchRepo + worteng в `download_patches_zip` (событие «Пробую зеркало...»), `check_patches_available` (HEAD по всем зеркалам, различает «не выпущено» / «нет сети»), `get_patches_latest_version` (`version.txt`); 404 → текст «Архив патчей не найден... возможно, обновление ещё не вышло».
  4. `os_name` в debug: «windows» → «Windows» (`os_display_name`, как `platform.system()` в Python).
- Методика: `--remote-debugging-port=9222` временно в tauri.conf.json → авто-rebuild → `node tools/cdp.js '<выражение>'`. **Ловушка PowerShell:** одинарный аргумент с двойными кавычками ломается — писать выражение в файл (см. `C:\Users\halfik\AppData\Local\Temp\opencode\expr_*.js`) и подавать `Get-Content -Raw`; в самих выражениях — только одинарные кавычки.

### Пункт 3 плана — сделан: связка «свежая установка → запуск»
- **Быстрый чек перед spawn** (`quick_game_check` в `start_game`): наличие + ненулевой размер 9 ключевых файлов по манифесту CDN (gta_sa.exe, arizona.asi, anim/cuts.img, models/gta3.img/peds.img/vehs_7.img, SAMP/SAMP.img, frontend.zip, data/script/script.img) — без MD5, миллисекунды. Нехватка → код `FILES_MISSING` со списком.
- **Авто-починка** (фронт): по `FILES_MISSING` launch-оверлей показывает «авто-починка...», без confirm вызывается `repair_game_api` + `_gameProgressStart('Починка')`; если уже идёт установка — уведомление «дождитесь». Карта `START_ERRORS` пополнена.
- **Связка после установки** (пост-хук `game_update`): помимо `game_path` пишется `launcher_path` (через `find_launcher_exe`) и дефолт метода запуска: лаунчер в папке есть → `air` (иначе прелоады не внедряются), нет → `native`. Работает только для свежей установки (`write_path_target`), «Обновить» не трогает выбор пользователя.
- **Живой тест:** песочница `%TEMP%\opencode\qcheck` (только dummy gta_sa.exe) → `set_game_paths` → `start_game` вернул `FILES_MISSING` + 8 отсутствующих файлов, spawn не выполнялся (чек стоит до kill/spawn) → пути откачены, конфиг как был.

### Рестайл профилей + ник в профиле — сделано (в рабочем дереве, не закоммичено)
- **Никнейм:** поле в создании (`#cpNickname`) и редактировании (`#editProfileNickname`); хранится в `profile.nickname`; показывается чипом в списке и `@ник` в hover-меню. `applyProfile` применяет ник только если поле непустое (старые профили не трогает) — как и `launch_method`.
- **Создание профиля:** нативный `prompt()` заменён модалкой `#createProfileOverlay` (стиль `cre-*`, поля «Название»+«Никнейм»); снапшот вынесен в `_createProfileFrom(name, nickname)`.
- **Рестайл под язык настроек:** `.profiles-modal` (blur 24px/sat 1.2, бордер 0.1, radius 18px, тень 0 8px 40px), `.profile-item` (bg 0.03/бордер 0.06/radius 12px), `.profile-icon` и `.ph-item-icon` (30/28px, radius 7–8px, bg 0.06), кнопки — как `.s-btn` (bg 0.07, бордер 0.1, radius 8px). Hover-меню — фон `rgba(14,16,24,0.96)`, бордер 0.08, radius 16px.
- **Бэкенд не менялся** (профили — generic JSON в конфиге).
- Проверки: JS-синтаксис обоих inline-блоков `index.html` — `node --check` ok. UI-прогон не делался.

### Осталось (по плану, в порядке)
5. **Пункт 5:** драйверы DX/VCRedist — в манифесте CDN уже лежит `drivers/` (directx_Jun2010_redist 100МБ, dxwebsetup, vc_redist 2010-2017 x86/x64): d3dx9_43.dll в System32, реестр VC Runtimes x86, тихая установка, опционально.
- Живой прогон UI-кликов (вкладки «Моды»/«Патчи» глазами, установка moonloader-скрипта, установка патчей после их релиза) — команды проверены поштучно, клики по UI не проверялись.
- Старое (2a/2b из прошлого HANDOFF): живой тест установки в пустую папку — не дошёл до конца из-за Ctrl+R; повторить.

## Важные детали
- **`--remote-debugging-port=9222` для CDP-тестов в конце снова убран** из `tauri.conf.json` (добавлять временно для тестов и убирать перед коммитом).
- dev: `npm run dev` пересобирает только backend при изменении конфига; **фронт (src/index.html) не перечитывается автоматически — Ctrl+R в окне** (или `node tools/cdp.js --reload`). Живые тесты через CDP все упёрлись в это — проверять, что JS-правки реально загружены.
- `game_path` = путь до `gta_sa.exe`; папка манифеста = родитель; CDN `https://pc.az-ins.com/release` (+резерв), файлы `{cdn}/game/{rel}`.
- Тестовые артефакты `fl_empty_test` (Рабочий стол + `%TEMP%\opencode`) созданы и удалены; временные скрипты `.agent/flstep1*` и др. удалены — остались только переиспользуемые (`flcap.ps1`, `flscreen.mjs`, `flshots.mjs`, `gu_*.mjs`).
- Отвечать кратко по-русски; локальные коммиты без пуша; общение в режиме «продолжать без вопросов».

## Передача и важные подробности окружения
- shell-инструмент здесь называется `shell` — иное имя даёт ошибку «No tool named ...» на каждой попытке.
- UIAutomation-кодировка — PS 5.1 с ANSI: если scripting PS-файл содержит русские литералы (кнопки, заголовки диалогов), может молча не найтись; лучше писать только ASCII-матч и/или искать диалог по `gta_sa` внутри заголовка.
