#!/usr/bin/env node
/**
 * build.js — сборка с номером сборки, не оставляющая мусора в git.
 *
 * Делает три вещи:
 *   1. штампует версию с номером сборки по дате/времени (tools/set-version.js --build),
 *      ЕСЛИ в tauri.conf.json стоит базовая версия 2.0.0-beta. Если там уже
 *      другая версия (например, CI выставил тег релиза step'ом
 *      `set-version.js --set v2.0.0-beta.1`) — штамп НЕ трогаем: релизная
 *      сборка должна называться ровно по тегу, без таймстампа;
 *   2. запускает `tauri build`;
 *   3. возвращает исходную версию (базовую или теговую) — даже если сборка упала.
 *
 * Шаг 3 важен: без него каждый билд подмазывал бы 5 файлов таймстемпом,
 * и рабочее дерево оставалось грязным (легко закоммитить случайно).
 * В git живёт базовая версия 2.0.0-beta, номер сборки получает только артефакт.
 *
 * Использование: npm run build  (любые аргументы уходят в tauri build)
 */

'use strict';

const { spawnSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const SET_VERSION = path.join(__dirname, 'set-version.js');
const CONF = path.join(ROOT, 'src-tauri', 'tauri.conf.json');
// База должна совпадать с BASE в set-version.js (там основная точка правды).
const BASE = '2.0.0-beta';

/** Запуск node-скрипта; падение сборки не должно мешать восстановлению версии. */
function node(scriptArgs) {
  const r = spawnSync(process.execPath, scriptArgs, { cwd: ROOT, stdio: 'inherit' });
  return r.status === null ? 1 : r.status;
}

/** Текущая версия приложения: из tauri.conf.json (единственный источник сборщика). */
function currentVersion() {
  const src = fs.readFileSync(CONF, 'utf8');
  const m = src.match(/^(\s*"version"\s*:\s*)"([^"]*)"/m);
  return m ? m[2] : null;
}

// 1. Если версия ещё не задана извне (CI с тегом) — штампуем номер сборки.
//    Иначе оставляем проставленную релизную версию как есть.
const versionAtStart = currentVersion();
const override = versionAtStart && versionAtStart !== BASE ? versionAtStart : null;
let code = 0;

if (override) {
  console.log(`[build] версия задана извне (${override}) — штамп по времени не применяем`);
} else {
  const stamp = node([SET_VERSION, '--build']);
  if (stamp !== 0) {
    console.error('[build] не удалось проставить версию, сборка отменена');
    process.exit(stamp);
  }
}

// 2. Сборка.
try {
  code = node([path.join(ROOT, 'node_modules', '@tauri-apps', 'cli', 'tauri.js'), 'build', ...process.argv.slice(2)]);
} finally {
  // 3. Возврат версии к состоянию до сборки — в finally, чтобы грязь не осталась после ошибки.
  //    Для сборки с тегом возвращаем саму теговую версию: в CI свежий checkout, грязи не остаётся.
  const restore = node(override ? [SET_VERSION, '--set', override] : [SET_VERSION]);
  if (restore !== 0) {
    console.error('[build] ВНИМАНИЕ: базовую версию не восстановить — проверьте git status');
    if (code === 0) code = restore;
  }
}

process.exit(code);
