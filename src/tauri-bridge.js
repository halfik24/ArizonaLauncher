/**
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
  "set_game_paths": ["game", "launcher"],
  "start_game": ["nickname", "serverData", "launchParams"],
  "update_nickname": ["nickname"],
  "update_server": ["serverNumber"],
  "update_launch_params": ["params"],
  "save_read_news_ids": ["ids"],
  "get_install_action": ["folderPath"],
  "get_install_action_api": ["folderPath"],
  "check_patches_available_api": ["version"],
  "fetch_image_as_base64": ["url"],
  "get_wallpaper_thumb": ["filename"],
  "get_wallpaper_full": ["filename"],
  "save_launcher_settings": ["settings"],
  "set_window_title": ["title"],
  "save_layout_order": ["order"],
  "save_profiles": ["profiles", "activeId"],
  "write_patches": ["data"],
  "restore_patches_backup": ["filename"],
  "delete_patches_backup": ["filename"],
  "preview_patches_backup": ["filename"],
  "download_and_install_patches": ["version"],
  "check_patches_available": ["version"],
  "install_patch_preset": ["url"],
  "export_patches": ["data"],
  "install_patches_file": ["filename", "contentB64"],
  "check_update_api": ["gamePath"],
  "start_launcher_install": ["folderPath", "version", "installType"],
  "toggle_moonloader_script": ["scriptName", "enable"],
  "delete_moonloader_script": ["filename"],
  "install_moonloader_script": ["url", "filename"],
  "install_lua_file": ["filename", "contentB64"],
  "install_library": ["name", "url"],
  "uninstall_library": ["name"],
  "install_other_file": ["url", "filename", "destination"],
  "remove_other_file": ["filename", "destination"],
  "check_other_installed": ["filename", "destination"]
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
