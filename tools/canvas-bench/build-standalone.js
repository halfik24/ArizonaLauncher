// Сборка автономного tools/canvas-bench/standalone.html: bench.html +
// bench.js + standalone-ui.js инлайном, обои — data:URL base64.
// Файл открывается на home-pc двойным кликом, без сервера и без раннера.
//
// Использование: node tools/canvas-bench/build-standalone.js
'use strict';

const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..', '..');
const html = fs.readFileSync(path.join(__dirname, 'bench.html'), 'utf8');
const js = fs.readFileSync(path.join(__dirname, 'bench.js'), 'utf8');
const ui = fs.readFileSync(path.join(__dirname, 'standalone-ui.js'), 'utf8');
const img = fs.readFileSync(path.join(root, 'wallpapers', '6.jpeg'));
const dataUrl = 'data:image/jpeg;base64,' + img.toString('base64');

const marker = '<script src="bench.js"></script>';
if (!html.includes(marker)) throw new Error('маркер script в bench.html не найден');

const inline = '<script>\n/* ==== bench.js ==== */\n' + js +
  '\n/* ==== standalone-ui.js ==== */\n' + ui + '\n</script>';

// функции-заменители: в коде есть '$', интерполяцию replace ломает
const out = html
  .replace(marker, () => inline)
  .replace(/\/wallpapers\/6\.jpeg/g, () => dataUrl);

const dst = path.join(__dirname, 'standalone.html');
fs.writeFileSync(dst, out);
console.log('ok: ' + dst + ' (' + Math.round(out.length / 1024) + ' КБ, ' +
  'обои ' + Math.round(img.length / 1024) + ' КБ в base64)');
