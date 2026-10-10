// Вспомогательный: печатает строки N-M из шаблона EXPR в perf-timeline.js
const fs = require('fs');
const src = fs.readFileSync('tools/perf-timeline.js', 'utf8');
const m = src.match(/const EXPR = `([\s\S]*?)`;/);
if (!m) { console.log('EXPR не найден'); process.exit(1); }
const lines = m[1].split('\n');
const from = parseInt(process.argv[2] || '60', 10);
const to = parseInt(process.argv[3] || '78', 10);
lines.forEach((l, i) => {
  const n = i + 1;
  if (n >= from && n <= to) console.log(String(n).padStart(3) + '| ' + l);
});
console.log('\nвсего строк:', lines.length);
