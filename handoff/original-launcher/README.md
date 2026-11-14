# Оригинальный Arizona Games Launcher — полный разбор

Изучена установленная копия на машине пользователя:
`C:\Users\halfik\AppData\Local\Programs\Arizona Games Launcher`,
версия **0.0.144** (Electron, автор dogoran-el, репозиторий
`github.com/dogoran-el/arizona-launcher`). Распакованный код лежит в
`.agent/orig-launcher/asar-extracted/` (вне git; как переполучить — ниже).

**Документы:**

| Файл | Содержание |
|---|---|
| `01-arhitektura.md` | Electron-архитектура: окна, preload-мост, IPC, updater, трей |
| `02-seti-i-api.md` | Карта всех URL/CDN/API и их роли |
| `03-ustanovka-i-obnovlenie-igry.md` | Манифест game.json, md5-словарь, скачивание, ремонт, драйверы |
| `04-novosti-i-mody.md` | Новости (projects/уведомления) и каталог модов/скриптов |
| `05-zapusk-igry.md` | Запуск SA-MP / ArizonaV (RAGE-MP) / Trilogy: флаги, реестр, -cdn |
| `06-dannye-i-logi.md` | settings.json, Roaming, кэш обновлений, логи, bin/ |
| `07-sravnenie-s-nashim.md` | Что совпадает с нашим лаунчером, что другое, где что подсмотреть |

## Как устроен в двух словах

Electron-приложение (Svelte-рендерер, локальный UI без remote URL). Состояние —
целиком в `settings.json` (electron-settings) в Roaming. Игра ставится в
`<exeDir>/bin/<проект>` или в выбранный пользователем путь; файлы сверяются
с манифестом `game.json` с CDN (тот же, что у нашего game_update), хэши
хранятся в настройках (`hashmap`, 1067 записей у пользователя). Моды —
отдельный API (`api-samp.arizona-five.com/launcher/mods/<gameId>`), файлы с
`/resource/mods` CDN, пути установки приходят из JSON мода. Новости —
`api.arizona-five.com/launcher/projects` (+ зашитый fallback в бандле).
Обновление самого лаунчера — electron-updater на GitHub releases
(dogoran-el/arizona-launcher). Телеметрия — Google Analytics MP.

## Как переполучить распаковку

```
cd .agent/orig-launcher
npm i @electron/asar
npx asar extract "C:\Users\halfik\AppData\Local\Programs\Arizona Games Launcher\resources\app.asar" ./asar-extracted
```

## Методика разбора

Три независимых прохода (агенты): main-процесс, рендерер, рабочие данные/логи.
Всё фактическое — с фрагментами-доказательствами из бандлов; места, где
вывод сделан по косвенным признакам, помечены в файлах как «вывод» и
собраны в список «не подтверждено» внутри каждого документа. Отдельные
загадки (`plug_text` из логов) не имеют отношения к этой сборке — отмечено.
