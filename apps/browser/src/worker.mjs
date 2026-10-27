// Sesión Patchright privada: interfaz acotada por stdio, sin servidor HTTP/CDP.
import { createRequire } from 'node:module';
import { mkdir, chmod, writeFile, stat, realpath, rename, unlink } from 'node:fs/promises';
import { randomUUID } from 'node:crypto';
import { createInterface } from 'node:readline';
import path from 'node:path';
import { createWebPlugins } from './plugins.mjs';

const TEXT_LIMIT = 20000;
const RESPONSE_LIMIT = 4 * 1024 * 1024;
const INTERACTIVE = 'a[href],button,input:not([type=hidden]),textarea,select,summary,[role=button],[role=link],[role=checkbox],[role=radio],[role=combobox],[role=tab],[contenteditable=true]';
function fail(code) { const error = new Error(code); error.code = code; throw error; }
function httpUrl(value) {
  let url;
  try { url = new URL(value); } catch { fail('invalid_url'); }
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) fail('invalid_url');
  return url.href;
}
// Esta función se serializa en la página: no depende de variables del helper.
function elementInfo(node) {
  const visible = !!node.getClientRects().length && getComputedStyle(node).visibility !== 'hidden';
  const label = node.getAttribute('aria-label') || [...(node.labels || [])].map(x => x.innerText).join(' ') || node.innerText || node.getAttribute('placeholder') || node.getAttribute('title') || node.getAttribute('alt') || '';
  return { tag: node.tagName.toLowerCase(), role: node.getAttribute('role'), type: node.getAttribute('type'), label: label.trim().slice(0, 400), href: node.getAttribute('href')?.slice(0, 1000) || null, disabled: !!node.disabled, visible, connected: node.isConnected };
}
async function bounded(promise, milliseconds) {
  let timer;
  try { return await Promise.race([promise, new Promise(resolve => { timer = setTimeout(() => resolve(null), milliseconds); })]); }
  finally { clearTimeout(timer); }
}
export function createBrowserSession({ runtime, profile, headless = false, homepage = 'https://gennie.io/', onBrowserClose = () => {} }) {
  const require = createRequire(path.join(runtime, 'package.json'));
  const { chromium } = require('patchright');
  const artifacts = path.join(path.dirname(profile), 'artifacts');
  const pages = new Map();
  const taskPages = new Set();
  const downloads = new Map();
  const downloadWaiters = new Set();
  let context, selected, nextPage = 0, nextSnapshot = 0, closing = false;
  let backgroundMode = headless, switchingMode = false, network;
  const plugins = createWebPlugins({
    directory: path.join(path.dirname(profile), 'plugins'),
    getContext: async () => { if (!context) await open({ background: true }); return context; },
  });
  function observeNetwork(response) {
    if (!network || Date.now() > network.expires_at) { network = undefined; return; }
    const url = new URL(response.url());
    if (url.origin !== network.origin || !['xhr', 'fetch', 'document'].includes(response.request().resourceType())) return;
    // Sin headers, cookies, cuerpos ni valores de query. Solo descubrimiento temporal.
    network.entries.push({ method: response.request().method(), origin: url.origin, path: url.pathname.slice(0, 2000), query_keys: [...url.searchParams.keys()].slice(0, 20), status: response.status(), content_type: (response.headers()['content-type'] || '').slice(0, 200) });
    if (network.entries.length > 100) { network.entries.shift(); network.truncated = true; }
  }

  function remember(page) {
    for (const state of pages.values()) if (state.page === page) return state;
    const state = { id: String(++nextPage), page, title: '', refs: new Map(), frames: new Map(), fullSnapshot: '', refsValid: false, dialog: null, chooser: null, pendingAction: null, onModal: null };
    pages.set(state.id, state);
    page.on('framenavigated', () => { state.refsValid = false; });
    page.on('close', () => { pages.delete(state.id); taskPages.delete(state.id); if (selected === state) selected = undefined; });
    page.on('dialog', dialog => { state.dialog = dialog; state.onModal?.({ kind: 'modal' }); });
    page.on('filechooser', chooser => { state.chooser = chooser; state.onModal?.({ kind: 'modal' }); });
    page.on('download', download => {
      const record = { id: randomUUID(), page_id: state.id, suggested_filename: path.basename(download.suggestedFilename()).slice(0, 200), status: 'pending', path: null, bytes: null, download, task: null };
      downloads.set(record.id, record);
      for (const notify of downloadWaiters) notify(record);
      // Retener y observar cada tarea de guardado hasta el cierre de la sesión.
      record.task = (async () => {
        let temporary;
        try {
          await prepareArtifacts();
          const filename = `${record.id}-${record.suggested_filename.replace(/[^\p{L}\p{N}._-]/gu, '_') || 'download'}`;
          const destination = path.join(artifacts, filename);
          temporary = destination + '.partial';
          await download.saveAs(temporary);
          await chmod(temporary, 0o600);
          await rename(temporary, destination);
          record.path = destination;
          record.bytes = (await stat(destination)).size;
          record.status = 'completed';
        } catch {
          record.status = record.cancelled ? 'cancelled' : 'failed';
          if (temporary) await unlink(temporary).catch(() => {});
        }
      })();
    });
    return state;
  }
  function stateFor(request) {
    const state = request.page_id ? pages.get(request.page_id) : selected || pages.values().next().value;
    if (!state || state.page.isClosed()) fail('closed');
    if (!request.background_scope) selected = state;
    return state;
  }
  function modal(state) {
    if (state.dialog) return { type: state.dialog.type(), message: state.dialog.message().slice(0, 4000), default_value: state.dialog.defaultValue().slice(0, 1000), resolve_with: 'browser_dialog' };
    if (state.chooser) return { type: 'file_chooser', multiple: state.chooser.isMultiple(), resolve_with: 'browser_upload' };
    return null;
  }
  function ready(state) {
    if (modal(state)) fail('modal_pending');
    if (state.pendingAction) fail('action_pending');
  }
  async function clearRefs(state) {
    state.refsValid = false;
    const refs = [...state.refs.values()];
    state.refs.clear();
    await Promise.all(refs.map(ref => ref.handle.dispose().catch(() => {})));
  }
  async function target(state, request, fallback) {
    if (request.target && request.selector) fail('invalid_arguments');
    if (request.target) {
      const ref = state.refs.get(request.target);
      if (!state.refsValid || !ref) fail('stale_reference');
      const info = await ref.handle.evaluate(elementInfo).catch(() => null);
      if (!info?.connected || JSON.stringify(info) !== ref.fingerprint) fail('stale_reference');
      return ref.handle;
    }
    const selector = request.selector || fallback;
    if (!selector) fail('missing_target');
    const frame = request.frame_id ? state.frames.get(request.frame_id) : state.page.mainFrame();
    if (!frame || frame.isDetached()) fail('stale_reference');
    return frame.locator(selector);
  }
  async function action(state, run) {
    ready(state);
    state.refsValid = false;
    let notify;
    const signal = new Promise(resolve => { notify = resolve; });
    state.onModal = notify;
    const task = Promise.resolve().then(run).then(value => ({ kind: 'done', value }), error => ({ kind: 'error', error }));
    state.pendingAction = task;
    const outcome = await Promise.race([task, signal]);
    // Recoger también avisos de selector de archivos que llegan justo después de click.
    if (outcome.kind !== 'modal') await bounded(signal, 100);
    state.onModal = null;
    if (outcome.kind === 'modal') return { ok: null, page_id: state.id, action_pending: true, modal: modal(state), needs_snapshot: true };
    state.pendingAction = null;
    await clearRefs(state);
    if (outcome.kind === 'error') throw outcome.error;
    return { ok: true, page_id: state.id, ...(outcome.value || {}), ...(modal(state) ? { modal: modal(state) } : {}), needs_snapshot: true };
  }
  async function settleAction(state) {
    if (!state.pendingAction) return null;
    const outcome = await bounded(state.pendingAction, 7000);
    if (!outcome) return { completed: false, uncertain: true };
    state.pendingAction = null;
    await clearRefs(state);
    return { completed: outcome.kind === 'done', uncertain: outcome.kind !== 'done' };
  }
  async function prepareArtifacts() {
    await mkdir(artifacts, { recursive: true, mode: 0o700 });
    await chmod(artifacts, 0o700);
  }
  async function artifact(prefix, extension, bytes) {
    await prepareArtifacts();
    const filename = path.join(artifacts, `${prefix}-${randomUUID()}.${extension}`);
    await writeFile(filename, bytes, { flag: 'wx', mode: 0o600 });
    return { path: filename, bytes: bytes.length };
  }
  async function open(options = {}) {
    if (context && backgroundMode && !options.background && !headless) {
      if ([...downloads.values()].some(item => item.status === 'pending') || [...pages.values()].some(state => state.pendingAction || modal(state))) fail('action_pending');
      switchingMode = true;
      try { await context.close(); } finally { switchingMode = false; }
    }
    const started = Date.now();
    if (!context) {
      await mkdir(profile, { recursive: true, mode: 0o700 });
      await chmod(profile, 0o700);
      backgroundMode = headless || options.background === true;
      const launched = await chromium.launchPersistentContext(profile, {
        headless: backgroundMode, viewport: backgroundMode ? { width: 1280, height: 800 } : null, timeout: 9000,
        executablePath: path.join(runtime, 'Gennie Web.app/Contents/MacOS/Google Chrome for Testing'),
        chromiumSandbox: true, acceptDownloads: true,
        ignoreDefaultArgs: ['--unsafely-disable-devtools-self-xss-warnings'],
        args: backgroundMode ? ['--restore-last-session'] : ['--restore-last-session', '--window-size=1280,800'],
      });
      context = launched;
      context.setDefaultTimeout(6000);
      context.setDefaultNavigationTimeout(8000);
      context.on('page', remember);
      context.on('response', observeNetwork);
      context.on('close', () => {
        if (context === launched) { context = undefined; selected = undefined; pages.clear(); taskPages.clear(); if (!switchingMode) onBrowserClose(); }
      });
      context.pages().forEach(remember);
      const page = context.pages().find(p => [homepage, 'https://www.gennie.io/'].includes(p.url()))
        || context.pages().find(p => p.url() === 'about:blank') || await context.newPage();
      selected = remember(page);
      try { if (!options.background) await page.goto(httpUrl(homepage), { waitUntil: 'domcontentloaded', timeout: Math.max(1, Math.min(8000, 11000 - (Date.now() - started))) }); }
      catch { fail('homepage'); }
    }
    selected = selected && !selected.page.isClosed() ? selected : pages.values().next().value || remember(await context.newPage());
    if (!options.background) await selected.page.bringToFront();
    return { open: true, page_id: selected.id, background: backgroundMode };
  }
  async function tabList() {
    const result = [];
    for (const state of pages.values()) result.push({ page_id: state.id, url: state.page.url(), title: state.title, selected: state === selected });
    return result;
  }
  async function snapshot(request) {
    if (!context || !pages.size) return { open: false, tabs: [] };
    const state = stateFor(request);
    const tabs = await tabList();
    if (modal(state)) return { open: true, page_id: state.id, tabs, modal: modal(state) };
    ready(state);
    state.title = await state.page.title();
    await clearRefs(state);
    state.frames.clear();
    const generation = ++nextSnapshot;
    const started = Date.now();
    const targets = [], frames = [], full = [];
    for (const [index, frame] of state.page.frames().slice(0, 16).entries()) {
      if (Date.now() - started > 7000) break;
      const frameId = `f${index}`;
      state.frames.set(frameId, frame);
      let tree = '';
      try {
        tree = await frame.locator('body').ariaSnapshot({ timeout: 1000 });
        const candidates = frame.locator(`:is(${INTERACTIVE}):visible`);
        const count = Math.min(await candidates.count(), 200 - targets.length);
        for (let index = 0; index < count && Date.now() - started < 8500; index++) {
          const handle = await candidates.nth(index).elementHandle({ timeout: 500 });
          if (!handle) continue;
          const info = await handle.evaluate(elementInfo).catch(() => null);
          if (!info?.visible) { await handle.dispose(); continue; }
          const ref = `p${state.id}s${generation}e${targets.length + 1}`;
          state.refs.set(ref, { handle, fingerprint: JSON.stringify(info) });
          targets.push({ ref, frame_id: frameId, ...info });
        }
      } catch { tree = '[Contenido no disponible en este frame]'; }
      full.push(`[${frameId}]\n${tree}`);
      frames.push({ frame_id: frameId, url: frame.url(), snapshot: tree.slice(0, index ? 4000 : 18000) });
    }
    state.fullSnapshot = full.join('\n').slice(0, 256000);
    state.refsValid = true;
    return { open: true, page_id: state.id, tabs, snapshot_id: generation, snapshot: frames[0]?.snapshot || '', frames, targets, truncated: state.fullSnapshot.length > 18000 || targets.length >= 200 };
  }
  function downloadInfo(record) {
    return { download_id: record.id, page_id: record.page_id, suggested_filename: record.suggested_filename, status: record.status, path: record.path, bytes: record.bytes };
  }
  async function execute(request) {
    if (!request || typeof request !== 'object') fail('invalid_arguments');
    if (request.method === 'open') return open(request);
    if (request.method === 'background') {
      if (context && !backgroundMode) {
        if (taskPages.size) fail('task_active');
        if ([...downloads.values()].some(item => item.status === 'pending') || [...pages.values()].some(state => state.pendingAction || modal(state))) fail('action_pending');
        switchingMode = true;
        try { await context.close(); } finally { switchingMode = false; }
      }
      return { status: 'background_ready' };
    }
    if (request.method === 'task_page') {
      if (request.action === 'create') {
        if (!context) await open({ background: true });
        const state = remember(await context.newPage());
        taskPages.add(state.id);
        return { page_id: state.id };
      }
      const state = pages.get(request.page_id);
      if (state) { await clearRefs(state); await state.page.close(); }
      taskPages.delete(request.page_id);
      return { status: 'closed' };
    }
    if (request.method === 'candidate') {
      if (!/^job_[0-9]+$/.test(request.candidate_id || '')) fail('invalid_arguments');
      const base = (await plugins.dispatch({ action: 'get', plugin_id: request.plugin_id || request.manifest?.id })).plugin;
      const candidate = createWebPlugins({ directory: path.join(path.dirname(profile), 'plugins', 'candidates', request.candidate_id), verificationBase: base, getContext: async () => { if (!context) await open({ background: true }); return context; } });
      return candidate.dispatch(request);
    }
    if (request.method === 'plugins' || request.method === 'plugin_run') return plugins.dispatch(request);
    if (request.method === 'network') {
      if (request.action === 'start') {
        const url = new URL(httpUrl(request.origin));
        if (url.origin !== request.origin) fail('invalid_arguments');
        network = { origin: url.origin, expires_at: Date.now() + 120000, entries: [], truncated: false };
        return { status: 'observing', origin: url.origin, expires_in_seconds: 120 };
      }
      const result = network && Date.now() <= network.expires_at ? { status: 'observing', origin: network.origin, entries: network.entries.slice(), truncated: network.truncated } : { status: 'inactive', entries: [] };
      if (request.action === 'stop' || result.status === 'inactive') network = undefined;
      if (!['read', 'stop'].includes(request.action)) fail('invalid_arguments');
      return result;
    }
    if (request.method === 'snapshot') return snapshot(request);
    if (request.method === 'tabs') {
      const operation = request.action || 'list';
      if (operation === 'list') return { open: !!context, tabs: await tabList() };
      if (!context) fail('closed');
      if (operation === 'create') {
        const url = request.url ? httpUrl(request.url) : null;
        const state = remember(await context.newPage()); selected = state;
        if (url) await state.page.goto(url, { waitUntil: 'domcontentloaded' });
        await state.page.bringToFront();
        return { page_id: state.id, tabs: await tabList() };
      }
      const state = stateFor(request);
      if (operation === 'select') await state.page.bringToFront();
      else if (operation === 'close') { await clearRefs(state); await state.page.close(); }
      else fail('invalid_arguments');
      return { open: !!context, tabs: await tabList(), page_id: selected?.id || null };
    }
    if (request.method === 'download') {
      const deadline = Date.now() + Math.min(request.timeout_ms ?? 6000, 8000);
      if ((request.action || 'list') === 'list') return { downloads: [...downloads.values()].map(downloadInfo) };
      let record = downloads.get(request.download_id);
      if (request.action === 'wait' && !request.download_id) {
        record = [...downloads.values()].at(-1);
        if (!record) {
          let notify;
          const next = new Promise(resolve => { notify = resolve; downloadWaiters.add(resolve); });
          try { record = await bounded(next, Math.max(1, deadline - Date.now())); }
          finally { downloadWaiters.delete(notify); }
          if (!record) return { status: 'pending', download_id: null };
        }
      }
      if (!record) fail('missing_download');
      if (request.action === 'cancel') { record.cancelled = true; await record.download.cancel(); await record.task; }
      else if (request.action === 'wait') await bounded(record.task, Math.max(1, deadline - Date.now()));
      else if (request.action !== 'status') fail('invalid_arguments');
      return downloadInfo(record);
    }
    const state = stateFor(request), page = state.page;
    if (request.method === 'dialog') {
      if ((request.action || 'status') === 'status') return { page_id: state.id, modal: modal(state) };
      if (!state.dialog) fail('missing_dialog');
      if (request.action === 'accept') await state.dialog.accept(request.text);
      else if (request.action === 'dismiss') await state.dialog.dismiss();
      else fail('invalid_arguments');
      state.dialog = null;
      return { ok: true, page_id: state.id, previous_action: await settleAction(state), needs_snapshot: true };
    }
    if (request.method === 'upload') {
      const files = [];
      for (const filename of request.files || []) {
        if (!path.isAbsolute(filename)) fail('invalid_file');
        const resolved = await realpath(filename).catch(() => null);
        if (!resolved) fail('invalid_file');
        const info = await stat(resolved);
        if (!info.isFile() || info.size > 25 * 1024 * 1024) fail('invalid_file');
        files.push(resolved);
      }
      if (files.length > 8) fail('invalid_file');
      if (state.dialog) fail('modal_pending');
      if (state.chooser) {
        await state.chooser.setFiles(files); state.chooser = null;
        return { ok: true, page_id: state.id, files: files.length, previous_action: await settleAction(state), needs_snapshot: true };
      }
      const element = await target(state, request);
      return action(state, async () => { await element.setInputFiles(files); return { files: files.length }; });
    }
    ready(state);
    switch (request.method) {
      case 'navigate': return action(state, async () => {
        const response = await page.goto(httpUrl(request.url), { waitUntil: 'domcontentloaded' });
        return { url: page.url(), status: response?.status() ?? null };
      });
      case 'history': return action(state, async () => {
        const options = { waitUntil: 'domcontentloaded' };
        let response;
        if (request.action === 'back') response = await page.goBack(options);
        else if (request.action === 'forward') response = await page.goForward(options);
        else if (request.action === 'reload') response = await page.reload(options);
        else fail('invalid_arguments');
        return { url: page.url(), status: response?.status() ?? null };
      });
      case 'find': {
        const current = await snapshot(request);
        const needle = request.text.toLocaleLowerCase();
        const lines = state.fullSnapshot.split('\n').filter(line => line.toLocaleLowerCase().includes(needle));
        const targets = current.targets.filter(item => item.label.toLocaleLowerCase().includes(needle)).slice(0, request.limit ?? 20);
        for (const [frameId, frame] of state.frames) {
          const candidates = frame.getByText(request.text);
          const count = Math.min(await candidates.count(), (request.limit ?? 20) - targets.length);
          for (let index = 0; index < count; index++) {
            const handle = await candidates.nth(index).elementHandle({ timeout: 500 });
            if (!handle) continue;
            const info = await handle.evaluate(elementInfo).catch(() => null);
            if (!info?.visible) { await handle.dispose(); continue; }
            const ref = `p${state.id}s${current.snapshot_id}e${state.refs.size + 1}`;
            state.refs.set(ref, { handle, fingerprint: JSON.stringify(info) });
            targets.push({ ref, frame_id: frameId, ...info });
          }
        }
        return { page_id: state.id, snapshot_id: current.snapshot_id, matches: lines.slice(0, request.limit ?? 20).map(line => line.slice(0, 1500)), targets, truncated: lines.length > (request.limit ?? 20) };
      }
      case 'read': {
        const element = await target(state, request, 'body');
        const data = await element.evaluate((node, options) => {
          const trim = value => (value || '').trim().slice(0, options.max_chars);
          if (options.mode === 'links') {
            const links = node.matches('a[href]') ? [node] : [...node.querySelectorAll('a[href]')];
            return links.slice(0, options.limit).map(link => ({ text: trim(link.innerText || link.getAttribute('aria-label')), url: link.href }));
          }
          if (options.mode === 'tables') {
            const tables = node.matches('table') ? [node] : [...node.querySelectorAll('table')];
            return tables.slice(0, 10).map(table => [...table.rows].slice(0, options.limit).map(row => [...row.cells].slice(0, 30).map(cell => trim(cell.innerText).slice(0, 1000))));
          }
          const source = options.mode === 'article' ? node.querySelector('article,main,[role=main]') || node : node;
          return trim(source.innerText || source.textContent);
        }, { mode: request.mode || 'text', limit: request.limit ?? 100, max_chars: request.max_chars ?? TEXT_LIMIT });
        const encoded = JSON.stringify(data);
        if (Buffer.byteLength(encoded) > 100000) fail('content_limit');
        return { page_id: state.id, url: page.url(), mode: request.mode || 'text', data };
      }
      case 'wait_for': {
        const timeout = Math.min(request.timeout_ms ?? 6000, 8000);
        if (request.kind === 'url') await page.waitForURL(request.url, { timeout });
        else if (request.kind === 'load') await page.waitForLoadState(request.state || 'domcontentloaded', { timeout });
        else if (request.kind === 'text') await page.getByText(request.text).first().waitFor({ state: request.state || 'visible', timeout });
        else if (request.kind === 'element') {
          const element = await target(state, request);
          if (request.target) await element.waitForElementState(request.state || 'visible', { timeout });
          else await element.waitFor({ state: request.state || 'visible', timeout });
        } else fail('invalid_arguments');
        return { ok: true, page_id: state.id, needs_snapshot: true };
      }
      case 'click': case 'fill': case 'press': case 'hover': case 'select_option': case 'check': {
        const element = await target(state, request);
        return action(state, async () => {
          if (request.method === 'click') await element.click();
          if (request.method === 'fill') await element.fill(request.text);
          if (request.method === 'press') await element.press(request.key);
          if (request.method === 'hover') await element.hover();
          if (request.method === 'check') await element.setChecked(request.checked);
          if (request.method === 'select_option') await element.selectOption(request.by === 'label' ? request.values.map(label => ({ label })) : request.values);
        });
      }
      case 'scroll': {
        const amount = request.amount ?? 600;
        const deltas = { down: [0, amount], up: [0, -amount], right: [amount, 0], left: [-amount, 0] };
        const delta = deltas[request.direction || 'down'];
        if (!delta) fail('invalid_arguments');
        const element = request.target || request.selector ? await target(state, request) : null;
        return action(state, async () => { if (element) await element.hover(); await page.mouse.wheel(...delta); });
      }
      case 'screenshot': {
        if (request.full_page && (request.target || request.selector)) fail('invalid_arguments');
        if (request.full_page && await page.evaluate(() => document.documentElement.scrollHeight) > 50000) fail('image_limit');
        const options = { type: 'jpeg', quality: 65, scale: 'css', timeout: 8000 };
        const image = request.target || request.selector ? await (await target(state, request)).screenshot(options) : await page.screenshot({ ...options, fullPage: request.full_page ?? false });
        if (image.length > 2 * 1024 * 1024) fail('image_limit');
        return { page_id: state.id, ...(request.background_scope ? {} : { artifact: await artifact('screenshot', 'jpg', image) }), image: { data: image.toString('base64'), mime_type: 'image/jpeg' } };
      }
      case 'pdf': {
        const bytes = await page.pdf({ format: request.format || 'A4', printBackground: true, preferCSSPageSize: true, ...(request.page_ranges ? { pageRanges: request.page_ranges } : {}) });
        if (bytes.length > 32 * 1024 * 1024) fail('content_limit');
        return { page_id: state.id, artifact: await artifact('page', 'pdf', bytes) };
      }
      case 'request': {
        if (!['GET', 'HEAD'].includes(request.http_method || 'GET')) fail('invalid_arguments');
        const response = await context.request.fetch(httpUrl(request.url), { method: request.http_method || 'GET', timeout: 8000, maxRedirects: 5, failOnStatusCode: false });
        try {
          const contentType = response.headers()['content-type'] || '';
          const textual = /json|text|xml|javascript/.test(contentType);
          const bytes = textual && request.http_method !== 'HEAD' ? await response.body() : null;
          if (bytes && bytes.length > 8 * 1024 * 1024) fail('content_limit');
          return { page_id: state.id, url: response.url(), status: response.status(), content_type: contentType, body: bytes?.toString('utf8').slice(0, TEXT_LIMIT) ?? null, truncated: !!bytes && bytes.toString('utf8').length > TEXT_LIMIT };
        } finally { await response.dispose(); }
      }
      default: fail('invalid_arguments');
    }
  }
  async function close() {
    if (closing) return;
    closing = true;
    const states = [...pages.values()];
    for (const record of downloads.values()) if (record.status === 'pending') await record.download.cancel().catch(() => {});
    await context?.close();
    await Promise.all([...downloads.values()].map(record => record.task));
    await Promise.all(states.map(state => state.pendingAction).filter(Boolean));
  }
  return {
    async dispatch(request) {
      try { return await execute(request); }
      catch (error) { return { error: errorMessage(error), code: error.code || (error.name === 'TimeoutError' ? 'timeout' : 'playwright_error') }; }
    },
    close,
  };
}
function errorMessage(error) {
  const messages = {
    homepage: 'Gennie Web está abierto, pero no se pudo cargar su página de inicio. Puedes recargarla.',
    closed: 'El navegador o la pestaña están cerrados. Usa browser_open o browser_tabs.',
    invalid_url: 'Usa una dirección HTTP/HTTPS válida, sin credenciales embebidas.',
    stale_reference: 'La referencia ya no es válida. Obtén un browser_snapshot nuevo antes de actuar.',
    missing_target: 'Indica target del último snapshot o un selector Playwright.',
    modal_pending: 'Hay un diálogo o selector de archivos pendiente. Consulta browser_dialog y resuélvelo con browser_dialog o browser_upload.',
    action_pending: 'La acción anterior sigue pendiente; comprueba el diálogo antes de repetirla.',
    task_active: 'Otra construcción está usando el navegador. Espera a que termine antes de reanudar este trabajo.',
    missing_dialog: 'No hay un diálogo JavaScript pendiente en esa pestaña.',
    missing_download: 'No se encuentra esa descarga. Consulta browser_download con action=list.',
    invalid_file: 'Selecciona hasta ocho archivos regulares con rutas absolutas existentes, de hasta 25 MiB cada uno.',
    image_limit: 'La captura excede el límite. Captura un elemento o una región visible más pequeña.',
    content_limit: 'El contenido excede el límite de esta operación. Acota la extracción o exportación.',
    invalid_plugin: 'La receta web no cumple el formato o los límites permitidos.',
    invalid_plugin_arguments: 'Los argumentos no corresponden a los parámetros de la función web.',
    plugin_storage_error: 'No se pudo leer o guardar el catálogo de plugins web de forma segura.',
    invalid_arguments: 'Los argumentos no corresponden a esta operación de navegador.',
  };
  if (messages[error.code]) return messages[error.code];
  if (error.name === 'TimeoutError') return 'La operación excedió el plazo; observa el estado antes de repetirla, pues el efecto podría haberse aplicado.';
  return 'El navegador no pudo completar la operación. Comprueba pestaña, elemento, permisos e instalación.';
}
async function runWorker() {
  const [runtime, profile, ...flags] = process.argv.slice(1);
  const session = createBrowserSession({ runtime, profile, onBrowserClose: () => { if (flags.includes('--standalone')) stop(); } });
  let stopping = false;
  async function stop() {
    if (stopping) return;
    stopping = true;
    setTimeout(() => process.exit(1), 3500).unref();
    try { await session.close(); } finally { process.exit(0); }
  }
  const input = createInterface({ input: process.stdin, crlfDelay: Infinity });
  process.stdin.on('end', stop);
  process.on('SIGTERM', stop);
  process.on('SIGINT', stop);
  try {
    for await (const line of input) {
      if (Buffer.byteLength(line) > 128 * 1024) break;
      let response;
      try { response = await session.dispatch(JSON.parse(line)); }
      catch { response = { error: 'Petición JSON inválida' }; }
      const encoded = JSON.stringify(response);
      process.stdout.write(Buffer.byteLength(encoded) < RESPONSE_LIMIT ? encoded + '\n' : '{"error":"La respuesta supera el límite del navegador"}\n');
    }
  } finally { await stop(); }
}
if (process.argv.includes('--gennie-browser-worker')) await runWorker();
