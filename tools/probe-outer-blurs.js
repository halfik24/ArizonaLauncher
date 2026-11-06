// Что происходит с 7 внешними блюрами во время анимации открытия настроек:
// принадлежность к контейнерам (right-panel/status-bar/...), меняется ли
// rect по кадрам (значит — пересчёт блюра каждый кадр) и z-index относительно
// панели настроек.
// node tools/probe-outer-blurs.js
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
    const c0 = document.querySelector('[onclick*="closeSettings"], .s-close, #settingsClose');
    if (p.style.display !== 'none' && c0) { c0.click(); await sleep(700); }

    const IDS = ['tbTitle','tbButtons','phTrigger','phDropdown','newsHandle','feedbackFab'];
    const CLS = ['version-stable'];
    const find = (id) => document.getElementById(id) ||
      document.querySelector('.' + id.split(' ')[0]);
    const sel = (el) => {
      let s = el.tagName.toLowerCase();
      if (el.id) s += '#' + el.id;
      if (el.className && typeof el.className === 'string')
        s += '.' + el.className.trim().split(/\\s+/).slice(0,2).join('.');
      return s;
    };
    const container = (el) => {
      const names = ['right-panel','left-panel','status-bar','title-bar','settings-panel','news'];
      let n = el.parentElement, chain = [];
      while (n && n !== document.body) {
        chain.push(sel(n));
        n = n.parentElement;
      }
      const hit = names.find(c => chain.some(x => x.includes(c)));
      return { hit: hit || '—', chain: chain.slice(0, 4).join(' < ') };
    };
    const zOf = (el) => {
      // порядок перекрытия с панелью настроек: кто выше
      const pz = getComputedStyle(p).zIndex, ez = getComputedStyle(el).zIndex;
      const pe = p.contains(el);
      return pe ? 'внутри панели' :
        'el.z=' + ez + ' vs panel.z=' + pz;
    };

    // снять снимки 3 раза во время анимации
    const track = {};
    for (const k of [...IDS, ...CLS]) {
      const el = find(k);
      if (el && getComputedStyle(el).backdropFilter !== 'none') track[k] = { el, shots: [] };
    }

    document.getElementById('settingsBtn').click();
    for (let i = 0; i < 6; i++) {
      await sleep(50);
      for (const k in track) {
        const r = track[k].el.getBoundingClientRect();
        track[k].shots.push(Math.round(r.x) + ',' + Math.round(r.y));
      }
    }
    await sleep(700);
    const c = document.querySelector('[onclick*="closeSettings"], .s-close, #settingsClose');
    if (c) { c.click(); await sleep(700); }

    const res = [];
    for (const k in track) {
      const el = track[k].el;
      const cont = container(el);
      const shots = track[k].shots;
      res.push({
        sel: k,
        container: cont.hit,
        parentChain: cont.chain,
        z: zOf(el),
        moves: new Set(shots).size > 1,
        shots: shots.join(' | '),
        rect: (r => Math.round(r.width)+'x'+Math.round(r.height))(el.getBoundingClientRect()),
      });
    }
    return res;
  })()`);

  for (const x of out) {
    console.log(`${x.sel.padEnd(16)} контейнер=${x.container.padEnd(13)} ${x.z}`);
    console.log(`   rect=${x.rect}  движется=${x.moves}`);
    console.log(`   родители: ${x.parentChain}`);
    console.log(`   кадры: ${x.shots}`);
  }
  cdp.close();
  process.exit(0);
})().catch((e) => { console.error('FAIL:', e.message); process.exit(1); });
