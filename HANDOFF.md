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

### Осталось (по плану, в порядке)
3. **Пункт 3 плана:** связка свежая установка→запуск: `find_launcher_exe` для скачанного клиента, дефолт `launch_method`, быстрый чек перед `spawn` (не полный MD5-скан), авто-«починить» при нехватке.
5. **Пункт 5:** драйверы DX/VCRedist (d3dx9_43.dll в System32, реестр VC Runtimes x86, тихая установка, опционально).
- **Живая проверка портированных команд** (ручная, через `npm run dev` + Ctrl+R): пресеты/другое/moonloader-каталог/установка патчей с прогрессом/выбор фона/debug-вкладка.
- Косметика: ~22 пустых catch в JS (add console.warn), ~21 `let _ =` в Rust (заменить на явное логирование/игнор с комментарием).
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
