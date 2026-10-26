import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm, readFile, stat, symlink } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createWebPlugins, validateWebPlugin } from '../src/plugins.mjs';

const manifest = () => ({ id: 'mail', name: 'Correo de prueba', origin: 'https://example.com', login_url: 'https://example.com/login', login_urls: ['https://example.com/login'], functions: [{ name: 'list', description: 'Consulta de prueba', parameters: { after: 'string' }, transport: 'http', path: '/api/messages', query: { after: '{after}' }, result_type: 'array', result_pointer: '/messages', fields: { id: '/id', subject: '/subject' } }] });
async function fixture(t) {
  const directory = await mkdtemp(path.join(tmpdir(), 'gennie-plugin-test-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  let code = 200, data = { messages: [{ id: 'one', subject: 'Prueba' }] }, headers = { 'content-type': 'application/json' };
  const calls = [], disposed = [];
  const plugins = createWebPlugins({ directory, getContext: () => ({ request: { get: async (url, options) => {
    calls.push({ url, options });
    return { status: () => code, headers: () => headers, body: async () => Buffer.from(JSON.stringify(data)), dispose: async () => disposed.push(url) };
  } } }) });
  const save = async (value = manifest(), expected_revision = 0) => plugins.dispatch({ action: 'save', manifest: value, expected_revision });
  const run = (action = 'verify', args = { after: 'today' }, revision = 1) => plugins.dispatch({ action, plugin_id: 'mail', function_name: 'list', expected_revision: revision, arguments: args });
  return { directory, plugins, calls, disposed, save, run, reply: (status, body = data, metadata = headers) => { code = status; data = body; headers = metadata; } };
}
test('valida contratos cerrados y rechaza credenciales/código antes de guardar', () => {
  assert.equal(validateWebPlugin(manifest()).id, 'mail');
  for (const patch of [
    { id: '../outside' }, { origin: 'https://example.com/path' }, { login_url: 'https://example.com/login?token=secret' },
    { functions: [{ ...manifest().functions[0], script: 'process.exit()' }] },
    { functions: [{ ...manifest().functions[0], query: { token: 'secret' } }] },
    { functions: [{ ...manifest().functions[0], path: '//other.example/api' }] },
    { functions: [{ ...manifest().functions[0], result_pointer: '/bad~escape' }] },
    { functions: [{ ...manifest().functions[0], query: { after: '{unknown}' } }] },
  ]) assert.throws(() => validateWebPlugin({ ...manifest(), ...patch }), { code: 'invalid_plugin' });
});
test('revisión, verificación por función, persistencia y edición invalidan evidencia', async t => {
  const f = await fixture(t);
  assert.equal((await f.save()).status, 'saved');
  assert.equal((await f.run('run')).status, 'verification_required');
  assert.equal(f.calls.length, 0);
  assert.equal((await f.run()).status, 'ok');
  assert.deepEqual((await f.run('run')).data, [{ id: 'one', subject: 'Prueba' }]);
  const reopened = createWebPlugins({ directory: f.directory, getContext: () => null });
  const stored = (await reopened.dispatch({ action: 'get', plugin_id: 'mail' })).plugin;
  assert.ok(stored.verified.list.verified_at.endsWith('Z'));
  assert.equal(stored.verified.list.revision, 1);
  assert.equal((await f.save()).status, 'revision_conflict');
  assert.equal((await f.save(manifest(), 1)).plugin.revision, 2);
  assert.equal((await f.run('run', { after: 'today' }, 2)).status, 'verification_required');
  assert.equal((await f.run()).status, 'revision_conflict');
  const disk = await readFile(path.join(f.directory, 'mail.json'), 'utf8');
  assert.ok(!disk.includes('"subject":"Prueba"'));
  assert.equal((await stat(path.join(f.directory, 'mail.json'))).mode & 0o777, 0o600);
  assert.equal((await stat(f.directory)).mode & 0o777, 0o700);
  assert.equal(f.disposed.length, f.calls.length);
});
test('acceso bajo demanda: público/sesión vigente, 401, 403, redirect y contrato', async t => {
  const f = await fixture(t); await f.save();
  assert.equal((await f.run()).status, 'ok');
  f.reply(401); const expired = await f.run();
  assert.equal(expired.status, 'authentication_required');
  assert.equal(expired.login_url, 'https://example.com/login');
  assert.equal((await f.run('run')).status, 'verification_required');
  f.reply(403); assert.equal((await f.run()).status, 'access_denied');
  f.reply(302, {}, { location: '/login?return=secret' }); assert.equal((await f.run()).status, 'authentication_required');
  f.reply(302, {}, { location: 'https://unrelated.example/' }); assert.equal((await f.run()).status, 'redirect_blocked');
  f.reply(200, {}, { 'content-type': 'text/html' }); assert.equal((await f.run()).status, 'site_changed');
  f.reply(200, { messages: [{ id: 'one' }] }, { 'content-type': 'application/json' }); assert.equal((await f.run()).status, 'site_changed');
  f.reply(429); assert.equal((await f.run()).status, 'http_error');
  assert.equal(f.disposed.length, f.calls.length);
});
test('argumentos codificados, sin parámetros extra ni recorrido de ruta', async t => {
  const f = await fixture(t); await f.save();
  await f.run('verify', { after: 'a&token=private' });
  assert.equal(new URL(f.calls[0].url).searchParams.get('after'), 'a&token=private');
  assert.equal(new URL(f.calls[0].url).searchParams.has('token'), false);
  const count = f.calls.length;
  for (const args of [{}, { after: 'a', extra: true }, { after: 0 }]) await assert.rejects(f.run('verify', args), { code: 'invalid_plugin_arguments' });
  assert.equal(f.calls.length, count);
});
test('fallo de transporte no pide login y no publica evidencia de éxito', async t => {
  const f = await fixture(t); await f.save();
  const broken = createWebPlugins({ directory: f.directory, getContext: () => ({ request: { get: async () => { throw new Error('sensitive details'); } } }) });
  const result = await broken.dispatch({ action: 'verify', plugin_id: 'mail', function_name: 'list', expected_revision: 1, arguments: { after: 'today' } });
  assert.equal(result.status, 'execution_failed');
  assert.ok(!JSON.stringify(result).includes('sensitive'));
  assert.deepEqual((await f.plugins.dispatch({ action: 'get', plugin_id: 'mail' })).plugin.verified, {});
});
test('no sigue enlaces simbólicos de recetas', async t => {
  const f = await fixture(t); await f.save();
  await symlink(path.join(f.directory, 'mail.json'), path.join(f.directory, 'other.json'));
  await assert.rejects(f.plugins.dispatch({ action: 'get', plugin_id: 'other' }), { code: 'plugin_storage_error' });
});
test('candidatos aislados: verifica antes de promover y protege la revisión publicada', async t => {
  const f = await fixture(t); await f.save(); await f.run();
  const candidate = createWebPlugins({ directory:path.join(f.directory,'candidates','job_1'),getContext:()=>({ request:{get:async()=>({status:()=>200,headers:()=>({'content-type':'application/json'}),body:async()=>Buffer.from(JSON.stringify({messages:[{id:'two',subject:'Candidato'}]})),dispose:async()=>{}})}}) });
  const changed={...manifest(),description:'Cobertura de lectura',capabilities:['Listar correos'],limitations:['Solo lectura']};
  await candidate.dispatch({action:'save',manifest:changed,expected_revision:0});
  const promote=()=>f.plugins.dispatch({action:'promote',candidate_id:'job_1',plugin_id:'mail',expected_revision:1});
  assert.equal((await promote()).status,'verification_required');
  assert.equal((await f.plugins.dispatch({action:'get',plugin_id:'mail'})).plugin.revision,1);
  await candidate.dispatch({action:'verify',plugin_id:'mail',function_name:'list',expected_revision:1,arguments:{after:'today'}});
  assert.equal((await promote()).status,'promoted');
  const published=(await f.plugins.dispatch({action:'get',plugin_id:'mail'})).plugin;
  assert.equal(published.revision,2);assert.equal(published.verified.list.revision,2);
  assert.deepEqual(published.manifest.limitations,['Solo lectura']);
  assert.equal((await promote()).status,'revision_conflict');
});
test('eliminar y recrear no permite sobrescribir con una revisión antigua', async t=>{
  const f=await fixture(t);await f.save();
  assert.equal((await f.plugins.dispatch({action:'delete',plugin_id:'mail',expected_revision:2})).status,'revision_conflict');
  assert.equal((await f.plugins.dispatch({action:'delete',plugin_id:'mail',expected_revision:1})).status,'deleted');
  assert.deepEqual((await f.plugins.dispatch({action:'list'})).plugins,[]);
  assert.equal((await f.plugins.dispatch({action:'get',plugin_id:'mail'})).status,'plugin_not_found');
  assert.equal((await f.save()).plugin.revision,3);
  assert.equal((await f.save(manifest(),1)).status,'revision_conflict');
});
test('una construcción anterior no recrea un plugin eliminado mientras trabajaba', async t=>{
  const f=await fixture(t);
  const candidate=createWebPlugins({directory:path.join(f.directory,'candidates','job_1'),getContext:()=>({request:{get:async()=>({status:()=>200,headers:()=>({'content-type':'application/json'}),body:async()=>Buffer.from(JSON.stringify({messages:[{id:'x',subject:'Prueba'}]})),dispose:async()=>{}})}})});
  await candidate.dispatch({action:'save',manifest:manifest(),expected_revision:0});
  await candidate.dispatch({action:'verify',plugin_id:'mail',function_name:'list',expected_revision:1,arguments:{after:'today'}});
  await f.save();await f.plugins.dispatch({action:'delete',plugin_id:'mail',expected_revision:1});
  assert.equal((await f.plugins.dispatch({action:'promote',candidate_id:'job_1',plugin_id:'mail',expected_revision:0})).status,'revision_conflict');
  assert.equal((await f.plugins.dispatch({action:'get',plugin_id:'mail'})).status,'plugin_not_found');
});
