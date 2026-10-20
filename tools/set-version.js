#!/usr/bin/env node
/**
 * set-version.js — единая точка правды по версии приложения.
 *
 * Одна команда синхронизирует версию во всех местах, где она хранится:
 *   package.json, src-tauri/Cargo.toml, src-tauri/Cargo.lock,
 *   src-tauri/tauri.conf.json  — версия для сборщиков
 *   src/version.js             — window.__APP_VERSION__ для интерфейса
 *
 * Интерфейс читает версию только из window.__APP_VERSION__, поэтому
 * захардкоженных строк версии в index.html быть не должно.
 *
 * Использование:
 *   node tools/set-version.js            базовая версия: 2.0.0-beta
 *   node tools/set-version.js --build    с номером сборки: 2.0.0-beta.20261020.1435
 *   node tools/set-version.js --print    только показать, ничего не писать
 *
 * Номер сборки — дата и время запуска (YYYYMMDD.HHMM, локальное время).
 * Часы пишутся без ведущего нуля: сборка в 09:35 даёт .935, а не .0935.
 * Это не косметика — Cargo/семвер запрещают ведущий ноль в числовом
 * идентификаторе prerelease, и версия вида 2.0.0-beta.20261020.0935
 * ломает сборку (проверено: "invalid leading zero in pre-release identifier").
 */

'use strict';

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');

// Базовая версия. Единственное место, где её нужно править при выходе релиза.
const BASE = '2.0.0-beta';

/**
 * Файлы, в которых хранится версия, и как именно её туда писать.
 * replace-функции работают идемпотентно: можно гонять скрипт сколько угодно раз.
 */
const TARGETS = [
  {
    // В обоих JSON ключ "version" встречается ровно один раз (проверено),
    // поэтому заменяем только значение, не трогая форматирование файла.
    file: 'package.json',
    update: (src, version) => src.replace(/^(\s*"version"\s*:\s*)"[^"]*"/m, `$1"${version}"`),
  },
  {
    file: 'src-tauri/tauri.conf.json',
    update: (src, version) => src.replace(/^(\s*"version"\s*:\s*)"[^"]*"/m, `$1"${version}"`),
  },
  {
    // Первая строка version = "..." в секции [package].
    file: 'src-tauri/Cargo.toml',
    update: (src, version) =>
      src.replace(/^(\[package\][\s\S]*?^version\s*=\s*)"[^"]*"/m, `$1"${version}"`),
  },
  {
    // Версия нашего пакета в lock-файле: строка version после name = "arizona-launcher".
    // В файле она встречается ровно один раз, но привязка к name делает замену устойчивой.
    file: 'src-tauri/Cargo.lock',
    update: (src, version) =>
      src.replace(
        /(name\s*=\s*"arizona-launcher"\s*\nversion\s*=\s*)"[^"]*"/,
        `$1"${version}"`
      ),
  },
  {
    // Источник версии для интерфейса. С "v" — так, как её показывают все экраны.
    file: 'src/version.js',
    update: (_src, version) => header() + `window.__APP_VERSION__ = 'v${version}';\n`,
    create: true,
  },
];

function header() {
  return [
    '/**',
    ' * ГЕНЕРИРУЕТСЯ скриптом tools/set-version.js — не редактировать вручную.',
    ' * Единственный источник версии для интерфейса (src/index.html).',
    ' *',
    ' * Версия для сборщиков живёт в package.json / Cargo.toml / tauri.conf.json,',
    ' * но править их руками не нужно — запустите set-version.js.',
    ' */',
    '',
  ].join('\n');
}

/** Сборка номера сборки: YYYYMMDD.HHMM, часы без ведущего нуля. */
function buildNumber(now = new Date()) {
  const p = (n) => String(n).padStart(2, '0');
  const date = `${now.getFullYear()}${p(now.getMonth() + 1)}${p(now.getDate())}`;
  const time = String(now.getHours()) + p(now.getMinutes());
  return `${date}.${time}`;
}

/** Полная версия: базовая или базовая с номером сборки. */
function resolveVersion(withBuild, now) {
  return withBuild ? `${BASE}.${buildNumber(now)}` : BASE;
}

function run() {
  const args = process.argv.slice(2);
  const withBuild = args.includes('--build');
  const printOnly = args.includes('--print');

  const version = resolveVersion(withBuild);

  if (printOnly) {
    console.log(version);
    return;
  }

  for (const target of TARGETS) {
    const abs = path.join(ROOT, target.file);
    const prev = fs.existsSync(abs) ? fs.readFileSync(abs, 'utf8') : null;
    if (prev === null && !target.create) {
      console.error(`[set-version] не найден файл: ${target.file}`);
      process.exitCode = 1;
      continue;
    }

    const next = target.update(prev || '', version);
    if (next === prev) {
      console.log(`[set-version] без изменений  ${target.file}`);
      continue;
    }
    fs.writeFileSync(abs, next, 'utf8');
    console.log(`[set-version] обновлён       ${target.file} -> ${version}`);
  }
}

run();
