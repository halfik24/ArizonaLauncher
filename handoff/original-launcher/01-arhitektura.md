# 01. Архитектура оригинального лаунчера

Версия разбора: **0.0.144** (`package.json` в app.asar). Electron + Svelte.

## Точка входа и процессы

- `package.json`: `"main": "build/main.js"` — webpack-бандл 846 КБ (263 модуля).
- `build/preload.js` (2 КБ) — bridge `window.launcherAPI` (полный список методов ниже).
- Рендерер: `static/app/index.html` → `bundle.js` (379 КБ, **Svelte** + svelte-spa-router,
  маршруты `#/`, `#/mods`, `#/shop`) + `bundle.css` (2.8 МБ).
- Отдельное loader-окно: `static/loader/` — визуальный оверлей при запуске игры.

## Окна

- **Главное**: `BrowserWindow {frame:null, transparent:true, minWidth:640, minHeight:410}`,
  размеры из настройки `bounds` (сохраняются при close). Грузит локальный
  `static/app/index.html`. `webPreferences: {preload, nodeIntegration:!0, enableRemoteModule:!0}`.
- **Loader** (только для `GAMES_WITH_CUSTOM_LOADER=[arizona, arizona_staging, rodina,
  rodina_staging]`): frameless, transparent, skipTaskbar, размер 866×546 (масштаб от
  workArea/1920), `loadFile("static/loader/index.html", {query:{game,bg,logo}})` —
  чисто визуальный экран с SVG-анимацией «проявления» (feTurbulence + displacement,
  reveal 10 с), `-webkit-app-region:drag`.
- **Tray**: иконка `assets/favicon.ico` (rodina-уведомления — `rodina.ico`),
  меню «Открыть / Закрыть» (Закрыть = `quitAndInstall`, если апдейт скачан).

## Флаги командной строки

- `--elevated` — перезапуск от админа: `second-instance` с этим флагом вызывает
  `releaseSingleInstanceLock()` и закрывает исходное окно.
- `--updated` — после обновления.
- `--pcClubMode` — компьютерные клубы: `getSettings()` возвращает
  `driversAlreadyRequested:true, autoLaunchAlreadyRequested:true` (без модалок).
- `installer-name=<имя>` — имя инсталлятора парсится на предмет проекта
  (`arizona/rodina/arizonav`) → `lastSelectedGameId`, `sourceOfInstall` + GA `InstallLauncher`.

## preload-мост (полный список)

`contextBridge.exposeInMainWorld("launcherAPI", {...})`:

**send** (one-way): `openUrlInBrowser`, `closeApp`, `maximizeWindow` (в рендерере не
вызывается), `minimizeWindow`, `validateAndStartGame(gameId, ip, port, serverNumber,
additionalIps, testBranch, force)`, `validateGameFiles(gameId)`,
`repairGameFiles(gameId, settings)`, `installDrivers(list)`, `setSettings(key, value)`,
`exitLauncher(runAfter)` (**= quitAndInstall отложенного обновления**),
`openGameFolder(gameId)`, `restartAsAdmin()`, `toggleAutoLaunch(bool)`,
`requestMods(gameId)`, `installMod(gameId, modId)`, `uninstallMod(gameId, modId)`,
`sendCrashRepairModalResponse("repair"|"skip")`.

**invoke**: `getSettings(key?)` (все electron-settings), `selectDirectory(gameId)` →
`[путь]`, `getThemeAssets()`.

**on**: `progress-update` (процент, текст, «ОТМЕНИТЬ», 4-й аргумент — скорость?),
`drivers:installProgress {currentIndex,count}`, `mods:load` (каталог),
`mod:onInstallChange (modId, installed)`, `theme-assets:update`, `update-modal-visibility`,
`update-download-in-progress`, `network-error-modal-visibility`,
`repair-modal-visibility`, `crash-repair-modal-visibility`,
`restart-as-admin-modal-visibility`, `auto-launch-modal-visibility`,
`unknown-error-modal-visibility`, `update-app-version`.

Навигация наружу запрещена: `setWindowOpenHandler` → http/https → `shell.openExternal`
+ GA `news_open` → `{action:"deny"}`.

## Таймеры main-процесса

| Интервал | Что делает |
|---|---|
| при старте + 1 ч | `autoUpdater.checkForUpdates()` |
| 30 мин | theme.json → скачивание картинок → `theme-assets:update` |
| 1 ч | `getAvailableCdnIndexes` (ping.json зеркал) |
| 60 с / 5 мин | `notifications?platform=pc` / `notifications/own/<fprint>` |
| 30 с | `checkNetworkDownloadConnection` |

## Обновление лаунчера

electron-updater 5.3.0 + GitHub provider (`owner: dogoran-el, repo: arizona-launcher`,
конфиг `resources/app-update.yml` вне asar). Дифф-обновление по blockmap; при ошибке
полный инсталлятор в `Local\arizona-launcher-updater\` (+ копия в `pending\`,
update-info.json c sha512) → `quitAndInstall(silent, forceRunAfter)`. Кэш не чистится
(у пользователя 492 МБ после одного обновления). Логи updater'а — `logs.log` рядом с exe.

## Телеметрия

Google Analytics **Measurement Protocol**: `POST https://google-analytics.com/mp/collect
?measurement_id=G-YB6E89EZLT&api_secret=qQSjIgv2TnSkiSirGK56Qg`, `client_id` = machineId
(node-machine-id). События: `campaign_details, install_launcher, start_launcher,
game_download_start/stop/end/error, try_start_game, start_game_error, install_drivers,
repair_game, news_open`. Параметры: версия, cdnType, utm.

## Хранилище

Единственное состояние — electron-settings (`Roaming\arizona-launcher\settings.json`,
atomic save, prettify). Ключи: `bounds, notification, ownNotification, fprint, utm,
lastSelectedGameId, sourceOfInstall, hashmap {путь→md5}, installedMods {gameId:{modId:{...}}},
_sys {trilogyLargeFileSize}, <gameId> {...опции}, <gameId>_staging`. Полный разбор — `06-dannye-i-logi.md`.
Дополнительно: `cdncfg` (JSON) рядом с exe — override CDN-карты (для staging).
