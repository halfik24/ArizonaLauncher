// Точное распределение времени внутри switchTab и refreshLucideIcons.
// Использование: node tools/perf-breakdown.js
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
(() => {
  const acc = { refresh: 0, createIcons: 0, queryAll: 0, total: 0, calls: 0 };
  const origRefresh = window.refreshLucideIcons;
  const origCreate = (typeof lucide !== 'undefined') ? lucide.createIcons : null;
  const origQSA = Document.prototype.querySelectorAll;

  window.refreshLucideIcons = function (...a) {
    const t = performance.now();
    const r = origRefresh.apply(this, a);
    acc.refresh += performance.now() - t;
    return r;
  };
  if (origCreate) {
    lucide.createIcons = function (...a) {
      const t = performance.now();
      const r = origCreate.apply(this, a);
      acc.createIcons += performance.now() - t;
      return r;
    };
  }

  const names = ['patches','launch','import','launcher','debug'];
  switchTab('launch'); // прогрев
  acc.refresh = acc.createIcons = acc.total = 0;

  const t0 = performance.now();
  const per = [];
  for (let i = 0; i < 10; i++) {
    const n = names[i % names.length];
    const s = performance.now();
    switchTab(n);
    const e = performance.now();
    per.push({ tab: n, ms: +(e - s).toFixed(2) });
    acc.calls++;
  }
  acc.total = performance.now() - t0;

  window.refreshLucideIcons = origRefresh;
  if (origCreate) lucide.createIcons = origCreate;

  // Стоимость одного createIcons "вхолостую"
  let idleCost = null;
  try {
    const t = performance.now();
    lucide.createIcons();
    idleCost = +(performance.now() - t).toFixed(2);
  } catch (e) {}

  const stats = per.map(p => p.ms).sort((a,b)=>a-b);
  return {
    switchTabCalls: acc.calls,
    switchTabTotalMs: +acc.total.toFixed(2),
    switchTabAvgMs: +(acc.total / acc.calls).toFixed(2),
    switchTabMedianMs: stats[Math.floor(stats.length/2)],
    refreshLucideIconsTotalMs: +acc.refresh.toFixed(2),
    createIconsTotalMs: +acc.createIcons.toFixed(2),
    refreshShareOfSwitchTabPct: +((acc.refresh / acc.total) * 100).toFixed(1),
    createIconsSharePct: +((acc.createIcons / acc.total) * 100).toFixed(1),
    createIconsIdleCostMs: idleCost,
    pendingIconsNow: document.querySelectorAll('i[data-lucide]').length,
    domNodes: document.getElementsByTagName('*').length
  };
})()`;

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
