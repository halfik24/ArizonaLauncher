'use strict';

history.scrollRestoration = 'manual';
if (location.hash) history.replaceState(null, '', location.pathname + location.search);
window.scrollTo(0, 0);
window.addEventListener('load', () => window.scrollTo(0, 0));

const shots = Array.from(document.querySelectorAll('.shot'));

if (shots.length > 1) {
  const dotsWrap = document.querySelector('.demo__dots');
  let current = 0;
  let timer = null;

  if (dotsWrap) {
    shots.forEach((_, i) => {
      const dot = document.createElement('button');
      dot.type = 'button';
      dot.className = 'dot';
      dot.setAttribute('aria-label', 'Кадр ' + (i + 1));
      dot.addEventListener('click', () => {
        show(i);
        restart();
      });
      dotsWrap.appendChild(dot);
    });
  }

  const dots = dotsWrap ? Array.from(dotsWrap.querySelectorAll('.dot')) : [];

  function show(i) {
    current = i;
    shots.forEach((s, k) => s.classList.toggle('is-active', k === i));
    dots.forEach((d, k) => d.classList.toggle('is-active', k === i));
  }

  function next() { show((current + 1) % shots.length); }

  function restart() {
    clearInterval(timer);
    if (!matchMedia('(prefers-reduced-motion: reduce)').matches) {
      timer = setInterval(next, 5000);
    }
  }

  document.querySelector('.demo__nav--prev')?.addEventListener('click', () => {
    show((current - 1 + shots.length) % shots.length);
    restart();
  });
  document.querySelector('.demo__nav--next')?.addEventListener('click', () => {
    show((current + 1) % shots.length);
    restart();
  });

  const demo = document.querySelector('.demo');
  demo.addEventListener('pointerenter', () => clearInterval(timer));
  demo.addEventListener('pointerleave', restart);

  show(0);
  restart();
}

(function () {
  const el = document.querySelector('.hero__version a');
  if (!el) return;

  const KEY = 'arl-release';
  const TTL = 6 * 60 * 60 * 1000;

  try {
    const cached = JSON.parse(localStorage.getItem(KEY) || 'null');
    if (cached && cached.tag && Date.now() - cached.t < TTL) {
      el.textContent = cached.tag;
      return;
    }
  } catch (_) {}

  fetch('https://api.github.com/repos/halfik24/ArizonaLauncher/releases?per_page=10')
    .then((r) => (r.ok ? r.json() : Promise.reject(new Error(r.status))))
    .then((list) => {
      if (!Array.isArray(list)) return;
      const rel = list.find((x) => !x.draft && !x.prerelease) || list.find((x) => !x.draft);
      if (!rel || !rel.tag_name) return;
      el.textContent = rel.tag_name;
      try { localStorage.setItem(KEY, JSON.stringify({ tag: rel.tag_name, t: Date.now() })); } catch (_) {}
    })
    .catch(() => {});
})();

const io = new IntersectionObserver((entries) => {
  entries.forEach((en) => {
    if (en.isIntersecting) {
      en.target.classList.add('is-visible');
      io.unobserve(en.target);
    }
  });
}, { threshold: 0.12 });

document.querySelectorAll('.section').forEach((el) => {
  el.classList.add('reveal');
  io.observe(el);
});

document.querySelectorAll('.copy').forEach((btn) => {
  const label = btn.querySelector('.copy__label');
  if (!label) return;
  const original = label.textContent;
  let timer;

  btn.addEventListener('click', async () => {
    const addr = btn.closest('.wallet')?.querySelector('code')?.textContent.trim();
    if (!addr) return;
    try {
      await navigator.clipboard.writeText(addr);
    } catch (_) {
      return;
    }

    clearTimeout(timer);
    label.textContent = 'Скопировано';
    btn.classList.add('is-done');
    timer = setTimeout(() => {
      btn.classList.remove('is-done');
      label.textContent = original;
    }, 1800);
  });
});

function makeSpark(cls, x, y) {
  const s = document.createElement('span');
  s.className = cls;
  s.style.left = x + 'px';
  s.style.top = y + 'px';
  document.body.appendChild(s);
  return s;
}

function sparkBurst(x, y) {
  for (let i = 0; i < 9; i++) {
    const a = (i / 9) * Math.PI * 2 + (Math.random() - .5) * .6;
    const d = 34 + Math.random() * 46;
    const s = makeSpark('fx-spark', x, y);
    s.style.setProperty('--x', Math.cos(a) * d + 'px');
    s.style.setProperty('--y', Math.sin(a) * d + 'px');
    s.style.animationDelay = (Math.random() * 60) + 'ms';
    s.addEventListener('animationend', () => s.remove());
  }
}

function weldOne(x, y) {
  const a = Math.random() * Math.PI * 2;
  const d = 30 + Math.random() * 72;
  const dx = Math.cos(a) * d;
  const dy = Math.sin(a) * d * .7 + 14 + Math.random() * 34;
  const s = makeSpark('fx-weld', x, y);
  s.style.setProperty('--x', dx + 'px');
  s.style.setProperty('--y', dy + 'px');
  s.style.setProperty('--a', Math.atan2(dy, dx) * 180 / Math.PI + 'deg');
  s.style.animationDuration = (.38 + Math.random() * .34) + 's';
  s.addEventListener('animationend', () => s.remove());
}

let weldTimer = null;
let weldCore = null;
let weldMove = null;

function stopWeld() {
  if (weldTimer) { clearInterval(weldTimer); weldTimer = null; }
  if (weldCore) { weldCore.remove(); weldCore = null; }
  if (weldMove) { document.removeEventListener('pointermove', weldMove); weldMove = null; }
  window.removeEventListener('pointerup', stopWeld);
  window.removeEventListener('pointercancel', stopWeld);
  window.removeEventListener('blur', stopWeld);
  document.removeEventListener('mouseleave', stopWeld);
}

document.addEventListener('pointerdown', (e) => {
  if (e.button !== 0) return;
  if (matchMedia('(prefers-reduced-motion: reduce)').matches) return;

  sparkBurst(e.clientX, e.clientY);

  stopWeld();
  let px = e.clientX;
  let py = e.clientY;
  weldCore = makeSpark('fx-weld-core', px, py);
  weldMove = (ev) => {
    px = ev.clientX;
    py = ev.clientY;
    weldCore.style.left = px + 'px';
    weldCore.style.top = py + 'px';
  };
  const tick = () => { for (let i = 0; i < 4; i++) weldOne(px, py); };
  tick();
  weldTimer = setInterval(tick, 55);

  document.addEventListener('pointermove', weldMove);
  window.addEventListener('pointerup', stopWeld);
  window.addEventListener('pointercancel', stopWeld);
  window.addEventListener('blur', stopWeld);
  document.addEventListener('mouseleave', stopWeld);
});
