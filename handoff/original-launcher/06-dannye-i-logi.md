# 06. Рабочие данные, логи, bin/

## Roaming\arizona-launcher\ (46 МБ)

```
.updaterId                 36 B   staging ID electron-updater
settings.json              97 КБ  ЕДИНСТВЕННОЕ хранилище состояния
theme-assets/              27 МБ  8 PNG + manifest.json (theme.json, каждые 30 мин)
Cache/Cache_Data           18 МБ  HTTP-кэш Chromium (картинки темы/новостей)
logs/                      ПУСТО  (логи пишутся рядом с exe, не сюда)
Local Storage/, Session Storage/, Network/ (Cookies), Preferences, Local State
```

## settings.json — все ключи

| Ключ | Значение (пример с машины) |
|---|---|
| `lastSelectedGameId` | `"arizona"` |
| `sourceOfInstall` | `"arizona"` (из `installer-name` инсталлятора) |
| `fprint` | sha256-фингерпринт (machineId) — `-userId`, ключ персональных уведомлений |
| `autoLaunchAlreadyRequested` | `true` (автозапуск уже предлагался) |
| `bounds` | `{x,y,width,height}` окна |
| `notification` | `{id:565, title, text, imageUrl, project}` — дедуп уведомлений |
| `hashmap` | **1067 записей** `"H:\Arizona Games Launcher/<путь>" → md5` — снимок игры |
| `installedMods` | `{gameId: {modId: {installed, installedVersion, installedFilesRelativePaths, replacedFilesPaths}}}` (пусто — моды не ставились) |
| `_sys` | `{trilogyLargeFileSize}` |
| `arizona` | `{gamePath:"H:\...", memory:{value:2048, values:[1024,2048,3072,4096]}, password:{value}, userName, options:[{id,value}×12], gameRepair:{}}` |
| `rodina` | аналогично, без gamePath (не установлена) |

**options**: `wideScreen, autoLogin, shitPc, oldResolution, hdrResolution, grass,
newGraphics, autoClean, preload, windowed, authCef, testBranch:{value,withSpecialBadge}`.

## Логи

`...\Arizona Games Launcher\logs.log` (+`.old`, ротация по запуску процесса, не по дню;
electron-log с resolvePath = dir(exe)). Формат `[дата] [level] сообщение`.

Жизненный цикл между запусками:

1. `Checking for update` → вердикт electron-updater (каждый час).
2. 3×`[PING ...]` по зеркалам cdn/sound/api (UA `Arizona PC/<версия>`, hash-валидация;
   в ответе ~218 КБ `plug_text` — заполнитель).
3. Строка `0,0,0` — вероятно, лог шаблона `-cdn` при пустом состоянии.
4. `[CND]: Selected CDN: <URL>` — после `GET <cdn>/game.json` (timeout 15 с).
5. md5-скан папки игры против `settings.hashmap` (падение при недоступном диске —
   `Permission error. Elevation required` / `Files validation error` — переживается молча).
6. Обновление лаунчера: blockmap-дифф с GitHub → при ошибке полный инсталлятор
   (у пользователя: `Cannot download differentially... size mismatch` ×3) →
   `quitAndInstall(silent:true, forceRunAfter:true)` через second-instance механику.

## Кэш обновлений

`Local\arizona-launcher-updater\` — **492 МБ**: `installer.exe` (246 МБ) + копия в
`pending\` + update-info.json (sha512, isAdminRightsRequired:false). Очистки нет —
каждое полное обновление оставляет ~0.5 ГБ.

## bin/

`bin/arizona` и `bin/rodina` — **пустые каталоги-заготовки** (дефолтные gamePath по
формуле `<exeDir>/bin/<gameId>`). Не «лаунчеры игр».

## Папка игры (из hashmap/логов; диск H: сейчас отключён)

1067 файлов: `data`(413), `cef`(288), `models`(147), `frontend`(40), `modloader`(33),
`audio`(32), `arizona`(15: `arizona.txd, handling.cfg, items.zip, LauncherLimits.ini,
UserLimits.ini, NamedModels.json, chat_rooms.json`), `fontcustom`(10), `drivers`(8),
`SAMP`(8), `text`(5), `realskybox`(4), `anim`(3), `cleo`(3), `neo`(2); в корне —
`arizona.asi, _CoreGame.asi, azrtc.asi, AZVoice.asi, arizona_configurator.asi,
$fastman92limitAdjuster.asi, SilentPatchSA.asi/.ini, bass*.dll, vorbis*.dll, stream.ini`.

**`moonloader` в hashmap НЕТ** — он в исключениях скана (только config/resources);
`#ArizonaPatches.json` и `preloading_plugins` оригинал вообще не пишет — управление
плагинами у него другое (см. `07-sravnenie-s-nashim.md`).
