// Замер производительности страницы: long tasks, FPS, layout thrashing.
// Использование: node tools/perf-measure.js [секунды]
const PORT = process.env.CDP_PORT || '9222';
const SECONDS = parseInt(process.argv[2] || '5', 10);

async function targets() {
  const r = await fetch(`http://127.0.0.1:${PORT}/json/list`);
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
    ws.onerror = (e) => reject(new Error('WS: ' + (e.message || e)));
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

const probe = `
new Promise((resolve) => {
  const longTasks = [];
  const frames = [];
  let last = performance.now();
  let running = true;

  const po = new PerformanceObserver((list) => {
    for (const e of list.getEntries()) longTasks.push({ d: Math.round(e.duration), name: e.name });
  });
  try { po.observe({ entryTypes: ['longtask'] }); } catch (e) {}

  function tick() {
    const now = performance.now();
    frames.push(now - last);
    last = now;
    if (running) requestAnimationFrame(tick);
  }
  requestAnimationFrame(tick);

  // фоновая активность: сколько раз перерисовывается что-то тяжёлое
  let forced = 0;
  const t0 = performance.now();
  const probeInt = setInterval(() => {
    const s = performance.now();
    void document.body.offsetHeight;           // вынужденный reflow
    const cost = performance.now() - s;
    if (cost > 4) forced++;
    if (performance.now() - t0 >= ${(SECONDS * 1000)}) {
      clearInterval(probeInt);
      running = false;
      po.disconnect();

      frames.shift(); // первый кадр — стартовый
      const sorted = frames.slice().sort((a, b) => a - b);
      const avg = frames.reduce((a, b) => a + b, 0) / (frames.length || 1);
      const p95 = sorted[Math.floor(sorted.length * 0.95)] || 0;
      const max = sorted[sorted.length - 1] || 0;
      const janky = frames.filter(f => f > 33).length;
      const veryBad = frames.filter(f => f > 100).length;

      resolve({
        seconds: ${SECONDS},
        fpsAvg: +(1000 / avg).toFixed(1),
        frameAvgMs: +avg.toFixed(1),
        frameP95Ms: +p95.toFixed(1),
        frameMaxMs: +max.toFixed(1),
        jankyFramesOver33ms: janky,
        framesOver100ms: veryBad,
        totalFrames: frames.length,
        longTasks: longTasks.length,
        longTaskTotalMs: longTasks.reduce((a, b) => a + b.d, 0),
        longestTaskMs: longTasks.reduce((a, b) => Math.max(a, b.d), 0),
        forcedReflowCostly: forced,
        heapMB: Math.round(performance.memory ? performance.memory.usedJSHeapSize / 1048576 : 0),
        domNodes: document.getElementsByTagName('*').length
      });
    }
  }, ${(SECONDS * 1000) / 40});
})`;

(async () => {
  const list = await targets();
  const page = list.find((t) => t.type === 'page');
  if (!page) { console.log('нет page target'); process.exit(1); }

  const cdp = await connect(page.webSocketDebuggerUrl);
  await cdp.send('Runtime.enable');

  console.log(`Меряю ${SECONDS}c...`);
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
