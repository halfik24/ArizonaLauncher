// Профилирование одного вызова: сколько времени занимает JS и что входит в кадр.
// Использование: node tools/perf-profile.js <switchTab|lucide|layout>
const PORT = process.env.CDP_PORT || '9222';
const WHAT = process.argv[2] || 'switchTab';

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
    ws.onerror = (e) => reject(new Error('WS: ' + (e.message || e)));
    ws.onmessage = (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id && pending.has(msg.id)) {
        const { res, rej } = pending.get(msg.id);
        pending.delete(msg.id);
        msg.error ? rej(new Error(msg.error.message)) : res(msg.result);
      } else listeners.forEach((fn) => fn(msg));
    };
  });
}

const exprs = {
  // Время чистого JS на каждый вызов switchTab
  switchTab: `
    (() => {
      const names = ['patches','launch','import','launcher','debug'];
      const res = [];
      // прогрев
      switchTab('launch');
      for (let round = 0; round < 3; round++) {
        for (const n of names) {
          const t0 = performance.now();
          switchTab(n);
          const t1 = performance.now();
          // форсируем перерисовку, чтобы учесть layout
          void document.querySelector('.settings-tab-content.active').offsetHeight;
          const t2 = performance.now();
          res.push({ tab: n, js: +(t1 - t0).toFixed(2), plusLayout: +(t2 - t1).toFixed(2) });
        }
      }
      const js = res.map(r => r.js).sort((a,b)=>a-b);
      const ly = res.map(r => r.plusLayout).sort((a,b)=>a-b);
      return {
        samples: res.length,
        jsMedian: js[Math.floor(js.length/2)],
        jsMax: js[js.length-1],
        layoutMedian: ly[Math.floor(ly.length/2)],
        layoutMax: ly[ly.length-1],
        detail: res.slice(0, 8)
      };
    })()`,

  // Время lucide.createIcons() на всём документе
  lucide: `
    (() => {
      const t0 = performance.now();
      lucide.createIcons();
      const t1 = performance.now();
      const nodes = document.querySelectorAll('[data-lucide]:not(svg)').length;
      const total = document.querySelectorAll('[data-lucide]').length;
      const t2 = performance.now();
      // повторный вызов — сколько стоит, когда конвертировать нечего
      lucide.createIcons();
      const t3 = performance.now();
      return {
        firstRunMs: +(t1 - t0).toFixed(2),
        queryMs: +(t2 - t1).toFixed(2),
        secondRunMs: +(t3 - t2).toFixed(2),
        pendingIcons: nodes,
        totalDataLucide: total,
        domNodes: document.getElementsByTagName('*').length
      };
    })()`,

  // Стоимость вынужденного reflow + сколько элементов с backdrop-filter видно
  layout: `
    (() => {
      const t0 = performance.now();
      void document.body.offsetHeight;
      const t1 = performance.now();

      // считаем элементы с backdrop-filter, которые сейчас непрямо скрыты
      const all = [...document.querySelectorAll('*')];
      let bd = 0, visibleBd = 0, filters = 0, shadows = 0;
      for (const el of all) {
        const cs = getComputedStyle(el);
        if (cs.backdropFilter && cs.backdropFilter !== 'none') {
          bd++;
          if (cs.display !== 'none' && cs.visibility !== 'hidden' && cs.opacity !== '0') visibleBd++;
        }
        if (cs.filter && cs.filter !== 'none') filters++;
        if (cs.boxShadow && cs.boxShadow !== 'none') shadows++;
      }
      const t2 = performance.now();

      const panel = document.getElementById('settingsPanel');
      const pcs = panel ? getComputedStyle(panel) : null;
      return {
        forcedReflowMs: +(t1 - t0).toFixed(2),
        scanMs: +(t2 - t1).toFixed(2),
        domNodes: all.length,
        backdropFilterTotal: bd,
        backdropFilterVisible: visibleBd,
        filterEls: filters,
        boxShadowEls: shadows,
        settingsPanel: pcs ? {
          backdropFilter: pcs.backdropFilter,
          display: pcs.display,
          opacity: pcs.opacity
        } : null
      };
    })()`,
};

(async () => {
  const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  const page = list.find((t) => t.type === 'page');
  if (!page) { console.log('нет page target'); process.exit(1); }

  const cdp = await connect(page.webSocketDebuggerUrl);
  await cdp.send('Runtime.enable');

  const expression = exprs[WHAT];
  if (!expression) { console.log('нет сценария:', WHAT, '| есть:', Object.keys(exprs).join(', ')); process.exit(1); }

  const r = await cdp.send('Runtime.evaluate', {
    expression, awaitPromise: true, returnByValue: true,
  });
  if (r.exceptionDetails) console.log('EXCEPTION:', r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  else console.log(JSON.stringify(r.result.value, null, 2));

  cdp.close();
  process.exit(0);
})().catch((e) => { console.error('FAIL:', e.message); process.exit(1); });
