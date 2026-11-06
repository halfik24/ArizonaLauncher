// Замер старта приложения и проверка «не блокируется ли главный поток».
//
// Использование:
//   node tools/perf-startup.js              — холодный старт: окно → webview → первый контент → init
//   node tools/perf-startup.js --blocktest  — параллельно: тяжёлая команда + пинг дешёвой команды
//   node tools/perf-startup.js --json       — только JSON (для автоматических прогонов)
//   node tools/perf-startup.js --keep       — не убивать приложение после замера
//
// Требует release-сборку с портом CDP, например:
//   $env:TAURI_CONFIG = '{"app":{"windows":[{...,"additionalBrowserArgs":"... --remote-debugging-port=9222 --remote-allow-origins=*"}]}}'
//   cargo build --release   (в окружении vcvars64.bat)
//
// ВАЖНО: longTaskCount — это long tasks РЕНДЕРЕРА (JS). Блокировка главного
// потока Rust в них НЕ попадает: она проявляется как задержка ответа IPC.
// Именно поэтому для неё есть --blocktest.
'use strict';

const { spawn, execFile } = require('child_process');
const path = require('path');
const fs = require('fs');

const EXE = path.resolve(__dirname, '..', 'src-tauri', 'target', 'release', 'arizona-launcher.exe');
const PORT = process.env.CDP_PORT || '9222';
const KEEP = process.argv.includes('--keep');
const BLOCKTEST = process.argv.includes('--blocktest');
const JSON_ONLY = process.argv.includes('--json');
const START_TIMEOUT_MS = 45000;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (...a) => { if (!JSON_ONLY) console.log(...a); };

function connect(wsUrl) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl);
    const pending = new Map();
    const listeners = [];
    let id = 0;
    ws.onopen = () => resolve({
      send(method, params = {}) {
        return new Promise((res, rej) => {
          const m = ++id;
          pending.set(m, { res, rej });
          ws.send(JSON.stringify({ id: m, method, params }));
        });
      },
      on(fn) { listeners.push(fn); },
      close() { ws.close(); },
    });
    ws.onerror = (e) => reject(new Error('WS: ' + (e.message || e)));
    ws.onmessage = (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id && pending.has(msg.id)) {
        const { res, rej } = pending.get(msg.id);
        pending.delete(msg.id);
        msg.error ? rej(new Error(msg.error.message)) : res(msg.result);
      } else listeners.forEach((fn) => fn(msg));
    };
  });
}

async function evaluate(cdp, expression, timeout = 15000) {
  const r = await cdp.send('Runtime.evaluate', {
    expression, awaitPromise: true, returnByValue: true, timeout,
  });
  if (r.exceptionDetails) {
    throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  }
  return r.result.value;
}

async function findPage() {
  const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  return list.find((t) => t.type === 'page');
}

async function waitPage(deadline) {
  while (Date.now() < deadline) {
    try {
      const page = await findPage();
      if (page) return page;
    } catch (_) { /* порт ещё не поднят */ }
    await sleep(50);
  }
  throw new Error('CDP: страница не появилась за отведённое время');
}

function findExistingInstances() {
  return new Promise((resolve) => {
    execFile('tasklist', ['/FI', 'IMAGENAME eq arizona-launcher.exe', '/FO', 'CSV', '/NH'],
      (err, stdout) => {
        if (err) return resolve([]);
        const pids = String(stdout)
          .split(/\r?\n/)
          .map((l) => l.match(/"arizona-launcher\.exe","(\d+)"/))
          .filter(Boolean)
          .map((m) => Number(m[1]));
        resolve(pids);
      });
  });
}

function startApp() {
  const child = spawn(EXE, [], {
    env: {
      ...process.env,
      // Не работает, если в tauri.conf.json задан свой additionalBrowserArgs —
      // конфиг перекрывает env-переменную. Порт CDP добавляется через TAURI_CONFIG.
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${PORT}`,
    },
    stdio: 'ignore',
  });
  return child;
}

function watchWindow(pid) {
  return new Promise((resolve) => {
    execFile(
      'powershell.exe',
      ['-NoProfile', '-File', path.join(__dirname, 'win-visible.ps1'), '-ProcessId', String(pid)],
      { timeout: START_TIMEOUT_MS + 5000 },
      (err, stdout) => {
        const line = String(stdout || '').trim();
        // "<epochMs>|<мс от старта скрипта>|<заголовок>"
        const [epochMs, ms, ...rest] = line.split('|');
        resolve({
          epochMs: Number(epochMs),
          ms: Number(ms),
          title: rest.join('|'),
          error: err ? err.message : null,
        });
      },
    );
  });
}

function killApp(pid) {
  return new Promise((resolve) => {
    execFile('taskkill', ['/PID', String(pid), '/T', '/F'], () => resolve());
  });
}

/** Таймлайн perfMark: время относительно старта процесса + дельта к предыдущей метке. */
function timeline(perfLog, timeOriginWall, t0) {
  if (!Array.isArray(perfLog) || !perfLog.length) return [];
  let prevOrigin = null;
  return perfLog.map((m) => {
    const row = {
      mark: m.name,
      sinceStartMs: Math.round(timeOriginWall + m.time - t0),
      deltaMs: prevOrigin === null ? null : Math.round(m.time - prevOrigin),
    };
    prevOrigin = m.time;
    return row;
  });
}

async function main() {
  if (!fs.existsSync(EXE)) {
    console.error('НЕТ БИНАРЯ:', EXE, '\nСобери сначала: cargo build --release');
    process.exit(2);
  }

  // WebView2 держит один browser-процесс на user-data-folder: пока жив старый
  // экземпляр, --remote-debugging-port не поднимется, и замер прилипнет к нему.
  const existing = await findExistingInstances();
  if (existing.length) {
    console.error(`Уже запущен arizona-launcher.exe (pid ${existing.join(', ')}). Закрой его и повтори.`);
    process.exit(2);
  }

  const t0 = Date.now();
  const app = startApp();
  log(`[perf-startup] запущен ${path.basename(EXE)} (pid ${app.pid})`);

  const winPromise = watchWindow(app.pid);
  const page = await waitPage(t0 + START_TIMEOUT_MS);
  const tCdp = Date.now();
  log(`[perf-startup] CDP готов через ${tCdp - t0} мс`);

  const cdp = await connect(page.webSocketDebuggerUrl);
  await cdp.send('Runtime.enable');

  // Ждём завершения инициализации фронта (последняя метка perfMark).
  const deadline = t0 + START_TIMEOUT_MS;
  let done = false;
  while (Date.now() < deadline) {
    done = await evaluate(cdp,
      `!!(window._perfLog && window._perfLog.some(m => m.name === 'all done'))`, 5000);
    if (done) break;
    await sleep(100);
  }
  const tAllDone = Date.now();
  if (!done) log('[perf-startup] ВНИМАНИЕ: метка "all done" не появилась');

  const data = await evaluate(cdp, `(() => {
    const nav = performance.getEntriesByType('navigation')[0] || null;
    const paints = performance.getEntriesByType('paint');
    return {
      wallNow: Date.now(),
      perfNow: performance.now(),
      title: document.title,
      visibility: document.visibilityState,
      nav: nav ? {
        ttfb: Math.round(nav.responseStart),
        domInteractive: Math.round(nav.domInteractive),
        dcl: Math.round(nav.domContentLoadedEventEnd),
        load: Math.round(nav.loadEventEnd),
        domComplete: Math.round(nav.domComplete),
      } : null,
      paint: paints.map(e => ({ name: e.name, t: Math.round(e.startTime) })),
      longTasks: (performance.getEntriesByType('longtask') || [])
        .map(e => ({ t: Math.round(e.startTime), d: Math.round(e.duration) }))
        .sort((a, b) => b.d - a.d).slice(0, 15),
      perfLog: window._perfLog || [],
      heapMB: Math.round((performance.memory?.usedJSHeapSize || 0) / 1048576),
      // Фон контейнера: проверка, что обои применились (data: или http), а не
      // остались пустыми после отказа от bg_data в конфиге.
      bg: (document.getElementById('container') || {}).style
        ? document.getElementById('container').style.backgroundImage : null,
      bgLen: (document.getElementById('container') || {}).style
        ? document.getElementById('container').style.backgroundImage.length : 0,
    };
  })()`);

  const win = await winPromise;
  // epochMs задаёт сам PowerShell; окно не может появиться раньше старта его часов.
  const windowVisibleMs = win.ms >= 0 && win.epochMs ? win.epochMs + win.ms - t0 : win.ms;

  // Фон подтягивается асинхронно (get_wallpaper_full) — перечитываем через паузу,
  // чтобы поймать и медленный диск, и гарантированно увидеть применённые обои.
  await sleep(1500);
  const bgLater = await evaluate(cdp, `document.getElementById('container')
    ? document.getElementById('container').style.backgroundImage : null`);

  // Стена времени, относительно которой живёт performance.now()
  const timeOriginWall = data.wallNow - data.perfNow;
  const since = (msFromOrigin) => Math.round(timeOriginWall + msFromOrigin - t0);
  const fcp = data.paint.find((p) => p.name === 'first-contentful-paint');

  const result = {
    binary: path.basename(EXE),
    binaryMbs: +(fs.statSync(EXE).size / 1048576).toFixed(2),
    windowVisibleMs,
    windowTitle: win.title || null,
    cdpReadyMs: tCdp - t0,
    allDoneMs: tAllDone - t0,
    pageNav: data.nav,
    firstPaintMs: data.paint.length ? since(data.paint[0].t) : null,
    firstContentfulPaintMs: fcp ? since(fcp.t) : null,
    // Сколько окно существует без единого пикселя контента
    blankWindowMs: fcp && windowVisibleMs >= 0
      ? Math.max(0, since(fcp.t) - windowVisibleMs)
      : null,
    longTaskCount: data.longTasks.length,
    topLongTasks: data.longTasks.slice(0, 8),
    heapMB: data.heapMB,
    // 'none' = пусто; "url(\"data:...\"" или "url(\"http..." = обои применены
    bgAtInit: data.bg || null,
    bgAfter1_5s: bgLater || null,
    timeline: timeline(data.perfLog, timeOriginWall, t0),
  };

  // Самый дорогой участок инициализации — искать по максимальной дельте.
  const worst = result.timeline
    .filter((r) => r.deltaMs != null)
    .sort((a, b) => b.deltaMs - a.deltaMs)[0];
  result.worstGap = worst || null;

  console.log(JSON.stringify(result, null, JSON_ONLY ? 0 : 2));

  if (BLOCKTEST) {
    log('\n--- blocktest: тяжёлая команда против пинга дешёвой ---');
    const bt = await evaluate(cdp, `(() => {
      const inv = (cmd, args) => window.__TAURI__.core.invoke(cmd, args || {});
      const results = {};

      async function probe(label, heavyCmd, heavyArgs) {
        const heavy = inv(heavyCmd, heavyArgs).then(
          () => 'ok', e => 'error: ' + e);
        const lat = [];
        for (let i = 0; i < 20; i++) {
          const s = performance.now();
          await inv('get_launcher_version');
          lat.push(+(performance.now() - s).toFixed(1));
          await new Promise(r => setTimeout(r, 15));
        }
        const status = await heavy;
        lat.sort((a, b) => a - b);
        results[label] = {
          heavy: status,
          pings: lat.length,
          avgMs: +(lat.reduce((a, b) => a + b, 0) / lat.length).toFixed(1),
          p50Ms: lat[Math.floor(lat.length * 0.5)],
          p95Ms: lat[Math.floor(lat.length * 0.95)] || lat[lat.length - 1],
          maxMs: lat[lat.length - 1],
        };
      }

      // Один вызов на свежем процессе = холодный кэш: длительность самого
      // декода/кодирования, без пингов рядом.
      async function timeOnce(cmd, args) {
        const s = performance.now();
        await inv(cmd, args);
        return +(performance.now() - s).toFixed(0);
      }

      return (async () => {
        // Сколько стоит запрос настроек против крошечной команды:
        // разница ≈ стоимость передачи bg_data (base64-обои) по IPC.
        async function lat(cmd, n) {
          const a = [];
          for (let i = 0; i < n; i++) {
            const s = performance.now();
            await inv(cmd);
            a.push(+(performance.now() - s).toFixed(1));
          }
          return { avg: +(a.reduce((x, y) => x + y, 0) / a.length).toFixed(1), max: Math.max(...a) };
        }
        results.get_launcher_version = await lat('get_launcher_version', 5);
        results.get_launcher_settings = await lat('get_launcher_settings', 5);

        results.coldThumb1jpgMs = await timeOnce('get_wallpaper_thumb', { filename: '1.jpg' });
        results.coldThumb8pngMs = await timeOnce('get_wallpaper_thumb', { filename: '8.png' });
        await probe('get_wallpaper_thumb(8.png, warm)', 'get_wallpaper_thumb', { filename: '8.png' });
        await probe('check_patches_status', 'check_patches_status', {});
        return results;
      })();
    })()`, 60000);
    log(JSON.stringify(bt, null, 2));
    if (JSON_ONLY) console.error(JSON.stringify(bt));
  }

  cdp.close();
  if (KEEP) {
    log(`[perf-startup] приложение оставлено запущенным (pid ${app.pid})`);
    process.exit(0);
  }
  await killApp(app.pid);
  process.exit(0);
}

main().catch((e) => {
  console.error('FAIL:', e.message);
  process.exit(1);
});
