// Функциональная проверка режимов анимации настроек: класс .anim-rise
// ставится/снимается, backdrop-блюр ведёт себя по режиму (pop/smooth — снят
// только во время анимации; fade/full — снят на время анимации, стекло с
// первого кадра даёт запечённый слой #settingsBaked — идея №1), закрытие
// работает.
// Использование: node tools/verify-anim-fix.js
// Режим определяется по классу anim-* на <html>.
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
    // режим можно задать аргументом: node tools/verify-anim-fix.js fade|pop|smooth|full
    const want = '${(process.argv[2] || '').replace(/[^a-z]/g, '')}';
    if (want && typeof window.setAnimMode === 'function') window.setAnimMode(want);
    const de = document.documentElement;
    const mode = ['fade','pop','smooth','full']
      .find(m => de.classList.contains('anim-' + m)) || 'unknown';
    const p = document.getElementById('settingsPanel');
    const bd = () => getComputedStyle(p).backdropFilter;
    const an = () => getComputedStyle(p).animationName;
    // Запечённый фон-блюр (идея №1): block только в режимах full/fade и
    // только пока на body стоит settings-anim (0.55с).
    const baked = () => {
      const c = document.getElementById('settingsBaked');
      return c ? getComputedStyle(c).display : 'missing';
    };
    const res = { mode, steps: [] };
    // закрываем, если открыто
    const c0 = document.querySelector('[onclick*="closeSettings"], .s-close, #settingsClose');
    if (p.style.display !== 'none' && c0) { c0.click(); await sleep(700); }

    res.closedBefore = (p.style.display === 'none');
    const extBd = () => getComputedStyle(document.getElementById('newsHandle') || p).backdropFilter;
    // ОТКРЫТИЕ
    document.getElementById('settingsBtn').click();
    await sleep(80);
    res.at80ms = {
      display: p.style.display,
      animRise: p.classList.contains('anim-rise'),
      backdrop: bd(),
      animName: an(),
      bodyAnim: document.body.classList.contains('settings-anim'),
      extBackdrop: extBd(),
      baked: baked(),
    };
    await sleep(650);   // 730мс: анимация + (в smooth) transition уже кончились
    res.at730ms = {
      animRise: p.classList.contains('anim-rise'),
      backdrop: bd(),
      baked: baked(),
    };
    // ЗАКРЫТИЕ
    const c = document.querySelector('[onclick*="closeSettings"], .s-close, #settingsClose');
    if (!c) { res.error = 'close button missing'; return res; }
    c.click();
    await sleep(120);
    res.closeAt120ms = {
      closing: p.classList.contains('closing'),
      backdrop: bd(),
      bodyAnim: document.body.classList.contains('settings-anim'),
      extBackdrop: extBd(),
      baked: baked(),
    };
    await sleep(600);
    res.afterClose = {
      display: p.style.display,
      closing: p.classList.contains('closing'),
      animRise: p.classList.contains('anim-rise'),
      backdrop: bd(),
      bodyAnim: document.body.classList.contains('settings-anim'),
      extBackdrop: extBd(),
      baked: baked(),
    };
    return res;
  })()`);

  const mode = out.mode;
  // ожидание по режиму: блюр во время анимации
  const offDuringAnim = (mode === 'pop' || mode === 'smooth');
  const checks = [];
  const ok = (name, cond, info) => checks.push({ name, pass: !!cond, info });
  ok('режим определён', mode !== 'unknown', mode);
  if (mode === 'fade') {
    ok('fade: анимация settingsRiseFade',
       out.at80ms?.animName === 'settingsRiseFade', out.at80ms?.animName);
  } else {
    ok(mode + ': анимация settingsRise',
       out.at80ms?.animName === 'settingsRise', out.at80ms?.animName);
  }
  ok('панель была закрыта до старта', out.closedBefore, String(out.closedBefore));
  ok('открытие: display=flex', out.at80ms?.display === 'flex', out.at80ms?.display);
  ok('открытие: .anim-rise стоит', out.at80ms?.animRise === true, String(out.at80ms?.animRise));
  if (offDuringAnim) {
    ok('открытие: backdrop снят во время анимации',
       out.at80ms?.backdrop === 'none', out.at80ms?.backdrop);
    ok('потом: блюр включён', out.at730ms?.backdrop !== 'none', out.at730ms?.backdrop);
    ok('закрытие: backdrop снят во время sink',
       out.closeAt120ms?.backdrop === 'none', out.closeAt120ms?.backdrop);
    ok('pop/smooth: запечённый слой НЕ показывается',
       out.at80ms?.baked === 'none', out.at80ms?.baked);
  } else {
    // Идея №1 (full/fade): во время анимации backdrop снят, стекло с первого
    // кадра даёт запечённый слой #settingsBaked; потом — обычный backdrop.
    ok('открытие: backdrop снят — стекло даёт запечённый слой',
       out.at80ms?.backdrop === 'none', out.at80ms?.backdrop);
    ok('открытие: запечённый слой виден',
       out.at80ms?.baked === 'block', out.at80ms?.baked);
    ok('потом: backdrop включён (живое стекло)',
       out.at730ms?.backdrop !== 'none', out.at730ms?.backdrop);
    ok('потом: запечённый слой скрыт',
       out.at730ms?.baked === 'none', out.at730ms?.baked);
    ok('закрытие: backdrop снят во время sink',
       out.closeAt120ms?.backdrop === 'none', out.closeAt120ms?.backdrop);
    ok('закрытие: запечённый слой виден',
       out.closeAt120ms?.baked === 'block', out.closeAt120ms?.baked);
  }
  ok('через 730мс: .anim-rise снят', out.at730ms?.animRise === false, String(out.at730ms?.animRise));
  // внешние слои: блюр гаснет на время анимации (body.settings-anim) и возвращается
  ok('открытие: body.settings-anim стоит', out.at80ms?.bodyAnim === true, String(out.at80ms?.bodyAnim));
  ok('открытие: внешний блюр (#newsHandle) снят', out.at80ms?.extBackdrop === 'none', out.at80ms?.extBackdrop);
  ok('закрытие: body.settings-anim стоит', out.closeAt120ms?.bodyAnim === true, String(out.closeAt120ms?.bodyAnim));
  ok('после закрытия: body.settings-anim снят', out.afterClose?.bodyAnim === false, String(out.afterClose?.bodyAnim));
  ok('после закрытия: внешний блюр восстановлен', out.afterClose?.extBackdrop !== 'none', out.afterClose?.extBackdrop);
  ok('закрытие: .closing стоит', out.closeAt120ms?.closing === true, String(out.closeAt120ms?.closing));
  ok('после закрытия: display=none', out.afterClose?.display === 'none', out.afterClose?.display);
  ok('после закрытия: .closing снят', out.afterClose?.closing === false, String(out.afterClose?.closing));
  ok('после закрытия: блюр восстановлен', out.afterClose?.backdrop !== 'none', out.afterClose?.backdrop);
  ok('после закрытия: запечённый слой скрыт', out.afterClose?.baked === 'none', out.afterClose?.baked);

  console.log(JSON.stringify({ steps: out, checks }, null, 2));
  const failed = checks.filter((c) => !c.pass);
  console.log(failed.length ? `FAIL: ${failed.length}/${checks.length}`
                            : `PASS: all ${checks.length} checks (${mode})`);
  cdp.close();
  process.exit(failed.length ? 1 : 0);
})().catch((e) => { console.error('FAIL:', e.message); process.exit(1); });
