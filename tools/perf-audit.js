// Аудит производительности UI: ищем типовые причины лагов в index.html
// Использование: node tools/perf-audit.js
const fs = require('fs');

const file = 'src/index.html';
const src = fs.readFileSync(file, 'utf8');
const lines = src.split('\n');

const findings = [];
function add(cat, line, detail) {
  findings.push({ cat, line, detail });
}

// 1. setInterval / requestAnimationFrame без обнуления
lines.forEach((l, i) => {
  if (/setInterval\s*\(/.test(l)) add('setInterval', i + 1, l.trim().slice(0, 140));
});

// 2. На каждую строку: layout thrashing — обращение к layout-свойствам
// в цикле рядом с записью (offsetHeight/scrollTop/getBoundingClientRect в цикле)
lines.forEach((l, i) => {
  if (/for\s*\(.*\)\s*\{/.test(l) || /while\s*\(.*\)\s*\{/.test(l)) {
    // ищем в ближайших 15 строках layout-чтения
    const win = lines.slice(i, i + 15).join('\n');
    if (/getBoundingClientRect|offsetHeight|offsetWidth|offsetTop|scrollHeight|clientHeight|\.offsetWidth/.test(win)) {
      if (/\.style\.|\.scrollTop\s*=|\.className\s*=|innerHTML\s*=/.test(win)) {
        add('layout-thrash?', i + 1, l.trim().slice(0, 120));
      }
    }
  }
});

// 3. innerHTML в цикле / частые перерисовки
lines.forEach((l, i) => {
  if (/innerHTML\s*=/.test(l) && i > 0 && /for\s*\(|forEach|while\s*\(/.test(lines.slice(Math.max(0, i - 8), i).join('\n'))) {
    add('innerHTML-in-loop', i + 1, l.trim().slice(0, 140));
  }
});

// 4. Мышиные обработчики без throttling, двигающие layout
lines.forEach((l, i) => {
  if (/addEventListener\(\s*'(mousemove|pointermove)'/.test(l)) {
    const win = lines.slice(i, i + 25).join('\n');
    if (/getBoundingClientRect|\.style\.left\s*=|\.style\.top\s*=|offsetWidth|scrollLeft/.test(win)) {
      add('mousemove-layout', i + 1, l.trim().slice(0, 140));
    } else {
      add('mousemove', i + 1, l.trim().slice(0, 140));
    }
  }
});

// 5. resize обработчики (debounce есть?)
lines.forEach((l, i) => {
  if (/addEventListener\(\s*'(resize|scroll)'/.test(l)) {
    add('resize/scroll', i + 1, l.trim().slice(0, 140));
  }
});

// 6. Частые вызовы localStorage в горячем пути
lines.forEach((l, i) => {
  if (/localStorage\.(get|set)Item/.test(l) && /mousemove|scroll|resize|raf|frame/.test(lines.slice(Math.max(0, i - 10), i + 3).join('\n'))) {
    add('localStorage-hot', i + 1, l.trim().slice(0, 140));
  }
});

// 7. box-shadow + blur на большом числе элементов (медиаторы)
const bigShadow = [];
lines.forEach((l, i) => {
  const m = l.match(/box-shadow:[^;]*blur\((\d+)px\)/) || l.match(/box-shadow:[^;]*(\d+)px\s+\d+px/);
  if (m) {
    const v = parseInt(m[1], 10);
    if (v >= 20) bigShadow.push({ line: i + 1, v, txt: l.trim().slice(0, 120) });
  }
});

// 8. backdrop-filter с большим радиусом
const bigBlur = [];
lines.forEach((l, i) => {
  const m = l.match(/backdrop-filter:[^;]*blur\((\d+(?:\.\d+)?)px\)/);
  if (m) bigBlur.push({ line: i + 1, v: parseFloat(m[1]), txt: l.trim().slice(0, 120) });
});

// 9. Итоговое число backdrop-filter, которые могут быть видимы одновременно
const bdLines = [];
lines.forEach((l, i) => { if (/backdrop-filter/.test(l)) bdLines.push(i + 1); });

// 10. Длинные синхронные вызовы API на старте
const invokeCount = (src.match(/invoke\(|pywebview\.api\./g) || []).length;

// ---- отчёт ----
console.log('=== АУДИТ ПРОИЗВОДИТЕЛЬНОСТИ UI ===\n');
console.log(`index.html: ${Math.round(src.length / 1024)} KB, ${lines.length} строк`);
console.log(`backdrop-filter: ${bdLines.length} определений (строки: ${bdLines.slice(0, 12).join(', ')}${bdLines.length > 12 ? '...' : ''})`);
console.log(`вызовов invoke/pywebview.api: ${invokeCount}`);
console.log('');

const grouped = {};
findings.forEach(f => { (grouped[f.cat] = grouped[f.cat] || []).push(f); });
Object.keys(grouped).forEach(cat => {
  console.log(`--- ${cat} (${grouped[cat].length}) ---`);
  grouped[cat].slice(0, 12).forEach(f => console.log(`  L${f.line}: ${f.detail}`));
  console.log('');
});

console.log(`--- backdrop-filter с blur >= 16px (${bigBlur.filter(b => b.v >= 16).length}) ---`);
bigBlur.filter(b => b.v >= 16).sort((a, b) => b.v - a.v).slice(0, 20)
  .forEach(b => console.log(`  L${b.line}: ${b.v}px`));
console.log('');

console.log(`--- box-shadow blur >= 20px (${bigShadow.length}) ---`);
bigShadow.sort((a, b) => b.v - a.v).slice(0, 15)
  .forEach(b => console.log(`  L${b.line}: ${b.v}px  ${b.txt}`));
