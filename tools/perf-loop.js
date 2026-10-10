// Диагностика петли MutationObserver -> lucide.createIcons
// Использование: node tools/perf-loop.js [секунды]
const PORT = process.env.CDP_PORT || '9222';
const SECONDS = parseInt(process.argv[2] || '4', 10);

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

const DIAG = `
new Promise((resolve) => {
  const stats = { addedByTag: {}, removedByTag: {}, totalMutations: 0, createIconsDuringObserve: 0 };
  const bump = (o, k) => o[k] = (o[k] || 0) + 1;

  // 1. Считаем, мутирует ли DOM при вызове createIcons в изоляции
  let isolated = null;
  try {
    const before = document.getElementsByTagName('*').length;
    const t0 = performance.now();
    lucide.createIcons();
    const t1 = performance.now();
    const after = document.getElementsByTagName('*').length;
    isolated = { costMs: +(t1 - t0).toFixed(2), nodesBefore: before, nodesAfter: after, delta: after - before };
  } catch (e) { isolated = { error: String(e) }; }

  // 2. Наблюдатель-следователь
  const probe = new MutationObserver((muts) => {
    for (const m of muts) {
      stats.totalMutations++;
      for (const n of m.addedNodes) if (n.nodeType === 1) bump(stats.addedByTag, n.tagName + (n.getAttribute('data-lucide') ? '[' + n.getAttribute('data-lucide') + ']' : ''));
      for (const n of m.removedNodes) if (n.nodeType === 1) bump(stats.removedByTag, n.tagName + (n.getAttribute && n.getAttribute('data-lucide') ? '[' + n.getAttribute('data-lucide') + ']' : ''));
    }
  });
  probe.observe(document.body, { childList: true, subtree: true });

  // 3. Сколько вызовов createIcons реально происходит
  let calls = 0;
  const origCreate = lucide.createIcons;
  lucide.createIcons = function (...a) { calls++; return origCreate.apply(this, a); };

  setTimeout(() => {
    probe.disconnect();
    lucide.createIcons = origCreate;
    const pendingIcons = document.querySelectorAll('i[data-lucide]').length;
    const svgs = document.querySelectorAll('svg[data-lucide]').length;
    resolve({
      seconds: ${SECONDS},
      createIconsCalls: calls,
      mutations: stats.totalMutations,
      addedByTag: stats.addedByTag,
      removedByTag: stats.removedByTag,
      isolatedCreateIcons: isolated,
      pendingI: pendingIcons,
      svgDataLucide: svgs,
      domNodes: document.getElementsByTagName('*').length
    });
  }, ${SECONDS * 1000});
})`;

(async () => {
  const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  const page = list.find(t => t.type === 'page');
  if (!page) { console.log('нет page'); process.exit(1); }
  const cdp = await connect(page.webSocketDebuggerUrl);
  await cdp.send('Runtime.enable');
  const r = await cdp.send('Runtime.evaluate', { expression: DIAG, awaitPromise: true, returnByValue: true, timeout: (SECONDS + 10) * 1000 });
  if (r.exceptionDetails) console.log('EXCEPTION:', r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  else console.log(JSON.stringify(r.result.value, null, 2));
  cdp.close(); process.exit(0);
})().catch(e => { console.error('FAIL:', e.message); process.exit(1); });
