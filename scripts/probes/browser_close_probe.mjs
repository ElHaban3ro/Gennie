// Transición visible/headless con servidor y perfil de prueba; no utiliza cuentas.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { spawn, execFileSync } from 'node:child_process';
import { createInterface } from 'node:readline';
import { pathToFileURL } from 'node:url';
import { mkdtemp, rm, lstat, rename } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createBrowserSession } from '../../apps/browser/src/worker.mjs';

const root = path.join(process.env.HOME, 'Library/Application Support/Gennie/browser');
const directory = await mkdtemp(path.join(tmpdir(), 'gennie-background-'));
process.env.PLAYWRIGHT_BROWSERS_PATH = path.join(root, 'browsers');
const requests = [];
let slowStarted, downloadStarted;
const server = createServer((request, response) => {
  requests.push(request.url);
  if (request.url === '/controls') { response.writeHead(200, { 'Content-Type': 'text/html' }); response.end('<button id="dialog" onclick="alert(\'prueba\')">Diálogo</button><a id="download" href="/download-slow">Descarga</a>'); return; }
  if (request.url === '/download-slow') { response.writeHead(200, { 'Content-Type': 'application/octet-stream', 'Content-Disposition': 'attachment; filename="fixture.bin"', 'Content-Length': '2' }); response.write('a'); downloadStarted?.(); setTimeout(() => response.end('b'), 3000); return; }
  const send = () => {
    response.writeHead(200, { 'Content-Type': 'text/html', 'Set-Cookie': 'fixture_session=valid; Path=/; Max-Age=3600' });
    response.end(`<title>Prueba local</title><p>${request.headers.cookie?.includes('fixture_session=valid') ? 'sesión conservada' : 'sin sesión'}</p>`);
  };
  if (request.url === '/slow') { slowStarted?.(); setTimeout(send, 1200); } else send();
});
server.listen(0, '127.0.0.1');
await once(server, 'listening');
const origin = `http://127.0.0.1:${server.address().port}`;
let closed = 0;
const activity = [];
const session = createBrowserSession({ runtime: path.join(root, 'runtime-patchright-1.64.2'), profile: path.join(directory, 'profile'), homepage: origin, restoreMs: 250, onBrowserClose: () => closed++, onActivity: event => activity.push(event) });
async function testMultiplex() {
  const moduleUrl = pathToFileURL(path.resolve('apps/browser/src/worker.mjs')).href;
  const child = spawn(process.execPath, ['--input-type=module', '--eval', `await import(${JSON.stringify(moduleUrl)})`, '--', path.join(root, 'runtime-patchright-1.64.2'), path.join(directory, 'multiplex-profile'), '--gennie-browser-worker'], { stdio: ['pipe', 'pipe', 'inherit'] });
  const pending = new Map();
  const lines = createInterface({ input: child.stdout });
  lines.on('line', line => { const message = JSON.parse(line); pending.get(message.request_id)?.(message.result); pending.delete(message.request_id); });
  let id = 0;
  const callWorker = request => new Promise(resolve => { const request_id = ++id; pending.set(request_id, resolve); child.stdin.write(JSON.stringify({request_id, request}) + '\n'); });
  try {
    assert.equal((await callWorker({method:'open',scope:'a'})).background, true);
    assert.equal((await callWorker({method:'open',scope:'b'})).background, true);
    const started = new Promise(resolve => { slowStarted = resolve; });
    const slow = callWorker({method:'navigate',scope:'a',url:`${origin}/slow`});
    await started;
    const fast = callWorker({method:'navigate',scope:'b',url:`${origin}/fast`});
    const winner = await Promise.race([fast.then(() => 'fast'), slow.then(() => 'slow')]);
    assert.equal(winner, 'fast');
    assert.equal((await fast).status, 200);
    assert.equal((await slow).status, 200);
    await callWorker({method:'scope_end',scope:'a'});
    await callWorker({method:'scope_end',scope:'b'});
  } finally {
    child.stdin.end();
    const [code] = await once(child, 'exit');
    assert.equal(code, 0);
    lines.close();
  }
}

try { await testMultiplex(); console.log('multiplex_close_ok'); } finally { await session.close(); server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); await rm(directory, { recursive: true, force: true }); }
