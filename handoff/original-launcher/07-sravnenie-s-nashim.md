# 07. Сравнение с нашим ArizonaLauncher v2 (наблюдения, НЕ переносим)

Наша реализация: `handoff/HANDOFF.md` §24 (карта кода). Здесь — факты совпадений
и отличий; это заметки для будущего, ничего из этого сейчас в наш код не переносится.

## Совпадает почти один-в-один

| Механизм | Оригинал | Наш |
|---|---|---|
| CDN игры | `pc.az-ins.com/release` + резерв `arz-pc.react-group.tech/release` | **те же** (engine.rs:92-93) |
| Манифест | `<CDN>/game.json` — дерево dir/file/res/check/delete, hash, date_change, variant_tag | **тот же формат**, наш `game_update/manifest.rs` |
| Верификация | size → date_change → md5, mtime выставляется по манифесту | size → date_change → md5-воркеры (отменяемые) |
| Кэш хэшей | `settings.json → hashmap` (путь→md5) | отдельный `game_update_state.json` |
| Резервный CDN | переключение main↔reserve, обратно 10 мин | фоллбэк в downloader (битый md5 → другой CDN) |
| Не трогает | moonloader/config, moonloader/resources, mods/mods.img, AZVoice | `get_moonloader_scripts`/repair — аналогичные защиты |
| Место/права | check-disk-space + accessTestFile → UAC | free_space (256 МБ запас) + access_test |
| Список серверов | `api.arizona-five.com/launcher/servers` | `get_servers` (наш формат, stale-while-revalidate) |

## Устроено по-другому

| Тема | Оригинал | Наш |
|---|---|---|
| Технология | Electron + Svelte, состояние в settings.json | Rust/Tauri 2, config.json атомарно |
| Плагины | Никаких манифестов: всё через .asi в корне игры + флаги модов в CLI | `preloading_plugins` + `#ArizonaPatches.json` (наша система патчей — собственного аналога в оригинале НЕТ) |
| Моды | API `launcher/mods/<gameId>` → JSON мода (формат game.json) → CDN `/resource/mods/...`; деинсталляция по списку файлов | Каталог `Lua/lua.txt` → скрипты в moonloader; libs zip из GitHub |
| Новости | `launcher/projects` (title/imageUrl/url) + OS-уведомления (`notifications`, дедуп по id) | `updatenews.txt` + рендер внутри UI |
| Обновление лаунчера | electron-updater + GitHub releases (blockmap) |Наш цикл: сборка/теги CI |
| Тема/брендинг | theme.json (bg/character/logo/loader*) + data-theme акценты | Monet из обоев |
| Выбор зеркал для ИГРЫ | `-cdn <i1,i2,i3>` (ping.json, hash `5aa4731d...`) | Наш -cdn в launch params (CDN-свитчер в настройках) |
| Фингерпринт | machineId → `-userId`, персональные уведомления | нет |
| Краш-политика | crashlog/last_crash.json → модалка «Починить?» → `-after_crash` | нет |
| Драйверы | DRIVERS_BY_PRIORITY: DirectX Jun2010, VC++ 2010/2013/2015+17 (x86/x64), `/quiet /norestart` | не реализовано (в очереди) |
| Multi-проект | 9 GameId (arizona/rodina/village/arizonav/trilogy+staging), cdncfg override | один проект |

## Идеи «на подумать» (не делаем без запроса)

1. **Новости-карточки `{title,imageUrl,url}`** — формат проще нашего текстового
   updatenews.txt; их API `launcher/projects` публичный и у нас уже досягаем (CDN тот же).
2. **Моды = game.json-дерево** — наш `game_update` движок мог бы качать их модификации
   без нового кода (формат совместим).
3. **OS-уведомления с дедупом по id** и персональные по фингерпринту.
4. **Crash-repair цикл** (last_crash.json + playtime<300с + модалка).
5. **Драйверы**: готовый список имён файлов и флагов установки (см. 03).
6. **`-cdn` с индексами зеркал** — игра сама умеет переключать CDN-контент.
7. **cdncfg** рядом с exe — механизм override CDN для отладки.

## Красные флаги оригинала (мы так не делаем)

- Телеметрия GA с api_secret в бинаре; machineId как client_id.
- Пароль сервера в открытом виде в settings.json.
- 492 МБ мусора в кэше обновлений; логи без ротации по дню.
- Ник-валидация `^[a-z0-9_]{3,20}$` (кириллица только подсвечивается) — у нас
  sanitize на бэке.
- Игровые процессы убиваются без разбора (`taskkill /F /IM gta_sa.exe /T`) — у нас
  kill_all_launchers по списку.

## Где лежат распакованные исходники

`.agent/orig-launcher/asar-extracted/` (вне git). Переполучение — README.md.
