// Запуск canvas-бенча в Chromium (Playwright) с --disable-gpu — прокси
// слабой GPU. Поднимает локальный HTTP-сервер (корень репо), открывает
// страницу, выполняет window.runBench(cycles) через page.evaluate,
// печатает таблицу и пишет last-result.json.
//
// Использование:
//   node tools/canvas-bench/run.js [cycles] [set] — set: heavy (дефолт) | light
//   node tools/canvas-bench/run.js --serve        — только сервер (внешний браузер)
'use strict';

const http = require('http');
const fs = require('fs');
const path = require('path');

const PORT = Number(process.env.BENCH_PORT || 9444);
const CYCLES = parseInt(process.argv[2] || '5', 10);
const SET = process.argv[3] || undefined;   // heavy (дефолт) | light
const ROOT = path.resolve(__dirname, '..', '..');
const PAGE = '/tools/canvas-bench/bench.html';
const URL = `http://127.0.0.1:${PORT}${PAGE}`;
const OUT = path.join(__dirname, 'last-result.json');
const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json',
  '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg', '.png': 'image/png',
};

function startServer() {
  return new Promise((resolve, reject) => {
    const server = http.createServer((req, res) => {
      try {
        let p = decodeURIComponent(req.url.split('?')[0]);
        if (p === '/') p = PAGE;
        const f = path.normalize(path.join(ROOT, p));
        if (!f.startsWith(ROOT)) { res.writeHead(403); res.end(); return; }
        const data = fs.readFileSync(f);
        res.writeHead(200, {
          'Content-Type': MIME[path.extname(f).toLowerCase()] || 'application/octet-stream',
        });
        res.end(data);
      } catch (e) {
        res.writeHead(404); res.end('not found');
      }
    });
    server.on('error', reject);
    server.listen(PORT, '127.0.0.1', () => resolve(server));
  });
}

async function serverAlive() {
  try {
    const r = await fetch(URL);
    return r.ok;
  } catch (e) {
    return false;
  }
}

function table(results) {
  const cols = [
    ['вариант', 14], ['avg', 7], ['p50', 7], ['p95', 7], ['max', 7],
    ['>33', 6], ['>50', 6], ['>100', 6], ['LT', 4], ['open-avg', 9],
  ];
  const line = vals => cols.map((c, i) => String(vals[i] ?? '').padEnd(c[1])).join(' ');
  const out = ['', line(cols.map(c => c[0])), line(cols.map(c => '─'.repeat(c[1])))];
  for (const [name, r] of Object.entries(results)) {
    if (r.error) { out.push(name.padEnd(14) + ' ERROR: ' + r.error); continue; }
    out.push(line([
      name, r.avgMs, r.p50Ms, r.p95Ms, r.maxMs,
      r.janky33, r.over50, r.over100, r.longTasks,
      r.openAnim ? r.openAnim.avgMs : '-',
    ]));
    out.push('  renderer: ' + (r.renderer || '?') +
      ' | open-anim: n=' + (r.openAnim ? r.openAnim.n : 0) +
      ' p95=' + (r.openAnim ? r.openAnim.p95Ms : '-') +
      ' max=' + (r.openAnim ? r.openAnim.maxMs : '-'));
  }
  return out.join('\n');
}

(async () => {
  let server = null;
  if (process.argv.includes('--serve')) {
    server = await startServer();
    console.log('СЕРВЕР: ' + URL);
    return; // сервер держит процесс живым
  }
  if (await serverAlive()) {
    console.log('сервер уже запущен: ' + URL);
  } else {
    server = await startServer();
    console.log('http: ' + URL);
  }

  const { chromium } = require('playwright');
  // CHROME_EXE — свой Chrome for Testing (когда штатная загрузка playwright
  // недоступна); без него берётся браузер из playwright install.
  const exe = process.env.CHROME_EXE || undefined;
  const browser = await chromium.launch({
    headless: false,
    executablePath: exe,
    args: [
      '--disable-gpu',
      '--enable-unsafe-swiftshader',
      '--window-size=1280,800',
      '--no-first-run',
      '--disable-extensions',
    ],
  });
  console.log('chromium: ' + browser.version() + ' | флаги: --disable-gpu --enable-unsafe-swiftshader');
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  page.on('console', m => {
    const t = m.type();
    if (t === 'error' || t === 'warning' || t === 'log')
      console.log('[' + t + '] ' + m.text());
  });
  page.on('pageerror', e => console.log('[pageerror] ' + e.message));

  let code = 0;
  try {
    await page.goto(URL, { waitUntil: 'load', timeout: 30000 });
    const t0 = Date.now();
    const res = await page.evaluate(
      ([cycles, set]) => window.runBench(cycles, set), [CYCLES, SET]);
    console.log('\n=== циклов: ' + CYCLES + ', ' +
      Math.round((Date.now() - t0) / 1000) + 'с ===');
    console.log(table(res.variants || {}));
    fs.writeFileSync(OUT, JSON.stringify(res, null, 2));
    console.log('\nзаписано: ' + OUT);
  } catch (e) {
    console.error('FAIL: ' + e.message);
    code = 1;
  } finally {
    await browser.close().catch(() => {});
    if (server) server.close();
    setTimeout(() => process.exit(code), 500);
  }
})().catch(e => {
  console.error('FAIL: ' + e.message);
  process.exit(1);
});
