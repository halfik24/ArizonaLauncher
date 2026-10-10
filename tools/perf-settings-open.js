// Детальный профиль открытия панели настроек: тайминги по этапам + кадры анимации.
// Использование: node tools/perf-settings-open.js
const PORT = process.env.CDP_PORT || '9222';

function connect(wsUrl) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl);
    const pending = new Map(); const listeners = []; let id = 0;
    ws.onopen = () => resolve({
      send(method, params = {}) {
        return new Promise((res, rej) => {
          const m = ++id; pending.set(m, { res, rej });
          ws.send(JSON.stringify({ id: m, method, params }));
        });
      },
      on(fn) { listeners.push(fn); }, close() { ws.close(); },
    });
    ws.onerror = (e) => reject(new Error('WS: ' + (e.message || e)));
    ws.onmessage = (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id && pending.has(msg.id)) {
        const { res, rej } = pending.get(msg.id);
        pending.delete(msg.id);
        msg.error ? rej(new Error(msg.error.message)) : res(msg.result);
      } else listeners.forEach(fn => fn(msg));
    };
  });
}

const EXPR = `
(async () => {
  const log = {};
  const timing = (name) => {
    const t0 = performance.now();
    return () => { log[name] = +(performance.now() - t0).toFixed(1); };
  };

  // 1) длина сетевых запросов внутри openSettings
  const origFetch = window.fetch;
  window.__net = [];
  window.fetch = function (...a) {
    const t = performance.now();
    const url = typeof a[0] === 'string' ? a[0] : (a[0] && a[0].url) || '?';
    const p = origFetch.apply(this, a);
    p.then(() => window.__net.push({ url: url.slice(0, 90), ms: +(performance.now() - t).toFixed(1) }))
     .catch(() => window.__net.push({ url: url.slice(0, 90), ms: +(performance.now() - t).toFixed(1), err: 1 }));
    return p;
  };

  // 2) сбор кадров во время анимации
  const frames = []; let last = 0; let raf = 0; let stop = false;
  const longTasks = [];
  const po = new PerformanceObserver(l => { for (const e of l.getEntries()) longTasks.push(Math.round(e.duration)); });
  try { po.observe({ entryTypes: ['longtask'] }); } catch (e) {}
  function tick() {
    if (stop) return;
    const n = performance.now();
    if (last) frames.push(n - last);
    last = n;
    raf = requestAnimationFrame(tick);
  }
  raf = requestAnimationFrame(tick);

  // 3) поэтапный профиль открытия
  const t0 = performance.now();
  const p = document.getElementById('settingsPanel');

  // патчим ключевые функции
  const patch = (name, label) => {
    if (typeof window[name] !== 'function') return;
    const o = window[name];
    window[name] = function (...a) {
      const t = performance.now();
      const r = o.apply(this, a);
      const d = performance.now() - t;
      (log.stages = log.stages || []).push({ f: label || name, ms: +d.toFixed(1) });
      return r;
    };
  };
  ['switchTab','refreshLucideIcons','renderCheckboxes','loadPatchesSchema','applySchemaDefaults','renderPatchStatusUI','checkPatchesStatus','loadPatchPresets','renderPatchPresets','showSkeletonCards','loadBackupsList','loadDebugInfo','memSync','setCdnBtns','_applyLauncherVersionUI','loadServers','_sbRefreshScripts'].forEach(f => patch(f));

  const btn = document.querySelector('#settingsBtn, [onclick*="openSettings"]');
  if (btn) btn.click();
  void p.offsetHeight;                     // форсируем layout здесь же
  const tClickSync = performance.now() - t0;

  // ждём завершения async-части openSettings (до 6с)
  await new Promise(r => setTimeout(r, 6000));
  const tAll = performance.now() - t0;

  stop = true; cancelAnimationFrame(raf); po.disconnect();
  window.fetch = origFetch;

  frames.shift();
  const s = frames.slice().sort((a, b) => a - b);
  const avg = frames.length ? frames.reduce((a, b) => a + b, 0) / frames.length : 0;

  return {
    clickToLayoutMs: +tClickSync.toFixed(1),
    fullAsyncMs: +tAll.toFixed(1),
    stages: (log.stages || []).sort((a, b) => b.ms - a.ms).slice(0, 15),
    stagesTotalMs: +((log.stages || []).reduce((a, b) => a + b.ms, 0)).toFixed(1),
    network: window.__net,
    anim: {
      frames: frames.length,
      avgMs: +avg.toFixed(1),
      p95Ms: +(s[Math.floor(s.length * 0.95)] || 0).toFixed(1),
      maxMs: +(s[s.length - 1] || 0).toFixed(1),
      janky33: frames.filter(f => f > 33).length,
      over50: frames.filter(f => f > 50).length,
      over100: frames.filter(f => f > 100).length,
    },
    longTasks: longTasks.length,
    longestTaskMs: longTasks.reduce((a, b) => Math.max(a, b), 0),
    domNodes: document.getElementsByTagName('*').length,
    panelDisplay: p ? getComputedStyle(p).display : null
  };
})()`;

(async () => {
  const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  const page = list.find(t => t.type === 'page');
  if (!page) { console.log('нет page'); process.exit(1); }
  const cdp = await connect(page.webSocketDebuggerUrl);
  await cdp.send('Runtime.enable');
  const r = await cdp.send('Runtime.evaluate', {
    expression: EXPR, awaitPromise: true, returnByValue: true, timeout: 30000,
  });
  if (r.exceptionDetails) console.log('EXCEPTION:', r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  else console.log(JSON.stringify(r.result.value, null, 2));
  cdp.close(); process.exit(0);
})().catch(e => { console.error('FAIL:', e.message); process.exit(1); });
