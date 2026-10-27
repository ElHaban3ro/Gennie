// Reproduce el cambio de identidad de nth() tras archivar, sin cuentas reales.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createWebPlugins } from '../../apps/browser/src/plugins.mjs';

const root = path.join(process.env.HOME, 'Library/Application Support/Gennie/browser');
const runtimePath = path.join(root, 'runtime-patchright-1.64.2');
const require = createRequire(path.join(runtimePath, 'package.json'));
const { chromium } = require('patchright');
const directory = await mkdtemp(path.join(tmpdir(), 'gennie-action-recovery-'));
let browser;
try {
  browser = await chromium.launch({ headless:true, executablePath:path.join(runtimePath,'Gennie Web.app/Contents/MacOS/Google Chrome for Testing') });
  const context = await browser.newContext();
  const archived = new Set();
  let writes = 0;
  await context.exposeFunction('fixtureArchive', id => { writes++; archived.add(id); });
  await context.exposeFunction('fixtureArchived', id => archived.has(id));
  const html = `<button onclick="const row=document.querySelector('[role=row]'); fixtureArchive(row.dataset.id).then(()=>row.remove())">Archivar</button><div role=row data-id=one><input type=checkbox aria-label=Seleccionar></div><div role=row data-id=two>Segundo</div>`;
  const setup = `await page.setContent(${JSON.stringify(html)}); const row=page.locator('[role=row]').nth(0);`;
  const action = `await row.getByRole('checkbox').check(); await page.getByRole('button',{name:'Archivar',exact:true}).click(); await row.waitFor({state:'detached',timeout:150});`;
  const plugins = createWebPlugins({ directory, getContext: () => context });
  const manifest = { id:'recovery',name:'Prueba local',origin:'https://example.com',functions:[
    {name:'old',description:'Patrón original',parameters:{},transport:'program',path:'/',result_type:'object',code:`${setup}${action}return {verified:true};`},
    {name:'recover',description:'Verificación por identidad',parameters:{},transport:'program',path:'/',result_type:'object',code:`${setup}return runtime.action('archive',async()=>{${action}},async()=>await page.evaluate(()=>fixtureArchived('one')));`},
  ]};
  assert.equal((await plugins.dispatch({action:'save',manifest,expected_revision:0})).status,'saved');
  const run = (function_name, action='verify') => plugins.dispatch({action,plugin_id:'recovery',function_name,expected_revision:1,arguments:{},program_allowed:true});
  assert.equal((await run('old')).status,'result_uncertain');
  assert.equal(writes,1);
  assert.ok(archived.has('one'));
  archived.clear(); writes=0;
  const recovered = await run('recover');
  assert.equal(recovered.status,'ok',JSON.stringify(recovered));
  assert.equal(recovered.data.verified,true);
  assert.equal(writes,1);
  const repeated = await run('recover','run');
  assert.equal(repeated.status,'ok');
  assert.equal(repeated.data.already_satisfied,true);
  assert.equal(writes,1);
  assert.equal(context.pages().length,0);
  console.log(JSON.stringify({ok:true,original_false_failure_reproduced:true,postcondition_recovered:true,write_executed_once:true,pages_closed:true}));
} finally {
  await browser?.close();
  await rm(directory,{recursive:true,force:true});
}
