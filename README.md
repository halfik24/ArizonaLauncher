# Arizona RP Launcher v2

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-2-green.svg)](https://tauri.app)
[![Version](https://img.shields.io/badge/version-2.0.0%20rc1-blue.svg)](https://github.com/halfik24/ArizonaLauncher/releases)
[![Status](https://img.shields.io/badge/status-RC-brightgreen.svg)](https://github.com/halfik24/ArizonaLauncher)

> Современный лаунчер для Arizona RP на Rust + Tauri 2. Полная переработка v1 (Python): быстрый запуск, нативный интерфейс, установщик без прав администратора.

## Установка

1. Откройте [Releases](https://github.com/halfik24/ArizonaLauncher/releases)
2. Скачайте `Arizona RP Launcher_2.0.0-rc1_x64-setup.exe`
3. Запустите — установка занимает пару секунд, **права администратора не нужны**
4. При первом запуске откроется мастер настройки (путь к игре, обои)

**Требования:** Windows 10/11 x64, WebView2 (встроен в Windows 11; для 10 ставится автоматически с установщиком).

**Поддерживается только лаунчер Air v8** (`ArizonaLauncher8.0_byAIR.exe`) — единственная стабильная версия.

<details>
<summary><b>Запуск игры</b></summary>

- Интеграция с Air v8 (native и air режимы запуска)
- Авто-определение пути к игре при первом запуске (онбординг)
- Профили с разными настройками, сохранение никнейма и сервера
- Параметры: память, CDN, оконный режим и др.

</details>

<details>
<summary><b>Патчи (ArizonaPatches)</b></summary>

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
<summary><b>Для разработчиков</b></summary>

```bash
npm install          # зависимости
npm run tauri dev    # dev-сборка с горячим обновлением
npm run tauri build  # релиз + установщик (src-tauri/target/release/bundle/nsis/)
```

Тесты бэкенда: `cargo test` в `src-tauri/`.

Структура проекта, известные проблемы и планы — в [HANDOFF.md](HANDOFF.md).

**Лог диагностики:** `Документы\ArizonaLauncher\launcher.log` (кнопка «Открыть лог» в лаунчере)

</details>

## Лицензия

MIT
