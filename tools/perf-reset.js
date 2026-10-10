// Полная перезагрузка страницы приложения + ожидание готовности.
// Использование: node tools/perf-reset.js [секунды ожидания]
const PORT = process.env.CDP_PORT || '9222';
const WAIT = parseInt(process.argv[2] || '6', 10);

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

(async () => {
  const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  const page = list.find(t => t.type === 'page');
  if (!page) { console.log('нет page'); process.exit(1); }
  const cdp = await connect(page.webSocketDebuggerUrl);
  await cdp.send('Page.enable');
  await cdp.send('Page.reload', { ignoreCache: true });
  console.log('перезагрузка...');
  await new Promise(r => setTimeout(r, WAIT * 1000));

  await cdp.send('Runtime.enable');
  const r = await cdp.send('Runtime.evaluate', {
    expression: `JSON.stringify({
      ready: document.readyState,
      bridge: typeof window.pywebview?.api?.get_config,
      domNodes: document.getElementsByTagName('*').length,
      pendingI: document.querySelectorAll('i[data-lucide]').length,
      svgs: document.querySelectorAll('svg[data-lucide]').length,
      heapMB: Math.round(performance.memory.usedJSHeapSize/1048576)
    })`,
    returnByValue: true,
  });
  console.log('состояние:', r.result.value);
  cdp.close(); process.exit(0);
})().catch(e => { console.error('FAIL:', e.message); process.exit(1); });
