# HANDOFF — Передатка для будущей сессии

> **Кто это:** ArizonaLauncher — кастомный лаунчер для Arizona RP (GTA SA).
> **Автор:** halfik24 (GitHub: halfik24, Telegram: @HalfikForEveryone)
> **Задача:** Переписать Python-бэкенд на Rust через Tauri v2. Фронтенд (HTML/CSS/JS) переносится как есть.

---

## 1. Текущее состояние (2 октября 2026)

### Что уже сделано
- ✅ Старый Python-код перенесён в `old_python/`
- ✅ Frontend скопирован в `src/` (index.html, fonts, wallpapers, lucide.min.js и пр.)
- ✅ `package.json` создан (Tauri CLI v2)
- ✅ `src-tauri/Cargo.toml` создан со всеми зависимостями
- ✅ `src-tauri/tauri.conf.json` создан (окно 1285x732, min 1032x583)
- ✅ `src-tauri/src/main.rs` — точка входа Tauri, регистрация ~80 команд
- ✅ `src-tauri/src/config.rs` — **ПОЛНОСТЬЮ РЕАЛИЗОВАН**: AppConfig, AppState, load/save config
- ✅ `src-tauri/src/launcher.rs` — заглушки для ~80 команд (большинство возвращают `"Not implemented"`)
- ✅ `Cargo.lock` существует (cargo запускался, 541 пакет скачан)

### Что заблокировано / НЕ сделано
- ❌ **Нет MSVC линкера (`link.exe`)** — нужна установка **Visual Studio Build Tools** с компонентом "Desktop development with C++". Без этого `cargo check` / `cargo build` падает.
  - Rust уже установлен: stable-x86_64-pc-windows-msvc (rustc 1.97.0)
  - Node.js уже установлен: v22.16.0, npm 10.9.2
- ❌ **Папка `src-tauri/icons/` пуста** — нужны иконки для Tauri (сгенерировать из `src/icon.ico` через `npx tauri icon src/icon.ico`)
- ❌ **Frontend не адаптирован** — в `src/index.html` (~12446 строк) вызовы `pywebview.api.xxx()` нужно заменить на Tauri invoke
- ❌ **`npm install` не запускался** — нужно поставить `@tauri-apps/cli`
- ❌ **Все функции в `launcher.rs` — заглушки** (кроме config-функций)
- ❌ **`.gitignore` не создан** для нового проекта

---

## 2. Структура проекта

```
C:\Users\DNS\Desktop\ArizonaLauncher\
├── HANDOFF.md              ← этот файл
├── package.json            ← npm + Tauri CLI
├── src/                    ← FRONTEND (Tauri frontendDist)
│   ├── index.html          ← основной UI (~12446 строк)
│   ├── lucide.min.js       ← иконки
│   ├── icon.ico
│   ├── fonts/              ← Montserrat Alternates (woff2)
│   ├── wallpapers/         ← 11 встроенных обоев
│   ├── moonloader/         ← тестовые .lua скрипты
│   ├── others/             ← архивы для установки (v8 zip, patches, d3d9.dll)
│   ├── presets/            ← dapodopeV2.1.json
│   ├── updates/            ← patches.json (remote manifest)
│   ├── site/               ← сайт проекта
│   ├── configs.txt         ← каталог пресетов патчей
│   ├── moonloader.txt      ← каталог скриптов
│   ├── others.txt          ← каталог доп. файлов
│   ├── libraries.txt       ← каталог библиотек
│   ├── updatenews.txt      ← новости лаунчера
│   ├── version.txt         ← версия лаунчера
│   ├── patches_schema.json ← схема UI патчей
│   └── wizard.bmp, wizard_small.bmp
├── src-tauri/              ← RUST BACKEND
│   ├── Cargo.toml          ← зависимости (создан)
│   ├── Cargo.lock          ← есть
│   ├── build.rs            ← tauri_build
│   ├── tauri.conf.json     ← конфиг Tauri (создан)
│   ├── icons/              ← ПУСТОЙ! нужна генерация
│   └── src/
│       ├── main.rs         ← точка входа, plugin init, generate_handler (~80 команд)
│       ├── config.rs       ← ✅ ГОТОВ: AppConfig, LaunchParams, LauncherSettings, AppState
│       └── launcher.rs     ← ⚠️ ЗАГЛУШКИ: все #[tauri::command] функции
└── old_python/             ← ВЕСЬ старый Python-код (эталон для логики)
    ├── main.py             ← 3422 строки — ГЛАВНЫЙ источник логики для переноса
    ├── requirements.txt
    ├── index.html (оригинал)
    ├── .github/workflows/  ← CI/CD (deploy.yml, static.yml)
    └── ... (все остальные файлы)
```

---

## 3. Что нужно сделать (по порядку)

### Шаг 1: Установить MSVC Build Tools
```powershell
winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```
Или скачать с https://visualstudio.microsoft.com/visual-cpp-build-tools/ и выбрать "Desktop development with C++".

Проверка: `cargo check` в `src-tauri/` должен пройти без ошибок про `link.exe`.

### Шаг 2: npm install + иконки
```powershell
cd C:\Users\DNS\Desktop\ArizonaLauncher
npm install
npx tauri icon src/icon.ico
```

### Шаг 3: cargo check и починка ошибок
```powershell
cd src-tauri
cargo check
```
**Известные проблемы:**
- `launcher.rs` использует `use std::collections::HashMap` — возможно неиспользуемый импорт (warning)
- Некоторые команды принимают `Option<String>` но не используют параметр — это ок (warning)
- `tauri.conf.json` ссылается на `icons/icon.png` для tray — нужно сгенерировать

### Шаг 4: Запустить dev-режим
```powershell
cd C:\Users\DNS\Desktop\ArizonaLauncher
npm run dev
```
Должно открыться окно с index.html. Если окно открылось — каркас работает.

### Шаг 5: Адаптация frontend (index.html)
В `src/index.html` найти все вызовы Python API:
```bash
grep -n "pywebview.api" src/index.html
```

**Лучший подход — Tauri bridge (минимум правок):**
Добавить в `<head>` index.html:
```html
<script>
  // Tauri API bridge — замена pywebview.api
  if (window.__TAURI__ && window.__TAURI__.core) {
    const invoke = window.__TAURI__.core.invoke;
    window.pywebview = { api: new Proxy({}, {
      get(_, method) {
        return (...args) => {
          // pywebview передает аргументы позиционно, Tauri — объектом
          // Нужна маппинг: сконвертировать args в объект по имени параметра
          return invoke(method, argsToObject(method, args));
        };
      }
    })};
    // Маппинг имен параметров (snake_case → camelCase для Tauri)
    function argsToObject(cmd, args) {
      // TODO: таблица параметров для каждой команды
      // Например: start_game(nickname, serverData, launchParams)
      //   → { nickname, serverData, launchParams }
      return {};
    }
  }
</script>
```

**ВАЖНО:** В Tauri v2 параметры команд передаются как объект с camelCase ключами. Например Rust-команда:
```rust
pub fn start_game(state: State<AppState>, nickname: String, server_data: Option<Value>, launch_params: Option<Value>)
```
Вызывается как:
```javascript
invoke('start_game', { nickname: 'Nick', serverData: {...}, launchParams: {...} })
```
Т.е. `server_data` → `serverData`, `launch_params` → `launchParams`, `install_type` → `installType`.

### Шаг 6: Реализация функций в launcher.rs (ПРИОРИТЕТЫ)

Референс — `old_python/main.py`. Каждая функция должна повторять логику из Python.

**Приоритет 1 (базовый функционал):**
1. `start_game` — построить cmdline (Python `launch_game`, строки 288-377)
2. `get_servers` — GET arizona-ping API, кэш TTL 60s, иконки base64
3. `get_launcher_news` — GET updatenews.txt (3 fallback URL)
4. `read_patches` / `write_patches` — `#ArizonaPatches.json` (strip `//` комментарии!)
5. `check_patches_status` — проверка файлов в `preloading_plugins/`

**Приоритет 2 (обновления):**
6. `download_and_install_patches` — zip скачивание + распаковка + прогресс emit
7. `fetch_patch_presets` — configs.txt парсинг (`[section]` формат)
8. `install_patch_preset` — скачивание JSON → write_patches
9. `check_update_api` / `download_update_api` / `start_launcher_install` — Air v8 (AES zip!)
10. `fetch_patches_schema`

**Приоритет 3 (moonloader/others):**
11. `get_moonloader_scripts` / `toggle_moonloader_script` / `delete_moonloader_script`
12. `fetch_moonloader_catalog` / `install_moonloader_script`
13. `fetch_libraries_catalog` / `install_library` / `list_installed_libraries` / `uninstall_library`
14. `fetch_others_catalog` / `install_other_file` / `remove_other_file` / `check_other_installed`

**Приоритет 4 (UI/прочее):**
15. `select_game_path` / `select_bg_image` / `export_patches` / `import_patches` — диалоги (tauri_plugin_dialog)
16. `get_wallpapers` / `get_wallpaper_thumb` / `get_wallpaper_full`
17. `minimize_to_tray` — трей с меню
18. `is_gta_running` — проверка процесса gta_sa.exe
19. `get_debug_info` — расширить

### Шаг 7: Сборка релиза
```powershell
npm run build
```

---

## 4. Ключевые технические детали

### Конфиг
- Путь: `%USERPROFILE%\Documents\ArizonaLauncher\config.json`
- Формат: JSON camelCase, атомарная запись (tmp → rename)
- Уже реализовано в `config.rs`

### API endpoint'ы
| Что | URL |
|-----|-----|
| Серверы | `https://arizona-ping.react.group/desktop/ping/Arizona/ping.json` |
| Новости | `https://raw.githubusercontent.com/worteng/ArizonaLauncher/main/updatenews.txt` |
| Патчи version | `https://raw.githubusercontent.com/worteng/ArizonaLauncher/main/patches/version.txt` |
| Патчи zip | `https://raw.githubusercontent.com/worteng/ArizonaLauncher/main/patches/arizonapatches_{ver}.zip` |
| Patches manifest | `https://raw.githubusercontent.com/worteng/ArizonaLauncher/main/updates/patches.json` |
| Air v8 zip | `https://raw.githubusercontent.com/worteng/ArizonaLauncher/refs/heads/main/others/ArizonaLauncher8.0_byAIR.zip` (пароль: `1111`) |
| Presets catalog | `https://raw.githubusercontent.com/worteng/ArizonaLauncher/main/configs.txt` |
| Moonloader catalog | `https://raw.githubusercontent.com/worteng/ArzLaunchRepo/main/Lua/lua.txt` (fallback: moonloader.txt) |
| Libraries catalog | `https://raw.githubusercontent.com/worteng/ArizonaLauncher/main/libraries.txt` |
| Others catalog | `https://raw.githubusercontent.com/worteng/ArizonaLauncher/main/others.txt` |
| Patches schema | `https://raw.githubusercontent.com/worteng/ArizonaLauncher/main/patches_schema.json` |

### Игровые пути
- Auto-detect: `~/AppData/Local/Programs/Arizona Games Launcher/bin/arizona/gta_sa.exe`
- Также: `C:/Program Files/Arizona Games Launcher/bin/arizona/`
- Лаунчеры: `ArizonaLauncher8.0_byAIR.exe` > `ArizonaLauncher7.0_byAIR.exe` > `ArizonaLauncher6_byAIR.exe`
- Патчи: `{game_dir}/preloading_plugins/#ArizonaPatches.json` + `#ArizonaPatches.dll`

### Зависимости Cargo (уже в Cargo.toml)
```
tauri 2 (tray-icon), tauri-plugin-shell, tauri-plugin-dialog, tauri-plugin-fs
serde, serde_json, reqwest (json+stream), tokio (full)
log, env_logger, chrono, dirs, base64, zip, regex, tempfile, winreg, open
```
**Возможно добавить:**
- `sysinfo` — для is_gta_running / kill_all_launchers
- `image` — для resize wallpaper thumbnails (Python: Pillow)
- `zip` feature `aes-crypto` — для AES zip (Air v8, Python: pyzipper)

### Особенности Python логики (НЕ потерять!)
1. **JSON с комментариями:** `#ArizonaPatches.json` может содержать `//` — strip перед parse
2. **Zip safety:** zip-slip проверка, лимиты (5000 файлов, 200MB/файл, 2000MB total)
3. **CDN валидация:** формат `\d+,\d+,\d+`, fallback `1,1,1`
4. **Nick санитайзер:** удалить `[\x00-\x1f\x7f]`, обрезка до 20 символов
5. **Config автозаполнение:** дефолты для отсутствующих ключей (deep merge)
6. **Mojibake fix:** UTF-8→CP1251→UTF-8 для каталогов (маркер: `Ð`/`Ñ` + 0x80-0xBF)
7. **Windows Defender:** `Add-MpPreference -ExclusionPath/-ExclusionProcess` через PowerShell (не исключать корень диска!)
8. **Backups:** max 15, имя `patches_{YYYYMMDD_HHMMSS}[_label].json`
9. **DLL version:** чтение PE resource (Python: win32api.GetFileVersionInfo)
10. **Stub launcher:** Air launcher завершается с кодом 0 — это НЕ краш (poll 2 сек)
11. **Window title:** кастомный заголовок из config
12. **Resize debounce:** 400ms после последнего resize-события

---

## 5. Полезные команды

```powershell
# Проверка компиляции
cd C:\Users\DNS\Desktop\ArizonaLauncher\src-tauri
cargo check

# Dev mode
cd C:\Users\DNS\Desktop\ArizonaLauncher
npm run dev

# Иконки Tauri
npx tauri icon src/icon.ico

# Релиз-сборка
npm run build

# Rust обновление
rustup update
```

---

## 6. Git

- Репозиторий: https://github.com/halfik24/ArizonaLauncher
- Ветка для Rust: создать `rust-rewrite` (или как назовёт пользователь)
- Коммитить по шагам
- `.gitignore` для нового проекта (создать в корне):
```
node_modules/
src-tauri/target/
src-tauri/gen/
*.log
```

---

## 7. Контекст

- **Пользователь:** halfik24, общается по-русски, любит когда объясняют что и почему
- **Лаунчер:** Arizona RP (GTA SA MP), Python-версия была v1.8.2, Rust будет v2.0.0
- **Референс:** `old_python/main.py` (3422 строки) — **главный источник истины**
- **Frontend:** `src/index.html` (12446 строк) — переносится БЕЗ изменений логики, только API вызовы
- **Принятые решения:** фронт как есть (не разбивать), Tauri v2, структура old_python/ + src/ + src-tauri/
