// Снять CPU-профиль страницы на N секунд и вывести горячие функции.
// Использование: node tools/perf-cpu.js [секунды]
const PORT = process.env.CDP_PORT || '9222';
const SECONDS = parseInt(process.argv[2] || '5', 10);

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

(async () => {
  const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  const page = list.find(t => t.type === 'page');
  if (!page) { console.log('нет page'); process.exit(1); }
  const cdp = await connect(page.webSocketDebuggerUrl);

  await cdp.send('Profiler.enable');
  await cdp.send('Profiler.setSamplingInterval', { interval: 200 }); // мкс
  await cdp.send('Profiler.start');

  console.log(`Снимаю профиль ${SECONDS}c (покоя)...`);
  await new Promise(r => setTimeout(r, SECONDS * 1000));

  const { profile } = await cdp.send('Profiler.stop');

  // агрегация self-time по функциям
  const nodes = new Map();
  profile.nodes.forEach(n => nodes.set(n.id, n));

  const selfTime = new Map();   // key -> microsec
  const total = new Map();
  const label = (n) => {
    const f = n.callFrame;
    const name = f.functionName || '(anonymous)';
    const url = f.url ? f.url.split('/').slice(-2).join('/') : '';
    return `${name}${url ? ' @ ' + url + ':' + (f.lineNumber + 1) : ''}`;
  };

  const deltas = profile.samples.map((sid, i) => ({
    sid, dt: (profile.timeDeltas[i] || 0),
  }));
  const byIdSelf = new Map();
  for (const { sid, dt } of deltas) {
    byIdSelf.set(sid, (byIdSelf.get(sid) || 0) + dt);
    const n = nodes.get(sid);
    if (n) {
      const key = label(n);
      selfTime.set(key, (selfTime.get(key) || 0) + dt);
    }
  }

  // total time (с рекурсией по children)
  const childrenOf = new Map();
  profile.nodes.forEach(n => {
    if (n.parent) {
      if (!childrenOf.has(n.parent)) childrenOf.set(n.parent, []);
      childrenOf.get(n.parent).push(n.id);
    }
  });
  function totalOf(id, seen = new Set()) {
    if (seen.has(id)) return 0;
    seen.add(id);
    let t = byIdSelf.get(id) || 0;
    for (const c of (childrenOf.get(id) || [])) t += totalOf(c, seen);
    return t;
  }
  profile.nodes.forEach(n => {
    const key = label(n);
    total.set(key, (total.get(key) || 0) + totalOf(n.id));
  });

  const sum = [...selfTime.values()].reduce((a, b) => a + b, 0);
  console.log(`\nВсего семплов: ${profile.samples.length}, суммарно: ${(sum / 1000).toFixed(0)} ms\n`);
  console.log('=== TOP self-time (сколько главный поток занят именно этой функцией) ===');
  [...selfTime.entries()].sort((a, b) => b[1] - a[1]).slice(0, 25).forEach(([k, v]) => {
    const pct = (v / sum * 100).toFixed(1);
    console.log(`  ${String((v / 1000).toFixed(0)).padStart(6)}ms  ${String(pct).padStart(5)}%  ${k}`);
  });

  cdp.close(); process.exit(0);
})().catch(e => { console.error('FAIL:', e.message); process.exit(1); });
