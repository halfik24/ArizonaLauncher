# 05. Запуск игры (main.js, модуль 4237)

Вход: `launcherAPI.validateAndStartGame(gameId, ip, port, serverNumber, additionalIps,
testBranch, force)` (7-й аргумент — force/пропуск repair, из модалки «Отказаться и
запустить игру»). Точка старта: если `percent===100` и не `driversAlreadyRequested`
→ сначала модалка драйверов.

## SA-MP-семейство (arizona/rodina/village + staging)

1. Убийство процессов: `tasklist` → `taskkill /F /IM <имя> /T` для
   `["gta_sa.exe","gtasa.exe","enb.exe","gta-sa.exe"]`.
2. **GameUX-фикс реестра**: `REG QUERY "HKCR\Local Settings\Software\Microsoft\
   Windows\GameUX\ServiceLocation"`; при отсутствии — `REG ADD ... /v Games /t REG_SZ /d ""`;
   перед каждым запуском — `REG ADD ... /f /d` (сброс значения `Games`; npm `regedit`
   НЕ используется — голый exec).
3. **Резолв сервера**: `dns.lookup(ip, {family:4})`; если провалился или вернулся
   127.0.0.1 — TCP-ping (npm `ping`, probe, timeout 10) по `additionalIps` из
   `launcher/servers`, берётся живой.
4. **startup.cfg** (UTM/referrer): мержит `{"referrer": <utm>}` в `<gamePath>/startup.cfg`
   (у Village — пусто).
5. **Команда запуска** (`cwd` = папка игры; при after-crash — `start cmd.exe /K "..."`):

   ```
   "<exe>" -c -h <ip> -p <port> <флаги> -cdn <i1,i2,i3>
   ```

   Флаги из настроек проекта (`options`):
   `-n <ник>`, `-mem <память>`, `-window`, `-modern_scale` (инверсия Windowed),
   `-x` (AutoLogin), `-widescreen`, `-z <пароль>` (только staging/TestBranch),
   `-ldo` (Preload), `-trees_new` (NewGraphics/Graphics), `-t` (ShitPc),
   `-cef_dirty_rects`, `-auth_cef_enable` (CefAuth), `-enable_grass`, `-16bpp`
   (OldResolution), `-allow_hdr`, `-arizona|-rodina|-village|-gtav` (маркер проекта),
   `-userId <fprint>`, `-old_n <старый ник>`, `-after_crash`, `-after_crash_azasi`,
   + флаги установленных модов (`flag: true` в installedMods).
   `-cdn` — индексы выбранных зеркал из ping.json (или `0,0,0`).

6. **Loader-окно** вместо главного (для arizona/rodina + staging): query `{game,bg,logo}`
   из theme.json; закрывается main'ом.
7. **Watcher**: цикл tasklist с фильтром `Imagename eq ... / Status eq RUNNING`;
   «в игре» = окно с title ≠ «device selection»; лаунчер `hide()` при старте,
   `show()` при выходе; если игра не появилась за 30 с — лаунчер возвращается.

## ArizonaV (GTA V, RAGE-MP)

Реестр `HKCU\SOFTWARE\RAGE-MP`: `launch2.ip`, `launch2.port`, `launch2.params = "launcher"`,
`rage_path = <gamePath>`, затем `"<gamePath>\updater.exe" -gtav`.

## Trilogy (GTA: DE)

- kill `SanAndreas.exe`, `Gameface.exe`.
- Если `<gamePath>/Game/Gameface/Content/Paks/gta.pak` не собран (размер ≠
  сохранённого `_sys.trilogyLargeFileSize`): проверка места ≥ 23.85 ГБ →
  `FilesConsolidator.exe "./GameArchive/" "./Game/Gameface/Content/Paks/gta.pak"`
  с прогрессом по stat'у (шаг 208 507 301.6 байт = 1%) → размер в `_sys`.
- Сезоны: rename `Snow.pak` ↔ `Snow.pak-disabled` в `...\Paks\Mods\`.
- Запуск: `"<gamePath>\Client\GameLauncher.exe" "<ник>" "<serverId-1>" "" "<cdn-индексы>"`.

## Путь к игре

`<настройка gamePath>` или по умолчанию `<exeDir>/bin/<gameId>` — каталоги `bin/`
у пользователя пустые-заготовки (реальная игра в `H:\Arizona Games Launcher`).

## Полезные отличия от нашего запуска

- `-cdn` — выбор зеркал средствами игры (мы в game_update выбираем CDN сами).
- `-userId` (machineId-фингерпринт) и `-old_n` (предыдущий ник) — у нас нет.
- `-after_crash` / `-after_crash_azasi` — автоперезапуск после краша с repair-модалкой.
- GameUX-фикс реестра перед каждым запуском.
- Резолв сервера с фоллбэком на additionalIps через TCP-ping.
