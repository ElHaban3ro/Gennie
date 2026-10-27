// Chromium propio, página vacía y perfil temporal: sin cuentas, red ni servidor.
import assert from 'node:assert/strict';
import { mkdtemp, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createBrowserSession } from '../../apps/browser/src/worker.mjs';

const installed = path.join(process.env.HOME, 'Library/Application Support/Gennie/browser');
const directory = await mkdtemp(path.join(tmpdir(), 'gennie-builder-captures-'));
const session = createBrowserSession({ runtime: path.join(installed, 'runtime-patchright-1.64.2'), profile: path.join(directory, 'profile'), headless: true });
async function files() {
  try { return await readdir(path.join(directory, 'artifacts')); }
  catch (error) { if (error.code === 'ENOENT') return []; throw error; }
}
try {
  const task = await session.dispatch({ method: 'task_page', action: 'create' });
  assert.ok(task.page_id);
  const shot = await session.dispatch({ method: 'screenshot', page_id: task.page_id, background_scope: true });
  assert.equal(shot.image.mime_type, 'image/jpeg');
  assert.ok(Buffer.from(shot.image.data, 'base64').length > 100);
  assert.equal(shot.artifact, undefined);
  assert.deepEqual(await files(), []);
  const exported = await session.dispatch({ method: 'screenshot', page_id: task.page_id });
  assert.ok(exported.artifact.path.startsWith(directory));
  assert.equal((await files()).length, 1);
  console.log('builder_screenshots transient_capture=true explicit_export_preserved=true isolated_profile=true');
} finally {
  await session.close();
  await rm(directory, { recursive: true, force: true });
}
