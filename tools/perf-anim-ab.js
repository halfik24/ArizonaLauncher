// A/B-замер анимации открытия/закрытия настроек на медленной GPU-прокси.
//
// Использование: node tools/perf-anim-ab.js [cycles]
// Требует запущенного exe с CDP-портом (собрать с TAURI_CONFIG).
//
// Что делает: N раз открывает и закрывает настройки, считая время кадров
// (rAF), затем применяет варианты CSS через инъекцию <style> и повторяет
// замер — baseline против вариантов без дорогих блюров.
'use strict';

const PORT = process.env.CDP_PORT || '9222';
const CYCLES = parseInt(process.argv[2] || '4', 10);

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

async function evaluate(cdp, expression, timeout = 60000) {
  const r = await cdp.send('Runtime.evaluate', {
    expression, awaitPromise: true, returnByValue: true, timeout,
  });
  if (r.exceptionDetails) {
    throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  }
  return r.result.value;
}

// Замер: циклы открыть/закрыть, сбор длин кадров через rAF.
function benchExpr(cycles) {
  return `(() => {
    const sleep = (ms) => new Promise(r => setTimeout(r, ms));
    const findOpen = () => document.querySelector(
      '[onclick*="openSettings"], #settingsBtn, [data-action="settings"]');
    const findClose = () => document.querySelector(
      '[onclick*="closeSettings"], .s-close, #settingsClose');
    const frames = [];
    let last = 0, raf = 0, running = true;
    const longTasks = [];
    const po = new PerformanceObserver(l => {
      for (const e of l.getEntries()) longTasks.push(Math.round(e.duration));
    });
    try { po.observe({ entryTypes: ['longtask'] }); } catch (e) {}
    const tick = () => {
      if (!running) return;
      const n = performance.now();
      if (last) frames.push({ t: n, dt: n - last });
      last = n;
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return (async () => {
      const open = findOpen(), close = findClose();
      if (!open || !close) {
        running = false; cancelAnimationFrame(raf); po.disconnect();
        return { error: 'knopki ne najdeny', open: !!open, close: !!close };
      }
      const marks = [];
      // Классы анимации ставит сам бенч ПЕРЕД кликом: реальные обработчики
      // привязаны через addEventListener (патч window.openSettings не цепляется).
      const mark = () => {
        document.body.classList.add('settings-anim');
        const p = document.getElementById('settingsPanel');
        if (p) p.classList.add('anim-rise');
        clearTimeout(window.__abAnimT);
        window.__abAnimT = setTimeout(() => {
          document.body.classList.remove('settings-anim');
          const pp = document.getElementById('settingsPanel');
          if (pp) pp.classList.remove('anim-rise');
        }, 500);
      };
      for (let i = 0; i < ${cycles}; i++) {
        const tOpen = performance.now();
        marks.push({ kind: 'open', t: tOpen });
        mark();
        open.click();
        await sleep(700);          // анимация 0.4s + запас
        const c = findClose();
        if (c) { marks.push({ kind: 'close', t: performance.now() }); mark(); c.click(); }
        await sleep(700);
      }
      const tEnd = performance.now();
      running = false; cancelAnimationFrame(raf);
      po.disconnect();
      // классификация кадров: анимация открытия (0-400мс), статика открытия
      // (400мс → клик закрытия), анимация закрытия (0-400мс), хвост
      const phase = (t) => {
        let last = null;
        for (const m of marks) { if (m.t <= t) last = m; }
        if (!last) return 'before';
        const d = t - last.t;
        if (last.kind === 'open')  return d < 400 ? 'open-anim'  : 'open-static';
        return d < 400 ? 'close-anim' : 'after';
      };
      const phases = {};
      for (const f of frames) {
        if (f.dt > 500) continue;   // паузы между циклами
        const p = phase(f.t);
        (phases[p] = phases[p] || []).push(f.dt);
      }
      const stat = (a) => {
        if (!a || !a.length) return null;
        const s = a.slice().sort((x, y) => x - y);
        const pct = (p) => +(s[Math.min(s.length - 1, Math.floor(s.length * p))] || 0).toFixed(1);
        return {
          n: a.length,
          avgMs: +(a.reduce((x, y) => x + y, 0) / a.length).toFixed(1),
          p95Ms: pct(0.95), maxMs: pct(1),
          janky33: a.filter(f => f > 33).length,
          over50: a.filter(f => f > 50).length,
        };
      };
      const all = Object.values(phases).flat();
      const s = all.slice().sort((a, b) => a - b);
      const pct = (p) => +(s[Math.min(s.length - 1, Math.floor(s.length * p))] || 0).toFixed(1);
      return {
        frames: all.length,
        avgMs: +(all.length ? all.reduce((a, b) => a + b, 0) / all.length : 0).toFixed(1),
        p50Ms: pct(0.5), p95Ms: pct(0.95), maxMs: pct(1),
        janky33: all.filter(f => f > 33).length,
        over50: all.filter(f => f > 50).length,
        over100: all.filter(f => f > 100).length,
        byPhase: {
          'open-anim':   stat(phases['open-anim']),
          'open-static': stat(phases['open-static']),
          'close-anim':  stat(phases['close-anim']),
          'after':       stat(phases['after']),
        },
        longTasks: longTasks.length,
        longestMs: longTasks.reduce((a, b) => Math.max(a, b), 0),
      };
    })();
  })()`;
}

// Варианты CSS (инъекция; переопределение keyframes тем же именем выигрывает
// у более раннего правила в index.html).
const VARIANTS = [
  {
    name: 'baseline',
    css: '',
  },
  {
    // 1) убираем анимируемый filter: blur() из rise/sink — остаются
    //    только transform + opacity (композиторный путь)
    name: 'no-element-blur',
    css: `
      @keyframes settingsRise {
        0%   { opacity: 0; transform: translate(-50%, -50%) scale(1.08); }
        100% { opacity: 1; transform: translate(-50%, -50%) scale(1); }
      }
      @keyframes settingsSink {
        0%   { opacity: 1; transform: translate(-50%, -50%) scale(1); }
        100% { opacity: 0; transform: translate(-50%, -50%) scale(1.08); }
      }
      @keyframes settingsSinkWp {
        0%   { opacity: 1; transform: translate(calc(12px - 50vw), -50%) scale(1); }
        100% { opacity: 0; transform: translate(calc(12px - 50vw), -50%) scale(1.08); }
      }`,
  },
  {
    // 2) + не пересчитываем backdrop-блюры во время анимации открытия/закрытия
    //    (панель почти непрозрачная, разница невидима, цена — каждый кадр)
    name: 'no-backdrop-during-anim',
    css: `
      @keyframes settingsRise {
        0%   { opacity: 0; transform: translate(-50%, -50%) scale(1.08); }
        100% { opacity: 1; transform: translate(-50%, -50%) scale(1); }
      }
      @keyframes settingsSink {
        0%   { opacity: 1; transform: translate(-50%, -50%) scale(1); }
        100% { opacity: 0; transform: translate(-50%, -50%) scale(1.08); }
      }
      @keyframes settingsSinkWp {
        0%   { opacity: 1; transform: translate(calc(12px - 50vw), -50%) scale(1); }
        100% { opacity: 0; transform: translate(calc(12px - 50vw), -50%) scale(1.08); }
      }
      .settings-panel,
      .settings-panel.closing,
      .settings-nav,
      .right-panel { backdrop-filter: none !important;
                     -webkit-backdrop-filter: none !important; }`,
  },
  {
    // 3) крайний вариант: без блюров вообще (включая статичные во время
    //    анимации) — нижняя граница того, что можно выжать
    name: 'all-blurs-off',
    css: `
      @keyframes settingsRise {
        0%   { opacity: 0; transform: translate(-50%, -50%) scale(1.08); }
        100% { opacity: 1; transform: translate(-50%, -50%) scale(1); }
      }
      @keyframes settingsSink {
        0%   { opacity: 1; transform: translate(-50%, -50%) scale(1); }
        100% { opacity: 0; transform: translate(-50%, -50%) scale(1.08); }
      }
      @keyframes settingsSinkWp {
        0%   { opacity: 1; transform: translate(calc(12px - 50vw), -50%) scale(1); }
        100% { opacity: 0; transform: translate(calc(12px - 50vw), -50%) scale(1.08); }
      }
      * { backdrop-filter: none !important; -webkit-backdrop-filter: none !important; }`,
  },
  {
    // 4) финальный кандидат А: backdrop-блюр выключен ТОЛЬКО во время
    //    анимации (класс .anim-rise ставит JS-патч открытия, .closing —
    //    закрытие), element-blur оставлен — сохраняем визуал «глубины»
    name: 'backdrop-anim-only+blur',
    css: `
      .settings-panel.anim-rise,
      .settings-panel.closing,
      .settings-panel.anim-rise .settings-nav,
      .settings-panel.closing .settings-nav {
        backdrop-filter: none !important;
        -webkit-backdrop-filter: none !important;
      }`,
    js: `(() => {
      if (window.__abPatched) return true;
      const wrap = (name) => {
        const orig = window[name];
        if (typeof orig !== 'function') return;
        window[name] = function (...a) {
          const p = document.getElementById('settingsPanel');
          if (p) { p.classList.add('anim-rise');
                   setTimeout(() => p.classList.remove('anim-rise'), 450); }
          return orig.apply(this, a);
        };
      };
      window.__abSaved = { o: window.openSettings, c: window.closeSettings };
      wrap('openSettings');
      window.__abPatched = true;
      return true;
    })()`,
    teardownJs: `(() => {
      if (window.__abSaved) {
        if (window.__abSaved.o) window.openSettings = window.__abSaved.o;
        if (window.__abSaved.c) window.closeSettings = window.__abSaved.c;
        window.__abSaved = null;
      }
      window.__abPatched = false;
      document.getElementById('settingsPanel')?.classList.remove('anim-rise');
      return true;
    })()`,
  },
  {
    // 5) финальный кандидат Б: то же + убран анимируемый filter: blur()
    //    (визуал: панель приходит/уходит со scale+opacity, без размытия)
    name: 'final: backdrop-anim + no-blur',
    css: `
      @keyframes settingsRise {
        0%   { opacity: 0; transform: translate(-50%, -50%) scale(1.08); }
        100% { opacity: 1; transform: translate(-50%, -50%) scale(1); }
      }
      @keyframes settingsSink {
        0%   { opacity: 1; transform: translate(-50%, -50%) scale(1); }
        100% { opacity: 0; transform: translate(-50%, -50%) scale(1.08); }
      }
      @keyframes settingsSinkWp {
        0%   { opacity: 1; transform: translate(calc(12px - 50vw), -50%) scale(1); }
        100% { opacity: 0; transform: translate(calc(12px - 50vw), -50%) scale(1.08); }
      }
      .settings-panel.anim-rise,
      .settings-panel.closing,
      .settings-panel.anim-rise .settings-nav,
      .settings-panel.closing .settings-nav {
        backdrop-filter: none !important;
        -webkit-backdrop-filter: none !important;
      }`,
    js: `(() => {
      if (window.__abPatched) return true;
      const wrap = (name) => {
        const orig = window[name];
        if (typeof orig !== 'function') return;
        window[name] = function (...a) {
          const p = document.getElementById('settingsPanel');
          if (p) { p.classList.add('anim-rise');
                   setTimeout(() => p.classList.remove('anim-rise'), 450); }
          return orig.apply(this, a);
        };
      };
      window.__abSaved = { o: window.openSettings, c: window.closeSettings };
      wrap('openSettings');
      window.__abPatched = true;
      return true;
    })()`,
    teardownJs: `(() => {
      if (window.__abSaved) {
        if (window.__abSaved.o) window.openSettings = window.__abSaved.o;
        if (window.__abSaved.c) window.closeSettings = window.__abSaved.c;
        window.__abSaved = null;
      }
      window.__abPatched = false;
      document.getElementById('settingsPanel')?.classList.remove('anim-rise');
      document.body.classList.remove('settings-anim');
      return true;
    })()`,
  },
  {
    // 6) кандидат В: body.settings-anim (ставится и на открытие, и на
    //    закрытие) снимает блюры со ВСЕХ движущихся слоёв: панель, её
    //    навигация и .right-panel (уезжает/возвращается через news-away).
    //    Element-blur (глубина) ОСТАЁТСЯ.
    name: 'V6: body-anim, blur kept',
    css: `
      body.settings-anim .settings-panel,
      body.settings-anim .settings-nav,
      body.settings-anim .right-panel {
        backdrop-filter: none !important;
        -webkit-backdrop-filter: none !important;
      }`,
    js: `(() => {
      if (window.__abPatched) return true;
      const mark = () => {
        document.body.classList.add('settings-anim');
        clearTimeout(window.__abAnimT);
        window.__abAnimT = setTimeout(
          () => document.body.classList.remove('settings-anim'), 500);
      };
      const wrap = (name) => {
        const orig = window[name];
        if (typeof orig !== 'function') return;
        window[name] = function (...a) { mark(); return orig.apply(this, a); };
      };
      window.__abSaved = { o: window.openSettings, c: window.closeSettings };
      wrap('openSettings');
      wrap('closeSettings');
      window.__abPatched = true;
      return true;
    })()`,
    teardownJs: `(() => {
      if (window.__abSaved) {
        if (window.__abSaved.o) window.openSettings = window.__abSaved.o;
        if (window.__abSaved.c) window.closeSettings = window.__abSaved.c;
        window.__abSaved = null;
      }
      window.__abPatched = false;
      document.body.classList.remove('settings-anim');
      return true;
    })()`,
  },
  {
    // 7) кандидат Г: то же, что В, + убран анимируемый filter: blur()
    name: 'V7: body-anim + no-blur',
    css: `
      @keyframes settingsRise {
        0%   { opacity: 0; transform: translate(-50%, -50%) scale(1.08); }
        100% { opacity: 1; transform: translate(-50%, -50%) scale(1); }
      }
      @keyframes settingsSink {
        0%   { opacity: 1; transform: translate(-50%, -50%) scale(1); }
        100% { opacity: 0; transform: translate(-50%, -50%) scale(1.08); }
      }
      @keyframes settingsSinkWp {
        0%   { opacity: 1; transform: translate(calc(12px - 50vw), -50%) scale(1); }
        100% { opacity: 0; transform: translate(calc(12px - 50vw), -50%) scale(1.08); }
      }
      body.settings-anim .settings-panel,
      body.settings-anim .settings-nav,
      body.settings-anim .right-panel {
        backdrop-filter: none !important;
        -webkit-backdrop-filter: none !important;
      }`,
    js: `(() => {
      if (window.__abPatched) return true;
      const mark = () => {
        document.body.classList.add('settings-anim');
        clearTimeout(window.__abAnimT);
        window.__abAnimT = setTimeout(
          () => document.body.classList.remove('settings-anim'), 500);
      };
      const wrap = (name) => {
        const orig = window[name];
        if (typeof orig !== 'function') return;
        window[name] = function (...a) { mark(); return orig.apply(this, a); };
      };
      window.__abSaved = { o: window.openSettings, c: window.closeSettings };
      wrap('openSettings');
      wrap('closeSettings');
      window.__abPatched = true;
      return true;
    })()`,
    teardownJs: `(() => {
      if (window.__abSaved) {
        if (window.__abSaved.o) window.openSettings = window.__abSaved.o;
        if (window.__abSaved.c) window.closeSettings = window.__abSaved.c;
        window.__abSaved = null;
      }
      window.__abPatched = false;
      document.body.classList.remove('settings-anim');
      return true;
    })()`,
  },
  {
    // 8) V7 + will-change на панели: форсируем compositor-слой ДО старта
    //    анимации — open-anim растеризует новый контент (switchTab) каждый
    //    кадр, слой должен уйти из main-thread-пути
    name: 'V8: V7 + will-change',
    css: `
      @keyframes settingsRise {
        0%   { opacity: 0; transform: translate(-50%, -50%) scale(1.08); }
        100% { opacity: 1; transform: translate(-50%, -50%) scale(1); }
      }
      @keyframes settingsSink {
        0%   { opacity: 1; transform: translate(-50%, -50%) scale(1); }
        100% { opacity: 0; transform: translate(-50%, -50%) scale(1.08); }
      }
      @keyframes settingsSinkWp {
        0%   { opacity: 1; transform: translate(calc(12px - 50vw), -50%) scale(1); }
        100% { opacity: 0; transform: translate(calc(12px - 50vw), -50%) scale(1.08); }
      }
      .settings-panel { will-change: transform, opacity; }
      body.settings-anim .settings-panel,
      body.settings-anim .settings-nav,
      body.settings-anim .right-panel {
        backdrop-filter: none !important;
        -webkit-backdrop-filter: none !important;
      }`,
    js: `(() => {
      if (window.__abPatched) return true;
      const mark = () => {
        document.body.classList.add('settings-anim');
        clearTimeout(window.__abAnimT);
        window.__abAnimT = setTimeout(
          () => document.body.classList.remove('settings-anim'), 500);
      };
      const wrap = (name) => {
        const orig = window[name];
        if (typeof orig !== 'function') return;
        window[name] = function (...a) { mark(); return orig.apply(this, a); };
      };
      window.__abSaved = { o: window.openSettings, c: window.closeSettings };
      wrap('openSettings');
      wrap('closeSettings');
      window.__abPatched = true;
      return true;
    })()`,
    teardownJs: `(() => {
      if (window.__abSaved) {
        if (window.__abSaved.o) window.openSettings = window.__abSaved.o;
        if (window.__abSaved.c) window.closeSettings = window.__abSaved.c;
        window.__abSaved = null;
      }
      window.__abPatched = false;
      document.body.classList.remove('settings-anim');
      return true;
    })()`,
  },
  {
    // 9) изоляция A: убрана только анимация появления панели (rise),
    //    контент рендерится и скелетоны живут — если фаза open-anim
    //    станет 16.7, джанк = взаимодействие rise-анимации с рендером
    name: 'V9: no-rise-anim',
    css: `
      body.settings-anim .settings-panel,
      body.settings-anim .settings-nav,
      body.settings-anim .right-panel {
        backdrop-filter: none !important;
        -webkit-backdrop-filter: none !important;
      }
      body.settings-anim .settings-panel { animation: none !important; }`,
    js: `(() => {
      if (window.__abPatched) return true;
      const mark = () => {
        document.body.classList.add('settings-anim');
        clearTimeout(window.__abAnimT);
        window.__abAnimT = setTimeout(
          () => document.body.classList.remove('settings-anim'), 500);
      };
      const wrap = (name) => {
        const orig = window[name];
        if (typeof orig !== 'function') return;
        window[name] = function (...a) { mark(); return orig.apply(this, a); };
      };
      window.__abSaved = { o: window.openSettings, c: window.closeSettings };
      wrap('openSettings');
      wrap('closeSettings');
      window.__abPatched = true;
      return true;
    })()`,
    teardownJs: `(() => {
      if (window.__abSaved) {
        if (window.__abSaved.o) window.openSettings = window.__abSaved.o;
        if (window.__abSaved.c) window.closeSettings = window.__abSaved.c;
        window.__abSaved = null;
      }
      window.__abPatched = false;
      document.body.classList.remove('settings-anim');
      return true;
    })()`,
  },
  {
    // 10) изоляция B: все CSS-анимации на паузе (shimmer скелетонов и т.п.),
    //     кроме самой панели (rise/sink работают) — если станет 16.7,
    //     виновник — бесконечные анимации контента во время загрузки
    name: 'V10: pause infinite anims',
    css: `
      @keyframes settingsRise {
        0%   { opacity: 0; transform: translate(-50%, -50%) scale(1.08); }
        100% { opacity: 1; transform: translate(-50%, -50%) scale(1); }
      }
      @keyframes settingsSink {
        0%   { opacity: 1; transform: translate(-50%, -50%) scale(1); }
        100% { opacity: 0; transform: translate(-50%, -50%) scale(1.08); }
      }
      * { animation-play-state: paused !important; }
      .settings-panel { animation-play-state: running !important; }
      body.settings-anim .settings-panel,
      body.settings-anim .settings-nav,
      body.settings-anim .right-panel {
        backdrop-filter: none !important;
        -webkit-backdrop-filter: none !important;
      }`,
    js: `(() => {
      if (window.__abPatched) return true;
      const mark = () => {
        document.body.classList.add('settings-anim');
        clearTimeout(window.__abAnimT);
        window.__abAnimT = setTimeout(
          () => document.body.classList.remove('settings-anim'), 500);
      };
      const wrap = (name) => {
        const orig = window[name];
        if (typeof orig !== 'function') return;
        window[name] = function (...a) { mark(); return orig.apply(this, a); };
      };
      window.__abSaved = { o: window.openSettings, c: window.closeSettings };
      wrap('openSettings');
      wrap('closeSettings');
      window.__abPatched = true;
      return true;
    })()`,
    teardownJs: `(() => {
      if (window.__abSaved) {
        if (window.__abSaved.o) window.openSettings = window.__abSaved.o;
        if (window.__abSaved.c) window.closeSettings = window.__abSaved.c;
        window.__abSaved = null;
      }
      window.__abPatched = false;
      document.body.classList.remove('settings-anim');
      return true;
    })()`,
  },
  // ── Раунд 2: сохраняем вид (блюр виден во время анимации), ищем компромисс ──
  {
    // 12) во время анимации блюр УМЕНЬШЕН (8px вместо 24px) — размытие есть,
    //     но считается дешевле
    name: 'V12: anim blur(8)',
    css: `
      .settings-panel.anim-rise,
      .settings-panel.closing {
        backdrop-filter: blur(8px) saturate(1.2) !important;
        -webkit-backdrop-filter: blur(8px) saturate(1.2) !important;
      }
      .settings-panel.anim-rise .settings-nav,
      .settings-panel.closing .settings-nav {
        backdrop-filter: blur(8px) !important;
        -webkit-backdrop-filter: blur(8px) !important;
      }`,
  },
  {
    // 13) то же, но радиус 4px — нижняя граница «видно размытие»
    name: 'V13: anim blur(4)',
    css: `
      .settings-panel.anim-rise,
      .settings-panel.closing {
        backdrop-filter: blur(4px) saturate(1.2) !important;
        -webkit-backdrop-filter: blur(4px) saturate(1.2) !important;
      }
      .settings-panel.anim-rise .settings-nav,
      .settings-panel.closing .settings-nav {
        backdrop-filter: blur(4px) !important;
        -webkit-backdrop-filter: blur(4px) !important;
      }`,
  },
  {
    // 14) текущий фикс (none во время анимации) + ПЛАВНОЕ возвращение блюра
    //     transition-ом вместо мгновенного попа в конце
    name: 'V14: none + smooth return',
    css: `
      .settings-panel.anim-rise,
      .settings-panel.closing,
      .settings-panel.anim-rise .settings-nav,
      .settings-panel.closing .settings-nav {
        backdrop-filter: none !important;
        -webkit-backdrop-filter: none !important;
      }
      .settings-panel {
        transition: backdrop-filter 0.2s ease, -webkit-backdrop-filter 0.2s ease;
      }`,
  },
  {
    // 15) компромисс: blur(8px) во время анимации + плавный переход к 24px
    name: 'V15: blur(8) + smooth to 24',
    css: `
      .settings-panel.anim-rise,
      .settings-panel.closing {
        backdrop-filter: blur(8px) saturate(1.2) !important;
        -webkit-backdrop-filter: blur(8px) saturate(1.2) !important;
      }
      .settings-panel.anim-rise .settings-nav,
      .settings-panel.closing .settings-nav {
        backdrop-filter: blur(8px) !important;
        -webkit-backdrop-filter: blur(8px) !important;
      }
      .settings-panel {
        transition: backdrop-filter 0.2s ease, -webkit-backdrop-filter 0.2s ease;
      }`,
  },
  {
    // 16) контроль: блюр ВКЛЮЧЁН всё время, анимация = только opacity
    //     (без transform — геометрия панели не меняется). Если станет 16.7 —
    //     статичный backdrop дешёвый и дорог именно блюр В ДВИЖЕНИИ.
    name: 'V16: opacity-only, blur on',
    css: `
      @keyframes settingsRise {
        0%   { opacity: 0; }
        100% { opacity: 1; }
      }
      @keyframes settingsSink {
        0%   { opacity: 1; }
        100% { opacity: 0; }
      }
      @keyframes settingsSinkWp {
        0%   { opacity: 1; }
        100% { opacity: 0; }
      }`,
  },
  {
    // 17) то же, что V16, + element-blur (глубина) остаётся в анимации:
    //     backdrop статичен (дёшево), а панель всё так же «приходит из глубины»
    name: 'V17: opacity+depth, blur on',
    css: `
      @keyframes settingsRise {
        0%   { opacity: 0; filter: blur(14px); }
        100% { opacity: 1; filter: blur(0); }
      }
      @keyframes settingsSink {
        0%   { opacity: 1; filter: blur(0); }
        100% { opacity: 0; filter: blur(14px); }
      }
      @keyframes settingsSinkWp {
        0%   { opacity: 1; filter: blur(0); }
        100% { opacity: 0; filter: blur(14px); }
      }`,
  },
  // ── Гипотеза «вложенные блюры подэлементов» (probe-outer-blurs.js) ──
  // 19) smooth + ВСЕ блюры вне панели отключены на время анимации
  //     (body.settings-anim ставит mark() бенча на 500мс). Внешние слои:
  //     #newsHandle (двигается с right-panel), .right-panel, #feedbackFab
  //     (z=9999, панель проходит под ним), #tbTitle/#tbButtons, #phTrigger,
  //     #phDropdown, .version-stable.
  {
    name: 'V19: smooth+внешние блюры off',
    mode: 'smooth',
    css: `
      body.settings-anim * {
        backdrop-filter: none !important;
        -webkit-backdrop-filter: none !important;
      }`,
  },
  {
    // 20) то же для pop — режим с лучшим замером
    name: 'V20: pop+внешние блюры off',
    mode: 'pop',
    css: `
      body.settings-anim * {
        backdrop-filter: none !important;
        -webkit-backdrop-filter: none !important;
      }`,
  },
  {
    // 21) узкий вариант: только двигающиеся слои (.right-panel с #newsHandle
    //     уезжает через news-away) + #feedbackFab (z=9999, панель/правая
    //     панель меняют его backdrop). Статичные (titlebar, version-stable)
    //     не трогаем — их backdrop не меняется каждый кадр, мигать не будут.
    //     Если ≈ V19 — берём узкий (минимум визуальных изменений).
    name: 'V21: smooth+только движущиеся',
    mode: 'smooth',
    css: `
      body.settings-anim .right-panel,
      body.settings-anim .right-panel *,
      body.settings-anim #feedbackFab {
        backdrop-filter: none !important;
        -webkit-backdrop-filter: none !important;
      }`,
  },
  {
    // 22) средний: движущиеся + статичные вокруг панели, но БЕЗ подэлементов
    //     самой панели (fade/full держат её блюр включённым всё время)
    name: 'V22: smooth+все кроме панели',
    mode: 'smooth',
    css: `
      body.settings-anim *:not(.settings-panel):not(.settings-panel *) {
        backdrop-filter: none !important;
        -webkit-backdrop-filter: none !important;
      }`,
  },
  // ── Реальные режимы index.html (класс anim-* на <html>) ──
  { name: 'mode: fade',                    mode: 'fade',  css: '' },
  { name: 'mode: pop',                     mode: 'pop',   css: '' },
  { name: 'mode: smooth',                  mode: 'smooth', css: '' },
  { name: 'mode: full (дефолт)',           mode: 'full',  css: '' },
  {
    // 18) fade БЕЗ element-blur: backdrop включён всегда, анимация = только
    //     opacity. Вопрос: инвалидирует ли анимация opacity backdrop-блюр?
    //     (V16/17 замерялись загрязнённо — правило .anim-rise уже снимало блюр)
    name: 'V18: fade, только opacity',
    mode: 'fade',
    css: `
      @keyframes settingsRiseFade {
        0%   { opacity: 0; }
        100% { opacity: 1; }
      }
      @keyframes settingsSinkFade {
        0%   { opacity: 1; }
        100% { opacity: 0; }
      }
      @keyframes settingsSinkWpFade {
        0%   { opacity: 1; }
        100% { opacity: 0; }
      }`,
  },
];

(async () => {
  const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  const page = list.find((t) => t.type === 'page');
  if (!page) { console.error('нет page'); process.exit(1); }
  const cdp = await connect(page.webSocketDebuggerUrl);
  await cdp.send('Runtime.enable');

  // закрываем панель, если открыта, — чистая точка входа
  await evaluate(cdp, `(() => { const c = document.querySelector(
      '[onclick*="closeSettings"], .s-close, #settingsClose');
    if (c && document.getElementById('settingsPanel')?.style.display !== 'none') c.click();
    return true; })()`);

  const results = [];
  const ONLY = process.env.AB_ONLY ? process.env.AB_ONLY.split(',') : null;
  // исходный режим index.html — восстановим после прогона
  const origMode = await evaluate(cdp, `(['fade','pop','smooth','full']
      .find(m => document.documentElement.classList.contains('anim-' + m)) || 'fade')`);
  for (const v of VARIANTS) {
    if (ONLY && !ONLY.some((s) => v.name.includes(s))) continue;
    if (v.mode) {
      // режим из index.html: класс на <html>, CSS уже вшит в страницу
      await evaluate(cdp, `(() => {
        const de = document.documentElement;
        ['anim-fade','anim-pop','anim-smooth','anim-full']
          .forEach(c => de.classList.remove(c));
        de.classList.add('anim-${v.mode}');
        try { localStorage.setItem('airAnimMode', '${v.mode}'); } catch (e) {}
        return true;
      })()`);
    }
    await evaluate(cdp, `(() => {
      let el = document.getElementById('perf-ab-style');
      if (!el) { el = document.createElement('style'); el.id = 'perf-ab-style';
                 document.head.appendChild(el); }
      el.textContent = ${JSON.stringify(v.css)};
      return true;
    })()`);
    // js/teardown вариантов больше не выполняются: реальные обработчики
    // привязаны через addEventListener, классы ставит сам бенч (mark).
    await evaluate(cdp, benchExpr(CYCLES)); // прогрев
    const r = await evaluate(cdp, benchExpr(CYCLES));
    results.push({ variant: v.name, ...r });
    // пауза между вариантами, чтобы GPU успокоился
    await evaluate(cdp, `new Promise(r => setTimeout(r, 600))`);
  }

  await evaluate(cdp, `(() => { document.getElementById('perf-ab-style')?.remove();
    return true; })()`);
  // возвращаем исходный режим
  await evaluate(cdp, `(() => {
    const de = document.documentElement;
    ['anim-fade','anim-pop','anim-smooth','anim-full']
      .forEach(c => de.classList.remove(c));
    de.classList.add('anim-${origMode}');
    try { localStorage.setItem('airAnimMode', '${origMode}'); } catch (e) {}
    return true;
  })()`);
  cdp.close();

  console.log(JSON.stringify({ cycles: CYCLES, results }, null, 2));
  if (process.env.AB_OUT) {
    require('fs').writeFileSync(process.env.AB_OUT,
      JSON.stringify({ cycles: CYCLES, results }, null, 2));
  }
  console.log('\nвариант                avg    p50    p95    max   >33мс  >50мс >100мс');
  for (const r of results) {
    console.log(
      r.variant.padEnd(22) +
      String(r.avgMs).padStart(6) + String(r.p50Ms).padStart(7) +
      String(r.p95Ms).padStart(7) + String(r.maxMs).padStart(7) +
      String(r.janky33).padStart(7) + String(r.over50).padStart(7) +
      String(r.over100).padStart(7));
  }
  process.exit(0);
})().catch((e) => { console.error('FAIL:', e.message); process.exit(1); });
