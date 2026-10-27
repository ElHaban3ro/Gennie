// Sonda real acotada con perfil temporal. No usa cuentas ni datos del perfil de Gennie.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';
import { createInterface } from 'node:readline';
const root = path.join(process.env.HOME, 'Library/Application Support/Gennie/browser');
const runtime = path.join(root, 'runtime-patchright-1.64.2');
process.env.PLAYWRIGHT_BROWSERS_PATH = path.join(root, 'browsers');
const { chromium } = createRequire(path.join(runtime, 'package.json'))('patchright');
const profile = await mkdtemp(path.join(tmpdir(), 'gennie-browser-probe-'));
const source = await readFile(new URL('../../apps/browser/src/worker.mjs', import.meta.url), 'utf8');
const origin = 'https://gennie-probe.invalid';
let context, child;
async function launch() {
  context = await chromium.launchPersistentContext(profile, {
    headless: true, executablePath: path.join(runtime, 'Gennie Web.app/Contents/MacOS/Google Chrome for Testing'), args: ['--restore-last-session'], timeout: 9000,
    chromiumSandbox: true,
    ignoreDefaultArgs: ['--unsafely-disable-devtools-self-xss-warnings'],
  });
  await context.route(`${origin}/**`, route => route.fulfill({ contentType: 'text/html', body: '<title>Prueba Gennie</title><p>Perfil temporal</p>' }));
  const page = context.pages()[0] || await context.newPage();
  await page.goto(origin);
  return page;
}
async function stopChild() {
  if (!child || child.exitCode !== null) return;
  const exited = once(child, 'exit');
  child.stdin.end();
  const watchdog = setTimeout(() => child.kill('SIGTERM'), 6000);
  try { const [code] = await exited; assert.equal(code, 0); }
  finally { clearTimeout(watchdog); }
}
try {
  const page = await launch();
  await context.addCookies([
    { name: 'persistent_probe', value: 'retained', url: origin, expires: Math.floor(Date.now() / 1000) + 3600 },
    { name: 'session_probe', value: 'retained', url: origin },
  ]);
  await page.evaluate(() => localStorage.setItem('gennie_probe', 'retained'));
  await context.close(); context = undefined;

  child = spawn(path.join(runtime, 'node'), ['--input-type=module', '--eval', source, '--', runtime, profile, '--gennie-browser-worker'], {
    env: process.env, stdio: ['pipe', 'pipe', 'pipe'],
  });
  const lines = createInterface({ input: child.stdout });
  const replies = lines[Symbol.asyncIterator]();
  async function call(request) {
    child.stdin.write(JSON.stringify(request) + '\n');
    const response = await Promise.race([
      replies.next(), new Promise((_, reject) => {
        const timeout = setTimeout(() => reject(new Error('Plazo de sonda excedido')), 12000); timeout.unref();
      }),
    ]);
    assert.equal(response.done, false);
    return JSON.parse(response.value);
  }
  const first = await call({ method: 'open' });
  assert.equal(first.open, true);
  const initial = await call({ method: 'snapshot' });
  assert.ok(initial.tabs.some(tab => tab.page_id === first.page_id
    && ['https://gennie.io/', 'https://www.gennie.io/'].includes(tab.url)));
  const second = await call({ method: 'open' });
  assert.equal(second.page_id, first.page_id);
  const navigation = await call({ method: 'navigate', page_id: first.page_id, url: 'https://gennie.io/' });
  assert.equal(navigation.status, 200);
  const invalid = await call({ method: 'navigate', url: 'file:///etc/passwd' });
  assert.ok(invalid.error);
  await stopChild();

  const restored = await launch();
  const cookies = await context.cookies(origin);
  assert.equal(cookies.find(c => c.name === 'persistent_probe')?.value, 'retained');
  assert.equal(await restored.evaluate(() => localStorage.getItem('gennie_probe')), 'retained');
  console.log(JSON.stringify({ persistent_cookie: true, local_storage: true,
    session_cookie_restored: cookies.some(c => c.name === 'session_probe'),
    homepage: 'https://gennie.io/', navigation_http_status: navigation.status, repeated_open_same_page: true, invalid_url_rejected: true, helper_graceful_exit: true }));
} finally {
  await stopChild();
  await context?.close();
  await rm(profile, { recursive: true, force: true });
}
