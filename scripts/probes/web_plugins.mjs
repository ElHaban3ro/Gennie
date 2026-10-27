// Sonda real y acotada: Chromium headless, cuenta ficticia y perfil temporal.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { mkdtemp, rm, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createBrowserSession } from '../../apps/browser/src/worker.mjs';
const browserRoot = path.join(process.env.HOME, 'Library/Application Support/Gennie/browser');
process.env.PLAYWRIGHT_BROWSERS_PATH = path.join(browserRoot, 'browsers');
const directory = await mkdtemp(path.join(tmpdir(), 'gennie-web-plugins-'));
let protectedCalls = 0, publicCalls = 0, writes = 0, authenticated = true;
const server = createServer((request, response) => {
  const url = new URL(request.url, 'http://fixture');
  const json = (status, body) => { response.writeHead(status, { 'Content-Type': 'application/json' }); response.end(JSON.stringify(body)); };
  if (url.pathname === '/api/action' && request.method === 'POST') { writes++; json(200, { writes }); }
  else if (url.pathname === '/api/public') { publicCalls++; json(200, { messages: [{ id: 'public', subject: 'Dato público' }] }); }
  else if (url.pathname === '/api/private') {
    protectedCalls++;
    if (!authenticated || !request.headers.cookie?.includes('fixture_session=valid')) json(401, {});
    else json(200, { messages: [{ id: 'private', subject: 'Dato privado' }] });
  } else if (url.pathname === '/login') { response.writeHead(200, { 'Content-Type': 'text/html' }); response.end('<title>Acceso de prueba</title><form id="login"><input type="password"></form>'); }
  else if (url.pathname === '/fixture-login') { response.writeHead(200, { 'Content-Type': 'text/html', 'Set-Cookie': 'fixture_session=valid; HttpOnly; Path=/; Max-Age=3600' }); response.end('<h1>Sesión ficticia preparada</h1>'); }
  else if (url.pathname === '/api/redirect') { response.writeHead(302, { Location: '/login' }); response.end(); }
  else if (url.pathname === '/many') { response.writeHead(200, { 'Content-Type': 'text/html' }); response.end('<main id=ready>' + '<article>fila</article>'.repeat(205) + '</main>'); }
  else if (url.pathname === '/api/denied') json(403, {});
  else if (url.pathname === '/api/changed') json(200, { incompatible: true });
  else if (url.pathname === '/private-page' && !request.headers.cookie?.includes('fixture_session=valid')) { response.writeHead(302, { Location: '/login' }); response.end(); }
  else {
    response.writeHead(200, { 'Content-Type': 'text/html' });
    response.end(`<title>Lista local</title><main id="ready"><article class="message"><b>uno</b><p>Asunto DOM</p></article></main><script>fetch('/api/public?cursor=do-not-expose').then(()=>document.body.dataset.loaded='yes')</script>`);
  }
});
server.listen(0, '127.0.0.1');
await once(server, 'listening');
const origin = `http://127.0.0.1:${server.address().port}`;
const makeSession = () => createBrowserSession({ runtime: path.join(browserRoot, 'runtime-patchright-1.64.2'), profile: path.join(directory, 'profile'), homepage: origin, headless: true });
let session = makeSession(), checks = 0;
async function call(method, args = {}) {
  const result = await session.dispatch({ method, ...args, program_allowed:true });
  assert.ok(!result.error, `${method}: ${result.error}`); checks++;
  return result;
}
const http = (name, path) => ({ name, description: 'Consulta fixture', parameters: {}, transport: 'http', path, result_type: 'array', result_pointer: '/messages', fields: { id: '/id', subject: '/subject' } });
const manifest = { id: 'fixture', name: 'Web de prueba', origin, login_url: origin + '/login', login_urls: [origin + '/login'], functions: [
  { name:'write_action', description:'Acción local', parameters:{}, transport:'program', path:'/', result_type:'object', code:"const r = await context.request.post(origin + '/api/action'); return await r.json();" },
  http('public_messages', '/api/public'), http('private_messages', '/api/private'), http('redirect', '/api/redirect'), http('denied', '/api/denied'), http('changed', '/api/changed'),
  { name: 'page_messages', description: 'Respaldo DOM', parameters: {}, transport: 'page', path: '/page', selector: '.message', ready_selector: '#ready', fields: { id: 'b', subject: 'p' }, result_type: 'array' },
  { name: 'many_page', description: 'Truncado explícito', parameters: {}, transport: 'page', path: '/many', selector: 'article', ready_selector: '#ready', result_type: 'array' },
  { name: 'empty_page', description: 'Lista vacía válida', parameters: {}, transport: 'page', path: '/page', selector: '.absent', ready_selector: '#ready', result_type: 'array' },
  { name: 'login_page', description: 'Página con acceso requerido', parameters: {}, transport: 'page', path: '/login', selector: '.message', login_selector: '#login', result_type: 'array' },
] };
const run = (function_name, action = 'verify', expected_revision = 1) => call('plugin_run', { action, plugin_id: 'fixture', function_name, expected_revision, arguments: {} });
try {
  assert.equal((await call('snapshot')).open, false);
  assert.equal((await call('plugins', { action: 'save', manifest, expected_revision: 0 })).status, 'saved');
  assert.equal((await call('snapshot')).open, false); // Guardar no abre navegador.
  assert.equal((await run('public_messages', 'run')).status, 'verification_required');
  assert.equal((await call('snapshot')).open, false);
  assert.equal((await run('public_messages')).status, 'ok'); // Abre headless sin pedir cuenta.
  assert.equal((await run('write_action')).data.writes, 1);
  assert.equal((await run('write_action', 'run')).data.writes, 2);
  assert.equal(writes, 2);
  assert.equal((await call('open', { background: true })).background, true);
  assert.equal(protectedCalls, 0);
  assert.equal((await run('private_messages')).status, 'authentication_required');
  assert.equal((await run('redirect')).status, 'authentication_required');
  assert.equal((await run('denied')).status, 'access_denied');
  assert.equal((await run('changed')).status, 'site_changed');
  assert.equal((await run('login_page')).status, 'authentication_required');
  const before = await call('snapshot');
  assert.deepEqual((await run('page_messages')).data, [{ id: 'uno', subject: 'Asunto DOM' }]);
  assert.deepEqual((await run('empty_page')).data, []);
  const many = await run('many_page');
  assert.equal(many.data.length, 200);
  assert.equal(many.truncated, true);
  const after = await call('snapshot');
  assert.equal(before.page_id, after.page_id);
  assert.equal(before.tabs.length, after.tabs.length); // Pestaña propia cerrada.
  await call('navigate', { url: origin + '/fixture-login' });
  assert.equal((await run('private_messages')).status, 'ok');
  assert.equal((await run('private_messages', 'run')).data[0].id, 'private');

  await call('network', { action: 'start', origin });
  await call('navigate', { url: origin + '/page' });
  await call('wait_for', { kind: 'element', selector: 'body[data-loaded=yes]' });
  const network = await call('network', { action: 'stop' });
  assert.ok(network.entries.some(item => item.path === '/api/public' && item.query_keys.includes('cursor')));
  assert.ok(!JSON.stringify(network).includes('do-not-expose'));
  assert.equal((await call('network', { action: 'read' })).status, 'inactive');

  await session.close();
  session = makeSession();
  assert.equal((await run('private_messages', 'run')).status, 'ok'); // Perfil y receta recuperados.
  authenticated = false;
  assert.equal((await run('private_messages', 'run')).status, 'authentication_required');
  authenticated = true;
  assert.equal((await run('private_messages', 'run')).status, 'ok');
  assert.equal((await call('plugins', { action: 'save', manifest, expected_revision: 0 })).status, 'revision_conflict');
  assert.equal((await call('plugins', { action: 'save', manifest, expected_revision: 1 })).plugin.revision, 2);
  assert.equal((await run('private_messages', 'run', 2)).status, 'verification_required');
  const persisted = await readFile(path.join(directory, 'plugins/fixture.json'), 'utf8');
  assert.ok(!persisted.includes('fixture_session'));
  assert.ok(!persisted.includes('Dato privado'));
  assert.ok(publicCalls > 0);
  await session.close();
  // Ejecutar también el formato embebido utilizado por el helper Rust/release.
  const worker = await readFile(new URL('../../apps/browser/src/worker.mjs', import.meta.url), 'utf8');
  const pluginSource = await readFile(new URL('../../apps/browser/src/plugins.mjs', import.meta.url), 'utf8');
  const embedded = worker.replace("import { createWebPlugins } from './plugins.mjs';", pluginSource);
  const child = spawn(path.join(browserRoot, 'runtime-patchright-1.64.2/node'), ['--input-type=module', '--eval', embedded, '--', path.join(browserRoot, 'runtime-patchright-1.64.2'), path.join(directory, 'embedded-profile'), '--gennie-browser-worker'], { stdio: ['pipe', 'pipe', 'ignore'] });
  const exited = once(child, 'exit');
  const lines = createInterface({ input: child.stdout });
  const timer = setTimeout(() => child.kill('SIGTERM'), 20000);
  try {
    const reply = Promise.race([
      once(lines, 'line'),
      exited.then(([code]) => { throw new Error(`El worker embebido terminó sin respuesta (código ${code})`); }),
    ]);
    child.stdin.write(JSON.stringify({ method: 'plugin_run', action: 'verify', plugin_id: 'fixture', function_name: 'write_action', expected_revision: 2, arguments: {}, program_allowed:true }) + '\n');
    const result = JSON.parse((await reply)[0]);
    assert.equal(result.status, 'ok'); assert.equal(result.data.writes, 3); checks++;
  } finally {
    child.stdin.end();
    const [code] = await exited;
    clearTimeout(timer); lines.close();
    assert.equal(code, 0);
  }
  console.log(JSON.stringify({ ok: true, checks, persistence: true, http_and_dom: true, session_on_demand: true, isolated_profile: true }));
} finally {
  await session.close();
  server.closeAllConnections();
  await new Promise(resolve => server.close(resolve));
  await rm(directory, { recursive: true, force: true });
}
