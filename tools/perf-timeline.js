// Таймлайн кадров после клика по настройкам: где именно провал.
// Использование: node tools/perf-timeline.js
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
new Promise((resolve) => {
 try {
  const events = [];        // {t, type, ...}
  const frames = [];
  let last = 0; let t0 = 0; let raf = 0; let stop = false;

  // Long Tasks (если поддерживается)
  try {
    new PerformanceObserver(l => {
      for (const e of l.getEntries()) events.push({ t: +(e.startTime - t0).toFixed(0), type: 'longtask', ms: Math.round(e.duration) });
    }).observe({ entryTypes: ['longtask'] });
  } catch (e) {}

  // Логгер вызовов
  const tracked = ['switchTab','refreshLucideIcons','renderCheckboxes','renderPatchesUI','renderPatchChips','renderPatchStatusUI','loadPatchPresets','checkPatchesStatus','loadBackupsList','loadDebugInfo','loadPatchesSchema'];
  const orig = {};
  tracked.forEach(n => {
    if (typeof window[n] !== 'function') return;
    orig[n] = window[n];
    window[n] = function (...a) {
      const s = performance.now();
      const r = orig[n].apply(this, a);
      const d = performance.now() - s;
      if (d >= 1) events.push({ t: +(s - t0).toFixed(0), ms: +d.toFixed(1), fn: n, sync: true });
      return r;
    };
  });

  function tick() {
    if (stop) return;
    const n = performance.now();
    if (last) {
      const dt = n - last;
      frames.push({ t: +(n - t0).toFixed(0), ms: +dt.toFixed(1) });
      if (dt > 33) events.push({ t: +(n - t0).toFixed(0), type: 'jank', ms: +dt.toFixed(1) });
    }
    last = n;
    raf = requestAnimationFrame(tick);
  }

  // старт
  t0 = performance.now();
  raf = requestAnimationFrame(tick);
  const btn = document.querySelector('#settingsBtn, [onclick*="openSettings"]');
  if (btn) btn.click();

  setTimeout(() => {
    stop = true; cancelAnimationFrame(raf);
    Object.keys(orig).forEach(k => window[k] = orig[k]);

    frames.sort((a, b) => b.ms - a.ms);
    events.sort((a, b) => a.t - b.t);
    const all = frames.map(f => f.ms).sort((a, b) => a - b);
    resolve({
      worstFrames: frames.slice(0, 12),
      events: events.slice(0, 40),
      stats: {
        frames: frames.length,
        avgMs: +(frames.reduce((a, b) => a + b.ms, 0) / (frames.length || 1)).toFixed(1),
        p95Ms: +(all[Math.floor(all.length * 0.95)] || 0).toFixed(1),
        maxMs: +(all[all.length - 1] || 0).toFixed(1),
        over33: frames.filter(f => f.ms > 33).length,
        over50: frames.filter(f => f.ms > 50).length,
      }
    });
  }, 3000);
 } catch (e) { resolve({ setupError: String(e && e.stack || e) }); }
})`;

(async () => {
  const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  const page = list.find(t => t.type === 'page');
  if (!page) { console.log('нет page'); process.exit(1); }
  const cdp = await connect(page.webSocketDebuggerUrl);
  await cdp.send('Runtime.enable');
  const r = await cdp.send('Runtime.evaluate', { expression: EXPR, awaitPromise: true, returnByValue: true, timeout: 30000 });
  if (r.exceptionDetails) console.log('EXCEPTION:', r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  else console.log(JSON.stringify(r.result.value, null, 2));
  cdp.close(); process.exit(0);
})().catch(e => { console.error('FAIL:', e.message); process.exit(1); });
