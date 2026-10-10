// Находит ИСТОЧНИК бесконечной вставки в DOM: перехватывает innerHTML/appendChild
// и пишет стек вызовов (без lucide, т.к. он лишь следствие).
// Использование: node tools/perf-source.js [секунды]
const PORT = process.env.CDP_PORT || '9222';
const SECONDS = parseInt(process.argv[2] || '4', 10);

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

const DIAG = `
new Promise((resolve) => {
  const stacks = {};      // normalized stack -> count
  const innerHits = {};
  let installing = false;

  const norm = (st) => (st || '')
    .split('\\n').slice(1, 6)
    .map(s => s.trim().replace(/^at\\s+/, ''))
    .filter(s => !/lucide\\.min\\.js/.test(s))
    .join(' | ');

  // 1) innerHTML setter
  const desc = Object.getOwnPropertyDescriptor(Element.prototype, 'innerHTML');
  Object.defineProperty(Element.prototype, 'innerHTML', {
    get() { return desc.get.call(this); },
    set(v) {
      if (!installing) {
        const st = norm(new Error().stack);
        if (st) innerHits[st] = (innerHits[st] || 0) + 1;
      }
      return desc.set.call(this, v);
    },
    configurable: true
  });

  // 2) appendChild / insertBefore / replaceChild
  for (const m of ['appendChild', 'insertBefore', 'replaceChild', 'append', 'prepend']) {
    const orig = Node.prototype[m];
    if (!orig) continue;
    Node.prototype[m] = function (...args) {
      if (!installing) {
        const st = norm(new Error().stack);
        if (st && !/lucide\\.min\\.js/.test(st)) stacks[m + ': ' + st] = (stacks[m + ': ' + st] || 0) + 1;
      }
      return orig.apply(this, args);
    };
  }

  // 3) кто вызывает createIcons
  const oc = window.lucide.createIcons;
  const creatorStacks = {};
  window.lucide.createIcons = function (...a) {
    const st = norm(new Error().stack);
    if (st) creatorStacks[st] = (creatorStacks[st] || 0) + 1;
    return oc.apply(this, a);
  };

  // 4) таймеры: что нас запускает
  const timers = {};
  const osi = window.setInterval;
  window.setInterval = function (fn, d) {
    try {
      const nm = (fn && (fn.name || (fn.toString().match(/function\\s+(\\w+)/) || [])[1])) || 'anon';
      const key = nm + ' @ ' + d + 'ms';
      timers[key] = (timers[key] || 0) + 1;
    } catch (e) {}
    return osi.apply(this, arguments);
  };

  setTimeout(() => {
    installing = true;
    window.lucide.createIcons = oc;
    window.setInterval = osi;
    const top = (o, n) => Object.entries(o).sort((a,b)=>b[1]-a[1]).slice(0, n)
      .map(([k,v]) => ({ count: v, src: k }));

    resolve({
      seconds: ${SECONDS},
      createIconsCallers: top(creatorStacks, 5),
      appendMutators: top(stacks, 8),
      innerHTMLMutators: top(innerHits, 8),
      intervalsCreated: timers
    });
  }, ${SECONDS * 1000});
})`;

(async () => {
  const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  const page = list.find(t => t.type === 'page');
  if (!page) { console.log('нет page'); process.exit(1); }
  const cdp = await connect(page.webSocketDebuggerUrl);
  await cdp.send('Runtime.enable');
  const r = await cdp.send('Runtime.evaluate', { expression: DIAG, awaitPromise: true, returnByValue: true, timeout: (SECONDS + 10) * 1000 });
  if (r.exceptionDetails) console.log('EXCEPTION:', r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  else console.log(JSON.stringify(r.result.value, null, 2));
  cdp.close(); process.exit(0);
})().catch(e => { console.error('FAIL:', e.message); process.exit(1); });
