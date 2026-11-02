# Arizona RP Launcher v2

[![CI](https://github.com/halfik24/ArizonaLauncher/actions/workflows/build.yml/badge.svg)](https://github.com/halfik24/ArizonaLauncher/actions/workflows/build.yml)
[![Rust](https://img.shields.io/badge/Rust-1.77.2+-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri_2-green.svg)](https://tauri.app)
[![Platform](https://img.shields.io/badge/platform-Windows_10/11-blue.svg)](#требования)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Status](https://img.shields.io/badge/status-beta-blueviolet.svg)](https://github.com/halfik24/ArizonaLauncher/releases)

> Лаунчер для Arizona RP на Rust + Tauri 2. Полная переработка v1 на Python/pywebview: нативный бэкенд, быстрый запуск, установщик без прав администратора.

## Установка

1. Откройте [Releases](https://github.com/halfik24/ArizonaLauncher/releases)
2. Скачайте установщик `Arizona RP Launcher_<версия>_x64-setup.exe`
3. Запустите — установка занимает пару секунд, **права администратора не нужны**
4. При первом запуске откроется мастер настройки (путь к игре, обои)

<a name="требования"></a>
**Требования:** Windows 10/11 x64, WebView2 (встроен в Windows 11; для 10 ставится вместе с установщиком).

**Поддерживается только лаунчер Air v8** (`ArizonaLauncher8.0_byAIR.exe`) — единственная стабильная версия.

## Возможности

<details>
<summary><b>Запуск игры</b></summary>

- Интеграция с Air v8 (native и air режимы запуска)
- Авто-определение пути к игре при первом запуске (онбординг)
- Профили с разными настройками, сохранение никнейма и сервера
- Параметры: память, CDN, оконный режим и др.

</details>

<details>
<summary><b>Патчи (ArizonaPatches)</b></summary>

> Патчи пока сырые: их применение может ломать игру. Пользуйтесь на свой риск, бэкапы создавайте.

- Редактор настроек с дифф-просмотром
- Импорт/экспорт конфигураций в файл
- Автобэкапы: список, восстановление, удаление в два нажатия
- Пресеты готовых конфигов

</details>

<details>
<summary><b>Каталоги (обновляются с GitHub, без перевыпуска лаунчера)</b></summary>

- Луаскрипты с установкой одной кнопкой
- Библиотеки MoonLoader (mimgui и др.)
- Дополнительные файлы (DxVK и др.)

Источник каталогов: [halfik24/ArzLaunchRepo](https://github.com/halfik24/ArzLaunchRepo)

</details>

<details>
<summary><b>Интерфейс</b></summary>

- Свой обрамлённый экран: рамка окна в стиле лаунчера, перетаскивание фона, сворачивание в трей
- Кастомные обои: встроенная коллекция + свои изображения
- Новости лаунчера с отметкой прочитанного, онлайн серверов в реальном времени
- Статус-бар, профили, кнопка обратной связи (форма в Google Forms)

</details>

<details>
<summary><b>Поддержать разработку</b></summary>

Хотя лаунчер распространяется бесплатно и с открытым исходным кодом (Open Source), разработка требует времени и ресурсов. Любая финансовая поддержка помогает мне откладывать деньги на оборудование и дальнейшее развитие проекта.

**Реквизиты для донатов:**

1. **Bitcoin (BTC):** `bc1pfk8stmu8ttutwvcegg8hkdaaqfs3t83wrlc4fnv7a934mac59m6qnrx5v3`
2. **Ethereum (ETH):** `0x9a7f1a6f1E2F18c6493aB1B51A742aB161507Ba3`
3. **Solana (SOL):** `HAkjcc35TKD4txDjtQ75jZBPk4eNtecGtAW45a78SCeN`

</details>

## Релизы

CI собирает установщик на каждый push в `rust-rewrite`/`main` и на каждый тег:

| Вид тега | Пример | Что происходит |
| --- | --- | --- |
| Стабильный | `v2.0.0-beta.123` | Сборка по тегу, версия берётся из тега |
| Ночной | `nightly-20261020` | Сборка со штампом версии по времени (хранение артефакта — стандартные для GitHub 90 дней) |

Установщики не имеют цифровой подписи: Windows SmartScreen может показывать предупреждение при первом запуске — это ожидаемо для сборок без сертификата.

Стабильные теги `v*` публикуются на вкладке [Releases](https://github.com/halfik24/ArizonaLauncher/releases) с установщиком в файле релиза.

## Сборка из исходников

Нужны [Rust](https://rustup.rs) (стабильный), Node.js 20+ и [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/).

```bash
npm install          # зависимости
npm run dev          # dev-сборка с горячим обновлением
npm run build        # релиз + установщик (src-tauri/target/release/bundle/nsis/)
```

`npm run build` штампует номер сборки по дате и времени и всегда возвращает базовую версию — рабочее дерево остаётся чистым.

Тесты бэкенда: `cargo test` в `src-tauri/`.

## Диагностика

Лог: `Документы\ArizonaLauncher\launcher.log` (кнопка «Открыть лог» в лаунчере).

Структура проекта, известные проблемы и планы — в [IDEA.md](IDEA.md).

## Лицензия

MIT — см. [LICENSE](LICENSE).
