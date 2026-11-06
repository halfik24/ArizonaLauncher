'use strict';
// Автономный режим standalone.html: кнопка запуска + таблица результатов
// прямо на странице (на home-pc нет раннера — всё показываем в браузере).
(() => {
  const CYCLES = Number(new URLSearchParams(location.search).get('cycles')) || 5;
  const SET = new URLSearchParams(location.search).get('set') || 'heavy';
  const esc = s => String(s).replace(/[&<>]/g, c =>
    ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' }[c]));

  const card = document.createElement('div');
  card.style.cssText = [
    'position:fixed', 'left:50%', 'top:50%', 'transform:translate(-50%,-50%)',
    'z-index:99', 'background:rgba(10,12,18,.94)',
    'border:1px solid rgba(255,255,255,.15)', 'border-radius:14px',
    'padding:22px 26px', 'color:#e8edf7',
    'font:14px/1.5 "Segoe UI",system-ui,sans-serif',
    'max-width:780px', 'max-height:86vh', 'overflow:auto',
    'box-shadow:0 20px 70px rgba(0,0,0,.6)',
  ].join(';');
  card.innerHTML =
    '<h2 style="margin:0 0 6px;font-size:18px">' +
    'canvas-bench: тяжёлая DOM-сцена лаунчера (backdrop/блюр/анимации)</h2>' +
    '<div id="ac-status" style="color:#9fb0cc;margin-bottom:12px">' +
    'Набор «' + SET + '», ' + CYCLES + ' циклов, ~1–2 минуты. Во время теста ' +
    'держите это окно на переднем плане и не переключайтесь.</div>' +
    '<div id="ac-out"></div>' +
    '<button id="ac-run" style="margin-top:14px;padding:9px 22px;font:600 14px ' +
    '"Segoe UI",system-ui,sans-serif;color:#fff;background:#2563eb;' +
    'border:0;border-radius:9px;cursor:pointer">▶ Запустить тест</button>';
  document.body.appendChild(card);

  const status = card.querySelector('#ac-status');
  const out = card.querySelector('#ac-out');
  const btn = card.querySelector('#ac-run');

  const btnCss = 'padding:6px 14px;font:13px "Segoe UI",system-ui,sans-serif;' +
    'color:#fff;background:#3b82f6;border:0;border-radius:8px;cursor:pointer';

  function render(res) {
    const rows = Object.entries(res.variants).map(([name, r]) => {
      if (r.error) {
        return '<tr><td>' + esc(name) + '</td><td colspan="9" style="color:#f88">' +
          esc(r.error) + '</td></tr>';
      }
      const c = [name, r.avgMs, r.p50Ms, r.p95Ms, r.maxMs,
        r.janky33, r.over50, r.over100, r.longTasks,
        r.openAnim ? r.openAnim.avgMs : '-'];
      return '<tr>' + c.map(v => '<td style="padding:2px 10px 2px 0">' +
        esc(v) + '</td>').join('') + '</tr>';
    }).join('');
    const rend = Object.entries(res.variants)
      .filter(([, r]) => r.renderer)
      .map(([n, r]) => '<div>' + esc(n) + ': ' + esc(r.renderer) + '</div>')
      .join('');
    const m = res.meta;
    out.innerHTML =
      '<table style="border-collapse:collapse;font:13px/1.5 Consolas,monospace">' +
      '<tr style="color:#9fb0cc;text-align:left">' +
      ['вариант', 'avg', 'p50', 'p95', 'max', '>33', '>50', '>100', 'LT', 'open-avg']
        .map(h => '<th style="padding:2px 10px 2px 0">' + h + '</th>').join('') +
      '</tr>' + rows + '</table>' +
      '<div style="color:#9fb0cc;font-size:12px;margin-top:10px">' + rend +
      '<div>meta: ' + m.vw + '×' + m.vh + ' @dpr' + m.dpr + ', панель ' +
      (m.hpw || '-') + '×' + (m.hph || '-') + ', blur ' +
      m.blur + 'px, набор ' + esc(m.set || '-') + ', циклов ' + m.cycles + '</div>' +
      '<div>UA: ' + esc(navigator.userAgent) + '</div></div>' +
      '<div style="display:flex;gap:10px;margin-top:12px">' +
      '<button id="ac-copy" style="' + btnCss + '">Скопировать JSON</button>' +
      '<button id="ac-show" style="' + btnCss +
      ';background:#4b5563">Показать JSON</button></div>' +
      '<pre id="ac-json" style="display:none;max-height:260px;overflow:auto;' +
      'background:#000;padding:10px;border-radius:8px;font-size:11px"></pre>';
    const json = out.querySelector('#ac-json');
    const jsonText = JSON.stringify(res, null, 2);
    out.querySelector('#ac-copy').onclick = async () => {
      try {
        await navigator.clipboard.writeText(jsonText);
        status.textContent = 'JSON скопирован в буфер обмена.';
      } catch (e) {
        json.style.display = 'block';
        json.textContent = jsonText;
        status.textContent = 'Буфер недоступен — JSON раскрыт, скопируйте вручную.';
      }
    };
    out.querySelector('#ac-show').onclick = () => {
      json.style.display = json.style.display === 'none' ? 'block' : 'none';
      if (json.style.display === 'block') json.textContent = jsonText;
    };
  }

  btn.onclick = async () => {
    btn.disabled = true;
    window.__acRes = null;
    // свернуть карточку в угол, чтобы не закрывать анимацию
    card.style.cssText = card.style.cssText
      .replace('left:50%;top:50%;transform:translate(-50%,-50%)',
        'left:8px;top:26px;transform:none');
    status.textContent = 'идёт тест… (обновление прогресса — в hud слева сверху)';
    out.innerHTML = '';
    const hud = document.getElementById('hud');
    try {
      const res = await window.runBench(CYCLES, SET);
      window.__acRes = res;
      const errs = Object.entries(res.variants).filter(([, r]) => r.error);
      status.textContent = errs.length
        ? 'Готово, но с ошибками (' + errs.length + ') — пришлите скриншот.'
        : 'Готово! Пришлите скриншот этой таблицы.';
      if (hud) hud.style.display = '';
      render(res);
    } catch (e) {
      window.__acRes = { error: String((e && e.message) || e) };
      status.textContent = 'Ошибка: ' + ((e && e.message) || e);
      if (hud) hud.style.display = '';
    }
    btn.disabled = false;
    btn.textContent = '↻ Повторить тест';
  };
})();
