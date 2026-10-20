#!/usr/bin/env node
/**
 * build.js — сборка с номером сборки, не оставляющая мусора в git.
 *
 * Делает три вещи:
 *   1. штампует версию с номером сборки по дате/времени (tools/set-version.js --build);
 *   2. запускает `tauri build`;
 *   3. ВСЕГДА возвращает базовую версию — даже если сборка упала.
 *
 * Шаг 3 важен: без него каждый билд подмазывал бы 5 файлов таймстемпом,
 * и рабочее дерево оставалось грязным (легко закоммитить случайно).
 * В git живёт базовая версия 2.0.0-beta, номер сборки получает только артефакт.
 *
 * Использование: npm run build  (любые аргументы уходят в tauri build)
 */

'use strict';

const { spawnSync } = require('child_process');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const SET_VERSION = path.join(__dirname, 'set-version.js');

/** Запуск node-скрипта; падение сборки не должно мешать восстановлению версии. */
function node(scriptArgs) {
  const r = spawnSync(process.execPath, scriptArgs, { cwd: ROOT, stdio: 'inherit' });
  return r.status === null ? 1 : r.status;
}

// 1. Штамп. Если не вышло — сборку и вовсе не начинаем.
let code = node([SET_VERSION, '--build']);
if (code !== 0) {
  console.error('[build] не удалось проставить версию, сборка отменена');
  process.exit(code);
}

// 2. Сборка.
try {
  code = node([path.join(ROOT, 'node_modules', '@tauri-apps', 'cli', 'tauri.js'), 'build', ...process.argv.slice(2)]);
} finally {
  // 3. Возврат базовой версии — в finally, чтобы грязь не осталась после ошибки.
  const restore = node([SET_VERSION]);
  if (restore !== 0) {
    console.error('[build] ВНИМАНИЕ: базовую версию не восстановить — проверьте git status');
    if (code === 0) code = restore;
  }
}

process.exit(code);
