// Замер с реалистичным темпом взаимодействия (как человек, а не каждый кадр).
// Использование: node tools/perf-human.js <scenario> <clicksPerSec>
const PORT = process.env.CDP_PORT || '9222';
const SCENARIO = process.argv[2] || 'tabs';
const CPS = parseFloat(process.argv[3] || '2');   // кликов в секунду
const SECONDS = parseInt(process.argv[4] || '8', 10);

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

const ACTIONS = {
  tabs: () => `
    const tabs = document.querySelectorAll('.settings-nav-item, [onclick*="switchTab"]');
    if (tabs.length) { window.__i = ((window.__i||0)+1) % tabs.length; tabs[window.__i].click(); }
  `,
  openClose: () => `
    const b = document.querySelector('#settingsBtn, [onclick*="openSettings"]');
    if (b) b.click();
  `,
  idle: () => `/* */`,
};

(async () => {
  const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  const page = list.find(t => t.type === 'page');
  if (!page) { console.log('нет page'); process.exit(1); }
  const cdp = await connect(page.webSocketDebuggerUrl);
  await cdp.send('Runtime.enable');

  const action = (ACTIONS[SCENARIO] || ACTIONS.idle)();
  const interval = Math.round(1000 / CPS);

  const probe = `
new Promise((resolve) => {
  const frames = []; let last = 0;
  const longTasks = []; const taskStarts = [];
  const po = new PerformanceObserver(l => { for (const e of l.getEntries()) longTasks.push(Math.round(e.duration)); });
  try { po.observe({entryTypes:['longtask']}); } catch(e) {}

  let clickCount = 0;
  const iv = setInterval(() => { try { ${action} clickCount++; } catch(e){} }, ${interval});

  function tick() { const n = performance.now(); if (last) frames.push(n - last); last = n; requestAnimationFrame(tick); }
  requestAnimationFrame(tick);

  setTimeout(() => {
    clearInterval(iv); frames.shift(); po.disconnect();
    const s = frames.slice().sort((a,b)=>a-b);
    const avg = frames.reduce((a,b)=>a+b,0)/(frames.length||1);
    const p99 = s[Math.floor(s.length*0.99)] || 0;
    resolve({
      scenario: '${SCENARIO}', clicksPerSec: ${CPS}, clicks: clickCount,
      fps: +(1000/avg).toFixed(1), avgMs: +avg.toFixed(1),
      p95Ms: +(s[Math.floor(s.length*0.95)]||0).toFixed(1),
      p99Ms: +p99.toFixed(1), maxMs: +(s[s.length-1]||0).toFixed(1),
      janky33: frames.filter(f=>f>33).length,
      framesOver50: frames.filter(f=>f>50).length,
      frames: frames.length,
      longTasks: longTasks.length, longestMs: longTasks.reduce((a,b)=>Math.max(a,b),0),
      heapMB: Math.round(performance.memory.usedJSHeapSize/1048576)
    });
  }, ${SECONDS*1000});
})`;

  const r = await cdp.send('Runtime.evaluate', { expression: probe, awaitPromise: true, returnByValue: true, timeout: (SECONDS+15)*1000 });
  if (r.exceptionDetails) console.log('EXCEPTION:', r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  else console.log(JSON.stringify(r.result.value, null, 2));
  cdp.close(); process.exit(0);
})().catch(e => { console.error('FAIL:', e.message); process.exit(1); });
