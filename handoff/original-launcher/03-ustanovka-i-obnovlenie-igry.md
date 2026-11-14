# 03. Установка, обновление и ремонт игры (main.js, модуль 4237)

## Манифест: game.json

`GET ${getCdnUrl(gameId, cdnType)}/game.json` (timeout 15 с) — дерево файлов:

```
{ name, type: "dir"|"file"|"res"|"check"|"delete", size, hash, date_change,
  install_as, variant_tag, data: [...], pure }
```

- `variant_tag` фильтруется по опции `NewGraphics` ("future"/"default") — вариант графики.
- `type:"res"` — не проверяется вовсе (пропуск в скане).
- `type:"check"` — проверка только хэшем.
- `type:"delete"` — удаляется, если `date_change` новее mtime локального файла.
- `data`+`pure` — чистка «лишних» файлов в каталоге (чего нет в манифесте).
- `install_as` — путь назначения (и в модах).

**Файлы**: `<CDN>/game/<относительный путь>`, качает electron-dl поштучно
(`downloadMultipleFiles`, очередь), прогресс → `progress-update` (процент,
«Загрузка файлов... [Xmb/Ymb]», кнопка ОТМЕНИТЬ), `win.setProgressBar`.
Один CDN — main; при ошибке переключение на Reserve и обратно через 10 минут.

## Верификация и инкрементальность: md5-словарь в настройках

- md5 считается потоково (crypto + createReadStream); результат кэшируется в
  настройке **`hashmap`**: `{ "<абсолютный путь>": "<md5>" }` (у пользователя
  1067 записей — полный снимок установленной игры).
- Быстрая фаза — сравнение `size` (statSync); дальше `date_change` из манифеста
  vs mtime: если серверная дата новее — md5. mtime локального файла **выставляется
  равным date_change** из манифеста (utimesSync) — чтобы следующий скан был дешёвым.
- Т.е. инкрементальность держится не на отдельном файле кэша, а на mtime-файлах +
  словаре md5 в settings.json. (Наш лаунчер: отдельный `game_update_state.json` —
  аналогичная идея.)

## Исключения из проверки (не трогает пользовательское)

`moonloader\config`, `moonloader\resources`, `mods\mods.img`, `AZVoice\...`,
CEF-кэши. При repair-модалке «Отказаться и запустить игру» можно стартовать без чинилки.

## Ремонт (repair=true)

- Удаляет `documents/GTA San Andreas User Files/<gameId>.set`.
- Сносит `Local\Arizona Games\NG` (arizona/rodina) / `...\DE` (trilogy) — кэши CEF.
- Удаляет все файлы, которых нет в манифесте, кроме защитного списка (CEF Cache,
  AZVoice, moonloader/config, moonloader/resources, mods/mods.img, пути установленных
  модов из `installedMods`).
- Первый `frontend.zip`/`frontend` — заодно чистка `LOCALAPPDATA\Arizona Games\NG\cef`.

## Место и права

- check-disk-space: `free < порог` → `DOWNLOAD_BLOCKED_ERROR` («Недостаточно места
  на диске <diskPath>»); для Trilogy отдельный порог 23 850 730 160 байт (~23.85 ГБ).
- Права записи: пробный write+unlink `<gamePath>/accessTestFile.txt`; при неудаче —
  модалка «перезапуск от администратора» (`restart-as-admin-modal-visibility`) →
  `runAsAdmin`: сначала sudo-prompt (`"${exe}" --elevated`), фоллбэк — node-powershell
  `Start-Process -Verb RunAs`.

## Crash-repair

`<gamePath>/crashlog/last_crash.json`: если `playtime` < 300 с и запись свежее часа →
модалка «Починить игру?» («Починить игру» / «Отказаться и запустить игру» — старт
с force). Если в краше виноват `arizona.asi` (`isAzasiCrash`) — при следующем старте
добавляется флаг `-after_crash_azasi`.

## Драйверы (DRIVERS_BY_PRIORITY)

Файлы ищутся в `<gamePath>/drivers/`, установка последовательная (прогресс
`drivers:installProgress {currentIndex,count}`):

| Файл | Аргументы |
|---|---|
| directx_Jun2010_redist.exe | `/Q /T:"{DIR}\DXTEMP"`, затем `DXSETUP.exe /silent` + rmdir |
| vcredist_x86_2010.exe, vcredist_x86_2013.exe, vcredist_x86.exe, vc_redist17.x86.exe, vc_redist17.x64.exe | `/quiet /norestart /install` |

Показ модалки драйверов — один раз (`driversAlreadyRequested` в настройках; в
`--pcClubMode` — сразу true). **Сравнение**: наш лаунчер драйверы ещё не делает
(§11 старой очереди) — здесь готовый список имён и флагов.

## Краевой случай из логов

При недоступном диске игры скан падает `Permission error. Elevation required` →
`Files validation error` — и лаунчер это переживает молча, продолжая кроны.
