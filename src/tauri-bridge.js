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
  "set_launch_method": ["method"],
  "save_read_news_ids": ["ids"],
  "get_install_action": ["folderPath"],
  "get_install_action_api": ["folderPath"],
  "check_patches_available_api": ["version"],
  "fetch_image_as_base64": ["url"],
  "get_wallpaper_thumb": ["filename"],
  "get_wallpaper_full": ["filename"],
  "save_launcher_settings": ["settings"],
  "set_window_title": ["title"],
  "set_window_frame": ["mode"],
  "is_gta_running": ["consumeExit"],
  "start_dragging": [],
  "start_resize": ["edge"],
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
  "repair_game_api": ["future"],
  "cancel_game_update": [],
  "game_update_status": [],
  "start_launcher_install": ["folderPath", "version", "installType"],
  "toggle_moonloader_script": ["scriptName", "enable"],
  "delete_moonloader_script": ["filename"],
  "install_moonloader_script": ["url", "filename"],
  "install_lua_file": ["filename", "contentB64"],
  "install_library": ["name", "url", "path"],
  "uninstall_library": ["name"],
  "install_other_file": ["url", "filename", "destination"],
  "remove_other_file": ["filename", "destination"],
  "check_other_installed": ["filename", "destination"],
  "open_url": ["url"]
  };

  // Имена реальных #[tauri::command] из src-tauri/src/launcher.rs.
  // Всё, чего здесь нет, прокси НЕ выдаёт: фронт пишет
  // if (window.pywebview.api.select_game_path) {...} и должен попадать
  // в свою запасную ветку «доступно только в приложении».
  var KNOWN = [
    "get_config",
    "get_saved_data",
    "get_init_data",
    "update_nickname",
    "update_server",
    "update_launch_params",
    "read_launch_params",
    "get_launch_method",
    "set_launch_method",
    "auto_detect_paths",
    "set_game_paths",
    "select_game_path",
    "start_game",
    "is_gta_running",
    "get_servers",
    "get_servers_and_news",
    "get_launcher_news",
    "get_read_news_ids",
    "save_read_news_ids",
    "get_launcher_version",
    "check_launcher_update",
    "get_install_action",
    "get_install_action_api",
    "check_patches_available_api",
    "fetch_image_as_base64",
    "get_debug_info",
    "minimize_window",
    "minimize_to_tray",
    "restore_window",
    "open_game_folder",
    "open_data_folder",
    "open_log_file",
    "open_url",
    "get_launcher_settings",
    "save_launcher_settings",
    "get_window_title",
    "set_window_title",
    "get_window_size",
    "get_window_frame",
    "set_window_frame",
    "toggle_maximize",
    "is_maximized",
    "start_dragging",
    "start_resize",
    "save_layout_order",
    "get_layout_order",
    "get_profiles",
    "save_profiles",
    "check_patches_status",
    "check_patches_files",
    "read_patches",
    "write_patches",
    "list_patches_backups",
    "restore_patches_backup",
    "delete_patches_backup",
    "preview_patches_backup",
    "download_and_install_patches",
    "get_patches_latest_version",
    "check_patches_available",
    "check_patches_update_status",
    "export_patches",
    "import_patches",
    "install_patches_file",
    "fetch_patch_presets",
    "install_patch_preset",
    "fetch_patches_schema",
    "check_update_api",
    "download_update_api",
    "repair_game_api",
    "cancel_game_update",
    "game_update_status",
    "start_launcher_install",
    "get_moonloader_scripts",
    "toggle_moonloader_script",
    "delete_moonloader_script",
    "open_moonloader_folder",
    "fetch_moonloader_catalog",
    "install_moonloader_script",
    "install_lua_file",
    "fetch_libraries_catalog",
    "list_installed_libraries",
    "install_library",
    "uninstall_library",
    "fetch_others_catalog",
    "install_other_file",
    "remove_other_file",
    "check_other_installed",
    "check_tray_support",
    "open_telegram_share",
    "get_wallpapers",
    "get_wallpaper_thumb",
    "get_wallpaper_full",
    "select_bg_image"
  ];

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
    if (KNOWN.indexOf(cmd) < 0) {
      return Promise.reject(new Error('[tauri-bridge] команда "' + cmd + '" не существует'));
    }
    var invoke = resolveInvoke();
    if (!invoke) {
      return Promise.reject(new Error('[tauri-bridge] window.__TAURI__ недоступен'));
    }
    return invoke(cmd, buildPayload(cmd, args));
  }

  // Прокси: для известной команды — функция, для неизвестной — undefined.
  // Тогда проверки вида if (window.pywebview.api.some_method) ведут себя
  // как в браузере: фронт уходит в свою запасную ветку, а не получает
  // reject от Tauri «command not found».
  var api = new Proxy({}, {
    get: function (_, method) {
      if (typeof method === 'symbol') return undefined;
      var name = String(method);
      if (KNOWN.indexOf(name) < 0) return undefined;
      return function () {
        return call(name, Array.prototype.slice.call(arguments));
      };
    }
  });

  window.pywebview = { api: api };

  // Прогресс установки лаунчера: Tauri-событие 'launcher-install-progress'
  // (эмит из src-tauri/src/launcher.rs) → глобальный хаб
  // window._onLauncherInstallProgress из index.html. Слушатель ставим до
  // диспатча 'pywebviewready'; если __TAURI__ ещё не готов — ретраим.
  function subscribeInstallProgress(attempt) {
    if (window.__installProgressSubscribed) return;
    var t = window.__TAURI__;
    if (t && t.event && typeof t.event.listen === 'function') {
      try {
        // listen() возвращает Promise — учтём возможное отклонение прав
        // (event.listen blocked по ACL) и перетянемся позже.
        var p = t.event.listen('launcher-install-progress', function (e) {
          if (typeof window._onLauncherInstallProgress === 'function') {
            window._onLauncherInstallProgress(e.payload);
          }
        });
        if (p && typeof p.then === 'function') {
          p.then(function () {
            window.__installProgressSubscribed = true;
            console.log('[tauri-bridge] подписка на launcher-install-progress установлена');
          }).catch(function (err) {
            console.warn('[tauri-bridge] подписка на launcher-install-progress отклонена:', err);
            if (attempt < 40) setTimeout(function () { subscribeInstallProgress(attempt + 1); }, 250);
          });
          return;
        }
        window.__installProgressSubscribed = true;
        console.log('[tauri-bridge] подписка на launcher-install-progress установлена');
        return;
      } catch (err) {
        console.warn('[tauri-bridge] подписка на launcher-install-progress не удалась:', err);
      }
    }
    if (attempt < 40) {
      setTimeout(function () { subscribeInstallProgress(attempt + 1); }, 250);
    } else {
      console.warn('[tauri-bridge] window.__TAURI__.event недоступен — прогресс установки не придёт');
    }
  }

  function fireReady() {
    if (window.__pywebviewReadyFired) return;
    window.__pywebviewReadyFired = true;
    subscribeInstallProgress(0);
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
