// Проверка силы блюра: setBlur() меняет --frost-blur, computed style панели
// и навигации обновляется, выбор сохраняется в localStorage 'airBlur'.
// node tools/check-blur.js
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

  const checks = [];
  const ok = (name, cond, info) => checks.push({ name, pass: !!cond, info });

  const res = await ev(`(() => {
    const p = document.getElementById('settingsPanel');
    const out = [];
    for (const px of [8, 12, 16, 24, 32]) {
      setBlur(px);
      const panel = getComputedStyle(p).backdropFilter;
      const nav = getComputedStyle(document.querySelector('.settings-nav')).backdropFilter;
      out.push({ px, panel, nav, stored: localStorage.getItem('airBlur') });
    }
    // невалидное значение -> откат на 24
    setBlur(999);
    out.push({ px: 999, panel: getComputedStyle(p).backdropFilter,
               nav: getComputedStyle(document.querySelector('.settings-nav')).backdropFilter,
               stored: localStorage.getItem('airBlur') });
    setBlur(24);
    return out;
  })()`);

  for (const r of res) {
    if (r.px === 999) {
      ok('невалид 999 -> откат 24px',
         r.panel === 'blur(24px) saturate(1.2)' && r.stored === '24',
         r.panel + ' stored=' + r.stored);
      continue;
    }
    ok(`${r.px}px: панель`, r.panel === `blur(${r.px}px) saturate(1.2)`, r.panel);
    // Навигация: backdrop-filter убран (fix «светлее → чернеет») — вместо
    // него статичная тинт-прослойка rgba(--surface-rgb, 0.7), см. .settings-nav.
    ok(`${r.px}px: навигация без backdrop (статичный тинт)`, r.nav === 'none', r.nav);
    ok(`${r.px}px: сохранено`, r.stored === String(r.px), r.stored);
  }

  console.log(JSON.stringify(res, null, 1));
  const failed = checks.filter((c) => !c.pass);
  console.log(failed.length
    ? 'FAIL: ' + failed.map((f) => f.name + ' (' + f.info + ')').join('; ')
    : 'PASS: all ' + checks.length + ' checks');
  cdp.close();
  process.exit(failed.length ? 1 : 0);
})().catch((e) => { console.error('FAIL:', e.message); process.exit(1); });
