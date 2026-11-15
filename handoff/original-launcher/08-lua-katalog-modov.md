# 08. Каталог скриптов оригинала: скачали всё и разобрали

Эксперимент 15.11: скачаны **все 54 script-мода** оригинального каталога
(50 из `mods` + 4 из `topOfMonth`, включая сам MoonLoader), 1015 файлов,
156 МБ, **md5 каждого файла сошёлся с манифестом**. Корпус:
`.agent/orig-launcher/mods-research/` (`files/<id>/...`, `meta/<id>.json`,
`summary.json`, `mods-arizona.json`) — вне git. Пруобы: `bulk-download.mjs`.

## Точная механика (уточнение к 04 — там формула URL была неверной)

1. **Каталог**: `GET https://api-samp.arizona-five.com/launcher/mods/arizona`
   (резерв `arz-api-reserve.s-amp.com/launcher/mods`) →
   `{topOfMonth:[...], mods:[...]}`. У нас: 79 модов = **50 script + 29 graphic**.
2. **JSON мода**: `GET .../launcher/mods/arizona/<id>` → `{data:[дерево]}` —
   **тот же формат, что game.json** (dir/file, `hash`, `size`, `date_change`).
3. **Файлы**: `<modsCDN>/<modId>/<относительный путь>` —
   `https://pc.az-ins.com/resource/mods/21/moonloader/Chat-Calculator v3.lua`.
   ⚠️ **без gameId** — формула из их main.js: `downloadPath = getCdnUrl(gameId,
   Mods) + "/" + modId + "/" + rel`. Резерв: `arz-pc.react-group.tech/resource/mods/<id>/...`.
   Main отдаёт 302 → `pc2.az-ins.com` (тот же путь).
4. Проверка: md5 дерева сверяется после качки (у нас сошлось у всех).
   Установка: файлы пишутся в `gamePath/<rel>` (пути приходят из дерева:
   `moonloader/`, `mods/`, корень), mtime — по `date_change`.

## Состав скачанного (1015 файл / 156 МБ)

- **Расширения**: `.lua` 467, `.txd` 430 (текстуры), `.dll` 32, `.png` 38, `.mp3` 13,
  `.asi` 7, `.ttf` 6, плюс `.ini/.cfg/.dat/.json/.wav`.
- **Корневые пути установки**: `moonloader/` 565, `mods/` 427 (текстуры),
  корень (`VehDeform.SA.asi`, `SkyGfx.asi`, `ScreenChat.asi`, `PedFuncs.asi`,
  `Hooks.asi`, `ArzAAfk.asi` + их .ini), `models/`, `data/`, `cleo/`.
- **MoonLoader (id 1)** — сам рантайм: 465 файлов, 28.7 МБ (`MoonLoader.asi`,
  `lua51.dll`, SAMPFUNCS, `lib/`, config). **Почти все скрипты имеют
  `dependencyModsIds: [1]`** — зависимость от MoonLoader.

## Скрипты (54): что за набор

Классические moonloader-скрипты сообщества: Arizona&Rodina Helper, CamHack,
Чат-Калькулятор, АвтоБазар, Money Separator (122k скачиваний), чат-улучшения,
хелперы работ (дальнобой, шахта, больница, тюрьма, рыбалка, нефть),
Mimgui ScoreBoard, Карта с районами (144 файла-тайла), + плагины .asi
(SkyGfx, VehDeform, ScreenChat, Anti-AFK). Автор — community (blast.hk),
у половины заголовки `script_name/script_author` в CP1251.

## Механика обновления (ответ на вопрос)

В мете мода `version`; лаунчер сравнивает с `installedMods[gameId][modId].version`
→ не совпало → кнопка «Обновить» + показ `changes` («Изменения в новой версии»).
Обновление = повторная установка по новому дереву (старые перезаписанные файлы
перезаписываются, список актуализируется). `changes` заполняют вручную авторы —
это заметно, они подробные.

## Что брать в наш лаунчер (выводы)

1. **Каталог — готовый источник скриптов.** `launcher/mods/arizona`,
   `category == "script"` → 54 позиции с авторами, версиями, описаниями,
   картинками, счётчиками, ТОПом. Наш `Lua/lua.txt` беднее на порядок.
   API публичный, без авторизации; файлы на публичном CDN.
2. **Установка — по дереву JSON** (как game.json): наш `game_update/manifest.rs`
   уже парсит этот формат; файлы пишутся в `gamePath/<rel>` — как раз
   `moonloader/...`. md5 даёт целостность, `date_change` — mtime.
3. **Установленное хранить списком** (`installedMods`-аналог: пути файлов +
   версия) → честная деинсталляция, детект «Обновить», показ `changes`.
4. **Зависимости**: `dependencyModsIds` — у скриптов это MoonLoader; в UI
   «требуется MoonLoader» с кнопкой установки мода 1 (он тоже из каталога).
5. **Скачивание**: main-CDN → 302 на pc2 (следовать), резерв
   arz-pc.react-group.tech; лимиты как у нас в `fetch_catalog_url`.
6. **Осторожно**: `.asi`/`.dll` в составе модов (MoonLoader, SkyGfx...) —
   это исполняемые библиотеки, ставить только осознанно; у нас уже есть
   whitelist-подход (аудит Б8). Каталожные `mods/*.txd` — чистые ресурсы.
