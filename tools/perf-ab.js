// Эксперимент: включает/выключает подозреваемые CSS-свойства и меряет FPS.
// Использование: node tools/perf-ab.js <baseline|no-backdrop|no-blur-panel|no-lucide|no-shadow|all-off> <сценарий> <сек>
const PORT = process.env.CDP_PORT || '9222';
const MODE = process.argv[2] || 'baseline';
const SCENARIO = process.argv[3] || 'tabs';
const SECONDS = parseInt(process.argv[4] || '6', 10);

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

const PATCH = {
  baseline: `/* как есть */`,
  'no-backdrop': `
    if (!window.__bdStyle) {
      window.__bdStyle = document.createElement('style');
      document.head.appendChild(window.__bdStyle);
      window.__bdPrev = window.__bdStyle.textContent;
    }
    window.__bdStyle.textContent = '* { backdrop-filter: none !important; -webkit-backdrop-filter: none !important; }';
  `,
  'no-blur-panel': `
    if (!window.__bdStyle) { window.__bdStyle = document.createElement('style'); document.head.appendChild(window.__bdStyle); }
    window.__bdStyle.textContent = '#settingsPanel, .settings-nav, .settings-panel, .glass { backdrop-filter: none !important; }';
  `,
  'no-shadow': `
    if (!window.__bdStyle) { window.__bdStyle = document.createElement('style'); document.head.appendChild(window.__bdStyle); }
    window.__bdStyle.textContent = '* { box-shadow: none !important; }';
  `,
  'no-transition': `
    if (!window.__bdStyle) { window.__bdStyle = document.createElement('style'); document.head.appendChild(window.__bdStyle); }
    window.__bdStyle.textContent = '*, *::before, *::after { transition: none !important; animation: none !important; }';
  `,
  'all-off': `
    if (!window.__bdStyle) { window.__bdStyle = document.createElement('style'); document.head.appendChild(window.__bdStyle); }
    window.__bdStyle.textContent = '* { backdrop-filter: none !important; -webkit-backdrop-filter: none !important; box-shadow: none !important; transition: none !important; animation: none !important; }';
  `,
  // Отключаем ПОЛНЫЙ скан документов lucide (главный кандидат в дорогие)
  'no-lucide': `
    if (window.refreshLucideIcons && !window.__origRefresh) {
      window.__origRefresh = window.refreshLucideIcons;
      window.refreshLucideIcons = function () { return; };
    }
  `,
  // lucide: конвертировать только добавленные узлы вместо всего документа
  'scoped-lucide': `
    if (window.refreshLucideIcons && !window.__origRefresh) {
      window.__origRefresh = window.refreshLucideIcons;
      window.refreshLucideIcons = function () {
        if (typeof lucide === 'undefined') return;
        const pend = document.querySelectorAll('i[data-lucide]');
        if (!pend.length) return;                       // ничего не предстоит — выходим сразу
        if (pend.length > 60) return window.__origRefresh();  // много — как раньше
        lucide.createIcons({ nodes: pend });
      };
    }
  `,
};

const ACTION = {
  tabs: `
    const tabs = document.querySelectorAll('.settings-nav-item, [onclick*="switchTab"]');
    if (tabs.length) { window.__i = ((window.__i||0)+1) % tabs.length; tabs[window.__i].click(); }
  `,
  settings: `
    if (!window.__once) { window.__once = true;
      const b = document.querySelector('#settingsBtn, [onclick*="openSettings"]'); if (b) b.click(); }
  `,
  openClose: `
    const b = document.querySelector('#settingsBtn, [onclick*="openSettings"]');
    if (b) b.click();
  `,
  idle: `/* */`,
};

const probe = `
new Promise((resolve) => {
  ${PATCH[MODE] || ''}
  const frames = []; let last = 0; let errors = 0;
  let raf = 0; let stopped = false;         // <-- останавливаем цикл, чтобы не оставлять "вечный" кликер
  const longTasks = [];
  const po = new PerformanceObserver(l => { for (const e of l.getEntries()) longTasks.push(Math.round(e.duration)); });
  try { po.observe({entryTypes:['longtask']}); } catch(e) {}
  function tick() {
    if (stopped) return;
    const n = performance.now();
    if (last) frames.push(n - last);
    last = n;
    try { ${ACTION[SCENARIO] || ''} } catch(e) { errors++; }
    raf = requestAnimationFrame(tick);
  }
  raf = requestAnimationFrame(tick);
  setTimeout(() => {
    stopped = true;
    cancelAnimationFrame(raf);
    // убираем инъецированный стиль, чтобы не загрязнять следующие замеры
    if (window.__bdStyle) { window.__bdStyle.remove(); window.__bdStyle = null; }
    if (window.__origRefresh) { window.refreshLucideIcons = window.__origRefresh; window.__origRefresh = null; }
    frames.shift();
    const s = frames.slice().sort((a,b)=>a-b);
    const avg = frames.reduce((a,b)=>a+b,0)/(frames.length||1);
    resolve({
      mode: '${MODE}', scenario: '${SCENARIO}',
      fps: +(1000/avg).toFixed(1),
      avgMs: +avg.toFixed(1),
      p95Ms: +(s[Math.floor(s.length*0.95)]||0).toFixed(1),
      maxMs: +(s[s.length-1]||0).toFixed(1),
      janky33: frames.filter(f=>f>33).length,
      frames: frames.length,
      longTasks: longTasks.length,
      longestMs: longTasks.reduce((a,b)=>Math.max(a,b),0),
      errors
    });
  }, ${SECONDS*1000});
})`;

(async () => {
  const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  const page = list.find((t) => t.type === 'page');
  if (!page) { console.log('нет page'); process.exit(1); }
  const cdp = await connect(page.webSocketDebuggerUrl);
  await cdp.send('Runtime.enable');
  const r = await cdp.send('Runtime.evaluate', { expression: probe, awaitPromise: true, returnByValue: true, timeout: (SECONDS+15)*1000 });
  if (r.exceptionDetails) console.log('EXCEPTION:', r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  else console.log(JSON.stringify(r.result.value));
  cdp.close(); process.exit(0);
})().catch(e => { console.error('FAIL:', e.message); process.exit(1); });
