// Recetas de lectura con vocabulario cerrado; nunca evalúa código del asistente.
import * as pluginFs from 'node:fs/promises';
import pluginPath from 'node:path';
import { randomUUID as pluginUUID } from 'node:crypto';

const PLUGIN_LIMIT = 65536;
const pluginId = value => typeof value === 'string' && /^[a-z][a-z0-9_]{0,63}$/.test(value);
const pluginObject = value => value && typeof value === 'object' && !Array.isArray(value);
function pluginFail(code) { const error = new Error(code); error.code = code; throw error; }
function pluginKeys(value, allowed, required = []) {
  if (!pluginObject(value) || Object.keys(value).some(key => !allowed.includes(key)) || required.some(key => !Object.hasOwn(value, key))) pluginFail('invalid_plugin');
}
function pluginText(value, max = 2000) {
  if (typeof value !== 'string' || !value.length || value.length > max || value.includes('\0')) pluginFail('invalid_plugin');
}
function pluginURL(value) {
  pluginText(value, 4000);
  let url;
  try { url = new URL(value); } catch { pluginFail('invalid_plugin'); }
  if (!['https:', 'http:'].includes(url.protocol) || url.username || url.password || url.hash) pluginFail('invalid_plugin');
  return url;
}
function pluginPointer(value) {
  if (typeof value !== 'string' || value.length > 500 || (value !== '' && !value.startsWith('/')) || /~(?![01])/u.test(value)) pluginFail('invalid_plugin');
}
export function validateWebPlugin(manifest) {
  pluginKeys(manifest, ['id', 'name', 'origin', 'login_url', 'login_urls', 'description', 'capabilities', 'limitations', 'functions'], ['id', 'name', 'origin', 'functions']);
  if (!pluginId(manifest.id)) pluginFail('invalid_plugin');
  pluginText(manifest.name, 200);
  if (manifest.description !== undefined) pluginText(manifest.description, 4000);
  for (const field of ['capabilities', 'limitations']) if (manifest[field] !== undefined) {
    if (!Array.isArray(manifest[field]) || manifest[field].length > 20) pluginFail('invalid_plugin');
    for (const text of manifest[field]) pluginText(text, 1000);
  }
  if (pluginURL(manifest.origin).origin !== manifest.origin) pluginFail('invalid_plugin');
  if (manifest.login_url !== undefined && pluginURL(manifest.login_url).search) pluginFail('invalid_plugin');
  if (manifest.login_urls !== undefined) {
    if (!Array.isArray(manifest.login_urls) || manifest.login_urls.length > 8) pluginFail('invalid_plugin');
    for (const url of manifest.login_urls) if (pluginURL(url).search) pluginFail('invalid_plugin');
  }
  if (!Array.isArray(manifest.functions) || !manifest.functions.length || manifest.functions.length > 20) pluginFail('invalid_plugin');
  const names = new Set();
  for (const fn of manifest.functions) {
    pluginKeys(fn, ['name', 'description', 'parameters', 'transport', 'path', 'query', 'result_type', 'result_pointer', 'fields', 'selector', 'ready_selector', 'login_selector'], ['name', 'description', 'parameters', 'transport', 'path', 'result_type']);
    if (!pluginId(fn.name) || names.has(fn.name)) pluginFail('invalid_plugin');
    names.add(fn.name); pluginText(fn.description, 1000);
    if (!pluginObject(fn.parameters) || Object.keys(fn.parameters).length > 12) pluginFail('invalid_plugin');
    for (const [key, type] of Object.entries(fn.parameters)) if (!pluginId(key) || !['string', 'integer'].includes(type)) pluginFail('invalid_plugin');
    pluginText(fn.path);
    if (!fn.path.startsWith('/') || fn.path.startsWith('//') || /[?#\\]/u.test(fn.path)) pluginFail('invalid_plugin');
    if (fn.query !== undefined && (!pluginObject(fn.query) || Object.keys(fn.query).length > 12)) pluginFail('invalid_plugin');
    for (const [key, value] of Object.entries(fn.query || {})) {
      if (!/^[a-zA-Z0-9_.-]{1,100}$/.test(key) || /token|secret|password|cookie|authorization|session|api.?key/i.test(key)) pluginFail('invalid_plugin');
      pluginText(value);
    }
    const used = new Set();
    for (const template of [fn.path, ...Object.values(fn.query || {})]) {
      const remaining = template.replace(/\{([a-z][a-z0-9_]{0,63})\}/g, (_, key) => {
        if (!Object.hasOwn(fn.parameters, key)) pluginFail('invalid_plugin');
        used.add(key); return '';
      });
      if (/[{}]/u.test(remaining)) pluginFail('invalid_plugin');
    }
    if (Object.keys(fn.parameters).some(key => !used.has(key))) pluginFail('invalid_plugin');
    if (fn.transport === 'http') {
      if (!['array', 'object', 'string', 'number', 'boolean'].includes(fn.result_type) || fn.selector !== undefined || fn.ready_selector !== undefined || fn.login_selector !== undefined) pluginFail('invalid_plugin');
      if (fn.result_pointer !== undefined) pluginPointer(fn.result_pointer);
    } else if (fn.transport === 'page') {
      if (!['array', 'string'].includes(fn.result_type) || fn.result_pointer !== undefined) pluginFail('invalid_plugin');
      pluginText(fn.selector, 1000);
      if (fn.ready_selector !== undefined) pluginText(fn.ready_selector, 1000);
      if (fn.login_selector !== undefined) pluginText(fn.login_selector, 1000);
    } else pluginFail('invalid_plugin');
    if (fn.fields !== undefined) {
      if (!['object', 'array'].includes(fn.result_type) || !pluginObject(fn.fields) || !Object.keys(fn.fields).length || Object.keys(fn.fields).length > 20) pluginFail('invalid_plugin');
      for (const [key, value] of Object.entries(fn.fields)) {
        if (!pluginId(key)) pluginFail('invalid_plugin');
        if (fn.transport === 'http') pluginPointer(value); else pluginText(value, 1000);
      }
    }
  }
  if (Buffer.byteLength(JSON.stringify(manifest)) > PLUGIN_LIMIT / 2) pluginFail('invalid_plugin');
  return manifest;
}
function pluginAt(value, pointer = '') {
  for (const token of pointer === '' ? [] : pointer.slice(1).split('/')) {
    const key = token.replace(/~1/g, '/').replace(/~0/g, '~');
    if (!value || !Object.hasOwn(value, key)) pluginFail('site_changed');
    value = value[key];
  }
  return value;
}
function pluginType(value) { return Array.isArray(value) ? 'array' : value === null ? 'null' : typeof value; }
function pluginLogin(manifest, value) {
  const url = new URL(value);
  return (manifest.login_urls || []).some(candidate => {
    const login = new URL(candidate);
    return url.origin === login.origin && (url.pathname === login.pathname || (login.pathname !== '/' && url.pathname.startsWith(login.pathname.replace(/\/$/, '') + '/')));
  });
}
function pluginAuth(manifest) {
  return { status: 'authentication_required', ...(manifest.login_url ? { login_url: manifest.login_url } : {}), message: 'Esta operación necesita una sesión válida. El usuario puede iniciar sesión en Gennie Web y volver a intentarla.' };
}
function pluginTarget(manifest, fn, args) {
  if (!pluginObject(args) || Object.keys(args).length !== Object.keys(fn.parameters).length) pluginFail('invalid_plugin_arguments');
  for (const [key, type] of Object.entries(fn.parameters)) {
    const value = args[key];
    if (type === 'integer' ? !Number.isSafeInteger(value) : typeof value !== 'string' || !value.length || value.length > 1000 || value.includes('\0')) pluginFail('invalid_plugin_arguments');
  }
  const render = (template, encode) => template.replace(/\{([a-z][a-z0-9_]{0,63})\}/g, (_, key) => {
    const value = String(args[key]);
    if (encode && (value === '.' || value === '..')) pluginFail('invalid_plugin_arguments');
    return encode ? encodeURIComponent(value) : value;
  });
  const url = pluginURL(manifest.origin + render(fn.path, true));
  if (url.origin !== manifest.origin) pluginFail('invalid_plugin_arguments');
  for (const [key, value] of Object.entries(fn.query || {})) url.searchParams.set(key, render(value, false));
  return url.href;
}

export function createWebPlugins({ directory, getContext }) {
  async function prepare() {
    await pluginFs.mkdir(directory, { recursive: true, mode: 0o700 });
    const info = await pluginFs.lstat(directory);
    if (!info.isDirectory() || info.isSymbolicLink()) pluginFail('plugin_storage_error');
    await pluginFs.chmod(directory, 0o700);
  }
  function filename(id) {
    if (!pluginId(id)) pluginFail('invalid_plugin');
    return pluginPath.join(directory, `${id}.json`);
  }
  async function load(id) {
    const file = filename(id);
    try {
      const directoryInfo = await pluginFs.lstat(directory);
      if (!directoryInfo.isDirectory() || directoryInfo.isSymbolicLink()) pluginFail('plugin_storage_error');
    } catch (error) { if (error.code === 'ENOENT') return null; throw error; }
    let info;
    try { info = await pluginFs.lstat(file); } catch (error) { if (error.code === 'ENOENT') return null; throw error; }
    if (!info.isFile() || info.isSymbolicLink() || info.size > PLUGIN_LIMIT) pluginFail('plugin_storage_error');
    const record = JSON.parse(await pluginFs.readFile(file, 'utf8'));
    if (record.deleted === true) {
      if (record.id !== id || !Number.isSafeInteger(record.revision) || record.revision < 1) pluginFail('plugin_storage_error');
      return record;
    }
    validateWebPlugin(record.manifest);
    if (record.manifest.id !== id || !Number.isSafeInteger(record.revision) || record.revision < 1 || !pluginObject(record.verified)) pluginFail('plugin_storage_error');
    return record;
  }
  async function store(record) {
    await prepare();
    const temporary = pluginPath.join(directory, `.${pluginUUID()}.tmp`);
    try {
      await pluginFs.writeFile(temporary, JSON.stringify(record), { mode: 0o600, flag: 'wx' });
      await pluginFs.rename(temporary, filename(record.manifest?.id || record.id));
    } finally { await pluginFs.unlink(temporary).catch(error => { if (error.code !== 'ENOENT') throw error; }); }
  }
  async function execute(manifest, fn, args) {
    const target = pluginTarget(manifest, fn, args);
    const context = await getContext();
    if (!context) return { status: 'browser_closed' };
    const deadline = Date.now() + 6500;
    const timeout = () => Math.max(1, deadline - Date.now());
    if (fn.transport === 'http') {
      let url = target;
      for (let redirect = 0; redirect <= 4; redirect++) {
        const response = await context.request.get(url, { timeout: timeout(), maxRedirects: 0, failOnStatusCode: false });
        try {
          const status = response.status();
          if (status === 401) return pluginAuth(manifest);
          if (status === 403) return { status: 'access_denied', http_status: status };
          if (status >= 300 && status < 400) {
            const location = response.headers().location;
            if (!location) return { status: 'site_changed', http_status: status };
            const next = pluginURL(new URL(location, url).href);
            if (pluginLogin(manifest, next.href)) return pluginAuth(manifest);
            if (next.origin !== manifest.origin) return { status: 'redirect_blocked', http_status: status };
            url = next.href; continue;
          }
          if (status < 200 || status >= 300) return { status: 'http_error', http_status: status };
          if (!/\bjson\b/i.test(response.headers()['content-type'] || '')) return { status: 'site_changed', http_status: status };
          const body = await response.body();
          if (body.length > 1024 * 1024) return { status: 'response_too_large' };
          let data;
          try { data = JSON.parse(body.toString('utf8')); } catch { return { status: 'site_changed' }; }
          data = pluginAt(data, fn.result_pointer);
          if (pluginType(data) !== fn.result_type) return { status: 'site_changed' };
          if (fn.fields) {
            const project = item => Object.fromEntries(Object.entries(fn.fields).map(([key, pointer]) => [key, pluginAt(item, pointer)]));
            data = Array.isArray(data) ? data.map(project) : project(data);
          }
          if (Buffer.byteLength(JSON.stringify(data)) > 100000) return { status: 'response_too_large' };
          return { status: 'ok', data };
        } finally { await response.dispose(); }
      }
      return { status: 'redirect_limit' };
    }
    const page = await context.newPage();
    try {
      // La extracción usa una pestaña propia, sin cambiar la seleccionada por el usuario.
      await page.route('**/*', async route => {
        const url = new URL(route.request().url());
        if (['GET', 'HEAD'].includes(route.request().method()) && (url.origin === manifest.origin || (route.request().isNavigationRequest() && pluginLogin(manifest, url.href)))) await route.continue();
        else await route.abort();
      });
      const response = await page.goto(target, { waitUntil: 'domcontentloaded', timeout: timeout() });
      if (pluginLogin(manifest, page.url()) || response?.status() === 401) return pluginAuth(manifest);
      if (response?.status() === 403) return { status: 'access_denied', http_status: 403 };
      if (new URL(page.url()).origin !== manifest.origin) return { status: 'redirect_blocked' };
      if (response && !response.ok()) return { status: 'http_error', http_status: response.status() };
      if (fn.login_selector && await page.locator(fn.login_selector).first().isVisible()) return pluginAuth(manifest);
      await page.locator(fn.ready_selector || fn.selector).first().waitFor({ state: 'attached', timeout: timeout() });
      const extracted = await page.locator(fn.selector).evaluateAll((nodes, spec) => {
        let truncated = false;
        const text = node => {
          const value = (node.textContent || '').trim();
          if (value.length > 2000) truncated = true;
          return value.slice(0, 2000);
        };
        if (spec.type === 'string') return { data: text(nodes[0]), truncated };
        if (nodes.length > 200) truncated = true;
        const data = nodes.slice(0, 200).map(node => spec.fields ? Object.fromEntries(Object.entries(spec.fields).map(([key, selector]) => {
          const child = node.querySelector(selector);
          return [key, child ? text(child) : null];
        })) : text(node));
        return { data, truncated };
      }, { type: fn.result_type, fields: fn.fields });
      const { data, truncated } = extracted;
      if (Array.isArray(data) && data.some(row => pluginObject(row) && Object.values(row).includes(null))) return { status: 'site_changed' };
      if (Buffer.byteLength(JSON.stringify(data)) > 100000) return { status: 'response_too_large' };
      return { status: 'ok', data, truncated };
    } finally { await page.close(); }
  }
  async function catalogueRecords() {
    await prepare();
    const files = (await pluginFs.readdir(directory)).filter(name => /^[a-z][a-z0-9_]{0,63}\.json$/.test(name)).sort();
    const records = [];
    for (const file of files) { const record = await load(file.slice(0, -5)); if (record && !record.deleted) records.push(record); }
    if (records.length > 50) pluginFail('plugin_storage_error');
    return records;
  }
  async function capacity(previous) {
    if ((!previous || previous.deleted) && (await catalogueRecords()).length >= 50) pluginFail('plugin_storage_error');
  }
  return {
    async dispatch(request) {
      if (request.action === 'list') return { plugins: await catalogueRecords() };
      if (request.action === 'promote') {
        if (!pluginId(request.candidate_id)) pluginFail('invalid_plugin_arguments');
        const candidateStore = createWebPlugins({ directory: pluginPath.join(directory, 'candidates', request.candidate_id), getContext });
        const candidate = (await candidateStore.dispatch({ action: 'get', plugin_id: request.plugin_id })).plugin;
        if (!candidate || candidate.manifest.functions.some(fn => candidate.verified[fn.name]?.revision !== candidate.revision)) return { status: 'verification_required' };
        const previous = await load(request.plugin_id);
        if (request.expected_revision !== (previous?.revision || 0)) return { status: 'revision_conflict' };
        await capacity(previous);
        const revision = (previous?.revision || 0) + 1;
        const record = { manifest: candidate.manifest, revision, updated_at: new Date().toISOString(), verified: Object.fromEntries(candidate.manifest.functions.map(fn => [fn.name, { ...candidate.verified[fn.name], revision }])) };
        await store(record);
        return { status: 'promoted', plugin: record };
      }
      if (request.action === 'save') {
        const manifest = validateWebPlugin(request.manifest);
        const previous = await load(manifest.id);
        if (request.expected_revision !== (previous && !previous.deleted ? previous.revision : 0)) return { status: 'revision_conflict' };
        await capacity(previous);
        const record = { manifest, revision: (previous?.revision || 0) + 1, updated_at: new Date().toISOString(), verified: {} };
        await store(record);
        return { status: 'saved', plugin: record };
      }
      const record = await load(request.plugin_id);
      if (!record || record.deleted) return { status: 'plugin_not_found', revision: record?.revision || 0 };
      if (request.action === 'delete') {
        if (request.expected_revision !== record.revision) return { status: 'revision_conflict' };
        await store({ id: record.manifest.id, revision: record.revision + 1, deleted: true });
        return { status: 'deleted', plugin_id: record.manifest.id };
      }
      if (request.action === 'get') return { plugin: record };
      if (!['run', 'verify'].includes(request.action)) pluginFail('invalid_plugin_arguments');
      const fn = record.manifest.functions.find(fn => fn.name === request.function_name);
      if (!fn) return { status: 'function_not_found' };
      if (request.expected_revision !== record.revision) return { status: 'revision_conflict' };
      if (request.action === 'run' && !Object.hasOwn(record.verified, fn.name)) return { status: 'verification_required' };
      let result;
      try { result = await execute(record.manifest, fn, request.arguments || {}); }
      catch (error) {
        if (error.code === 'invalid_plugin_arguments') throw error;
        result = { status: error.code === 'site_changed' ? 'site_changed' : error.name === 'TimeoutError' ? 'timeout' : 'execution_failed' };
      }
      if (request.action === 'verify') {
        delete record.verified[fn.name];
        if (result.status === 'ok') record.verified[fn.name] = { verified_at: new Date().toISOString(), revision: record.revision };
        await store(record);
      }
      return { ...result, plugin_id: record.manifest.id, function_name: fn.name, revision: record.revision };
    },
  };
}
