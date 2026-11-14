# 02. Сеть и API — карта всех адресов

## Проекты (GameId)

`arizona, arizona_staging, arizonav (GTA V/RAGE-MP), arizonav_staging, village
(= bone-country, «Жизнь в деревне»), village_staging, rodina, rodina_staging, trilogy
(GTA DE)`. Bone-country как отдельный GameId НЕ существует — это Village (site
bone-country.ru в рендерере).

## Игровые CDN (master-константы main.js)

| GameId | main | reserve | mods | modsReserve |
|---|---|---|---|---|
| arizona | `pc.az-ins.com/release` | `arz-pc.react-group.tech/release` | `pc.az-ins.com/resource/mods` | `arz-pc.react-group.tech/resource/mods` |
| arizona_staging | `pc.az-ins.com/staging` | `arz-pc.react-group.tech/staging` | те же resource/mods | |
| village | `pc.az-ins.com/village` | `arz-pc.react-group.tech/village` | `pc.az-ins.com/village/mods` | |
| rodina | `pc.rod-ins.com/release` | `rod-pc.react-group.tech/release` | `pc.rod-ins.com/resource/mods` | `rod-pc.react-group.tech/resource/mods` |
| arizonav | `pc.rod-ins.com/arizonav` | `rod-pc.react-group.tech/arizonav` | `pc.rod-ins.com/arizonav/mods` | |
| trilogy | `mob.azinternal.com/trilogy` | `rod-mob.react-group.tech/trilogy` | `pc.az-ins.com/resource/launcher/mods/trilogy` | тот же |

**Recovery-фоллбэк** (для staging-без-записи-в-карте и неизвестных id):
`arizona-recovery.react.domains/desktop/{game|mods}` +
`arizona-recovery.react-group.tech/desktop/game`. Формула фоллбэка:
`${common[type]}/${gameId}`.

**cdncfg**: JSON-файл рядом с exe (ключи main/reserve/mods/modsReserve) —
override карты, читается в `getCdnUrl`.

## Основной API (failover main↔Reserve, 5 мин, timeout 15 с)

- **main**: `https://api.arizona-five.com/`
- **Reserve**: `https://arz-api-reserve.s-amp.com/`

| Путь | Метод | Роль |
|---|---|---|
| `launcher/servers` | GET | серверы: `{gameId: [{number,name,online,maxplayers,queue,icon,ip,port,additionalIps}]}`; рендерер сам, есть зашитый фолбэк |
| `launcher/projects` | GET | проекты+новости: `{gameId, youtubeVideoUrl, multipliers{donate,experience}, shop{donateHandlerUrl,exchangeRate,bundles,packs}, news[{title,imageUrl,url,createDate?}], trilogyAvailable?}`; рендерер сам, fallback зашит в bundle.js |
| `notifications?platform=pc` | GET | пуш-уведомления OS: `{id,title,text,project}` (каждые 60 с), дедуп по id в settings |
| `notifications/own/<fprint>?platform=pc` | GET | персональные (каждые 5 мин) |
| `app/metadata` | POST | при старте с `installer-name`: `{data}` → парс utm (`utm_campaign/source/medium`) → GA |

## Mods-API (отдельный failover, та же пара base)

- **main**: `https://api-samp.arizona-five.com/launcher/mods`
- **Reserve**: `https://arz-api-samp-reserve.s-amp.com/launcher/mods`

`GET /launcher/mods/<gameId>` → каталог `{topOfMonth:[], mods:[{id, version, visible,
flag, conflictModsIds, requiredDependencyModsIds, category, monthDownloads,
totalDownloads, source, ...}]}`. JSON конкретного мода: `GET /launcher/mods/<gameId>/<modId>`
— дерево файлов с путями установки (тот же формат, что game.json). Файлы мода — с
CDN `<mods-CDN>/<gameId>/<modId>/<path>`, НЕ с этого домена.

## Пинг зеркал (ping.json, UA `Arizona PC/<версия>`, timeout 5 с)

| Группа | Кандидаты |
|---|---|
| cdn (контент) | `cdn.azresources.cloud`, `resources.azgames-cdn.ru` |
| sound | `cdn.azsounds.net`, `azgamesound-cdn.ru` |
| api | `server-api.arizona.games`, `server-api.azgames-cdn.ru` |

`GET <зеркало>/ping.json` → валидность по `hash === '5aa4731d5d84e09e2f7e7141e560104f'`
(не совпал → следующий URL). Итог — индексы, уходящие в игру флагом `-cdn i1,i2,i3`.
Ответ содержит гигантский `plug_text` (~218 КБ болтовни про C++) — похоже,
заполнитель, чтобы ответ нельзя было легко подделать/сожать; к UI отношения не имеет.

## Тема

`GET https://pc.rod-ins.com/resource/pc-launcher/theme/theme.json` (+ резерв
rod-pc.react-group.tech), `?t=<Date.now()>` против кэша. Формат:
`{projects:{<gameId>:{bg, character, logo, loaderBg, loaderLogo}}}` — картинки
качаются в `userData/theme-assets` (имя `md5(url)+ext`, ≤30 МБ, только image/*,
manifest.json + cleanup лишнего), рендереру — file:// URL.

## Прочее

- `api-samp.arizona-five.com/uploads/*.jpg` — картинки новостей (fallback-данные).
- `https://google-analytics.com/mp/collect` — телеметрия (см. 01).
- GitHub: `github.com/dogoran-el/arizona-launcher/releases/download/...` — blockmap/полные
  инсталляторы для electron-updater.
- В бандле объявлены, но НЕ используются: npm-пакеты `regedit`, `electron-sudo`,
  `shell-exec`, `node-process-windows` (мёртвые зависимости; реестр пишется голым
  `REG ADD` через child_process).

## Что ходит по сети при обычном старте (из логов)

`Checking for update` → 3×ping.json (UA `Arizona PC/0.0.144`, валидация по hash) →
строка `0,0,0` (формат `-cdn`, вероятно лог при пустом состоянии) → вердикт
electron-updater → `GET ${CDN}/game.json` → `[CND]: Selected CDN: ...` → md5-скан
папки игры против `settings.hashmap`. Далее по крону: ping/обновления/тема/уведомления.
