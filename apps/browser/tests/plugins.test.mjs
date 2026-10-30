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
  const executed = await f.run('run');
  assert.deepEqual(executed.data, [{ id: 'one', subject: 'Prueba' }]);
  assert.equal(executed.plugin_name, 'Correo de prueba');
  assert.equal(executed.plugin_id, 'mail');
  assert.equal(executed.function_name, 'list');
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
test('preflight valida verificación y ejecución sin obtener contexto ni ejecutar', async t => {
  const f = await fixture(t); await f.save();
  const inspect=createWebPlugins({directory:f.directory,getContext:()=>{throw new Error('El preflight no debe obtener contexto');}});
  const base={action:'preflight',plugin_id:'mail',function_name:'list',expected_revision:1,arguments:{after:'today'}};
  const prepared=await inspect.dispatch({...base,operation:'verify'});
  assert.equal(prepared.status,'ready');assert.equal(prepared.executed,false);
  assert.equal(prepared.plugin_name,'Correo de prueba');
  assert.equal((await inspect.dispatch({...base,operation:'run'})).status,'verification_required');
  assert.equal((await inspect.dispatch({...base,operation:'verify',arguments:{}})).status,'input_required');
  await assert.rejects(inspect.dispatch({...base,operation:'review'}),{code:'invalid_plugin_arguments'});
  assert.equal(f.calls.length,0);
  assert.deepEqual((await f.plugins.dispatch({action:'get',plugin_id:'mail'})).plugin.verified,{});
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
  for (const [args, status] of [[{}, 'input_required'], [{ after: 'a', extra: true }, 'invalid_arguments'], [{ after: 0 }, 'invalid_arguments']]) {
    const result = await f.run('verify', args);
    assert.equal(result.status, status);
    assert.equal(result.executed, false);
  }
  assert.equal(f.calls.length, count);
});

test('descriptores tipados rechazan contratos ambiguos y conservan parámetros anteriores', () => {
  const fn = manifest().functions[0];
  const typed = spec => ({ ...manifest(), functions: [{ ...fn, parameters: { after: spec } }] });
  assert.equal(validateWebPlugin(typed({type:'string',label:'Desde',description:'Fecha inicial',required:false,maxLength:80})).id, 'mail');
  for (const spec of [
    {type:'string',unknown:true}, {type:'date'}, {type:'integer',required:'false'},
    {type:'string',minLength:0}, {type:'string',minLength:10,maxLength:2},
    {type:'boolean',minimum:0}, {type:'integer',minimum:.5},
    {type:'number',minimum:2,maximum:1}, {type:'number',maximum:Number.MAX_SAFE_INTEGER+1},
    {type:'string',enum:[]}, {type:'string',enum:['a','a']}, {type:'string',enum:[true]},
    {type:'integer',enum:[1,1.5]}, {type:'boolean',enum:[null]},
  ]) assert.throws(() => validateWebPlugin(typed(spec)), {code:'invalid_plugin'});
  assert.throws(() => validateWebPlugin({...typed({type:'string',required:false}),functions:[{...fn,path:'/items/{after}',query:{},parameters:{after:{type:'string',required:false}}}]}), {code:'invalid_plugin'});
  assert.equal(validateWebPlugin(manifest()).functions[0].parameters.after, 'string');
});

test('faltan datos: no abre contexto ni pierde evidencia y permite completar después', async t => {
  const f = await fixture(t); await f.save(); await f.run();
  const before = (await f.plugins.dispatch({action:'get',plugin_id:'mail'})).plugin;
  let opened = 0;
  const plugins = createWebPlugins({directory:f.directory,getContext:() => { opened++; throw new Error('No debe abrir'); }});
  const result = await plugins.dispatch({action:'verify',plugin_id:'mail',function_name:'list',expected_revision:1,arguments:{}});
  assert.equal(result.status,'input_required'); assert.equal(result.executed,false); assert.equal(opened,0);
  assert.deepEqual(result.fields,[{name:'after',required:true,type:'string',reason:'required'}]);
  assert.deepEqual((await f.plugins.dispatch({action:'get',plugin_id:'mail'})).plugin,before);
  assert.equal((await f.run('run',{after:'fecha indicada'})).status,'ok');
  assert.equal(f.calls.length,2);
});

test('opcionales se omiten de query y tipos, límites y elecciones se validan sin efectos', async t => {
  const f = await fixture(t);
  const value = manifest();
  value.functions[0].parameters = {
    after:{type:'string',label:'Desde',required:false,minLength:2,maxLength:12},
    count:{type:'integer',label:'Cantidad',minimum:0,maximum:20},
    include:{type:'boolean',label:'Incluir leídos'},
    ratio:{type:'number',label:'Proporción',minimum:-1,maximum:1},
    sort:{type:'string',label:'Orden',enum:['recent','old']},
  };
  value.functions[0].query = {after:'{after}',count:'{count}',include:'{include}',ratio:'{ratio}',sort:'{sort}'};
  await f.save(value);
  const args = {count:0,include:false,ratio:-.5,sort:'recent'};
  assert.equal((await f.run('verify',args)).status,'ok');
  const query = new URL(f.calls[0].url).searchParams;
  assert.equal(query.has('after'),false); assert.equal(query.get('count'),'0'); assert.equal(query.get('include'),'false'); assert.equal(query.get('ratio'),'-0.5');
  for (const patch of [{count:21},{count:1.5},{include:'false'},{ratio:2},{sort:'invented'},{after:'a'},{after:'x'.repeat(13)},{after:null}]) {
    const invalid = await f.run('verify',{...args,...patch});
    assert.equal(invalid.status,'invalid_arguments'); assert.equal(invalid.executed,false);
    assert.ok(invalid.fields.length);
    assert.ok(!JSON.stringify(invalid).includes('invented'));
  }
  assert.equal(f.calls.length,1);
});

test('función puntual programable pide el mensaje y ejecuta solo con los datos completos', async t => {
  const f = await fixture(t);
  const value = {id:'mail',name:'Mensajes',origin:'https://example.com',functions:[{
    name:'list',description:'Enviar un mensaje de prueba',parameters:{
      recipient:{type:'string',label:'Destinatario',description:'Persona indicada por el usuario'},
      message:{type:'string',label:'Mensaje',description:'Texto exacto que se enviará',maxLength:1000},
      urgent:{type:'boolean',label:'Urgente',required:false},
    },transport:'program',path:'/',result_type:'object',code:'await context.send(args); return {sent:true};',
  }]};
  await f.save(value);
  let opened = 0; const sent = [];
  const plugins = createWebPlugins({directory:f.directory,getContext:() => {opened++;return {newPage:async()=>({close:async()=>{}}),send:async args=>sent.push(args)};}});
  const run = arguments_ => plugins.dispatch({action:'verify',plugin_id:'mail',function_name:'list',expected_revision:1,program_allowed:true,arguments:arguments_});
  const missing = await run({recipient:'Cata'});
  assert.equal(missing.status,'input_required'); assert.equal(missing.fields[0].name,'message'); assert.equal(missing.fields[0].label,'Mensaje'); assert.equal(opened,0); assert.deepEqual(sent,[]);
  const malformed = await run({recipient:'Cata',message:123});
  assert.equal(malformed.status,'invalid_arguments'); assert.equal(opened,0);
  assert.equal((await run({recipient:'Cata',message:'Llegaré a las 8',urgent:false})).status,'ok');
  assert.deepEqual(sent,[{recipient:'Cata',message:'Llegaré a las 8',urgent:false}]); assert.equal(opened,1);
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
  const changed={...manifest(),description:'Cobertura de lectura',capabilities:['Listar correos'],limitations:['Solo lectura'],functions:[{...manifest().functions[0],path:'/api/new-messages'}]};
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
test('copiar un candidato a otro trabajo no copia verificaciones', async t => {
  const f = await fixture(t);
  const context = () => ({ request: { get: async () => ({ status: () => 200, headers: () => ({'content-type':'application/json'}), body: async () => Buffer.from(JSON.stringify({messages:[{id:'one',subject:'Prueba'}]})), dispose: async () => {} }) } });
  const source = createWebPlugins({directory:path.join(f.directory,'candidates','job_1'),getContext:context});
  const target = createWebPlugins({directory:path.join(f.directory,'candidates','job_2'),getContext:context});
  await source.dispatch({action:'save',manifest:manifest(),expected_revision:0});
  await source.dispatch({action:'verify',plugin_id:'mail',function_name:'list',expected_revision:1,arguments:{after:'today'}});
  const previous = (await source.dispatch({action:'get',plugin_id:'mail'})).plugin;
  const copied = (await target.dispatch({action:'save',manifest:previous.manifest,expected_revision:0})).plugin;
  assert.deepEqual(copied.manifest, previous.manifest);
  assert.deepEqual(copied.verified, {});
  assert.equal((await f.plugins.dispatch({action:'promote',candidate_id:'job_2',plugin_id:'mail',expected_revision:0})).status,'verification_required');
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
test('bloques programables guardan, verifican y ejecutan acciones sin repetir fallos', async t => {
  const directory = await mkdtemp(path.join(tmpdir(), 'gennie-program-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const effects = [];
  const plugins = createWebPlugins({ directory, getContext: () => ({newPage: async () => ({close: async () => {}}), effect: async value => effects.push(value)}) });
  const value = { id:'action',name:'Acción',origin:'https://example.com',functions:[{name:'archive',description:'Archivar',parameters:{id:'string'},transport:'program',path:'/',result_type:'object',code:'await context.effect(args.id); return {ok:true};'}] };
  assert.equal((await plugins.dispatch({action:'save',manifest:value,expected_revision:0})).status, 'saved');
  const run = action => plugins.dispatch({action,plugin_id:'action',function_name:'archive',expected_revision:1,arguments:{id:'example'},program_allowed:true});
  assert.equal((await plugins.dispatch({action:'verify',plugin_id:'action',function_name:'archive',expected_revision:1,arguments:{id:'example'}})).status, 'permission_required');
  assert.equal((await run('run')).status, 'verification_required');
  assert.equal((await run('verify')).status, 'ok');
  assert.deepEqual(effects, ['example']);
  assert.equal((await run('run')).status, 'ok');
  assert.deepEqual(effects, ['example','example']);
  value.functions[0].code='await context.effect(args.id); throw new Error("after effect");';
  await plugins.dispatch({action:'save',manifest:value,expected_revision:1});
  const result = await plugins.dispatch({action:'verify',plugin_id:'action',function_name:'archive',expected_revision:2,arguments:{id:'example'},program_allowed:true});
  assert.equal(result.status,'result_uncertain');
  assert.equal(effects.length,3);
  assert.deepEqual((await plugins.dispatch({action:'get',plugin_id:'action'})).plugin.verified,{});
});
test('guarda y recupera bloques grandes dentro del contrato ampliado', async t => {
  const f = await fixture(t);
  const value = { id:'large', name:'Bloque largo', origin:'https://example.com', functions:[{name:'large',description:'Prueba',parameters:{},transport:'program',path:'/',result_type:'object',code:' '.repeat(40000)+'return {ok:true};'}] };
  const result = await f.plugins.dispatch({action:'save',manifest:value,expected_revision:0});
  assert.equal(result.status,'saved');
  assert.equal((await f.plugins.dispatch({action:'get',plugin_id:'large'})).plugin.manifest.functions[0].code, value.functions[0].code);
  value.functions[0].code=' '.repeat(48001);
  assert.throws(() => validateWebPlugin(value), {code:'invalid_plugin'});
});
test('admite más de 20 funciones y más de 20 parámetros', async t => {
  const f = await fixture(t);
  const parameters = Object.fromEntries(Array.from({length: 25}, (_, index) => [`p${index}`, 'string']));
  const functions = Array.from({length: 25}, (_, index) => ({name:`f${index}`,description:'Prueba',parameters,transport:'program',path:'/',result_type:'object',code:'return args;'}));
  const value = { id:'many', name:'Muchas funciones', origin:'https://example.com', functions };
  assert.equal((await f.plugins.dispatch({action:'save',manifest:value,expected_revision:0})).status,'saved');
  assert.equal((await f.plugins.dispatch({action:'get',plugin_id:'many'})).plugin.manifest.functions.length,25);
  const runnable = createWebPlugins({directory:f.directory,getContext:() => ({newPage:async () => ({close:async()=>{}})})});
  const args = Object.fromEntries(Object.keys(parameters).map(key => [key,'value']));
  const output = await runnable.dispatch({action:'verify',plugin_id:'many',function_name:'f0',expected_revision:1,arguments:args,program_allowed:true});
  assert.equal(output.status,'ok');
  assert.deepEqual(output.data,args);
});

async function recoveryFixture(t, code) {
  const directory = await mkdtemp(path.join(tmpdir(), 'gennie-runtime-'));
  t.after(() => rm(directory, {recursive:true,force:true}));
  const state = { writes:0, reads:0, closed:0, confirmed:false };
  const context = { state, newPage: async () => ({close:async () => { state.closed++; }}) };
  const plugins = createWebPlugins({directory,getContext:() => context});
  await plugins.dispatch({action:'save',expected_revision:0,manifest:{id:'recover',name:'Prueba',origin:'https://example.com',functions:[{name:'action',description:'Prueba',parameters:{},transport:'program',path:'/',result_type:'object',code}]}});
  return { directory, state, plugins, run: (action='verify') => plugins.dispatch({action,plugin_id:'recover',function_name:'action',expected_revision:1,arguments:{},program_allowed:true}) };
}
test('recupera lecturas transitorias y confirma una escritura cuyo paso final falló', async t => {
  const f = await recoveryFixture(t, `
    await runtime.read('load', async()=>{ context.state.reads++; if(context.state.reads<2){ const e=new Error('private detail');e.name='TimeoutError';throw e; } });
    return runtime.action('archive',async()=>{context.state.writes++;context.state.confirmed=true;throw new TypeError('private detail');},async()=>context.state.confirmed);
  `);
  const result = await f.run();
  assert.equal(result.status,'ok');
  assert.equal(result.data.recovered,true);
  assert.equal(f.state.reads,2);
  assert.equal(f.state.writes,1);
  assert.equal((await f.run('run')).data.already_satisfied,true);
  assert.equal(f.state.writes,1);
  assert.equal(f.state.closed,2);
});
test('una poscondición no confirmada conserva el paso y nunca repite la escritura', async t => {
  const f = await recoveryFixture(t, `return runtime.action('archive',async()=>{context.state.writes++;throw new TypeError('private detail');},async()=>false);`);
  const result = await f.run();
  assert.equal(result.status,'result_uncertain');
  assert.deepEqual(result.diagnostic,{stage:'archive',phase:'verify',action_status:'uncertain',read_attempts:1,action_error_code:'type_error',code:'postcondition_failed'});
  assert.equal(f.state.writes,1);
  assert.equal(f.state.closed,1);
  assert.ok(!JSON.stringify(result).includes('private detail'));
  assert.deepEqual((await f.plugins.dispatch({action:'get',plugin_id:'recover'})).plugin.verified,{});
});
test('acota las lecturas fallidas y distingue un error de código de un timeout', async t => {
  const f = await recoveryFixture(t, `await runtime.read('load',async()=>{context.state.reads++;const e=new Error('private detail');e.name='TimeoutError';throw e;});return {};`);
  const result = await f.run();
  assert.equal(result.diagnostic.code,'timeout');
  assert.equal(result.diagnostic.stage,'load');
  assert.equal(f.state.reads,3);
  assert.equal(f.state.writes,0);
  const invalid = await recoveryFixture(t, `await runtime.read('decode',async()=>{context.state.reads++;throw new TypeError('private detail');});return {};`);
  assert.equal((await invalid.run()).diagnostic.code,'type_error');
  assert.equal(invalid.state.reads,1);
});
test('tolera la consistencia eventual al comprobar el efecto sin repetirlo', async t => {
  const f = await recoveryFixture(t, `return runtime.action('archive',async()=>{context.state.writes++;},async()=>{context.state.reads++;return context.state.reads>=3;});`);
  const result = await f.run();
  assert.equal(result.status,'ok');
  assert.equal(f.state.writes,1);
  assert.equal(f.state.reads,3);
});
test('un fallo de cierre se comunica sin ocultar el resultado de la acción', async t => {
  const f = await recoveryFixture(t, 'return {verified:true};');
  const plugins = createWebPlugins({directory:f.directory,getContext:()=>({newPage:async()=>({close:async()=>{throw new Error('private cleanup detail');}})})});
  const result = await plugins.dispatch({action:'verify',plugin_id:'recover',function_name:'action',expected_revision:1,arguments:{},program_allowed:true});
  assert.equal(result.status,'ok');
  assert.equal(result.data.verified,true);
  assert.equal(result.cleanup_status,'failed');
  assert.ok(!JSON.stringify(result).includes('private cleanup detail'));
});

test('prepare valida revisión, verificación y argumentos sin abrir contexto ni ejecutar', async t => {
  const f = await fixture(t);
  await f.save();
  assert.equal((await f.run('prepare')).status, 'verification_required');
  assert.equal(f.calls.length, 0);
  await f.run('verify');
  const count = f.calls.length;
  const ready = await f.run('prepare');
  assert.equal(ready.status, 'ready');
  assert.equal(ready.executed, false);
  assert.equal((await f.run('prepare', {})).status, 'input_required');
  assert.equal(f.calls.length, count);
  await f.save(manifest(), 1);
  assert.equal((await f.run('prepare', { after: 'today' }, 2)).status, 'verification_required');
  assert.equal(f.calls.length, count);
});
