// Замер FPS во время типовых взаимодействий с UI.
// Использование: node tools/perf-interaction.js <сценарий>
// Сценарии: settings | mods | news | hover | idle
const PORT = process.env.CDP_PORT || '9222';
const SCENARIO = process.argv[2] || 'settings';
const SECONDS = parseInt(process.argv[3] || '6', 10);

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

// Что делаем в каждом кадре — имитация реального взаимодействия
const ACTIONS = {
  idle: `/* бездействие */`,
  settings: `
    if (!window.__onceSettings) {
      window.__onceSettings = true;
      const btn = document.querySelector('[onclick*="openSettings"], .icon-btn[title*="астрос"], #settingsBtn, [data-action="settings"]');
      if (btn) btn.click();
    }
  `,
  settingsClose: `
    if (!window.__onceSettings) {
      window.__onceSettings = true;
      const btn = document.querySelector('[onclick*="openSettings"], .icon-btn[title*="астрос"], #settingsBtn, [data-action="settings"]');
      if (btn) btn.click();
      setTimeout(() => { const b2 = document.querySelector('[onclick*="closeSettings"], .s-close, #settingsClose'); if (b2) b2.click(); }, 500);
    }
  `,
  mods: `
    const btn = document.querySelector('[onclick*="openFolder"], [onclick*="mods"], [onclick*="Mods"]');
    if (btn) btn.click();
  `,
  news: `
    const p = document.querySelector('.right-panel');
    if (p) p.classList.toggle('open');
  `,
  hover: `
    const els = document.querySelectorAll('.settings-tile, .s-tile, .glass-tile, .card, button, .icon-btn');
    if (els.length) {
      window.__hoverIdx = ((window.__hoverIdx || 0) + 1) % els.length;
      const el = els[window.__hoverIdx];
      el.dispatchEvent(new MouseEvent('mouseenter', { bubbles: false }));
      el.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
    }
  `,
  // Переключение вкладок в настройках (каждый кадр)
  tabs: `
    if (!window.__onceSettings) {
      window.__onceSettings = true;
      const b = document.querySelector('[onclick*="openSettings"], #settingsBtn');
      if (b) b.click();
    }
    const tabs = document.querySelectorAll('.settings-nav-item, .settings-tab, [onclick*="switchTab"]');
    if (tabs.length) {
      window.__tabIdx = ((window.__tabIdx || 0) + 1) % tabs.length;
      tabs[window.__tabIdx].click();
    }
  `,
  // Ресайз окна как при перетаскивании края
  resize: `
    if (window.__TAURI__ && window.__TAURI__.window) {
      const w = window.__TAURI__.window.getCurrentWindow();
      const base = window.__resizeBase || (window.__resizeBase = { w: 1285, h: 732 });
      const t = (window.__rn = ((window.__rn || 0) + 1) % 40);
      const dw = t < 20 ? t : (40 - t);
      w.setSize(new (window.__TAURI__.window.LogicalSize)(base.w + dw, base.h + Math.round(dw / 2))).catch(() => {});
    }
  `,
  // Скролл в длинном списке
  scroll: `
    const sc = document.querySelector('.settings-content, .settings-tab-content[style*="flex"], .mods-list, .news-list, .pane');
    if (sc) sc.scrollTop = (sc.scrollTop + 40) % Math.max(1, sc.scrollHeight);
  `,
};

const probe = `
new Promise((resolve) => {
  const longTasks = [];
  const frames = [];
  let last = performance.now();
  let running = true;
  let actionErrors = 0;
  window.__onceSettings = false;

  const po = new PerformanceObserver((list) => {
    for (const e of list.getEntries()) longTasks.push(Math.round(e.duration));
  });
  try { po.observe({ entryTypes: ['longtask'] }); } catch (e) {}

  function tick() {
    const now = performance.now();
    if (last) frames.push(now - last);
    last = now;
    try { ${ACTIONS[SCENARIO] || ''} } catch (e) { actionErrors++; }
    if (running) requestAnimationFrame(tick);
  }
  requestAnimationFrame(tick);

  setTimeout(() => {
    running = false;
    po.disconnect();
    frames.shift();
    const sorted = frames.slice().sort((a, b) => a - b);
    const avg = frames.length ? frames.reduce((a, b) => a + b, 0) / frames.length : 0;
    const p95 = sorted[Math.floor(sorted.length * 0.95)] || 0;
    resolve({
      scenario: '${SCENARIO}',
      fpsAvg: +(1000 / (avg || 16.7)).toFixed(1),
      frameAvgMs: +avg.toFixed(1),
      frameP95Ms: +p95.toFixed(1),
      frameMaxMs: +(sorted[sorted.length - 1] || 0).toFixed(1),
      jankyFramesOver33ms: frames.filter(f => f > 33).length,
      framesOver100ms: frames.filter(f => f > 100).length,
      totalFrames: frames.length,
      longTasks: longTasks.length,
      longestTaskMs: longTasks.reduce((a, b) => Math.max(a, b), 0),
      actionErrors,
      heapMB: Math.round(performance.memory ? performance.memory.usedJSHeapSize / 1048576 : 0),
      domNodes: document.getElementsByTagName('*').length
    });
  }, ${SECONDS * 1000});
})`;

(async () => {
  const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  const page = list.find((t) => t.type === 'page');
  if (!page) { console.log('нет page target'); process.exit(1); }

  const cdp = await connect(page.webSocketDebuggerUrl);
  await cdp.send('Runtime.enable');

  console.log(`Сценарий "${SCENARIO}", ${SECONDS}c...`);
  const r = await cdp.send('Runtime.evaluate', {
    expression: probe,
    awaitPromise: true,
    returnByValue: true,
    timeout: (SECONDS + 10) * 1000,
  });
  if (r.exceptionDetails) {
    console.log('EXCEPTION:', r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  } else {
    console.log(JSON.stringify(r.result.value, null, 2));
  }
  cdp.close();
  process.exit(0);
})().catch((e) => { console.error('FAIL:', e.message); process.exit(1); });
