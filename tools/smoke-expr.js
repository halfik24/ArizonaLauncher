(async () => {
  const a = window.pywebview.api;
  const out = {};
  const t = async (name, fn, short) => {
    try {
      const v = await fn();
      let s = JSON.stringify(v);
      if (s && s.length > 400) s = s.slice(0, 400) + '...[' + s.length + ']';
      out[name] = s;
    } catch (e) {
      out[name] = 'ERR: ' + ((e && e.message) || e);
    }
  };
  await t('fetch_patch_presets', () => a.fetch_patch_presets());
  await t('fetch_patches_schema', () => a.fetch_patches_schema());
  await t('fetch_others_catalog', () => a.fetch_others_catalog());
  await t('get_debug_info', () => a.get_debug_info());
  await t('check_tray_support', () => a.check_tray_support());
  await t('check_launcher_update', () => a.check_launcher_update());
  await t('check_other_installed', () => a.check_other_installed('gta_sa.exe', 'root'));
  await t('get_wallpapers', () => a.get_wallpapers());
  const w = await a.get_wallpapers().catch(() => null);
  const first = w && w.wallpapers && w.wallpapers[0];
  if (first && (first.file || first.filename || first.name)) {
    const fname = first.file || first.filename || first.name;
    await t('get_wallpaper_thumb', () => a.get_wallpaper_thumb(fname));
  }
  await t('get_launch_method', () => a.get_launch_method());
  await t('get_saved_data', () => a.get_saved_data());
  return out;
})()
