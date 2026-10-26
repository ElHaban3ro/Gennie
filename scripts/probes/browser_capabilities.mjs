// Navegador real y sitio de prueba local: sin cuentas, perfil propio y cierre garantizado.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { mkdtemp, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createBrowserSession } from '../../apps/browser/src/worker.mjs';
const root = path.join(process.env.HOME, 'Library/Application Support/Gennie/browser');
process.env.PLAYWRIGHT_BROWSERS_PATH = path.join(root, 'browsers');
const directory = await mkdtemp(path.join(tmpdir(), 'gennie-browser-capabilities-'));
const html = `<!doctype html><html><head><meta charset='utf-8'><title>Prueba local</title><style>body{font-family:sans-serif}.hovered{display:none}#menu:hover .hovered{display:block}</style></head><body>
<article><h1>Artículo de prueba</h1><p>Contenido ORBITA-42 para extracción.</p><a href='/next'>Otra página</a></article>
<label>Nombre<input id='name' oninput='document.querySelector("#echo").textContent=this.value'></label><p id='echo'></p>
<label>Opción<select id='select' onchange='document.querySelector("#choice").textContent=this.value'><option value='a'>Primera</option><option value='b'>Segunda</option></select></label><p id='choice'></p>
<label>Aceptar<input id='check' type='checkbox' onchange='document.querySelector("#checked").textContent=this.checked'></label><p id='checked'></p>
<button id='timer' onclick='setTimeout(()=>document.querySelector("#late").textContent="Listo",60)'>Cargar texto</button><p id='late'></p>
<div id='menu' role='button'>Menú<span class='hovered'>Submenú visible</span></div>
<button id='alert' onclick='alert("Aviso local")'>Alerta</button>
<button id='confirm' onclick='document.querySelector("#answer").textContent=confirm("Confirmar local")?"sí":"no"'>Confirmación</button>
<button id='prompt' onclick='document.querySelector("#answer").textContent=prompt("Nombre de prueba")'>Pregunta</button><p id='answer'></p>
<button id='replace' onclick='this.outerHTML=this.outerHTML'>Reemplazar</button>
<input id='file' type='file' hidden onchange='document.querySelector("#uploaded").textContent=this.files[0]?.name||"vacío"'><button id='choose' onclick='document.querySelector("#file").click()'>Elegir archivo</button><p id='uploaded'></p>
<a id='download' href='/download'>Descargar archivo</a><a id='slow' href='/slow'>Descarga lenta</a><table><tr><th>Nombre</th><th>Valor</th></tr><tr><td>Ada</td><td>42</td></tr></table>
<iframe src='/frame' title='Marco de prueba'></iframe><div style='height:1800px'></div><p>Pie de página</p></body></html>`;
const server = createServer((request, response) => {
  if (request.url === '/api') {
    response.writeHead(200, { 'Content-Type': 'application/json', 'Set-Cookie': 'from_api=retained; Path=/; HttpOnly' });
    response.end(JSON.stringify({ cookie_shared: request.headers.cookie?.includes('fixture=present') || false, from_api: request.headers.cookie?.includes('from_api=retained') || false }));
  } else if (request.url === '/denied') { response.writeHead(403, { 'Content-Type': 'text/plain' }); response.end('Acceso denegado de prueba'); }
  else if (request.url === '/download') { response.writeHead(200, { 'Content-Type': 'text/plain', 'Content-Disposition': 'attachment; filename="fixture.txt"' }); response.end('download ORBITA-42'); }
  else if (request.url === '/slow') { response.writeHead(200, { 'Content-Type': 'application/octet-stream', 'Content-Disposition': 'attachment; filename="slow.bin"', 'Content-Length': '1048576' }); response.write(Buffer.alloc(65536, 1)); const timer = setTimeout(() => response.end(Buffer.alloc(1048576 - 65536, 1)), 3000); response.on('close', () => clearTimeout(timer)); }
  else if (request.url === '/frame') { response.setHeader('Content-Type','text/html'); response.end('<button onclick="document.querySelector(\'p\').textContent=\'Marco listo\'">Botón del marco</button><p>Marco inicial</p>'); }
  else if (request.url === '/next') { response.setHeader('Content-Type','text/html'); response.end('<title>Siguiente</title><h1>Página siguiente</h1>'); }
  else { response.writeHead(200, { 'Content-Type': 'text/html', 'Set-Cookie': 'fixture=present; Path=/' }); response.end(html); }
});
server.listen(0, '127.0.0.1');
await once(server, 'listening');
const origin = `http://127.0.0.1:${server.address().port}`;
const session = createBrowserSession({ runtime: path.join(root, 'runtime-1.64.1'), profile: path.join(directory, 'profile'), homepage: origin, headless: true });
let assertions = 0;
async function call(method, args = {}) {
  const result = await session.dispatch({ method, ...args });
  assert.ok(!result.error, `${method}: ${result.error}`); assertions++;
  return result;
}
async function ref(label) {
  const snapshot = await call('snapshot');
  const result = snapshot.targets.find(item => item.label === label);
  assert.ok(result, `No hay target para ${label}`); return result.ref;
}
async function text() { return (await call('read')).data; }
try {
  const opened = await call('open');
  assert.equal(opened.open, true);
  const initial = await call('snapshot');
  assert.ok(initial.frames.length === 2);
  const name = initial.targets.find(item => item.label === 'Nombre').ref;
  await call('fill', { target: name, text: 'Ada Lovelace' });
  assert.ok((await text()).includes('Ada Lovelace'));
  assert.equal((await session.dispatch({ method: 'fill', target: name, text: 'Stale' })).code, 'stale_reference');
  await call('check', { target: await ref('Aceptar'), checked: true });
  assert.ok((await text()).includes('true'));
  await call('select_option', { selector: '#select', values: ['Segunda'], by: 'label' });
  assert.ok((await text()).includes('b'));
  await call('press', { target: await ref('Nombre'), key: 'End' });
  await call('click', { target: await ref('Cargar texto') });
  await call('wait_for', { kind: 'text', text: 'Listo', timeout_ms: 1000 });
  await call('wait_for', { kind: 'element', selector: '#name', state: 'visible', timeout_ms: 1000 });
  await call('wait_for', { kind: 'url', url: origin + '/', timeout_ms: 1000 });
  await call('wait_for', { kind: 'load', state: 'domcontentloaded', timeout_ms: 1000 });
  assert.equal((await session.dispatch({ method: 'wait_for', kind: 'text', text: 'Nunca existe', timeout_ms: 50 })).code, 'timeout');
  await call('hover', { selector: '#menu' });
  await call('wait_for', { kind: 'text', text: 'Submenú visible', timeout_ms: 1000 });
  const found = await call('find', { text: 'ORBITA-42' });
  assert.ok(found.matches.length > 0 || found.targets.length > 0);
  assert.ok((await call('read', { mode: 'article' })).data.includes('Artículo de prueba'));
  assert.ok((await call('read', { mode: 'links' })).data.some(link => link.url === origin + '/next'));
  assert.equal((await call('read', { mode: 'tables' })).data[0][1][0], 'Ada');
  const frameSnapshot = await call('snapshot');
  const frameButton = frameSnapshot.targets.find(item => item.label === 'Botón del marco');
  await call('click', { target: frameButton.ref });
  assert.equal((await call('read', { selector: 'p', frame_id: frameButton.frame_id })).data, 'Marco listo');
  const stale = await ref('Reemplazar');
  await call('click', { target: stale });
  assert.equal((await session.dispatch({ method: 'click', target: stale })).code, 'stale_reference');

  for (const [selector, action, value] of [['#alert','accept',null],['#confirm','dismiss',null],['#prompt','accept','Ada']]) {
    const result = await call('click', { selector });
    assert.ok(result.modal);
    assert.ok((await call('dialog')).modal);
    assert.ok((await call('snapshot')).modal);
    await call('dialog', { action, ...(value ? { text: value } : {}) });
  }
  assert.ok((await text()).includes('Ada'));
  const filename = path.join(directory, 'upload.txt');
  await writeFile(filename, 'archivo de prueba');
  const chooser = await call('click', { selector: '#choose' });
  assert.equal((chooser.modal || (await call('dialog')).modal)?.type, 'file_chooser');
  await call('upload', { files: [filename] });
  assert.ok((await text()).includes('upload.txt'));
  await call('upload', { selector: '#file', files: [] });
  assert.ok((await text()).includes('vacío'));
  assert.equal((await session.dispatch({ method: 'upload', selector: '#file', files: ['/missing/file'] })).code, 'invalid_file');

  await call('click', { selector: '#download' });
  const saved = await call('download', { action: 'wait' });
  const downloads = (await call('download')).downloads;
  assert.equal(downloads.length, 1);
  assert.equal(saved.status, 'completed');
  assert.equal(await readFile(saved.path, 'utf8'), 'download ORBITA-42');
  await call('click', { selector: '#slow' });
  const slow = (await call('download')).downloads.find(item => item.suggested_filename === 'slow.bin');
  assert.ok(slow);
  assert.equal(slow.status, 'pending');
  assert.equal((await call('download', { action: 'cancel', download_id: slow.download_id })).status, 'cancelled');
  const screenshot = await call('screenshot');
  assert.equal(Buffer.from(screenshot.image.data, 'base64').readUInt16BE(0), 0xffd8);
  assert.ok((await readFile(screenshot.artifact.path)).length > 100);
  assert.ok((await call('screenshot', { full_page: true })).artifact.bytes > 100);
  const elementShot = await call('screenshot', { target: await ref('Nombre') });
  assert.ok(elementShot.artifact.bytes > 100);
  const pdf = await call('pdf');
  assert.equal((await readFile(pdf.artifact.path)).subarray(0, 4).toString(), '%PDF');
  const api = await call('request', { url: origin + '/api' });
  assert.equal(JSON.parse(api.body).cookie_shared, true);
  assert.equal(JSON.parse((await call('request', { url: origin + '/api' })).body).from_api, true);
  assert.equal((await call('request', { url: origin + '/denied' })).status, 403);
  assert.equal((await call('request', { url: origin + '/api', http_method: 'HEAD' })).body, null);
  assert.equal((await session.dispatch({ method: 'request', url: origin, http_method: 'POST' })).code, 'invalid_arguments');
  await call('scroll', { direction: 'down', amount: 800 });
  assert.equal((await session.dispatch({ method: 'tabs', action: 'create', url: 'file:///etc/passwd' })).code, 'invalid_url');
  assert.equal((await call('tabs')).tabs.length, 1);
  const created = await call('tabs', { action: 'create', url: origin + '/next' });
  assert.equal(created.tabs.length, 2);
  await call('tabs', { action: 'select', page_id: opened.page_id });
  await call('navigate', { url: origin + '/next' });
  assert.equal((await call('history', { action: 'back' })).url, origin + '/');
  assert.equal((await call('history', { action: 'forward' })).url, origin + '/next');
  assert.equal((await call('history', { action: 'reload' })).status, 200);
  await call('tabs', { action: 'close', page_id: created.page_id });
  assert.equal((await call('tabs')).tabs.length, 1);
  await session.close();
  assert.equal(await readFile(saved.path, 'utf8'), 'download ORBITA-42');
  console.log(JSON.stringify({ passed: true, operations: assertions, capabilities: 'refs,frames,tabs,history,find,read,wait,forms,hover,scroll,dialogs,upload,download,screenshot,pdf,session_http' }));
} finally {
  await session.close();
  server.closeAllConnections();
  await new Promise(resolve => server.close(resolve));
  await rm(directory, { recursive: true, force: true });
}
