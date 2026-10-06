/* Arizona RP Launcher v2 — new simple UI.
 * Uses Tauri invoke directly (only working backend commands). */
(function () {
  'use strict';

  const $ = (id) => document.getElementById(id);

  function invoke(cmd, args) {
    const t = window.__TAURI__;
    if (t && t.core && typeof t.core.invoke === 'function') return t.core.invoke(cmd, args || {});
    return Promise.reject(new Error('Tauri API unavailable'));
  }

  function toast(msg, kind) {
    const box = $('toasts');
    const el = document.createElement('div');
    el.className = 'toast' + (kind ? ' ' + kind : '');
    el.textContent = msg;
    box.appendChild(el);
    setTimeout(() => el.remove(), 4500);
  }

  function setStatus(msg, kind) {
    const el = $('play-status');
    el.textContent = msg || '';
    el.className = 'status' + (kind ? ' ' + kind : '');
  }

  const state = { servers: [], selected: null, config: null, launchParams: null };

  function onlineBadge(s) {
    const online = Number(s.online) || 0;
    const max = Number(s.maxplayers) || 1000;
    return '<span class="online">' + online + '</span>/' + max;
  }

  function renderServers(filter) {
    const list = $('server-list');
    list.innerHTML = '';
    const q = (filter || '').toLowerCase();
    const items = state.servers.filter((s) => !q || String(s.name).toLowerCase().includes(q) || String(s.number).includes(q));
    $('sb-servers').textContent = 'Серверов: ' + state.servers.length;
    if (!items.length) {
      list.innerHTML = '<div class="empty">Нет серверов</div>';
      return;
    }
    for (const s of items) {
      const b = document.createElement('button');
      b.className = 'server' + (state.selected && String(state.selected.number) === String(s.number) ? ' active' : '');
      const icon = s.icon ? '<img src="' + s.icon + '" alt="">' : '<img alt="">';
      b.innerHTML = icon + '<span class="sinfo"><span class="sname">' + escapeHtml(String(s.number)) + ' · ' + escapeHtml(String(s.name)) +
        (s.recommended ? '<span class="rec">REC</span>' : '') + '</span><br><span class="smeta">' + onlineBadge(s) + ' · очер. ' + escapeHtml(String(s.queue || 0)) + '</span></span>';
      b.addEventListener('click', () => selectServer(s));
      list.appendChild(b);
    }
    if (window.lucide) lucide.createIcons();
  }

  function escapeHtml(s) {
    return s.replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  }

  function selectServer(s) {
    state.selected = s;
    invoke('update_server', { serverNumber: Number(s.number) || 0 }).catch(() => {});
    renderServers($('server-search').value);
    const card = $('server-card');
    card.classList.remove('hidden');
    card.innerHTML = '<strong>Сервер ' + escapeHtml(String(s.number)) + ' — ' + escapeHtml(String(s.name)) + '</strong><br>Онлайн: ' +
      onlineBadge(s) + ' · Очередь: ' + escapeHtml(String(s.queue || 0)) + ' · ' + escapeHtml(String(s.ip || '')) + ':' + escapeHtml(String(s.port || 7777));
  }

  async function loadServers() {
    const list = $('server-list');
    try {
      const res = await invoke('get_servers');
      const arr = Array.isArray(res) ? res : (res && Array.isArray(res.servers) ? res.servers : []);
      state.servers = arr;
      $('conn-dot').classList.remove('off');
      renderServers($('server-search').value);
      // restore last server
      if (state.config && state.config.last_server != null) {
        const found = arr.find((s) => String(s.number) === String(state.config.last_server));
        if (found) selectServer(found);
      }
    } catch (e) {
      $('conn-dot').classList.add('off');
      list.innerHTML = '<div class="empty">Нет связи с сервером</div>';
    }
  }

  async function loadNews() {
    const box = $('news');
    try {
      const res = await invoke('get_launcher_news');
      if (res && res.success && res.text) box.textContent = res.text.trim();
      else box.textContent = (res && res.message) || 'Новостей пока нет';
    } catch (e) {
      box.textContent = 'Не удалось загрузить новости';
    }
  }

  async function loadPatches() {
    const box = $('patches');
    try {
      const res = await invoke('check_patches_status');
      if (!res) { box.textContent = 'Нет данных'; return; }
      box.innerHTML = '<div><strong>' + escapeHtml(String(res.message || res.status || '')) + '</strong></div>' +
        (res.message_detail ? '<div style="color:var(--dim)">' + escapeHtml(String(res.message_detail)) + '</div>' : '') +
        (res.version ? '<div>Версия: ' + escapeHtml(String(res.version)) + '</div>' : '') +
        (res.can_update && res.download_url ? '<div style="margin-top:8px"><button class="btn" id="btn-dl-patches">Скачать патчи</button></div>' : '');
      const dl = document.getElementById('btn-dl-patches');
      if (dl) dl.addEventListener('click', () => {
        if (window.__TAURI__ && window.__TAURI__.opener) return;
        toast('Скачивание патчей — в разработке бэкенда');
      });
    } catch (e) {
      box.textContent = 'Не удалось проверить патчи';
    }
  }

  async function loadConfig() {
    try {
      const cfg = await invoke('get_config');
      state.config = cfg || {};
      if (cfg.last_nickname) $('nickname').value = cfg.last_nickname;
      $('sb-paths').textContent = 'Пути: ' + (cfg.paths_configured ? 'OK' : 'не настроены');
      $('sb-mode').textContent = 'Лаунчер: ' + (cfg.launcher_version || 'v8');
      try {
        const lp = await invoke('read_launch_params');
        state.launchParams = lp || {};
      } catch (e) { state.launchParams = {}; }
    } catch (e) {
      $('sb-paths').textContent = 'Пути: ошибка';
    }
  }

  async function play() {
    const nick = $('nickname').value.trim();
    if (!nick) { setStatus('Введи ник', 'err'); return; }
    if (!state.selected) { setStatus('Выбери сервер слева', 'err'); return; }
    const btn = $('btn-play');
    btn.disabled = true;
    setStatus('Запуск…');
    try {
      await invoke('update_nickname', { nickname: nick }).catch(() => {});
      const res = await invoke('start_game', {
        nickname: nick,
        serverData: state.selected,
        launchParams: state.launchParams || null
      });
      if (res && res.success) {
        setStatus(res.message || 'Игра запускается', 'ok');
        toast('Игра запускается', 'ok');
      } else {
        setStatus((res && res.message) || 'Ошибка запуска', 'err');
        if (res && (res.code === 'GAME_MISSING' || res.code === 'LAUNCHER_MISSING')) openSettings('Укажи пути к игре в настройках');
      }
    } catch (e) {
      setStatus(String(e && e.message || e), 'err');
    } finally {
      btn.disabled = false;
    }
  }

  /* ===== settings ===== */
  function openSettings(hint) {
    $('settings-modal').classList.remove('hidden');
    if (hint) { $('settings-status').textContent = hint; $('settings-status').className = 'status err'; }
    fillSettings();
  }
  function closeSettings() { $('settings-modal').classList.add('hidden'); }

  async function fillSettings() {
    const cfg = state.config || {};
    $('set-game').value = cfg.game_path || '';
    $('set-launcher').value = cfg.launcher_path || '';
    const lp = state.launchParams || {};
    $('set-memory').value = lp.memory || 4096;
    $('set-cdn').value = lp.cdn || '1,1,1';
    $('set-windowed').checked = !!lp.window_mode;
    $('set-autologin').checked = lp.autologin !== false;
    $('set-widescreen').checked = !!lp.widescreen;
    $('set-grass').checked = !!lp.enable_grass;
  }

  async function saveSettings() {
    const st = $('settings-status');
    try {
      const game = $('set-game').value.trim();
      const launcher = $('set-launcher').value.trim();
      if (game || launcher) {
        await invoke('set_game_paths', { game: game, launcher: launcher });
      }
      const lp = Object.assign({}, state.launchParams || {}, {
        memory: Math.max(512, Number($('set-memory').value) || 4096),
        cdn: $('set-cdn').value.trim() || '1,1,1',
        window_mode: $('set-windowed').checked,
        autologin: $('set-autologin').checked,
        widescreen: $('set-widescreen').checked,
        enable_grass: $('set-grass').checked
      });
      await invoke('update_launch_params', { params: lp });
      state.launchParams = lp;
      await loadConfig();
      st.textContent = 'Сохранено';
      st.className = 'status ok';
      toast('Настройки сохранены', 'ok');
    } catch (e) {
      st.textContent = String(e && e.message || e);
      st.className = 'status err';
    }
  }

  function bind() {
    $('btn-play').addEventListener('click', play);
    $('nickname').addEventListener('keydown', (e) => { if (e.key === 'Enter') play(); });
    $('server-search').addEventListener('input', (e) => renderServers(e.target.value));
    $('btn-refresh').addEventListener('click', loadServers);
    $('btn-news').addEventListener('click', loadNews);
    $('btn-settings').addEventListener('click', () => openSettings());
    $('btn-settings-close').addEventListener('click', closeSettings);
    $('settings-modal').addEventListener('click', (e) => { if (e.target.id === 'settings-modal') closeSettings(); });
    document.addEventListener('keydown', (e) => { if (e.key === 'Escape') closeSettings(); });
    $('btn-settings-save').addEventListener('click', saveSettings);
    $('btn-autodetect').addEventListener('click', async () => {
      const st = $('settings-status');
      st.textContent = 'Поиск…'; st.className = 'status';
      try {
        const res = await invoke('auto_detect_paths');
        if (res && res.success) {
          $('set-game').value = res.game_path || '';
          $('set-launcher').value = res.launcher_path || '';
          st.textContent = 'Найдено'; st.className = 'status ok';
        } else { st.textContent = 'Не найдено — укажи вручную'; st.className = 'status err'; }
      } catch (e) { st.textContent = String(e); st.className = 'status err'; }
    });
    $('btn-open-data').addEventListener('click', () => invoke('open_data_folder').catch((e) => toast(String(e), 'err')));
    $('btn-folder').addEventListener('click', () => invoke('open_game_folder').then((r) => {
      if (r && r.success === false) { toast(r.message || 'Путь не задан', 'err'); openSettings('Укажи путь к игре'); }
    }).catch((e) => toast(String(e), 'err')));
    $('btn-tg').addEventListener('click', () => invoke('open_telegram_share').catch((e) => toast(String(e), 'err')));
    $('btn-min').addEventListener('click', () => invoke('minimize_window').catch(() => {}));
    $('btn-tray').addEventListener('click', () => invoke('minimize_to_tray').catch(() => {}));
  }

  async function init() {
    bind();
    if (window.lucide) lucide.createIcons();
    await loadConfig();
    await Promise.all([loadServers(), loadNews(), loadPatches()]);
    if (window.lucide) lucide.createIcons();
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();
