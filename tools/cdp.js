// Подключение к WebView2 приложения по CDP (нужен запуск с
// WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222)
// Использование: node tools/cdp.js ["выражение для evaluate"]
const PORT = process.env.CDP_PORT || '9222';

async function targets() {
  const r = await fetch(`http://127.0.0.1:${PORT}/json/list`);
  if (!r.ok) throw new Error(`CDP HTTP ${r.status}`);
  return r.json();
}

function connect(wsUrl) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl);
    const pending = new Map();
    const listeners = [];
    let id = 0;
    ws.onopen = () => resolve({
      send(method, params = {}) {
        return new Promise((res, rej) => {
          const msgId = ++id;
          pending.set(msgId, { res, rej });
          ws.send(JSON.stringify({ id: msgId, method, params }));
        });
      },
      on(fn) { listeners.push(fn); },
      close() { ws.close(); },
    });
    ws.onerror = (e) => reject(new Error('WebSocket error: ' + (e.message || e)));
    ws.onmessage = (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id && pending.has(msg.id)) {
        const { res, rej } = pending.get(msg.id);
        pending.delete(msg.id);
        msg.error ? rej(new Error(msg.error.message)) : res(msg.result);
      } else {
        listeners.forEach((fn) => fn(msg));
      }
    };
  });
}

(async () => {
  const list = await targets();
  const page = list.find((t) => t.type === 'page') || list[0];
  if (!page) { console.log('нет target, всего:', list.length); process.exit(1); }
  console.log('target:', page.title || page.url);

  const cdp = await connect(page.webSocketDebuggerUrl);

  if (process.argv.includes('--reload')) {
    await cdp.send('Page.enable');
    await cdp.send('Page.reload', { ignoreCache: true });
    console.log('страница перезагружена');
    await new Promise((s) => setTimeout(s, 2500));
  }

  const logs = [];
  cdp.on((m) => {
    if (m.method === 'Runtime.consoleAPICalled') {
      const args = (m.params.args || []).map((a) => a.value !== undefined ? a.value : a.description || a.type).join(' ');
      logs.push(`[${m.params.type}] ${args}`);
    }
    if (m.method === 'Runtime.exceptionThrown') {
      const d = m.params.exceptionDetails;
      logs.push(`[EXCEPTION] ${d.text} ${d.exception ? d.exception.description || '' : ''}`);
    }
  });
  await cdp.send('Runtime.enable');
  await cdp.send('Log.enable');

  const expr = process.argv.slice(2).filter((a) => !a.startsWith('--')).join(' ') ||
    `JSON.stringify({
      tauri: typeof window.__TAURI__ !== 'undefined' && typeof window.__TAURI__.core?.invoke,
      bridge: typeof window.pywebview?.api?.get_config,
      readyFired: window.__pywebviewReadyFired === true,
      title: document.title
    })`;

  const r = await cdp.send('Runtime.evaluate', {
    expression: expr,
    awaitPromise: true,
    returnByValue: true,
  });
  console.log('EVAL:', JSON.stringify(r.result && r.result.value !== undefined ? r.result.value : r.result, null, 2));
  if (r.exceptionDetails) console.log('EXCEPTION:', r.exceptionDetails.text, r.exceptionDetails.exception?.description);

  await new Promise((s) => setTimeout(s, 1200));
  if (logs.length) { console.log('--- LOGS ---'); logs.forEach((l) => console.log(l)); }
  cdp.close();
})().catch((e) => { console.error('FAIL:', e.message); process.exit(1); });
