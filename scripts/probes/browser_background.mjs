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
async function testRecovery() {
  const moduleUrl = pathToFileURL(path.resolve('apps/browser/src/worker.mjs')).href;
  const profile = path.join(directory, 'recovery-profile');
  const program = `
    import { createBrowserSession } from ${JSON.stringify(moduleUrl)};
    const [runtime,profile,origin] = process.argv.slice(1);
    const session = createBrowserSession({runtime,profile,homepage:origin});
    async function call(method,args={}) { const value=await session.dispatch({method,...args}); if(value.error) throw Error(value.error); return value; }
    await call('open');
    await call('tabs',{action:'create',url:origin+'/recovery-before'});
    await call('navigate',{url:origin+'/recovery-current'});
    await call('open',{scope:'interrupted'});
    console.log('ready');
    setInterval(()=>{},1000);
  `;
  const child = spawn(process.execPath, ['--input-type=module','--eval',program,'--',path.join(root,'runtime-patchright-1.64.2'),profile,origin], { stdio:['ignore','pipe','inherit'], detached:true });
  const lines = createInterface({input:child.stdout});
  try {
    const [line] = await Promise.race([once(lines,'line'), new Promise((_,reject)=>{ const timer=setTimeout(()=>reject(Error('timeout preparing recovery')),20000); timer.unref(); })]);
    assert.equal(line,'ready');
    child.kill('SIGKILL');
    await once(child,'exit');
    const deadline = Date.now() + 8000;
    while (Date.now() < deadline && await lstat(path.join(profile,'SingletonLock')).catch(()=>null)) await new Promise(resolve=>setTimeout(resolve,100));
    assert.equal(await lstat(path.join(profile,'SingletonLock')).catch(()=>null), null, 'el Chromium del fixture debe terminar');
    const recovered = createBrowserSession({runtime:path.join(root,'runtime-patchright-1.64.2'),profile,homepage:origin});
    try {
      const tabs = await recovered.dispatch({method:'tabs',action:'list'});
      assert.ok(!tabs.error, JSON.stringify(tabs));
      const restored = tabs.tabs.find(tab=>tab.url===`${origin}/recovery-current`);
      assert.ok(restored, 'recupera la pestaña visible tras la interrupción');
      const back = await recovered.dispatch({method:'history',page_id:restored.page_id,action:'back'});
      assert.ok(!back.error, JSON.stringify(back));
      assert.ok((await recovered.dispatch({method:'tabs',action:'list'})).tabs.some(tab=>tab.url===`${origin}/recovery-before`));
      assert.equal(await lstat(path.join(profile,'.gennie-visible-restore')).catch(()=>null), null);
    } finally { await recovered.close(); }
  } finally {
    if (child.exitCode === null && child.signalCode === null) { child.kill('SIGKILL'); await once(child,'exit'); }
    lines.close();
  }
}
async function testRestoreWarning() {
  const profile = path.join(directory, 'warning-profile');
  let notify;
  const warning = new Promise(resolve => { notify = resolve; });
  const testSession = createBrowserSession({ runtime:path.join(root,'runtime-patchright-1.64.2'), profile, homepage:origin, restoreMs:100, onWarning:notify });
  const pending = path.join(profile,'.gennie-visible-restore'), held = `${pending}.fixture`;
  try {
    assert.equal((await testSession.dispatch({method:'open'})).background, false);
    assert.equal((await testSession.dispatch({method:'open',scope:'warning'})).background, true);
    await rename(pending, held);
    assert.equal((await testSession.dispatch({method:'scope_end',scope:'warning'})).status, 'closed');
    const result = await Promise.race([warning, new Promise((_,reject)=>{const timer=setTimeout(()=>reject(Error('timeout waiting for warning')),5000);timer.unref();})]);
    assert.equal(result, 'restore_failed');
    await rename(held, pending);
    const reopened = await testSession.dispatch({method:'open'});
    assert.equal(reopened.background, false, JSON.stringify(reopened));
  } finally {
    if (await lstat(held).catch(()=>null)) await rename(held,pending);
    await testSession.close();
  }
}
async function testChromiumExit() {
  const profile = path.join(directory, 'unexpected-profile');
  const testSession = createBrowserSession({ runtime:path.join(root,'runtime-patchright-1.64.2'), profile, homepage:origin, restoreMs:100 });
  try {
    assert.equal((await testSession.dispatch({method:'open'})).background, false);
    await testSession.dispatch({method:'tabs',action:'create',url:`${origin}/unexpected-user`});
    assert.equal((await testSession.dispatch({method:'open',scope:'unexpected'})).background, true);
    const processes = execFileSync('/bin/ps',['-axo','pid=,command='],{encoding:'utf8'}).split('\n').filter(line => line.includes(`--user-data-dir=${profile}`) && !line.includes('--type=') && line.includes('Google Chrome for Testing'));
    assert.equal(processes.length, 1, 'solo se termina el Chromium de este perfil temporal');
    process.kill(Number(processes[0].trim().split(/\s+/)[0]), 'SIGTERM');
    await new Promise(resolve=>setTimeout(resolve,1000));
    const tabs = await testSession.dispatch({method:'tabs',action:'list'});
    assert.equal(tabs.open, true, JSON.stringify(tabs));
    assert.ok(tabs.tabs.some(tab=>tab.url===`${origin}/unexpected-user`));
  } finally { await testSession.close(); }
}
async function testDeferredCleanup() {
  const profile = path.join(directory, 'deferred-profile');
  const warnings = [];
  const testSession = createBrowserSession({ runtime:path.join(root,'runtime-patchright-1.64.2'), profile, homepage:origin, restoreMs:100, onWarning:code=>warnings.push(code) });
  const callTest = async (method, args={}) => { const result=await testSession.dispatch({method,...args}); assert.ok(!result.error, JSON.stringify(result)); return result; };
  try {
    await callTest('open');
    await callTest('open',{scope:'download'});
    await callTest('open',{scope:'other'});
    await callTest('navigate',{scope:'download',url:`${origin}/controls`});
    const started = new Promise(resolve => { downloadStarted = resolve; });
    const click = callTest('click',{scope:'download',selector:'#download'});
    await started;
    assert.equal((await callTest('scope_end',{scope:'download'})).status, 'deferred');
    assert.equal((await callTest('navigate',{scope:'other',url:`${origin}/other`})).status, 200);
    await click;
    await new Promise(resolve=>setTimeout(resolve,1300));
    const downloads = await callTest('download',{action:'list'});
    assert.equal(downloads.downloads.at(-1).status, 'completed');
    assert.equal((await callTest('snapshot',{scope:'other'})).open, true);
    await callTest('scope_end',{scope:'other'});
    await new Promise(resolve=>setTimeout(resolve,300));
    assert.equal((await callTest('open',{background:true})).background, false);
    const task = await callTest('task_page',{action:'create'});
    await callTest('navigate',{page_id:task.page_id,background_scope:true,url:`${origin}/controls`});
    const taskStarted = new Promise(resolve => { downloadStarted = resolve; });
    const taskClick = callTest('click',{page_id:task.page_id,background_scope:true,selector:'#download'});
    await taskStarted;
    assert.equal((await callTest('task_page',{action:'close',page_id:task.page_id})).status, 'deferred');
    assert.equal((await callTest('open',{scope:'other-task'})).background, true);
    await taskClick;
    await new Promise(resolve=>setTimeout(resolve,1300));
    await callTest('scope_end',{scope:'other-task'});
    await new Promise(resolve=>setTimeout(resolve,300));
    assert.equal((await callTest('open',{background:true})).background, false);
    const modalTask = await callTest('task_page',{action:'create'});
    await callTest('navigate',{page_id:modalTask.page_id,background_scope:true,url:`${origin}/controls`});
    assert.equal((await callTest('click',{page_id:modalTask.page_id,background_scope:true,selector:'#dialog'})).action_pending,true);
    const deferred = await callTest('task_page',{action:'close',page_id:modalTask.page_id});
    assert.equal(deferred.status,'deferred'); assert.equal(deferred.modal_pending,true);
    assert.ok(warnings.includes('cleanup_pending'));
    await callTest('dialog',{page_id:modalTask.page_id,background_scope:true,action:'accept'});
    await new Promise(resolve=>setTimeout(resolve,1400));
    assert.equal((await callTest('open',{background:true})).background,false);
  } finally { await testSession.close(); }
}
async function call(method, args = {}) {
  const result = await session.dispatch({ method, ...args });
  assert.ok(!result.error, `${method}: ${result.error}`);
  return result;
}
try {
  assert.equal((await call('open')).background, false);
  await call('navigate', { url: `${origin}/controls` });
  assert.equal((await call('click', { selector: '#dialog' })).action_pending, true);
  assert.equal((await session.dispatch({ method: 'task_page', action: 'create' })).code, 'action_pending');
  await call('dialog', { action: 'accept' });
  activity.length = 0;
  const startedDownload = new Promise(resolve => { downloadStarted = resolve; });
  const downloadClick = call('click', { selector: '#download' });
  await startedDownload;
  await new Promise(resolve => setTimeout(resolve, 200));
  const downloading = (await call('download', { action: 'list' })).downloads.at(-1);
  assert.ok(activity.includes('navigation_start'));
  assert.ok(!activity.includes('navigation_end'));
  assert.equal((await session.dispatch({ method: 'task_page', action: 'create' })).code, 'action_pending');
  assert.ok(!downloading || downloading.status === 'pending');
  await downloadClick;
  assert.equal((await call('download', { action: 'wait' })).status, 'completed');
  const manifest = { id:'fixture', name:'Prueba', origin, functions:[{ name:'read', description:'Lee la página local', parameters:{}, transport:'page', path:'/controls', result_type:'array', selector:'button' }] };
  assert.equal((await call('plugins', { action:'save', manifest, expected_revision:0 })).status, 'saved');
  assert.equal((await call('plugin_run', { action:'verify', plugin_id:'fixture', function_name:'read', expected_revision:1, arguments:{} })).status, 'ok');
  await new Promise(resolve => setTimeout(resolve, 400));
  assert.equal((await call('open', { background:true })).background, false);
  await call('tabs', { action: 'create', url: `${origin}/previous` });
  await call('navigate', { url: `${origin}/user-tab` });
  requests.length = 0;
  const first = await call('open', { scope: 'chat-a' });
  assert.ok(!requests.includes('/user-tab'), 'headless no debe reabrir pestañas visibles del usuario');
  assert.ok(!(await call('tabs', { action: 'list' })).tabs.some(tab => tab.url === `${origin}/user-tab`), 'headless no debe contener pestañas visibles del usuario');
  const competitor = createBrowserSession({ runtime: path.join(root, 'runtime-patchright-1.64.2'), profile: path.join(directory, 'profile'), homepage: origin });
  try { assert.equal((await competitor.dispatch({ method: 'open', background: true })).code, 'task_active'); }
  finally { await competitor.close(); }
  const second = await call('open', { scope: 'chat-b' });
  const sharedPage = (await call('open', { background: true })).page_id;
  assert.notEqual(first.page_id, second.page_id);
  await call('network', { scope: 'chat-a', action: 'start', origin });
  await call('network', { scope: 'chat-b', action: 'start', origin });
  await call('navigate', { scope: 'chat-a', url: `${origin}/chat-a` });
  assert.ok((await call('network', { scope: 'chat-a', action: 'read' })).entries.length > 0);
  assert.equal((await call('network', { scope: 'chat-b', action: 'read' })).entries.length, 0);
  await call('navigate', { scope: 'chat-b', url: `${origin}/chat-b` });
  const firstTabs = await call('tabs', { scope: 'chat-a', action: 'list' });
  assert.equal(firstTabs.tabs.length, 1);
  assert.equal(firstTabs.tabs[0].url, `${origin}/chat-a`);
  assert.equal((await session.dispatch({ method: 'snapshot', scope: 'chat-b', page_id: first.page_id })).code, 'closed');
  await call('scope_end', { scope: 'chat-a' });
  assert.equal((await call('snapshot', { scope: 'chat-b' })).open, true);
  await call('scope_end', { scope: 'chat-b' });
  const reused = await call('open', { background: true });
  assert.equal(reused.background, true);
  assert.equal(reused.page_id, sharedPage);
  const task = await call('task_page', { action: 'create' });
  assert.equal((await call('open', { background: true })).background, true);
  assert.equal((await session.dispatch({ method: 'open' })).code, 'task_active');
  await call('task_page', { action: 'close', page_id: task.page_id });
  await new Promise(resolve => setTimeout(resolve, 400));
  assert.equal((await call('open', { background: true })).background, false);
  const restored = await call('tabs', { action: 'list' });
  assert.ok(restored.tabs.some(tab => tab.url === `${origin}/user-tab`), 'la pestaña visible debe restaurarse');
  const userTab = restored.tabs.find(tab => tab.url === `${origin}/user-tab`);
  await call('history', { page_id: userTab.page_id, action: 'back' });
  assert.ok((await call('tabs', { action: 'list' })).tabs.some(tab => tab.url === `${origin}/previous`), 'la pestaña debe conservar su historial');
  const forward = await session.dispatch({ method: 'history', page_id: userTab.page_id, action: 'forward' });
  if (forward.error) console.error(JSON.stringify({ forward, tabs: (await call('tabs', { action: 'list' })).tabs, requests }));
  assert.ok(!forward.error, `history forward: ${forward.error}`);
  assert.equal((await call('open')).background, false);
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
  const idleSession = createBrowserSession({ runtime: path.join(root, 'runtime-patchright-1.64.2'), profile: path.join(directory, 'idle-profile'), homepage: origin, headless: true, idleMs: 250 });
  try {
    const idleTask = await idleSession.dispatch({ method: 'task_page', action: 'create' });
    assert.ok(idleTask.page_id, JSON.stringify(idleTask));
    await new Promise(resolve => setTimeout(resolve, 600));
    assert.equal((await idleSession.dispatch({ method: 'snapshot', page_id: idleTask.page_id })).open, true);
    await idleSession.dispatch({ method: 'task_page', action: 'close', page_id: idleTask.page_id });
    await new Promise(resolve => setTimeout(resolve, 1000));
    assert.equal((await idleSession.dispatch({ method: 'snapshot' })).open, false);
  } finally { await idleSession.close(); }
  await testMultiplex();
  await testRecovery();
  await testRestoreWarning();
  await testChromiumExit();
  await testDeferredCleanup();
  console.log(JSON.stringify({ visible_to_headless: true, cookie_retained: true, pending_dialog_protected: true, pending_download_protected: true, deferred_cleanup: true, manual_plugin_headless: true, active_task_rejected: true, headless_to_visible: true, visible_history_retained: true, close_callback_suppressed: true, idle_close: true, idle_active_task_preserved: true, scope_isolation: true, multiplex_concurrent: true, crash_recovery: true, restore_warning: true, chromium_exit_recovery: true }));
} finally {
  await session.close();
  server.closeAllConnections();
  await new Promise(resolve => server.close(resolve));
  await rm(directory, { recursive: true, force: true });
}
