// Ensayo aislado de restauración nativa: tamaño, pestañas e historial.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { execFileSync } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createBrowserSession } from '../../apps/browser/src/worker.mjs';

const root = path.join(process.env.HOME, 'Library/Application Support/Gennie/browser');
const runtime = path.join(root, 'runtime-patchright-1.64.2');
const { chromium } = createRequire(path.join(runtime, 'package.json'))('patchright');
const directory = await mkdtemp(path.join(tmpdir(), 'gennie-window-'));
const profile = path.join(directory, 'profile');
const server = createServer((request, response) => response.end(`<title>${request.url}</title>`));
server.listen(0, '127.0.0.1');
await once(server, 'listening');
const origin = `http://127.0.0.1:${server.address().port}`;
let initial, session;

function windowSize() {
  const lines = execFileSync('ps', ['-axo', 'pid=,command='], { encoding: 'utf8' }).split('\n').filter(line => line.includes(`--user-data-dir=${profile}`) && !line.includes('--type=') && line.includes('Google Chrome for Testing'));
  assert.equal(lines.length, 1);
  const pid = Number(lines[0].trim().split(/\s+/)[0]);
  const swift = `import CoreGraphics; import Foundation
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly,.excludeDesktopElements], kCGNullWindowID) as? [[String:Any]] ?? []
for w in windows where (w[kCGWindowOwnerPID as String] as? Int) == ${pid} && (w[kCGWindowLayer as String] as? Int) == 0 {
 if let b = w[kCGWindowBounds as String] as? [String:Double], let width = b["Width"], let height = b["Height"], width >= 400 && height >= 300 { print("\\(Int(width)) \\(Int(height))") }
}`;
  return execFileSync('swift', ['-e', swift], { encoding: 'utf8' }).trim().split('\n').filter(Boolean).map(line => line.split(' ').map(Number));
}
async function call(method, args = {}) {
  const result = await session.dispatch({ method, ...args });
  assert.ok(!result.error, JSON.stringify(result));
  return result;
}
try {
  initial = await chromium.launchPersistentContext(profile, {
    headless: false, viewport: null, executablePath: path.join(runtime, 'Gennie Web.app/Contents/MacOS/Google Chrome for Testing'),
    args: ['--window-size=500,375'],
  });
  const first = initial.pages()[0];
  await first.goto(`${origin}/before`);
  await first.goto(`${origin}/current`);
  const second = await initial.newPage();
  await second.goto(`${origin}/second`);
  assert.ok(windowSize().some(([width, height]) => width === 500 && height === 375));
  await initial.close(); initial = undefined;
  session = createBrowserSession({ runtime, profile, homepage: `${origin}/home`, restoreMs: 200 });
  await call('open');
  await new Promise(resolve => setTimeout(resolve, 2200));
  const sizes = windowSize();
  assert.ok(sizes.length && sizes.every(([width, height]) => width >= 1000 && height >= 700), JSON.stringify(sizes));
  const tabs = (await call('tabs', { action: 'list' })).tabs;
  assert.ok(tabs.some(tab => tab.url === `${origin}/second`));
  const current = tabs.find(tab => tab.url === `${origin}/current`);
  assert.ok(current);
  await call('history', { page_id: current.page_id, action: 'back' });
  assert.ok((await call('tabs', { action: 'list' })).tabs.some(tab => tab.url === `${origin}/before`));
  await call('open', { scope: 'fixture' });
  await call('scope_end', { scope: 'fixture' });
  await new Promise(resolve => setTimeout(resolve, 2400));
  const restored = windowSize();
  assert.ok(restored.length && restored.every(([width, height]) => width >= 1000 && height >= 700), JSON.stringify(restored));
  assert.ok((await call('tabs', { action: 'list' })).tabs.some(tab => tab.url === `${origin}/before`));
  console.log('Ventana restaurada, pestañas, Atrás y retorno de segundo plano: correctos');
} finally {
  if (initial) await initial.close();
  if (session) await session.close();
  await new Promise(resolve => server.close(resolve));
  await rm(directory, { recursive: true, force: true });
}
