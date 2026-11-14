# 04. Новости и каталог модов/скриптов

## Новости

**Данные — из рендерера напрямую (axios, не через bridge).**
`GET https://api.arizona-five.com/launcher/projects` (failover на
`arz-api-reserve.s-amp.com`, 5 мин; при недоступности — зашитый fallback-JSON прямо
в bundle.js + переполоска каждые 30 с; проекты также обновляются раз в час).

Структура элемента:

```json
{ "gameId": "arizona",
  "youtubeVideoUrl": "...",
  "multipliers": { "donate": 2, "experience": 2 },
  "shop": { "donateHandlerUrl": "...", "exchangeRate": ..., "bundles": [], "packs": [] },
  "trilogyAvailable": false,
  "news": [ { "title": "...", "imageUrl": "https://pc.rod-ins.com/resource/web/arizona/news/6_new_year.png",
              "url": "https://arizona-rp.com/posts/6" } ] }
```

- Карточка новости — только `{title, imageUrl, url}`. Рендер: карусель
  `.update-list > .update` (cover-картинка + название), `target=_blank` — открытие
  идёт через `setWindowOpenHandler` → `shell.openExternal` + GA `news_open`.
- Источники картинок/ссылок по проектам (fallback): Arizona — `pc.rod-ins.com/resource/web/...`
  → `arizona-rp.com/posts/N`; Rodina — `rodina-rp.com/posts/N`; ArizonaV —
  `api-samp.arizona-five.com/uploads/<ts>.jpg` → `arizona-v.com/#/desc?id=N`;
  Village/bone-country — `bone-country.ru/news/view/N`.
- **Пуш-уведомления** (не лента!): `notifications?platform=pc` — OS Notification
  Electron (заголовок/текст/проект), дедуп по id (`notification` в settings);
  персональные по фингерпринту — `notifications/own/<fprint>`.
- `plug_text` из логов в этой сборке НЕТ (загадочный 218 КБ текст в ping.json —
  заполнитель ответа, к UI не идёт).

## Каталог модов/скриптов

**Схема двухуровневая: API отдаёт метаданные, файлы — с игрового CDN.**

1. Рендерер: `launcherAPI.requestMods(gameId)` → IPC `mods:request`.
2. Main: `GET https://api-samp.arizona-five.com/launcher/mods/<gameId>`
   (Reserve: `arz-api-samp-reserve.s-amp.com/launcher/mods`) → `{topOfMonth, mods}`.
   Мета мода: `id, version, visible, flag, conflictModsIds, requiredDependencyModsIds,
   category, monthDownloads, totalDownloads, source...`.
3. Каждому моду дописывается `installed/installedVersion` из настройки
   **`installedMods`**: `{<gameId>: {<modId>: {installed, installedVersion,
   installedFilesRelativePaths, replacedFilesPaths}}}`. Моды с `visible:false` среди
   установленных — автоматически удаляются.
4. Рендерер получает всё событием `mods:load`. UI (`#/mods`): фильтры
   `ВСЕ МОДЫ / Графические моды / Скрипты·Плагины / Установленные` (поле `category`),
   сортировки `Стандарту / Популярности (monthDownloads) / Кол-ву скачиваний
   (totalDownloads)`, секция «ТОП» (topOfMonth → top:true, position), карточки
   `#mod{id}` с автором/версией/описанием/«Изменения в новой версии» (видно при
   `installedVersion !== version`), бейджи Новый/ТОП, red-dot на вкладке при новых.

**Установка** (всё в main): `installMod(gameId, modId)` →
`parseGameFilesFromJson(gameId, <modsAPI>/<gameId>/<modId>, <modsCDN>/<gameId>/<modId>, gamePath)`
— JSON мода имеет тот же формат дерева, что game.json (пути назначения `install_as`:
`moonloader/`, `lib/` и т.п.), файлы качаются с CDN (`downloadMultipleFiles`).
Перезаписанные файлы сохраняются в `installedMods` (для деинсталляции). Прогресс в UI —
без процентов, спиннер «Установка мода "..."». Деинсталляция — удаление по
`installedFilesRelativePaths`.

**Интеграция с игрой**: файлы установленных модов исключаются из проверки/repair
(`getAllModsRelativePaths`), а `flag` модов превращается в флаги командной строки
запуска игры.

## Сравнение с нашим лаунчером (смотрим, не переносим)

| Оригинал | Наш ArizonaLauncher v2 |
|---|---|
| Моды: API JSON (`launcher/mods/<gameId>`) с деревом файлов и `install_as` | Каталог `Lua/lua.txt` (строчный формат) → скрипты в `moonloader/`; библиотеки zip |
| Файлы модов с `/resource/mods/<gameId>/<modId>/...` | Файлы с GitHub ArzLaunchRepo |
| Состояние модов: settings `installedMods` + дерево JSON мода | `get_moonloader_scripts` сканирует папку + `.disabled` |
| Новости: `launcher/projects` (title/imageUrl/url) | `updatenews.txt` с GitHub (наш формат) |
| Вкладки модов: графические/скрипты/установленные + конфликты (`modsConflicts`, `requiredDependencyModsIds`) | Наши теги в каталоге; конфликты патчей — в настройках патчей |
| `topOfMonth`, счётчики скачиваний | Нет (нет бэкенда со счётчиками) |

Совпадает ключевое: **формат дерева файлов мода == формат game.json** (dir/file/res/
check/delete, install_as, hash, date_change) — наш `game_update/manifest.rs` умеет
парсить такой же; т.е. «моды» и «обновление игры» у оригинала — один и тот же движок
на двух источниках.
