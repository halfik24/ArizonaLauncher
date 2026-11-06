// Сэмпл свойств панели во время анимации открытия в заданном режиме.
// node tools/probe-anim-props.js [fade|pop|smooth|full]
'use strict';
const PORT = process.env.CDP_PORT || '9222';

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

(async () => {
  const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  const page = list.find((t) => t.type === 'page');
  if (!page) { console.error('no page'); process.exit(1); }
  const cdp = await connect(page.webSocketDebuggerUrl);
  await cdp.send('Runtime.enable');
  const ev = async (expression) => {
    const r = await cdp.send('Runtime.evaluate', {
      expression, awaitPromise: true, returnByValue: true, timeout: 30000,
    });
    if (r.exceptionDetails) {
      throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
    }
    return r.result.value;
  };

  const want = (process.argv[2] || 'fade').replace(/[^a-z]/g, '');
  const out = await ev(`(async () => {
    const sleep = (ms) => new Promise(r => setTimeout(r, ms));
    if (typeof window.setAnimMode === 'function') window.setAnimMode('${want}');
    const p = document.getElementById('settingsPanel');
    // закрыть, если открыто
    const c0 = document.querySelector('[onclick*="closeSettings"], .s-close, #settingsClose');
    if (p.style.display !== 'none' && c0) { c0.click(); await sleep(700); }

    const samples = [];
    document.getElementById('settingsBtn').click();
    for (let i = 0; i < 10; i++) {
      await sleep(50);
      const cs = getComputedStyle(p);
      samples.push({
        t: (i + 1) * 50,
        anim: cs.animationName,
        transform: cs.transform,
        filter: cs.filter.slice(0, 20),
        backdrop: cs.backdropFilter.slice(0, 24),
        opacity: cs.opacity,
        animRise: p.classList.contains('anim-rise'),
        htmlCls: document.documentElement.className,
      });
    }
    const c = document.querySelector('[onclick*="closeSettings"], .s-close, #settingsClose');
    if (c) { c.click(); await sleep(700); }
    return samples;
  })()`);

  for (const s of out) {
    console.log(
      `t=${String(s.t).padStart(3)} anim=${s.anim.padEnd(20)} op=${s.opacity.padEnd(5)} ` +
      `filter=${s.filter.padEnd(20)} backdrop=${s.backdrop}`);
    console.log(`      transform=${s.transform}`);
  }
  console.log('html class:', out[0].htmlCls, '| anim-rise during:', out.map(s => s.animRise).join(','));
  cdp.close();
  process.exit(0);
})().catch((e) => { console.error('FAIL:', e.message); process.exit(1); });
