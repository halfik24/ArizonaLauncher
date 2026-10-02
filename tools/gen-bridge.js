// Генерирует src/tauri-bridge.js из сигнатур Rust-команд
const fs = require('fs');

const rs = fs.readFileSync('src-tauri/src/launcher.rs', 'utf8');
const main = fs.readFileSync('src-tauri/src/main.rs', 'utf8');

// имена зарегистрированных команд
const handlerBlock = main.match(/generate_handler!\[([\s\S]*?)\]/)[1];
const registered = [...handlerBlock.matchAll(/launcher::(\w+)/g)].map(m => m[1]);

// сигнатуры: (pub )?(async )?fn name(params) — params могут быть многострочными
const sigs = {};
const fnRe = /(?:pub\s+)?(?:async\s+)?fn\s+([A-Za-z0-9_]+)\s*\(([\s\S]*?)\)\s*->/g;
let m;
while ((m = fnRe.exec(rs))) {
  const name = m[1];
  if (sigs[name]) continue;
  // разрезать параметры по запятым с учётом <>, (), [] и строк
  const raw = m[2];
  const parts = []; let depth = 0, start = 0, quote = null;
  for (let i = 0; i < raw.length; i++) {
    const c = raw[i];
    if (quote) { if (c === '\\') i++; else if (c === quote) quote = null; continue; }
    // только двойные кавычки: одинарная в Rust — это lifetime ('_), а не строка
    if (c === '"') { quote = c; continue; }
    if ('<([{'.includes(c)) depth++;
    else if ('>)]}'.includes(c)) depth--;
    else if (c === ',' && depth === 0) { parts.push(raw.slice(start, i).trim()); start = i + 1; }
  }
  parts.push(raw.slice(start).trim());
  sigs[name] = parts
    .filter(p => p)
    .filter(p => !/^state\b/.test(p) && !/^app\b:/i.test(p))
    .map(p => p.replace(/:([\s\S]*)$/, '').trim())
    .filter(p => p && p !== '_');
}

const toCamel = s => s.replace(/_([a-z0-9])/g, (_, c) => c.toUpperCase());

const entries = registered.map(name => {
  const params = sigs[name];
  if (!params) {
    console.error(`!! нет сигнатуры для ${name}`);
    return null;
  }
  return [name, params.map(toCamel)];
}).filter(Boolean);

const missing = registered.filter(n => !sigs[n]);
console.log(`команд: ${registered.length}, без сигнатуры: ${missing.length}`);
const withArgs = entries.filter(([, p]) => p.length);
console.log(`с аргументами: ${withArgs.length}`);
for (const [n, p] of withArgs) console.log(`  ${n}: [${p.join(', ')}]`);

const table = entries.map(([n, p]) =>
  p.length ? `  ${JSON.stringify(n)}: [${p.map(x => JSON.stringify(x)).join(', ')}]` : null
).filter(Boolean).join(',\n');

const out = `/**
 * Tauri bridge — замена pywebview.api для фронтенда.
 *
 * Фронтенд (src/index.html) написан под Python/pywebview и вызывает
 * методы как позиционные: pywebview.api.start_game(nick, server, null).
 * Tauri же принимает именованные аргументы в camelCase:
 *   invoke('start_game', { nickname, serverData, launchParams })
 *
 * Поэтому здесь таблица параметров каждой команды (генерируется из
 * сигнатур в src-tauri/src/launcher.rs — см. tools/gen-bridge.js)
 * и позиции превращаются в именованные ключи.
 *
 * Событие 'pywebviewready' диспатчится, чтобы фронт запустил init()
 * (в Python это делала сама pywebview).
 */
(function () {
  'use strict';

  // commandName: [имена параметров в порядке следования (camelCase)]
  var CMD_PARAMS = {
${table}
  };

  function toCamel(s) {
    return s.replace(/_([a-z0-9])/g, function (_, c) { return c.toUpperCase(); });
  }

  function resolveInvoke() {
    var t = window.__TAURI__;
    if (t && t.core && typeof t.core.invoke === 'function') return t.core.invoke;
    return null;
  }

  // позиционные аргументы → объект {имяПараметра: значение}
  function buildPayload(cmd, args) {
    var names = CMD_PARAMS[cmd];
    var payload = {};
    if (!names) {
      if (args && args.length) {
        console.warn('[tauri-bridge] нет параметров для команды "' + cmd + '", аргументы:', args);
      }
      return payload;
    }
    for (var i = 0; i < names.length; i++) {
      var v = args[i];
      // undefined не передаём: в Rust это Option<T> => None
      if (v !== undefined) payload[names[i]] = v;
      // null передаём: фронт намеренно шлёт null (например start_game(..., null))
    }
    if (args.length > names.length) {
      console.warn('[tauri-bridge] команда "' + cmd + '": передано ' + args.length +
        ' аргументов, ожидается ' + names.length);
    }
    return payload;
  }

  function call(cmd, args) {
    var invoke = resolveInvoke();
    if (!invoke) {
      return Promise.reject(new Error('[tauri-bridge] window.__TAURI__ недоступен'));
    }
    return invoke(cmd, buildPayload(cmd, args));
  }

  // Прокси: возвращает функцию для ЛЮБОГО имени — проверки вида
  // if (window.pywebview.api.some_method) всегда истинны (как в pywebview)
  var api = new Proxy({}, {
    get: function (_, method) {
      if (typeof method === 'symbol') return undefined;
      return function () {
        return call(String(method), Array.prototype.slice.call(arguments));
      };
    }
  });

  window.pywebview = { api: api };

  function fireReady() {
    if (window.__pywebviewReadyFired) return;
    window.__pywebviewReadyFired = true;
    console.log('[tauri-bridge] PyWebView готов (Tauri)');
    try {
      window.dispatchEvent(new Event('pywebviewready'));
    } catch (e) {
      var evt = document.createEvent('Event');
      evt.initEvent('pywebviewready', false, false);
      window.dispatchEvent(evt);
    }
  }

  // Фронт регистрирует listener синхронно при разборе <body>, поэтому к
  // DOMContentLoaded он уже зарегистрирован — диспатчим событие там.
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', fireReady);
  } else {
    setTimeout(fireReady, 0);
  }
})();
`;

fs.writeFileSync('src/tauri-bridge.js', out, 'utf8');
console.log('записан src/tauri-bridge.js, байт:', out.length);
