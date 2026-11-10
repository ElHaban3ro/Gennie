// Recetas y bloques ejecutables reutilizables de plugins.
import * as pluginFs from 'node:fs/promises';
import pluginPath from 'node:path';
import { randomUUID as pluginUUID, createHash as pluginHash } from 'node:crypto';

const PLUGIN_LIMIT = 128 * 1024;
const pluginId = value => typeof value === 'string' && /^[a-z][a-z0-9_]{0,63}$/.test(value);
const pluginObject = value => value && typeof value === 'object' && !Array.isArray(value);
function pluginFail(code) { const error = new Error(code); error.code = code; throw error; }
function pluginKeys(value, allowed, required = []) {
  if (!pluginObject(value) || Object.keys(value).some(key => !allowed.includes(key)) || required.some(key => !Object.hasOwn(value, key))) pluginFail('invalid_plugin');
}
function pluginText(value, max = 2000) {
  if (typeof value !== 'string' || !value.length || value.length > max || value.includes('\0')) pluginFail('invalid_plugin');
}
function pluginURL(value) {
  pluginText(value, 4000);
  let url;
  try { url = new URL(value); } catch { pluginFail('invalid_plugin'); }
  if (!['https:', 'http:'].includes(url.protocol) || url.username || url.password || url.hash) pluginFail('invalid_plugin');
  return url;
}
function pluginPointer(value) {
  if (typeof value !== 'string' || value.length > 500 || (value !== '' && !value.startsWith('/')) || /~(?![01])/u.test(value)) pluginFail('invalid_plugin');
}
function parameterSpec(value) {
  return typeof value === 'string' ? { type: value, required: true } : { required: true, ...value };
}
function parameterIssue(spec, value) {
  if (value === undefined) return spec.required ? 'required' : null;
  if (spec.type === 'string') {
    if (typeof value !== 'string' || value.includes('\0')) return 'type';
    const length = [...value].length;
    if (length < (spec.minLength ?? 1)) return 'minLength';
    if (length > (spec.maxLength ?? 1000)) return 'maxLength';
  } else if (spec.type === 'boolean') {
    if (typeof value !== 'boolean') return 'type';
  } else {
    if (typeof value !== 'number' || !Number.isFinite(value) || (spec.type === 'integer' && !Number.isSafeInteger(value))) return 'type';
    if (value < (spec.minimum ?? -Number.MAX_SAFE_INTEGER)) return 'minimum';
    if (value > (spec.maximum ?? Number.MAX_SAFE_INTEGER)) return 'maximum';
  }
  if (spec.enum && !spec.enum.includes(value)) return 'enum';
  return null;
}
function validateParameter(value) {
  if (typeof value === 'string') {
    if (!['string', 'integer'].includes(value)) pluginFail('invalid_plugin');
    return;
  }
  pluginKeys(value, ['type', 'label', 'description', 'required', 'enum', 'minLength', 'maxLength', 'minimum', 'maximum'], ['type']);
  if (!['string', 'integer', 'number', 'boolean'].includes(value.type) || (value.required !== undefined && typeof value.required !== 'boolean')) pluginFail('invalid_plugin');
  if (value.label !== undefined) pluginText(value.label, 120);
  if (value.description !== undefined) pluginText(value.description, 1000);
  const spec = parameterSpec(value);
  for (const key of ['minLength', 'maxLength']) if (value[key] !== undefined && (spec.type !== 'string' || !Number.isSafeInteger(value[key]) || value[key] < 1 || value[key] > 1000)) pluginFail('invalid_plugin');
  if ((spec.minLength ?? 1) > (spec.maxLength ?? 1000)) pluginFail('invalid_plugin');
  for (const key of ['minimum', 'maximum']) if (value[key] !== undefined && (!['integer', 'number'].includes(spec.type) || !Number.isFinite(value[key]) || Math.abs(value[key]) > Number.MAX_SAFE_INTEGER || (spec.type === 'integer' && !Number.isSafeInteger(value[key])))) pluginFail('invalid_plugin');
  if ((spec.minimum ?? -Number.MAX_SAFE_INTEGER) > (spec.maximum ?? Number.MAX_SAFE_INTEGER)) pluginFail('invalid_plugin');
  if (value.enum !== undefined) {
    if (!Array.isArray(value.enum) || !value.enum.length || value.enum.length > 32 || new Set(value.enum).size !== value.enum.length) pluginFail('invalid_plugin');
    for (const item of value.enum) if (parameterIssue({ ...spec, enum: undefined }, item)) pluginFail('invalid_plugin');
  }
}
function pluginArguments(fn, args) {
  if (!pluginObject(args) || Object.keys(args).some(key => !Object.hasOwn(fn.parameters, key))) return { status: 'invalid_arguments', executed: false, fields: [], message: 'Usa únicamente los parámetros declarados en la función.' };
  const fields = [];
  for (const [name, value] of Object.entries(fn.parameters)) {
    const spec = parameterSpec(value);
    const reason = parameterIssue(spec, Object.hasOwn(args, name) ? args[name] : undefined);
    if (reason) fields.push({ name, ...spec, reason });
  }
  if (!fields.length) return null;
  return { status: fields.every(field => field.reason === 'required') ? 'input_required' : 'invalid_arguments', executed: false, fields, message: 'Faltan datos o necesitan corrección. Pide solo lo necesario antes de ejecutar la función; no inventes valores.' };
}
export function validateWebPlugin(manifest) {
  pluginKeys(manifest, ['id', 'name', 'origin', 'login_url', 'login_urls', 'description', 'capabilities', 'limitations', 'functions'], ['id', 'name', 'origin', 'functions']);
  if (!pluginId(manifest.id)) pluginFail('invalid_plugin');
  pluginText(manifest.name, 200);
  if (manifest.description !== undefined) pluginText(manifest.description, 4000);
  for (const field of ['capabilities', 'limitations']) if (manifest[field] !== undefined) {
    if (!Array.isArray(manifest[field]) || manifest[field].length > 20) pluginFail('invalid_plugin');
    for (const text of manifest[field]) pluginText(text, 1000);
  }
  if (pluginURL(manifest.origin).origin !== manifest.origin) pluginFail('invalid_plugin');
  if (manifest.login_url !== undefined && pluginURL(manifest.login_url).search) pluginFail('invalid_plugin');
  if (manifest.login_urls !== undefined) {
    if (!Array.isArray(manifest.login_urls) || manifest.login_urls.length > 8) pluginFail('invalid_plugin');
    for (const url of manifest.login_urls) if (pluginURL(url).search) pluginFail('invalid_plugin');
  }
  if (!Array.isArray(manifest.functions) || !manifest.functions.length) pluginFail('invalid_plugin');
  const names = new Set();
  for (const fn of manifest.functions) {
    pluginKeys(fn, ['name', 'description', 'parameters', 'transport', 'path', 'query', 'result_type', 'result_pointer', 'fields', 'selector', 'ready_selector', 'login_selector', 'code'], ['name', 'description', 'parameters', 'transport', 'path', 'result_type']);
    if (!pluginId(fn.name) || names.has(fn.name)) pluginFail('invalid_plugin');
    names.add(fn.name); pluginText(fn.description, 1000);
    if (!pluginObject(fn.parameters)) pluginFail('invalid_plugin');
    for (const [key, spec] of Object.entries(fn.parameters)) {
      if (!pluginId(key)) pluginFail('invalid_plugin');
      validateParameter(spec);
    }
    pluginText(fn.path);
    if (!fn.path.startsWith('/') || fn.path.startsWith('//') || /[?#\\]/u.test(fn.path)) pluginFail('invalid_plugin');
    if (fn.query !== undefined && (!pluginObject(fn.query))) pluginFail('invalid_plugin');
    for (const [key, value] of Object.entries(fn.query || {})) {
      if (!/^[a-zA-Z0-9_.-]{1,100}$/.test(key) || /token|secret|password|cookie|authorization|session|api.?key/i.test(key)) pluginFail('invalid_plugin');
      pluginText(value);
    }
    const used = new Set();
    for (const template of [fn.path, ...Object.values(fn.query || {})]) {
      const remaining = template.replace(/\{([a-z][a-z0-9_]{0,63})\}/g, (_, key) => {
        if (!Object.hasOwn(fn.parameters, key)) pluginFail('invalid_plugin');
        used.add(key); return '';
      });
      if (/[{}]/u.test(remaining)) pluginFail('invalid_plugin');
    }
    for (const match of fn.path.matchAll(/\{([a-z][a-z0-9_]{0,63})\}/g)) if (!parameterSpec(fn.parameters[match[1]]).required) pluginFail('invalid_plugin');
    if (!['program','gmail_inbox','gmail_print','gmail_archive','gmail_importance'].includes(fn.transport) && Object.keys(fn.parameters).some(key => !used.has(key))) pluginFail('invalid_plugin');
    if (fn.transport === 'program') {
      pluginText(fn.code, 48000);
      if (!['array', 'object', 'string', 'number', 'boolean'].includes(fn.result_type) || fn.query !== undefined || fn.fields !== undefined || fn.selector !== undefined || fn.ready_selector !== undefined || fn.result_pointer !== undefined) pluginFail('invalid_plugin');
      // Una expresión flecha aislada devuelve la función como expresión descartada:
      // el cuerpo no se ejecuta. Se comprueba sintaxis, sin evaluar código del plugin.
      if (/^\s*(?:async\s+)?(?:\([^)]*\)|[a-zA-Z_$][\w$]*)\s*=>/u.test(fn.code)) {
        try { new Function(`return (${fn.code.trim().replace(/;\s*$/u, '')}\n)`); pluginFail('invalid_program_body'); }
        catch (error) { if (error.code === 'invalid_program_body') throw error; }
      }
      try { new (Object.getPrototypeOf(async function(){}).constructor)('context', 'page', 'args', 'origin', 'runtime', fn.code); } catch { pluginFail('invalid_plugin'); }
    } else if (fn.transport === 'http') {
      if (fn.code !== undefined) pluginFail('invalid_plugin');
      if (!['array', 'object', 'string', 'number', 'boolean'].includes(fn.result_type) || fn.selector !== undefined || fn.ready_selector !== undefined || fn.login_selector !== undefined) pluginFail('invalid_plugin');
      if (fn.result_pointer !== undefined) pluginPointer(fn.result_pointer);
    } else if (fn.transport === 'gmail_inbox') {
      const keys=Object.keys(fn.parameters);
      if (manifest.origin !== 'https://mail.google.com' || fn.path !== '/mail/u/0/' || fn.result_type !== 'object' || keys.length !== 1 || keys[0] !== 'pagina' || parameterSpec(fn.parameters.pagina).type !== 'integer' || !parameterSpec(fn.parameters.pagina).required || ['code','query','fields','selector','ready_selector','login_selector','result_pointer'].some(key => fn[key] !== undefined)) pluginFail('invalid_plugin');
    } else if (fn.transport === 'gmail_print' || fn.transport === 'gmail_archive' || fn.transport === 'gmail_importance') {
      const keys=Object.keys(fn.parameters);
      if (manifest.origin !== 'https://mail.google.com' || fn.path !== '/mail/u/0/' || fn.result_type !== 'object' || keys.length !== 1 || keys[0] !== 'id' || parameterSpec(fn.parameters.id).type !== 'string' || !parameterSpec(fn.parameters.id).required || ['code','query','fields','selector','ready_selector','login_selector','result_pointer'].some(key => fn[key] !== undefined)) pluginFail('invalid_plugin');
    } else if (fn.transport === 'observe') {
      if (fn.code !== undefined || fn.query !== undefined || fn.fields !== undefined || fn.ready_selector !== undefined || fn.login_selector !== undefined || fn.result_pointer !== undefined || !['array','string'].includes(fn.result_type)) pluginFail('invalid_plugin');
      pluginText(fn.selector, 1000);
    } else if (fn.transport === 'page') {
      if (fn.code !== undefined) pluginFail('invalid_plugin');
      if (!['array', 'string'].includes(fn.result_type) || fn.result_pointer !== undefined) pluginFail('invalid_plugin');
      pluginText(fn.selector, 1000);
      if (fn.ready_selector !== undefined) pluginText(fn.ready_selector, 1000);
      if (fn.login_selector !== undefined) pluginText(fn.login_selector, 1000);
    } else pluginFail('invalid_plugin');
    if (fn.fields !== undefined) {
      if (!['object', 'array'].includes(fn.result_type) || !pluginObject(fn.fields) || !Object.keys(fn.fields).length || Object.keys(fn.fields).length > 20) pluginFail('invalid_plugin');
      for (const [key, value] of Object.entries(fn.fields)) {
        if (!pluginId(key)) pluginFail('invalid_plugin');
        if (fn.transport === 'http') pluginPointer(value); else pluginText(value, 1000);
      }
    }
  }
  if (Buffer.byteLength(JSON.stringify(manifest)) > PLUGIN_LIMIT / 2) pluginFail('invalid_plugin');
  return manifest;
}
function pluginAt(value, pointer = '') {
  for (const token of pointer === '' ? [] : pointer.slice(1).split('/')) {
    const key = token.replace(/~1/g, '/').replace(/~0/g, '~');
    if (!value || !Object.hasOwn(value, key)) pluginFail('site_changed');
    value = value[key];
  }
  return value;
}
function pluginType(value) { return Array.isArray(value) ? 'array' : value === null ? 'null' : typeof value; }
function pluginLogin(manifest, value) {
  const url = new URL(value);
  return (manifest.login_urls || []).some(candidate => {
    const login = new URL(candidate);
    return url.origin === login.origin && (url.pathname === login.pathname || (login.pathname !== '/' && url.pathname.startsWith(login.pathname.replace(/\/$/, '') + '/')));
  });
}
function pluginAuth(manifest) {
  return { status: 'authentication_required', ...(manifest.login_url ? { login_url: manifest.login_url } : {}), message: 'Esta operación necesita una sesión válida. El usuario puede iniciar sesión en Gennie Web y volver a intentarla.' };
}
function pluginTarget(manifest, fn, args) {
  const render = (template, encode) => template.replace(/\{([a-z][a-z0-9_]{0,63})\}/g, (_, key) => {
    const value = String(args[key]);
    if (encode && (value === '.' || value === '..')) pluginFail('invalid_plugin_arguments');
    return encode ? encodeURIComponent(value) : value;
  });
  const url = pluginURL(manifest.origin + render(fn.path, true));
  if (url.origin !== manifest.origin) pluginFail('invalid_plugin_arguments');
  for (const [key, value] of Object.entries(fn.query || {})) {
    if ([...value.matchAll(/\{([a-z][a-z0-9_]{0,63})\}/g)].some(match => !Object.hasOwn(args, match[1]))) continue;
    url.searchParams.set(key, render(value, false));
  }
  return url.href;
}

function programErrorCode(error) {
  if (error?.code === 'postcondition_failed') return 'postcondition_failed';
  if (error?.name === 'TimeoutError') return 'timeout';
  if (['ECONNRESET', 'ECONNREFUSED', 'ETIMEDOUT', 'EPIPE', 'ENOTFOUND', 'EAI_AGAIN'].includes(error?.code)) return 'connection_failed';
  if (error instanceof TypeError) return 'type_error';
  if (error instanceof ReferenceError) return 'reference_error';
  return 'program_error';
}

// Busca por identidad estable sin abrir una conversación. La ausencia solo se
// acepta cuando Gmail muestra explícitamente la vista de resultados vacíos.
async function gmailLocate(context, id, archived, importance=null) {
  const page=await context.newPage();
  try {
    const decimal=BigInt('0x'+id).toString();
    const query=(archived?'in:anywhere -in:inbox -in:trash -in:spam':'in:inbox')+(importance===null?'':importance?' is:important':' -is:important')+' threadid:'+decimal;
    await page.goto('https://mail.google.com/mail/u/0/#search/'+encodeURIComponent(query),{waitUntil:'domcontentloaded',timeout:25000});
    if (new URL(page.url()).origin !== 'https://mail.google.com') { const e=new Error();e.code='authentication_required';throw e; }
    await page.waitForFunction(({id,query})=>{
      if (!location.hash.startsWith('#search/')) return false;
      let actual;try{actual=decodeURIComponent(location.hash.slice(8)).replace(/\+/g,' ');}catch{return false;}
      if (actual!==query) return false;
      const grid=[...document.querySelectorAll('[role=grid]')].find(n=>n.getClientRects().length);
      if (!grid) return false;
      const rows=[...grid.querySelectorAll('[role=row]')].filter(n=>n.getClientRects().length);
      const exact=rows.filter(r=>[...r.querySelectorAll('[data-legacy-thread-id]')].some(n=>n.getAttribute('data-legacy-thread-id')?.toLowerCase()===id));
      const main=[...document.querySelectorAll('[role=main]')].find(n=>n.getClientRects().length);
      const empty=rows.length===0 && !!main && /No hay mensajes que coincidan con tus criterios|No conversations found|No messages matched your search/i.test(main.innerText);
      return (rows.length===1 && exact.length===1) || empty;
    },{id:id.toLowerCase(),query},{timeout:20000});
    const result=await page.evaluate(id=>{
      const grid=[...document.querySelectorAll('[role=grid]')].find(n=>n.getClientRects().length);
      const rows=[...grid.querySelectorAll('[role=row]')].filter(n=>n.getClientRects().length);
      const row=rows.find(r=>[...r.querySelectorAll('[data-legacy-thread-id]')].some(n=>n.getAttribute('data-legacy-thread-id')?.toLowerCase()===id));
      if (!row) return {found:false};
      const unread=row.classList.contains('zE'),read=row.classList.contains('yO');
      return {found:true,no_leida:unread===read?null:unread};
    },id.toLowerCase());
    return {page,...result};
  } catch(error) { await page.close();throw error; }
}

// La identidad se obtiene del control oficial de cuenta de la página consultada.
async function gmailAccount(page) {
  const email=await page.evaluate(()=>{
    const links=[...document.querySelectorAll('a[aria-label]')].filter(n=>{
      if (!n.getClientRects().length || !/^(Google Account|Cuenta de Google)\s*:/i.test(n.getAttribute('aria-label')||'')) return false;
      try { const url=new URL(n.href); return url.protocol==='https:' && ['accounts.google.com','myaccount.google.com'].includes(url.hostname); } catch { return false; }
    });
    const matches=links.flatMap(n=>[...(n.getAttribute('aria-label')||'').matchAll(/[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/gi)].map(m=>m[0].toLowerCase()));
    return matches.length===1 ? matches[0] : null;
  });
  return email ? pluginHash('sha256').update(email).digest('hex') : null;
}

async function gmailEvidence(context,id) {
  let inbox;
  try {
    inbox=await gmailLocate(context,id,false);
    const account=await gmailAccount(inbox.page);
    if (!account || !inbox.found || typeof inbox.no_leida!=='boolean') return {status:'evidence_unavailable'};
    return {status:'ready',account_hash:account,thread_id:id.toLowerCase(),unread_before:inbox.no_leida};
  } catch { return {status:'evidence_unavailable'}; }
  finally { await inbox?.page.close().catch(()=>{}); }
}

async function gmailReview(context,evidence) {
  let inbox, archived;
  try {
    inbox=await gmailLocate(context,evidence.thread_id,false);
    if (await gmailAccount(inbox.page)!==evidence.account_hash) return {status:'inconclusive'};
    if (inbox.found) return {status:inbox.no_leida===evidence.unread_before?'not_applied':'inconclusive'};
    archived=await gmailLocate(context,evidence.thread_id,true);
    if (await gmailAccount(archived.page)!==evidence.account_hash) return {status:'inconclusive'};
    return {status:archived.found && archived.no_leida===evidence.unread_before?'applied':'inconclusive'};
  } catch { return {status:'inconclusive'}; }
  finally { await inbox?.page.close().catch(()=>{}); await archived?.page.close().catch(()=>{}); }
}

async function gmailImportance(context, id) {
  let important,other;
  try {
    important=await gmailLocate(context,id,false,true);
    other=await gmailLocate(context,id,false,false);
    if (important.found===other.found || (important.found && important.no_leida===null) || (other.found && other.no_leida===null))
      return {status:'importance_unknown',message:'No se pudo determinar la importancia de la conversación en Recibidos.'};
    const match=important.found?important:other;
    return {status:'ok',data:{id,importante:important.found,no_leida:match.no_leida}};
  } catch(error) {
    return {status:error.code==='authentication_required'?'authentication_required':'execution_failed',diagnostic:{code:programErrorCode(error),phase:'read'},message:'No se pudo comprobar la importancia en Gmail.'};
  } finally { await important?.page.close().catch(()=>{}); await other?.page.close().catch(()=>{}); }
}

async function gmailArchive(context, id, evidence) {
  let initial,attempted=false,before=null;
  try {
    initial=await gmailLocate(context,id,false);
    if (evidence && (await gmailAccount(initial.page)!==evidence.account_hash || !initial.found || initial.no_leida!==evidence.unread_before)) return {status:'archive_not_started',executed:false,diagnostic:{code:'program_error',phase:'prepare'},message:'La cuenta o el estado de la conversación cambió; no se archivó.'};
    if (!initial.found) {
      await initial.page.close();initial=null;
      const archived=await gmailLocate(context,id,true);
      try {
        if (!archived.found || archived.no_leida===null) return {status:'archive_not_started',executed:false,diagnostic:{code:'program_error',phase:'prepare'},message:'No se pudo identificar la conversación en Recibidos o Archivo.'};
        return {status:'ok',data:{id,archivada:true,ya_archivada:true,no_leida:archived.no_leida}};
      } finally { await archived.page.close(); }
    }
    before=initial.no_leida;
    if (before===null) return {status:'archive_not_started',executed:false,diagnostic:{code:'program_error',phase:'prepare'},message:'No se pudo comprobar el estado de lectura; no se archivó.'};
    const row=initial.page.locator('[role=grid]:visible [role=row]').filter({has:initial.page.locator('[data-legacy-thread-id="'+id+'" i]')});
    if (await row.count()!==1) throw new Error('row_count');
    const box=row.locator('[role=checkbox]');
    if (await box.count()!==1 || await box.getAttribute('aria-checked')!=='false') throw new Error('selection');
    await box.click({timeout:5000});
    if (await box.getAttribute('aria-checked')!=='true') throw new Error('selection');
    const selected=await initial.page.evaluate(()=>{const grid=[...document.querySelectorAll('[role=grid]')].find(n=>n.getClientRects().length);return [...grid.querySelectorAll('[role=row] [role=checkbox][aria-checked=true]')].filter(n=>n.getClientRects().length).length;});
    if (selected!==1) throw new Error('selection');
    const archive=initial.page.locator('[role=button][aria-label="Archivar"]:visible, [role=button][aria-label="Archive"]:visible');
    if (await archive.count()!==1 || await archive.getAttribute('aria-disabled')==='true') throw new Error('archive_control');
    attempted=true;
    try { await archive.click({timeout:10000}); } catch {}
  } catch(error) {
    if (!attempted) return {status:error.code==='authentication_required'?'authentication_required':'archive_not_started',executed:false,diagnostic:{code:programErrorCode(error),phase:'prepare'},message:'No se pudo preparar el archivo; no se archivó ningún mensaje.'};
  } finally { if (initial && !attempted) await initial.page.close().catch(()=>{}); }
  // Si el clic perdió su respuesta, solo se repiten observaciones, nunca el clic.
  try {
  for (let n=0;n<3;n++) {
    if(n) await new Promise(resolve=>setTimeout(resolve,n*750));
    let inbox,archived;
    try {
      inbox=await gmailLocate(context,id,false);
      if (inbox.found) continue;
      archived=await gmailLocate(context,id,true);
      if (archived.found && archived.no_leida===before) return {status:'ok',data:{id,archivada:true,ya_archivada:false,no_leida:archived.no_leida}};
    } catch {} finally { await inbox?.page.close().catch(()=>{});await archived?.page.close().catch(()=>{}); }
  }
  return {status:'result_uncertain',diagnostic:{code:'postcondition_failed',phase:'verify'},message:'No se pudo confirmar el archivo y el estado de lectura; no se repetirá automáticamente.'};
  } finally { if (initial) await initial.page.close().catch(()=>{}); }
}

function programRuntime() {
  // Solo etiquetas técnicas cerradas: nunca incluir mensajes, selectores ni contenido del sitio.
  const diagnostic = { stage:'program', phase:'prepare', action_status:'unknown', read_attempts:0 };
  function stage(name) {
    if (typeof name !== 'string' || !/^[a-z][a-z0-9_]{0,63}$/.test(name)) throw new TypeError();
    diagnostic.stage = name;
  }
  async function read(name, operation) {
    stage(name);
    diagnostic.phase = 'read';
    // El callback puede contener navegación o escrituras. El nombre no demuestra
    // que sea seguro repetirlo; una sola invocación conserva esa incertidumbre.
    diagnostic.read_attempts = 1;
    return await operation();
  }
  return {
    diagnostic,
    api: {
      read,
      async action(name, perform, verify) {
        stage(name);
        const confirmedBefore = await read(name, verify);
        if (typeof confirmedBefore !== 'boolean') throw new TypeError();
        if (confirmedBefore) {
          diagnostic.action_status = 'confirmed';
          return { verified:true, already_satisfied:true, recovered:false };
        }
        diagnostic.phase = 'action';
        diagnostic.action_status = 'uncertain';
        let failed = false;
        try { await perform(); }
        catch (error) { failed = true; diagnostic.action_error_code = programErrorCode(error); }
        let confirmedAfter = false;
        for (let observation = 0; observation < 3; observation++) {
          if (observation) await new Promise(resolve => setTimeout(resolve, observation * 250));
          confirmedAfter = await read(name, verify);
          if (typeof confirmedAfter !== 'boolean' || confirmedAfter) break;
        }
        if (typeof confirmedAfter !== 'boolean') throw new TypeError();
        if (!confirmedAfter) {
          diagnostic.phase = 'verify';
          const error = new Error(); error.code = 'postcondition_failed'; throw error;
        }
        diagnostic.action_status = 'confirmed';
        return { verified:true, already_satisfied:false, recovered:failed };
      },
    },
  };
}

async function executeProgram(context, manifest, fn, args) {
  const { api, diagnostic } = programRuntime();
  let page, result;
  try {
    page = await context.newPage();
    diagnostic.phase = 'execute';
    const fnBody = new (Object.getPrototypeOf(async function(){}).constructor)('context', 'page', 'args', 'origin', 'runtime', fn.code);
    const data = await fnBody(context, page, args, manifest.origin, api);
    if (pluginType(data) !== fn.result_type) {
      result = { status:'result_uncertain', diagnostic:{ ...diagnostic, code:'result_type_mismatch' }, message:'El resultado no coincide con el contrato; comprueba el efecto antes de repetir la acción.' };
    } else result = { status:'ok', data };
  } catch (error) {
    result = { status:'result_uncertain', diagnostic:{ ...diagnostic, code:programErrorCode(error) }, message:'El bloque falló; el diagnóstico identifica el paso. Comprueba el resultado antes de repetir una acción.' };
  } finally {
    try { await page?.close(); }
    catch { result.cleanup_status = 'failed'; }
  }
  return result;
}

export function createWebPlugins({ directory, getContext, verificationBase }) {
  async function prepare() {
    await pluginFs.mkdir(directory, { recursive: true, mode: 0o700 });
    const info = await pluginFs.lstat(directory);
    if (!info.isDirectory() || info.isSymbolicLink()) pluginFail('plugin_storage_error');
    await pluginFs.chmod(directory, 0o700);
  }
  function filename(id) {
    if (!pluginId(id)) pluginFail('invalid_plugin');
    return pluginPath.join(directory, `${id}.json`);
  }
  async function load(id) {
    const file = filename(id);
    try {
      const directoryInfo = await pluginFs.lstat(directory);
      if (!directoryInfo.isDirectory() || directoryInfo.isSymbolicLink()) pluginFail('plugin_storage_error');
    } catch (error) { if (error.code === 'ENOENT') return null; throw error; }
    let info;
    try { info = await pluginFs.lstat(file); } catch (error) { if (error.code === 'ENOENT') return null; throw error; }
    if (!info.isFile() || info.isSymbolicLink() || info.size > PLUGIN_LIMIT) pluginFail('plugin_storage_error');
    const record = JSON.parse(await pluginFs.readFile(file, 'utf8'));
    if (record.deleted === true) {
      if (record.id !== id || !Number.isSafeInteger(record.revision) || record.revision < 1) pluginFail('plugin_storage_error');
      return record;
    }
    validateWebPlugin(record.manifest);
    if (record.manifest.id !== id || !Number.isSafeInteger(record.revision) || record.revision < 1 || !pluginObject(record.verified)) pluginFail('plugin_storage_error');
    // Solo evidencia publicada vigente y funciones idénticas, nunca pruebas de otro candidato.
    if (verificationBase?.manifest?.id === id && verificationBase.manifest.origin === record.manifest.origin) {
      for (const fn of record.manifest.functions) {
        const previous = verificationBase.manifest.functions.find(item => item.name === fn.name);
        const proof = verificationBase.verified?.[fn.name];
        if (proof?.revision === verificationBase.revision && JSON.stringify(previous) === JSON.stringify(fn)) {
          record.verified[fn.name] = { ...proof, revision: record.revision, source_revision: verificationBase.revision };
        }
      }
    }
    return record;
  }
  async function store(record) {
    await prepare();
    const temporary = pluginPath.join(directory, `.${pluginUUID()}.tmp`);
    try {
      await pluginFs.writeFile(temporary, JSON.stringify(record), { mode: 0o600, flag: 'wx' });
      await pluginFs.rename(temporary, filename(record.manifest?.id || record.id));
    } finally { await pluginFs.unlink(temporary).catch(error => { if (error.code !== 'ENOENT') throw error; }); }
  }
  async function execute(manifest, fn, args, evidence) {
    const target = pluginTarget(manifest, fn, args);
    let context;
    try { context = await getContext(); }
    catch (error) { return {status:'browser_unavailable',executed:false,diagnostic:{code:programErrorCode(error),phase:'prepare'},message:'No se pudo preparar el navegador; la función no se ejecutó.'}; }
    if (!context) return { status: 'browser_unavailable', executed:false, diagnostic:{code:'program_error',phase:'prepare'}, message:'El navegador no está disponible; la función no se ejecutó.' };
    if (fn.transport === 'program') {
      return executeProgram(context, manifest, fn, args);
    }
    if (fn.transport === 'gmail_archive') return gmailArchive(context,args.id,evidence);
    if (fn.transport === 'gmail_importance') return gmailImportance(context,args.id);
    if (fn.transport === 'gmail_print') {
      let response, page;
      try {
        const url = new URL('/mail/u/0/', manifest.origin);
        url.search = new URLSearchParams({view:'pt',search:'all',th:args.id}).toString();
        response=await context.request.get(url.href,{timeout:20000,maxRedirects:0});
        const status=response.status();
        if (status === 401) return pluginAuth(manifest);
        if (status >= 300 && status < 400) {
          const location=response.headers().location;
          let login=false;
          try { const dest=new URL(location,url); login=dest.protocol==='https:' && dest.hostname==='accounts.google.com'; } catch {}
          return login ? pluginAuth(manifest) : {status:'redirect_blocked'};
        }
        if (status === 403) return {status:'access_denied'};
        if (status !== 200) return {status:'http_error',http_status:status};
        if (!/\btext\/html\b/i.test(response.headers()['content-type'] || '')) return {status:'site_changed'};
        const body=await response.body();
        if (body.length > 1024*1024) return {status:'response_too_large'};
        page=await context.newPage();
        await page.route('**/*',route=>route.abort());
        const texto=await page.evaluate(html=>{
          const doc=new DOMParser().parseFromString(html,'text/html');
          const main=doc.querySelector('.maincontent');
          if (!main) return null;
          for (const node of main.querySelectorAll('script,style,noscript,iframe,object,embed')) node.remove();
          const blocks=new Set(['ADDRESS','ARTICLE','BLOCKQUOTE','BR','DIV','H1','H2','H3','H4','H5','H6','LI','P','PRE','SECTION','TABLE','TR']);
          const parts=[];
          const walker=doc.createTreeWalker(main,NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT);
          while (walker.nextNode()) {
            const node=walker.currentNode;
            if (node.nodeType===Node.TEXT_NODE) parts.push(node.nodeValue || '');
            else if (blocks.has(node.tagName)) parts.push('\n');
            else if (node.tagName==='TD' || node.tagName==='TH') parts.push(' ');
          }
          return parts.join('').replace(/\r/g,'').replace(/[ \t]+\n/g,'\n').replace(/\n{3,}/g,'\n\n').trim();
        },body.toString('utf8'));
        if (!texto) return {status:'site_changed'};
        if (Buffer.byteLength(texto)>100000) return {status:'response_too_large'};
        return {status:'ok',data:{id:args.id,texto}};
      } catch (error) {
        return {status:error?.name === 'TimeoutError'?'timeout':'execution_failed',diagnostic:{code:programErrorCode(error),phase:'read'},message:'No se pudo leer la conversación de Gmail.'};
      } finally {
        await page?.close();
        await response?.dispose();
      }
    }
    if (fn.transport === 'gmail_inbox') {
      if (!Number.isSafeInteger(args.pagina) || args.pagina < 1 || args.pagina > 100) return {status:'invalid_arguments',executed:false};
      const page = await context.newPage();
      let stage='open';
      try {
        await page.goto(target,{waitUntil:'domcontentloaded',timeout:20000});
        if (new URL(page.url()).origin !== manifest.origin) return pluginAuth(manifest);
        stage='search';
        const search=page.getByRole('textbox',{name:'Buscar en el correo electrónico',exact:true});
        await search.waitFor({timeout:15000});
        await search.fill('in:inbox'); await search.press('Enter');
        await page.waitForFunction(()=>location.hash.startsWith('#search/'),null,{timeout:15000});
        if (new URL(page.url()).origin !== manifest.origin) return pluginAuth(manifest);
        stage='rows';
        await page.locator('[aria-label="Mostrar más mensajes"]:visible').first().waitFor({state:'visible',timeout:15000});
        const rows=page.locator('[role=grid]:visible [role=row] [data-legacy-thread-id]');
        await rows.first().waitFor({state:'attached',timeout:15000});
        stage='pagination';
        for (let i=1;i<args.pagina;i++) {
          const next=page.getByRole('button',{name:'Resultados siguientes',exact:true});
          const matches=await next.count();
          const disabled=matches === 1 ? await next.getAttribute('aria-disabled') === 'true' : null;
          if (matches !== 1 || disabled) return {status:'page_not_available',diagnostic:{code:'page_unavailable',phase:'read',matches,disabled},message:'No se encontró una página siguiente disponible.'};
          const before=await rows.first().getAttribute('data-legacy-thread-id');
          await next.click({timeout:10000});
          if (new URL(page.url()).origin !== manifest.origin) return {status:'redirect_blocked'};
          await page.waitForFunction(old=>{const grids=Array.from(document.querySelectorAll('[role=grid]'));const grid=grids.find(node=>node.getClientRects().length>0);const first=grid?.querySelector('[role=row] [data-legacy-thread-id]');return first && first.getAttribute('data-legacy-thread-id') !== old;},before,{timeout:15000});
        }
        const data=await page.evaluate(()=>{
          const visible=selector=>Array.from(document.querySelectorAll(selector)).find(node=>node.getClientRects().length>0);
          const grid=visible('[role=grid]');
          const range=visible('[aria-label="Mostrar más mensajes"]');
          const next=visible('[aria-label="Resultados siguientes"]');
          return {rango:range?.innerText?.slice(0,200) || null,siguiente:!!next && next.getAttribute('aria-disabled') !== 'true',conversaciones:Array.from(grid?.querySelectorAll('[role=row]') || []).slice(0,50).map(row=>({id:row.querySelector('[data-legacy-thread-id]')?.getAttribute('data-legacy-thread-id') || null,texto:(row.innerText || '').slice(0,2000),no_leida:row.classList.contains('zE')===row.classList.contains('yO')?null:row.classList.contains('zE')})).filter(row=>row.id)};
        });
        if (!data.conversaciones.length) return {status:'site_changed',message:'No se encontraron filas de conversaciones en la página.'};
        if (Buffer.byteLength(JSON.stringify(data)) > 100000) return {status:'response_too_large'};
        return {status:'ok',data};
      } catch (error) {
        return {status:error?.name === 'TimeoutError' ? 'timeout' : 'execution_failed', diagnostic:{code:programErrorCode(error),phase:'read',stage}, message:'No se completó la consulta de Recibidos.'};
      } finally { await page.close(); }
    }
    if (fn.transport === 'observe') {
      // No abre ni navega: lee únicamente una página ya presente, sin código del plugin.
      const page = context.pages().find(candidate => candidate.url() === target);
      if (!page) return { status:'page_not_available', message:'La página necesaria no está abierta.' };
      const values = await page.locator(fn.selector).allTextContents({timeout:6500});
      if (page.url() !== target) return { status:'site_changed' };
      if (!values.length) return { status:'site_changed' };
      const data = values.slice(0,200).map(value => value.trim().slice(0,2000));
      return { status:'ok', data:fn.result_type === 'string' ? data[0] : data, truncated:values.length>200 || values.some(value=>value.length>2000) };
    }
    const deadline = Date.now() + 6500;
    const timeout = () => Math.max(1, deadline - Date.now());
    if (fn.transport === 'http') {
      let url = target;
      for (let redirect = 0; redirect <= 4; redirect++) {
        const response = await context.request.get(url, { timeout: timeout(), maxRedirects: 0, failOnStatusCode: false });
        try {
          const status = response.status();
          if (status === 401) return pluginAuth(manifest);
          if (status === 403) return { status: 'access_denied', http_status: status };
          if (status >= 300 && status < 400) {
            const location = response.headers().location;
            if (!location) return { status: 'site_changed', http_status: status };
            const next = pluginURL(new URL(location, url).href);
            if (pluginLogin(manifest, next.href)) return pluginAuth(manifest);
            if (next.origin !== manifest.origin) return { status: 'redirect_blocked', http_status: status };
            url = next.href; continue;
          }
          if (status < 200 || status >= 300) return { status: 'http_error', http_status: status };
          if (!/\bjson\b/i.test(response.headers()['content-type'] || '')) return { status: 'site_changed', http_status: status };
          const body = await response.body();
          if (body.length > 1024 * 1024) return { status: 'response_too_large' };
          let data;
          try { data = JSON.parse(body.toString('utf8')); } catch { return { status: 'site_changed' }; }
          data = pluginAt(data, fn.result_pointer);
          if (pluginType(data) !== fn.result_type) return { status: 'site_changed' };
          if (fn.fields) {
            const project = item => Object.fromEntries(Object.entries(fn.fields).map(([key, pointer]) => [key, pluginAt(item, pointer)]));
            data = Array.isArray(data) ? data.map(project) : project(data);
          }
          if (Buffer.byteLength(JSON.stringify(data)) > 100000) return { status: 'response_too_large' };
          return { status: 'ok', data };
        } finally { await response.dispose(); }
      }
      return { status: 'redirect_limit' };
    }
    const page = await context.newPage();
    try {
      // La extracción usa una pestaña propia, sin cambiar la seleccionada por el usuario.
      await page.route('**/*', async route => {
        const url = new URL(route.request().url());
        if (['GET', 'HEAD'].includes(route.request().method()) && (url.origin === manifest.origin || (route.request().isNavigationRequest() && pluginLogin(manifest, url.href)))) await route.continue();
        else await route.abort();
      });
      const response = await page.goto(target, { waitUntil: 'domcontentloaded', timeout: timeout() });
      if (pluginLogin(manifest, page.url()) || response?.status() === 401) return pluginAuth(manifest);
      if (response?.status() === 403) return { status: 'access_denied', http_status: 403 };
      if (new URL(page.url()).origin !== manifest.origin) return { status: 'redirect_blocked' };
      if (response && !response.ok()) return { status: 'http_error', http_status: response.status() };
      if (fn.login_selector && await page.locator(fn.login_selector).first().isVisible()) return pluginAuth(manifest);
      await page.locator(fn.ready_selector || fn.selector).first().waitFor({ state: 'attached', timeout: timeout() });
      const extracted = await page.locator(fn.selector).evaluateAll((nodes, spec) => {
        let truncated = false;
        const text = node => {
          const value = (node.textContent || '').trim();
          if (value.length > 2000) truncated = true;
          return value.slice(0, 2000);
        };
        if (spec.type === 'string') return { data: text(nodes[0]), truncated };
        if (nodes.length > 200) truncated = true;
        const data = nodes.slice(0, 200).map(node => spec.fields ? Object.fromEntries(Object.entries(spec.fields).map(([key, selector]) => {
          const child = node.querySelector(selector);
          return [key, child ? text(child) : null];
        })) : text(node));
        return { data, truncated };
      }, { type: fn.result_type, fields: fn.fields });
      const { data, truncated } = extracted;
      if (Array.isArray(data) && data.some(row => pluginObject(row) && Object.values(row).includes(null))) return { status: 'site_changed' };
      if (Buffer.byteLength(JSON.stringify(data)) > 100000) return { status: 'response_too_large' };
      return { status: 'ok', data, truncated };
    } finally { await page.close(); }
  }
  async function catalogueRecords() {
    await prepare();
    const files = (await pluginFs.readdir(directory)).filter(name => /^[a-z][a-z0-9_]{0,63}\.json$/.test(name)).sort();
    const records = [];
    for (const file of files) { const record = await load(file.slice(0, -5)); if (record && !record.deleted) records.push(record); }
    if (records.length > 50) pluginFail('plugin_storage_error');
    return records;
  }
  async function capacity(previous) {
    if ((!previous || previous.deleted) && (await catalogueRecords()).length >= 50) pluginFail('plugin_storage_error');
  }
  return {
    async dispatch(request) {
      if (request.action === 'list') return { plugins: await catalogueRecords() };
      if (request.action === 'promote') {
        if (!pluginId(request.candidate_id)) pluginFail('invalid_plugin_arguments');
        const previous = await load(request.plugin_id);
        if (request.expected_revision !== (previous?.revision || 0)) return { status: 'revision_conflict' };
        const candidateStore = createWebPlugins({ directory: pluginPath.join(directory, 'candidates', request.candidate_id), getContext, verificationBase: previous });
        const candidate = (await candidateStore.dispatch({ action: 'get', plugin_id: request.plugin_id })).plugin;
        if (!candidate || candidate.manifest.functions.some(fn => candidate.verified[fn.name]?.revision !== candidate.revision)) return { status: 'verification_required' };
        await capacity(previous);
        const revision = (previous?.revision || 0) + 1;
        const record = { manifest: candidate.manifest, revision, updated_at: new Date().toISOString(), verified: Object.fromEntries(candidate.manifest.functions.map(fn => [fn.name, { ...candidate.verified[fn.name], revision }])) };
        await store(record);
        return { status: 'promoted', plugin: record };
      }
      if (request.action === 'save') {
        const manifest = validateWebPlugin(request.manifest);
        const previous = await load(manifest.id);
        if (request.expected_revision !== (previous && !previous.deleted ? previous.revision : 0)) return { status: 'revision_conflict' };
        await capacity(previous);
        const record = { manifest, revision: (previous?.revision || 0) + 1, updated_at: new Date().toISOString(), verified: {} };
        await store(record);
        return { status: 'saved', plugin: record };
      }
      const record = await load(request.plugin_id);
      if (!record || record.deleted) return { status: 'plugin_not_found', revision: record?.revision || 0 };
      if (request.action === 'delete') {
        if (request.expected_revision !== record.revision) return { status: 'revision_conflict' };
        await store({ id: record.manifest.id, revision: record.revision + 1, deleted: true });
        return { status: 'deleted', plugin_id: record.manifest.id };
      }
      if (request.action === 'get') return { plugin: record };
      if (!['run', 'verify', 'prepare', 'preflight', 'gmail_evidence', 'gmail_review'].includes(request.action)) pluginFail('invalid_plugin_arguments');
      if (request.action === 'preflight' && !['run','verify'].includes(request.operation)) pluginFail('invalid_plugin_arguments');
      const fn = record.manifest.functions.find(fn => fn.name === request.function_name);
      if (!fn) return { status: 'function_not_found' };
      if (fn.transport === 'program' && request.program_allowed !== true) return { status: 'permission_required', message: 'El bloque programable requiere activar lectura y escritura de archivos, comandos y control de aplicaciones.' };
      if (request.expected_revision !== record.revision) return { status: 'revision_conflict' };
      if (request.checked_transport !== undefined && request.checked_transport !== fn.transport) return { status: 'revision_conflict', executed: false };
      const args = request.arguments === undefined ? {} : request.arguments;
      const invalid = pluginArguments(fn, args);
      if (invalid) return { ...invalid, plugin_id: record.manifest.id, plugin_name: record.manifest.name, function_name: fn.name, revision: record.revision };
      if ((fn.transport === 'gmail_print' && !/^[a-f0-9]{1,128}$/i.test(args.id)) || (['gmail_archive','gmail_importance'].includes(fn.transport) && !/^[a-f0-9]{1,16}$/i.test(args.id))) return {status:'invalid_arguments',executed:false,message:'La identidad de la conversación de Gmail no es válida.'};
      if (request.action === 'gmail_evidence' || request.action === 'gmail_review') {
        if (fn.transport !== 'gmail_archive') return {status:'inconclusive'};
        const context=await getContext();
        if (!context) return {status:'inconclusive'};
        return request.action==='gmail_evidence' ? gmailEvidence(context,args.id) : gmailReview(context,request.gmail_evidence);
      }
      if (request.action !== 'verify' && !(request.action === 'preflight' && request.operation === 'verify') && record.verified[fn.name]?.revision !== record.revision) return { status: 'verification_required', executed: false };
      if (['prepare','preflight'].includes(request.action)) return { status: 'ready', executed: false, plugin_id: record.manifest.id, plugin_name: record.manifest.name, function_name: fn.name, revision: record.revision, transport: fn.transport };
      let result;
      try { result = await execute(record.manifest, fn, args, request.gmail_evidence); }
      catch (error) {
        if (error.code === 'invalid_plugin_arguments') throw error;
        result = fn.transport === 'program' ? { status: 'result_uncertain', message: 'El bloque pudo realizar cambios antes de fallar. Comprueba el resultado antes de repetirlo.' } : { status: error.code === 'site_changed' ? 'site_changed' : error.name === 'TimeoutError' ? 'timeout' : 'execution_failed' };
      }
      if (request.action === 'verify') {
        delete record.verified[fn.name];
        if (result.status === 'ok') record.verified[fn.name] = { verified_at: new Date().toISOString(), revision: record.revision };
        await store(record);
      }
      return { ...result, plugin_id: record.manifest.id, plugin_name: record.manifest.name, function_name: fn.name, revision: record.revision };
    },
  };
}
