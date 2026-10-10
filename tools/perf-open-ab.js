// A/B одного открытия панели настроек: патчим CSS, открываем один раз,
// меряем кадры первые N мс. После прогона патч снимается.
// Использование: node tools/perf-open-ab.js <baseline|no-backdrop|no-shadow|no-anim|no-blur-panel> [мс наблюдения]
const PORT = process.env.CDP_PORT || '9222';
const MODE = process.argv[2] || 'baseline';
const OBSERVE = parseInt(process.argv[3] || '1200', 10);

const PATCH = {
  baseline: '',
  'no-backdrop': `* { backdrop-filter: none !important; -webkit-backdrop-filter: none !important; }`,
  'no-shadow': `* { box-shadow: none !important; }`,
  'no-anim': `#settingsPanel { animation: none !important; transition: none !important; }
              .right-panel { transition: none !important; }`,
  'no-panel-anim': `#settingsPanel { animation: none !important; }`,
  'no-panel-transition': `#settingsPanel { transition: none !important; }`,
  'no-news-away': `.right-panel { transition: none !important; }`,
  'no-news-away2': `.right-panel.news-away { transition: none !important; }`,
  // та же анимация, но БЕЗ filter: blur() в keyframes (оставляем opacity+transform)
  'rise-no-filter': `@keyframes settingsRise {
                       0%   { opacity: 0; transform: translate(-50%,-50%) scale(1.08); }
                       100% { opacity: 1; transform: translate(-50%,-50%) scale(1); }
                     }`,
  // blur есть, но слабее (7px вместо 14)
  'rise-blur-half': `@keyframes settingsRise {
                       0%   { opacity: 0; transform: translate(-50%,-50%) scale(1.08); filter: blur(7px); }
                       100% { opacity: 1; transform: translate(-50%,-50%) scale(1); filter: blur(0); }
                     }`,
  // blur сильнее (28px) — для контроля, должно быть хуже
  'rise-blur-double': `@keyframes settingsRise {
                       0%   { opacity: 0; transform: translate(-50%,-50%) scale(1.08); filter: blur(28px); }
                       100% { opacity: 1; transform: translate(-50%,-50%) scale(1); filter: blur(0); }
                     }`,
  // анимируем только opacity (самый дешёвый вариант)
  'rise-opacity-only': `@keyframes settingsRise {
                       0%   { opacity: 0; }
                       100% { opacity: 1; }
                     }`,
  // scale есть, но у панели ВЫКЛЮЧЕН backdrop-filter -> проверяем гипотезу
  'scale-no-bdf': `@keyframes settingsRise {
                       0%   { opacity: 0; transform: translate(-50%,-50%) scale(1.08); filter: blur(14px); }
                       100% { opacity: 1; transform: translate(-50%,-50%) scale(1); filter: blur(0); }
                     }
                     #settingsPanel { backdrop-filter: none !important; -webkit-backdrop-filter: none !important; }`,
  // scale слабее (1.03 вместо 1.08) — если жадность растра пропорциональна
  'rise-scale-small': `@keyframes settingsRise {
                       0%   { opacity: 0; transform: translate(-50%,-50%) scale(1.03); filter: blur(14px); }
                       100% { opacity: 1; transform: translate(-50%,-50%) scale(1); filter: blur(0); }
                     }`,
  // scale есть, filter убран — чисто transform+opacity
  'rise-transform-only': `@keyframes settingsRise {
                       0%   { opacity: 0; transform: translate(-50%,-50%) scale(1.08); }
                       100% { opacity: 1; transform: translate(-50%,-50%) scale(1); }
                     }`,
  // opacity+filter, но БЕЗ scale — чисто проверка scale
  'rise-no-scale': `@keyframes settingsRise {
                       0%   { opacity: 0; filter: blur(14px); }
                       100% { opacity: 1; filter: blur(0); }
                     }`,
  // мягкий вариант: слабый blur + слабый scale
  'rise-soft': `@keyframes settingsRise {
                       0%   { opacity: 0; transform: translate(-50%,-50%) scale(1.04); filter: blur(6px); }
                       100% { opacity: 1; transform: translate(-50%,-50%) scale(1); filter: blur(0); }
                     }`,
  // та же анимация целиком, но у панели выключен backdrop-filter
  'fullanim-no-bdf': `#settingsPanel { backdrop-filter: none !important; -webkit-backdrop-filter: none !important; }`,
  // blur фиксирован (не анимируется), анимируется только opacity+scale
  'rise-static-blur': `#settingsPanel { filter: blur(0) !important; }
                       @keyframes settingsRise {
                         0%   { opacity: 0; transform: translate(-50%,-50%) scale(1.08); filter: blur(0); }
                         100% { opacity: 1; transform: translate(-50%,-50%) scale(1); filter: blur(0); }
                       }`,
  // scale+opacity, но backdrop-filter выкл И filter-анимация выкл
  'scale-nobdf-nofilter': `@keyframes settingsRise {
                       0%   { opacity: 0; transform: translate(-50%,-50%) scale(1.08); }
                       100% { opacity: 1; transform: translate(-50%,-50%) scale(1); }
                     }
                     #settingsPanel { backdrop-filter: none !important; -webkit-backdrop-filter: none !important; }`,
  // blur ослаблен до 8px, bdf ослаблен до 8px, scale ослаблен
  'all-soft': `@keyframes settingsRise {
                       0%   { opacity: 0; transform: translate(-50%,-50%) scale(1.05); filter: blur(8px); }
                       100% { opacity: 1; transform: translate(-50%,-50%) scale(1); filter: blur(0); }
                     }
                     #settingsPanel { backdrop-filter: blur(8px) saturate(1.2) !important; -webkit-backdrop-filter: blur(8px) saturate(1.2) !important; }`,
  // всплытие translateY (без scale), bdf остаётся
  'rise-translate': `@keyframes settingsRise {
                       0%   { opacity: 0; transform: translate(-50%,-42%) scale(1); filter: blur(8px); }
                       100% { opacity: 1; transform: translate(-50%,-50%) scale(1); filter: blur(0); }
                     }`,
  // ИТОГ: opacity-only keyframes + scoped lucide (JS-часть)
  'fix-both': `@keyframes settingsRise {
                       0%   { opacity: 0; transform: translate(-50%,-50%) scale(1.06); }
                       100% { opacity: 1; transform: translate(-50%,-50%) scale(1); }
                     }
               #settingsPanel { backdrop-filter: none !important; -webkit-backdrop-filter: none !important; }`,
  // opacity + едва заметный scale, bdf остаётся
  'rise-scale-102': `@keyframes settingsRise {
                       0%   { opacity: 0; transform: translate(-50%,-50%) scale(1.02); }
                       100% { opacity: 1; transform: translate(-50%,-50%) scale(1); }
                     }`,
  // opacity + scale, но bdf заменён на статичный фон (без размытия подложки)
  'rise-scale-flatbg': `@keyframes settingsRise {
                       0%   { opacity: 0; transform: translate(-50%,-50%) scale(1.06); filter: blur(6px); }
                       100% { opacity: 1; transform: translate(-50%,-50%) scale(1); filter: blur(0); }
                     }
                     #settingsPanel { backdrop-filter: none !important; -webkit-backdrop-filter: none !important;
                                      background: rgba(24,26,34,0.92) !important; }`,
  // более короткая анимация (0.2s) с bdf — меньше времени на джанк
  'rise-fast': `#settingsPanel { animation-duration: 0.2s !important; }`,
  // слойная промоция: поможет ли will-change / translateZ(0)
  'rise-willchange': `#settingsPanel { will-change: transform, opacity, filter; transform: translate(-50%,-50%) translateZ(0); }`,
  // bdf убираем ТОЛЬКО на время анимации (класс .animating)
  'rise-bdf-lazy': `#settingsPanel.animating { backdrop-filter: none !important; -webkit-backdrop-filter: none !important; }`,
  'no-blur-panel': `#settingsPanel { backdrop-filter: none !important; -webkit-backdrop-filter: none !important; }`,
  'anim-only-opacity': `#settingsPanel { animation: none !important; transition: none !important; }
              #settingsPanel.rise { animation: settingsRise 0.4s cubic-bezier(0.4,0,0.2,1) !important; }`,
};

const JSPATCH = {
  baseline: '',
  'fix-both': `
    if (window.refreshLucideIcons && !window.__origRefresh) {
      window.__origRefresh = window.refreshLucideIcons;
      window.refreshLucideIcons = function () {
        if (typeof lucide === 'undefined') return;
        const pend = document.querySelectorAll('i[data-lucide]');
        if (!pend.length) return;
        if (pend.length > 60) return window.__origRefresh();
        lucide.createIcons({ nodes: pend });
      };
    }`,
};

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
  const frames = []; let last = 0; let raf = 0; let stop = false;
  const longTasks = [];
  let po = null;
  try {
    po = new PerformanceObserver(l => { for (const e of l.getEntries()) longTasks.push(Math.round(e.duration)); });
    po.observe({ entryTypes: ['longtask'] });
  } catch (e) { po = null; }

  // патч
  let st = document.getElementById('__abPatch');
  if (st) st.remove();
  if (${JSON.stringify(PATCH[MODE] || '')}) {
    st = document.createElement('style');
    st.id = '__abPatch';
    st.textContent = ${JSON.stringify(PATCH[MODE] || '')};
    document.head.appendChild(st);
  }
  const JS = ${JSON.stringify(JSPATCH[MODE] || '')};
  if (JS) (0, eval)(JS);

  function tick() {
    if (stop) return;
    const n = performance.now();
    if (last) frames.push(n - last);
    last = n;
    raf = requestAnimationFrame(tick);
  }
  raf = requestAnimationFrame(tick);

  const t0 = performance.now();
  const btn = document.querySelector('#settingsBtn, [onclick*="openSettings"]');
  if (btn) btn.click();

  setTimeout(() => {
    stop = true; cancelAnimationFrame(raf);
    if (po) { try { po.disconnect(); } catch (e) {} }
    if (st) st.remove();
    frames.shift();
    const s = frames.slice().sort((a, b) => a - b);
    const avg = frames.reduce((a, b) => a + b, 0) / (frames.length || 1);
    resolve({
      mode: '${MODE}',
      frames: frames.length,
      avgMs: +avg.toFixed(1),
      p95Ms: +(s[Math.floor(s.length * 0.95)] || 0).toFixed(1),
      maxMs: +(s[s.length - 1] || 0).toFixed(1),
      over33: frames.filter(f => f > 33).length,
      over50: frames.filter(f => f > 50).length,
      longTasks: longTasks.length,
      longestMs: longTasks.reduce((a, b) => Math.max(a, b), 0)
    });
  }, ${OBSERVE});
})`;

(async () => {
  const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  const page = list.find(t => t.type === 'page');
  if (!page) { console.log('нет page'); process.exit(1); }
  const cdp = await connect(page.webSocketDebuggerUrl);
  await cdp.send('Runtime.enable');
  const r = await cdp.send('Runtime.evaluate', { expression: EXPR, awaitPromise: true, returnByValue: true, timeout: 30000 });
  if (r.exceptionDetails) console.log('EXCEPTION:', r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  else console.log(JSON.stringify(r.result.value));
  cdp.close(); process.exit(0);
})().catch(e => { console.error('FAIL:', e.message); process.exit(1); });
