// Перечисляет все элементы внутри #settingsPanel с backdrop-filter !== 'none':
// (1) статически — что вообще объявлено; (2) во время анимации открытия (t=150мс)
// — что реально активно и видимо. Плюс внешние слои, которые двигаются вместе
// с открытием (right-panel и т.п.).
// node tools/list-inner-blurs.js
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

  const out = await ev(`(async () => {
    const sleep = (ms) => new Promise(r => setTimeout(r, ms));
    const p = document.getElementById('settingsPanel');
    const sel = (el) => {
      let s = el.tagName.toLowerCase();
      if (el.id) s += '#' + el.id;
      if (el.className && typeof el.className === 'string')
        s += '.' + el.className.trim().split(/\\s+/).slice(0, 2).join('.');
      return s;
    };
    const info = (el) => ({
      sel: sel(el),
      backdrop: getComputedStyle(el).backdropFilter,
      visible: !!(el.offsetWidth || el.offsetHeight || el.getClientRects().length),
      rect: (r => Math.round(r.width) + 'x' + Math.round(r.height))(
        el.getBoundingClientRect()),
    });

    // закрыть, если открыто
    const c0 = document.querySelector('[onclick*="closeSettings"], .s-close, #settingsClose');
    if (p.style.display !== 'none' && c0) { c0.click(); await sleep(700); }

    // (1) статически: все потомки панели с блюром (панель скрыта, но
    //     computed value = заданный)
    const inner = [...p.querySelectorAll('*')]
      .filter(el => getComputedStyle(el).backdropFilter !== 'none')
      .map(info);

    // (2) во время анимации открытия
    document.getElementById('settingsBtn').click();
    await sleep(150);
    const during = [...document.querySelectorAll('body *')]
      .filter(el => {
        const cs = getComputedStyle(el);
        return cs.backdropFilter !== 'none' && cs.display !== 'none' &&
               cs.visibility !== 'hidden' && el.getClientRects().length;
      })
      .map(info);
    await sleep(800);
    const c = document.querySelector('[onclick*="closeSettings"], .s-close, #settingsClose');
    if (c) { c.click(); await sleep(700); }

    // (3) внешние слои, которые двигаются при открытии
    const movers = ['right-panel', 'left-panel', 'status-bar', 'interface']
      .map(cls => {
        const el = document.querySelector('.' + cls);
        return el ? { sel: cls, backdrop: getComputedStyle(el).backdropFilter } : null;
      }).filter(Boolean);

    return { inner, during, movers };
  })()`);

  console.log('=== ВНУТРИ settingsPanel (задано) ===');
  for (const x of out.inner) console.log(` ${x.sel.padEnd(46)} ${x.backdrop}`);
  console.log('=== АКТИВНО ВО ВРЕМЯ АНИМАЦИИ (t=150мс, видимо) ===');
  for (const x of out.during) console.log(` ${x.sel.padEnd(46)} ${x.backdrop.padEnd(28)} ${x.rect}`);
  console.log('=== ВНЕШНИЕ СЛОИ ===');
  for (const x of out.movers) console.log(` ${x.sel.padEnd(46)} ${x.backdrop}`);
  console.log(`Итого внутри: ${out.inner.length}, активно в анимации: ${out.during.length}`);
  cdp.close();
  process.exit(0);
})().catch((e) => { console.error('FAIL:', e.message); process.exit(1); });
