// Проверка кандидата фикса refreshLucideIcons (scoped lucide).
// Использование: node tools/test-lucide-fix.js
const PORT = process.env.CDP_PORT || '9222';

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
(() => {
  const R = {};
  const q = (s) => document.querySelectorAll(s).length;

  // ── Кандидат фикса ──
  function refreshScoped() {
    if (typeof lucide === 'undefined') return;
    const pending = document.querySelectorAll('i[data-lucide]');
    if (!pending.length) return;                       // нечего делать
    pending.forEach(el => el.setAttribute('data-lucide-tmp', el.getAttribute('data-lucide')));
    lucide.createIcons({ nameAttr: 'data-lucide-tmp' });
    document.querySelectorAll('svg[data-lucide-tmp]').forEach(s => s.removeAttribute('data-lucide-tmp'));
  }

  // Помечаем существующие SVG
  const existing = [...document.querySelectorAll('svg[data-lucide]')];
  existing.forEach(s => { s.__m = true; });
  R.existingMarked = existing.length;

  // 1) вызов ВХОЛОСТУЮ
  let t = performance.now();
  refreshScoped();
  R.idleMs = +(performance.now() - t).toFixed(2);
  R.survivedAfterIdle = [...document.querySelectorAll('svg[data-lucide]')].filter(s => s.__m).length;

  // 2) вставка новых <i> и конвертация
  const box = document.createElement('div');
  box.id = '__fixtest';
  box.style.cssText = 'position:fixed;left:-9999px;top:0';
  box.innerHTML = "<i data-lucide='rocket'></i><i data-lucide='shield'></i><i data-lucide='bug'></i>";
  document.body.appendChild(box);
  R.iBefore = box.querySelectorAll('i[data-lucide]').length;

  t = performance.now();
  refreshScoped();
  R.workMs = +(performance.now() - t).toFixed(2);

  R.iAfter = box.querySelectorAll('i[data-lucide]').length;
  R.svgInBox = box.querySelectorAll('svg[data-lucide]').length;
  R.names = [...box.querySelectorAll('svg[data-lucide]')].map(s => s.getAttribute('data-lucide'));
  R.classes = [...box.querySelectorAll('svg[data-lucide]')].map(s => s.getAttribute('class'));
  R.svgBox = box.innerHTML.slice(0, 400);
  R.strayTmp = document.querySelectorAll('[data-lucide-tmp]').length;
  R.survivedAfterWork = [...document.querySelectorAll('svg[data-lucide]')].filter(s => s.__m).length;

  // 3) повторный вызов вхолостую (после конвертации)
  t = performance.now();
  refreshScoped();
  R.idleAfterWorkMs = +(performance.now() - t).toFixed(2);

  // 4) сравнение со старым поведением
  t = performance.now();
  lucide.createIcons();
  R.oldCallMs = +(performance.now() - t).toFixed(2);
  R.survivedAfterOld = [...document.querySelectorAll('svg[data-lucide]')].filter(s => s.__m).length;

  box.remove();
  return R;
})()`;

(async () => {
  const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  const page = list.find(t => t.type === 'page');
  if (!page) { console.log('нет page'); process.exit(1); }
  const cdp = await connect(page.webSocketDebuggerUrl);
  await cdp.send('Runtime.enable');
  const r = await cdp.send('Runtime.evaluate', { expression: EXPR, awaitPromise: true, returnByValue: true, timeout: 30000 });
  if (r.exceptionDetails) console.log('EXCEPTION:', r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  else console.log(JSON.stringify(r.result.value, null, 2));
  cdp.close(); process.exit(0);
})().catch(e => { console.error('FAIL:', e.message); process.exit(1); });
