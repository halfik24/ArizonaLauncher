// Проверка дефолтного режима: удаляет airAnimMode из localStorage,
// перезагружает страницу и смотрит класс на <html> (должен быть anim-full).
// node tools/check-default-mode.js
'use strict';
const PORT = process.env.CDP_PORT || '9222';

function connect(wsUrl) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl);
    const pending = new Map();
    const loadWaiters = [];
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
      waitLoad() { return new Promise((res) => loadWaiters.push(res)); },
    });
    ws.onerror = (e) => reject(new Error('WS: ' + (e.message || e)));
    ws.onmessage = (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.method === 'Page.loadEventFired') {
        while (loadWaiters.length) loadWaiters.pop()();
      }
      if (msg.id && pending.has(msg.id)) {
        const { res, rej } = pending.get(msg.id);
        pending.delete(msg.id);
        msg.error ? rej(new Error(msg.error.message)) : res(msg.result);
      }
    };
  });
}

(async () => {
  const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  const page = list.find((t) => t.type === 'page');
  if (!page) { console.error('no page'); process.exit(1); }
  const cdp = await connect(page.webSocketDebuggerUrl);
  await cdp.send('Runtime.enable');
  await cdp.send('Page.enable');
  const ev = async (expression) => {
    const r = await cdp.send('Runtime.evaluate', {
      expression, awaitPromise: true, returnByValue: true, timeout: 30000,
    });
    if (r.exceptionDetails) {
      throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
    }
    return r.result.value;
  };

  await ev(`localStorage.removeItem('airAnimMode')`);
  const loaded = cdp.waitLoad();
  await ev(`location.reload()`);
  await Promise.race([loaded, new Promise((r) => setTimeout(r, 5000))]);
  await new Promise((r) => setTimeout(r, 800));
  const cls = await ev(`document.documentElement.className`);
  const has = /\banim-full\b/.test(cls);
  console.log('html class after reload:', cls);
  console.log(has ? 'PASS: default mode is full'
                  : 'FAIL: default is NOT full');
  cdp.close();
  process.exit(has ? 0 : 1);
})().catch((e) => { console.error('FAIL:', e.message); process.exit(1); });
