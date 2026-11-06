// Функциональный смоук-тест пути обоев и настроек после правок производительности.
// Проверяет то, что сломалось бы при убирании bg_data из конфига:
//   1. фон применён при старте;
//   2. переключение на другой папер даёт другой фон и пишет bg_image;
//   3. сохранение настройки не раздувает config.json (нет bg_data, < 10 КБ);
//   4. сброс фона очищает фон и пишет bg_image = '';
//   5. возврат к исходным обоим восстанавливает фон.
//
// Использование: node tools/smoke-wallpaper.js   (нужен release с CDP-портом)
'use strict';

const { spawn, execFile } = require('child_process');
const path = require('path');
const fs = require('fs');

const EXE = path.resolve(__dirname, '..', 'src-tauri', 'target', 'release', 'arizona-launcher.exe');
const PORT = process.env.CDP_PORT || '9222';
const CONFIG = path.join(
  process.env.USERPROFILE || process.env.HOME, 'Documents', 'ArizonaLauncher', 'config.json');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const results = [];
function check(name, ok, detail) {
  results.push({ name, ok: !!ok, detail });
  console.log(`${ok ? 'OK  ' : 'FAIL'} ${name}${detail ? ' — ' + detail : ''}`);
}

function connect(wsUrl) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl);
    const pending = new Map();
    let id = 0;
    ws.onopen = () => resolve({
      send(method, params = {}) {
        return new Promise((res, rej) => {
          const m = ++id;
          pending.set(m, { res, rej });
          ws.send(JSON.stringify({ id: m, method, params }));
        });
      },
      close() { ws.close(); },
    });
    ws.onerror = (e) => reject(new Error('WS: ' + (e.message || e)));
    ws.onmessage = (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id && pending.has(msg.id)) {
        const { res, rej } = pending.get(msg.id);
        pending.delete(msg.id);
        msg.error ? rej(new Error(msg.error.message)) : res(msg.result);
      }
    };
  });
}

async function evaluate(cdp, expression, timeout = 20000) {
  const r = await cdp.send('Runtime.evaluate', {
    expression, awaitPromise: true, returnByValue: true, timeout,
  });
  if (r.exceptionDetails) {
    throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  }
  return r.result.value;
}

function killApp(pid) {
  return new Promise((resolve) => execFile('taskkill', ['/PID', String(pid), '/T', '/F'], () => resolve()));
}

function readConfig() {
  return JSON.parse(fs.readFileSync(CONFIG, 'utf8'));
}

async function main() {
  if (!fs.existsSync(EXE)) {
    console.error('НЕТ БИНАРЯ:', EXE);
    process.exit(2);
  }

  const app = spawn(EXE, [], {
    env: { ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${PORT}` },
    stdio: 'ignore',
  });

  let page = null;
  const deadline = Date.now() + 45000;
  while (Date.now() < deadline && !page) {
    try {
      const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
      page = list.find((t) => t.type === 'page');
    } catch (_) { /* порт не поднят */ }
    if (!page) await sleep(100);
  }
  if (!page) {
    console.error('FAIL: CDP не поднялся (нужен release, собранный с TAURI_CONFIG)');
    await killApp(app.pid);
    process.exit(2);
  }

  const cdp = await connect(page.webSocketDebuggerUrl);
  await cdp.send('Runtime.enable');

  // Ждём конца инициализации
  for (let i = 0; i < 150; i++) {
    const done = await evaluate(cdp,
      `!!(window._perfLog && window._perfLog.some(m => m.name === 'all done'))`, 5000).catch(() => false);
    if (done) break;
    await sleep(100);
  }
  await sleep(1200); // даём доехать get_wallpaper_full

  const bg = () => evaluate(cdp,
    `document.getElementById('container').style.backgroundImage`);
  const bgAtStart = await bg();
  check('1. фон применён при старте', /^url\("(data:|https?:|file:)/.test(bgAtStart || ''),
    String(bgAtStart).slice(0, 40) + '…');

  // 2. Переключение на другой папер
  const switched = await evaluate(cdp, `(async () => {
    const other = _wallpapers.find(w => w.filename !== launcherSettings.bg_image);
    if (!other) return { ok: false, why: 'в wallpapers/ только один файл' };
    const before = launcherSettings.bg_image;
    applyLocalWallpaper(other);
    for (let i = 0; i < 100; i++) {
      await new Promise(r => setTimeout(r, 50));
      if (launcherSettings.bg_image === other.filename && launcherSettings.bg_data) break;
    }
    return {
      ok: launcherSettings.bg_image === other.filename && !!launcherSettings.bg_data,
      before, after: launcherSettings.bg_image,
      bg: document.getElementById('container').style.backgroundImage.slice(0, 30),
    };
  })()`);
  check('2. переключение обоев', switched.ok,
    `${switched.before} → ${switched.after}`);
  await sleep(800);
  const bgAfterSwitch = await bg();
  check('2b. фон сменился', /^url\("data:/.test(bgAfterSwitch || ''),
    String(bgAfterSwitch).slice(0, 30) + '…');

  // 3. Сохранение настройки не раздувает конфиг
  await evaluate(cdp, `(async () => {
    launcherSettings.status_bar = !launcherSettings.status_bar;
    await applyLauncherSettings();
  })()`);
  await sleep(700);
  let cfg = readConfig();
  const cfgBytes = fs.statSync(CONFIG).size;
  const hasBgData = 'bg_data' in (cfg.launcherSettings || {});
  check('3. config.json маленький после сохранения', cfgBytes < 10240 && !hasBgData,
    `${cfgBytes} байт, bg_data=${hasBgData ? 'ЕСТЬ' : 'нет'}`);
  check('3b. настройка записалась',
    cfg.launcherSettings.status_bar === false || cfg.launcherSettings.status_bar === true,
    `status_bar=${cfg.launcherSettings.status_bar}`);
  check('3c. bg_image записан', typeof cfg.launcherSettings.bgImage === 'string',
    `bgImage=${cfg.launcherSettings.bgImage}`);

  // 4. Сброс фона
  await evaluate(cdp, `clearBgImage()`);
  await sleep(600);
  const bgCleared = await bg();
  check('4. сброс фона очистил фон', bgCleared === 'none' || bgCleared === '',
    `bg="${bgCleared}"`);
  cfg = readConfig();
  check('4b. bg_image = "" в конфиге', cfg.launcherSettings.bgImage === '',
    `bgImage=${JSON.stringify(cfg.launcherSettings.bgImage)}`);

  // 5. Возврат к исходным обоим
  const restored = await evaluate(cdp, `(async () => {
    const defWp = _wallpapers.find(w => String(w.filename).replace(/\\.[^.]+$/, '') === '10')
      || _wallpapers[0];
    if (!defWp) return { ok: false };
    applyLocalWallpaper(defWp);
    for (let i = 0; i < 100; i++) {
      await new Promise(r => setTimeout(r, 50));
      if (launcherSettings.bg_data) break;
    }
    return { ok: !!launcherSettings.bg_data && launcherSettings.bg_image === defWp.filename,
             file: defWp.filename };
  })()`);
  await sleep(800);
  const bgRestored = await bg();
  check('5. обои восстановлены', restored.ok && /^url\("data:/.test(bgRestored || ''),
    `${restored.file} / ${String(bgRestored).slice(0, 30)}…`);
  cfg = readConfig();
  check('5b. конфиг финально маленький', fs.statSync(CONFIG).size < 10240,
    `${fs.statSync(CONFIG).size} байт, bgImage=${cfg.launcherSettings.bgImage}`);

  cdp.close();
  await killApp(app.pid);

  const failed = results.filter(r => !r.ok);
  console.log(`\n=== ${results.length - failed.length}/${results.length} проверок прошло ===`);
  process.exit(failed.length ? 1 : 0);
}

main().catch((e) => {
  console.error('FAIL:', e.message);
  process.exit(1);
});
