// Proceso privado de Playwright. Solo JSON por stdio; no hay servidor ni puerto CDP.
import { createRequire } from 'node:module';
import { mkdir, chmod } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import path from 'node:path';

const [runtime, profile, standalone] = process.argv.slice(1);
const require = createRequire(path.join(runtime, 'package.json'));
const { chromium } = require('playwright');
let context;
let selected;
let closing = false;
let nextId = 0;
const pages = new Map();
function remember(page) {
  if ([...pages.values()].includes(page)) return;
  const id = String(++nextId);
  pages.set(id, page);
  page.on('close', () => { pages.delete(id); if (selected === page) selected = undefined; });
}
async function open() {
  const started = Date.now();
  if (!context) {
    await mkdir(profile, { recursive: true, mode: 0o700 });
    await chmod(profile, 0o700);
    const launched = await chromium.launchPersistentContext(profile, {
      headless: false, viewport: null, timeout: 9000,
      executablePath: path.join(runtime, 'Gennie Web.app/Contents/MacOS/Google Chrome for Testing'),
      chromiumSandbox: true,
      ignoreDefaultArgs: ['--unsafely-disable-devtools-self-xss-warnings'],
      args: ['--restore-last-session'],
    });
    context = launched;
    context.setDefaultTimeout(6000);
    context.setDefaultNavigationTimeout(8000);
    context.on('page', remember);
    context.on('close', () => {
      if (context === launched) {
        context = undefined; selected = undefined; pages.clear();
        if (standalone === '--standalone') process.exit(0);
      }
    });
    context.pages().forEach(remember);
    // Abrir inicio una sola vez por sesión, conservando las pestañas restauradas.
    selected = context.pages().find(page => ['https://gennie.io/', 'https://www.gennie.io/'].includes(page.url()))
      || context.pages().find(page => page.url() === 'about:blank')
      || await context.newPage();
    try {
      await selected.goto('https://gennie.io/', {
        waitUntil: 'domcontentloaded', timeout: Math.max(1, Math.min(8000, 11000 - (Date.now() - started))),
      });
    } catch {
      throw new Error('homepage');
    }
  }
  selected = selected && !selected.isClosed() ? selected : context.pages()[0] || await context.newPage();
  await selected.bringToFront();
  return { open: true, page_id: idOf(selected) };
}
function idOf(page) { return [...pages].find(([, value]) => value === page)?.[0]; }
function pageFor(request) {
  const page = request.page_id ? pages.get(request.page_id) : selected || [...pages.values()][0];
  if (!page || page.isClosed()) throw new Error('closed');
  selected = page;
  return page;
}
async function snapshot(request) {
  if (!context) return { open: false, tabs: [] };
  const page = pageFor(request);
  const tabs = [];
  for (const [id, tab] of pages) tabs.push({ page_id: id, url: tab.url(), title: await tab.title().catch(() => '') });
  return { open: true, page_id: idOf(page), tabs, snapshot: (await page.locator('body').ariaSnapshot()).slice(0, 18000) };
}
async function dispatch(request) {
  switch (request.method) {
    case 'open': return open();
    case 'snapshot': return snapshot(request);
    case 'navigate': {
      const url = new URL(request.url);
      if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) throw new Error('url');
      // No abrir implícitamente tras cierre: primero browser_open por acción del usuario/agente.
      const page = pageFor(request);
      const response = await page.goto(url.href, { waitUntil: 'domcontentloaded' });
      return { ok: true, page_id: idOf(page), url: page.url(), status: response?.status() ?? null };
    }
    case 'click': await pageFor(request).locator(request.selector).click(); return { ok: true };
    case 'fill': await pageFor(request).locator(request.selector).fill(request.text); return { ok: true };
    case 'press': await pageFor(request).locator(request.selector).press(request.key); return { ok: true };
    default: throw new Error('method');
  }
}
function message(error) {
  if (error?.message === 'homepage') return 'Gennie Web está abierto, pero no se pudo cargar gennie.io. Puedes recargar la página desde el navegador.';
  if (error?.message === 'closed') return 'El navegador o la pestaña están cerrados. Abre el navegador de Gennie.';
  if (error?.message === 'url' || error instanceof TypeError) return 'La dirección o los argumentos no son válidos; usa HTTP o HTTPS.';
  if (error?.name === 'TimeoutError') return 'La operación excedió el plazo; comprueba el navegador antes de repetirla, pues el efecto podría haberse aplicado.';
  // No devolver stderr del navegador: puede incluir URLs, cookies o contenido sensible.
  return 'Playwright no pudo completar la operación. Comprueba la ventana, el perfil ocupado o la instalación de Chromium.';
}
async function stop() {
  if (closing) return;
  closing = true;
  // Salir normalmente ejecuta también la limpieza de hijos registrada por Playwright.
  // Evita llegar a SIGKILL si context.close() se atasca.
  setTimeout(() => process.exit(1), 3500).unref();
  try { await context?.close(); } finally { process.exit(0); }
}
const input = createInterface({ input: process.stdin, crlfDelay: Infinity });
process.stdin.on('end', stop);
process.on('SIGTERM', stop);
process.on('SIGINT', stop);
try {
  for await (const line of input) {
    if (Buffer.byteLength(line) > 128 * 1024) break;
    let response;
    try { response = await dispatch(JSON.parse(line)); }
    catch (error) { response = { error: message(error) }; }
    const encoded = JSON.stringify(response);
    process.stdout.write(Buffer.byteLength(encoded) < 128 * 1024 ? encoded + '\n' : '{"error":"La respuesta supera el límite del navegador"}\n');
  }
} finally { await stop(); }
