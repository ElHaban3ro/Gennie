// Sonda aislada del perfil Seatbelt; no ejecuta recetas ni usa cuentas reales.
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { randomInt } from 'node:crypto';
import { once } from 'node:events';
import { mkdtemp, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { createInterface } from 'node:readline';
import { setTimeout as delay } from 'node:timers/promises';

if (process.platform !== 'darwin') throw new Error('Esta sonda necesita macOS');
const runtime = await realpath(path.join(process.env.HOME, 'Library/Application Support/Gennie/browser/runtime-patchright-1.64.2/node'));
const temporary = await mkdtemp('/private/tmp/gennie-plugin-sandbox-');
const marker = path.join(temporary, 'fixture.txt');
await writeFile(marker, 'fixture', {mode:0o600});
const quoted = value => `"${value.replaceAll('\\','\\\\').replaceAll('"','\\"')}"`;
const profile = `(version 1)
(deny default)
(allow file-read* (literal "/") (subpath "/usr/lib") (subpath "/System/Library") (subpath "/opt/homebrew") (subpath "/dev") (literal ${quoted(runtime)}))
(allow file-read-metadata)
(allow file-write-data (literal "/dev/null"))
(allow mach-lookup)
(allow sysctl-read)
(allow process-exec (literal ${quoted(runtime)}))
(allow process-info*)
(allow signal (target self))
(allow ipc-posix*)`;
const code = `
import fs from 'node:fs';
import net from 'node:net';
import {spawnSync} from 'node:child_process';
const result={};
try { fs.readFileSync(${JSON.stringify(marker)}); result.read='allowed'; } catch(e) { result.read=e.code; }
try { fs.writeFileSync(${JSON.stringify(marker)},'changed'); result.write='allowed'; } catch(e) { result.write=e.code; }
try { process.kill(${process.pid},0); result.signal='allowed'; } catch(e) { result.signal=e.code; }
const child=spawnSync('/usr/bin/true'); result.spawn=child.error?.code || 'allowed';
const socket=net.connect(9,'127.0.0.1');
socket.on('connect',()=>{result.network='allowed';socket.destroy();process.stdout.write(JSON.stringify(result));});
socket.on('error',e=>{result.network=e.code;process.stdout.write(JSON.stringify(result));});
`;
try {
  const child = spawnSync('/usr/bin/sandbox-exec', ['-p',profile,runtime,'--input-type=module','-e',code], {cwd:'/',env:{PATH:'/usr/bin:/bin'},encoding:'utf8',timeout:10000,maxBuffer:16384});
  assert.equal(child.status, 0, 'El proceso aislado debe iniciar y terminar');
  const result = JSON.parse(child.stdout);
  for (const key of ['read','write','spawn','network','signal']) assert.ok(['EPERM','EACCES'].includes(result[key]), `${key} debe quedar bloqueado`);
  assert.equal(await readFile(marker,'utf8'),'fixture');
  // Protocolo mínimo ficticio: solo el padre puede aplicar las acciones.
  const agentCode = `
import {createInterface} from 'node:readline';
const input=createInterface({input:process.stdin}); const lines=input[Symbol.asyncIterator]();
process.stdout.write(JSON.stringify({id:1,action:'click'})+'\\n');
await lines.next();
process.stdout.write(JSON.stringify({id:2,action:'click'})+'\\n');
const reply=JSON.parse((await lines.next()).value);
process.stdout.write(JSON.stringify({cancelled:reply.cancelled})+'\\n'); input.close();
`;
  const agent = spawn('/usr/bin/sandbox-exec',['-p',profile,runtime,'--input-type=module','-e',agentCode],{cwd:'/',env:{PATH:'/usr/bin:/bin'},stdio:['pipe','pipe','pipe']});
  const done = once(agent,'exit');
  const reader = createInterface({input:agent.stdout});
  const lines = reader[Symbol.asyncIterator]();
  const watchdog = setTimeout(()=>agent.kill('SIGKILL'),10000);
  let performed = 0;
  try {
    assert.deepEqual(JSON.parse((await lines.next()).value),{id:1,action:'click'});
    performed++; agent.stdin.write('{"id":1,"ok":true}\n');
    assert.deepEqual(JSON.parse((await lines.next()).value),{id:2,action:'click'});
    for (let i=0;i<64;i++) { const ms=randomInt(5001); assert.ok(ms>=0 && ms<=5000); }
    const cancellation=new AbortController();
    const pending=delay(5000,undefined,{signal:cancellation.signal});
    cancellation.abort();
    await assert.rejects(pending,{name:'AbortError'});
    agent.stdin.write('{"id":2,"cancelled":true}\n');
    assert.deepEqual(JSON.parse((await lines.next()).value),{cancelled:true});
    assert.equal(performed,1);
    agent.stdin.end();
    assert.equal((await done)[0],0);
  } finally { clearTimeout(watchdog);reader.close();if(agent.exitCode===null) agent.kill('SIGKILL'); }
  process.stdout.write(JSON.stringify({ok:true,os:'macos',checks:['read','write','spawn','network','signal','mediated_cancellation']})+'\n');
} finally { await rm(temporary,{recursive:true,force:true}); }
