// Transición visible/headless con servidor y perfil de prueba; no utiliza cuentas.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createBrowserSession } from '../../apps/browser/src/worker.mjs';

const root = path.join(process.env.HOME, 'Library/Application Support/Gennie/browser');
const directory = await mkdtemp(path.join(tmpdir(), 'gennie-background-'));
process.env.PLAYWRIGHT_BROWSERS_PATH = path.join(root, 'browsers');
const server = createServer((request, response) => {
  response.writeHead(200, { 'Content-Type': 'text/html', 'Set-Cookie': 'fixture_session=valid; Path=/; Max-Age=3600' });
  response.end(`<title>Prueba local</title><p>${request.headers.cookie?.includes('fixture_session=valid') ? 'sesión conservada' : 'sin sesión'}</p>`);
});
server.listen(0, '127.0.0.1');
await once(server, 'listening');
const origin = `http://127.0.0.1:${server.address().port}`;
let closed = 0;
const session = createBrowserSession({ runtime: path.join(root, 'runtime-patchright-1.64.2'), profile: path.join(directory, 'profile'), homepage: origin, onBrowserClose: () => closed++ });
async function call(method, args = {}) {
  const result = await session.dispatch({ method, ...args });
  assert.ok(!result.error, `${method}: ${result.error}`);
  return result;
}
try {
  assert.equal((await call('open')).background, false);
  const task = await call('task_page', { action: 'create' });
  assert.equal((await session.dispatch({ method: 'background' })).code, 'task_active');
  await call('task_page', { action: 'close', page_id: task.page_id });
  assert.equal((await call('background')).status, 'background_ready');
  assert.equal(closed, 0);
  const backgroundTask = await call('task_page', { action: 'create' });
  assert.equal((await call('open', { background: true })).background, true);
  const response = await call('request', { page_id: backgroundTask.page_id, url: origin });
  assert.equal(response.status, 200);
  assert.ok(response.body.includes('sesión conservada'));
  await call('task_page', { action: 'close', page_id: backgroundTask.page_id });
  await call('open');
  const page = await call('request', { url: origin });
  assert.equal(page.status, 200);
  console.log(JSON.stringify({ visible_to_headless: true, cookie_retained: true, active_task_rejected: true, headless_to_visible: true, close_callback_suppressed: true }));
} finally {
  await session.close();
  server.closeAllConnections();
  await new Promise(resolve => server.close(resolve));
  await rm(directory, { recursive: true, force: true });
}
