// Apertura manual sin reiniciar el escritorio. El proceso termina al cerrar Chromium.
import { spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { createInterface } from 'node:readline';
const root = path.join(process.env.HOME, 'Library/Application Support/Gennie/browser');
const runtime = path.join(root, 'runtime-1.64.1');
const worker = await readFile(new URL('../apps/browser/src/worker.mjs', import.meta.url), 'utf8');
const child = spawn(path.join(runtime, 'node'), ['--input-type=module', '--eval', worker, '--', runtime, path.join(root, 'profile'), '--standalone'], {
  env: { ...process.env, PLAYWRIGHT_BROWSERS_PATH: path.join(root, 'browsers') }, stdio: ['pipe', 'pipe', 'ignore'],
});
child.on('error', () => { console.error('Falta preparar Playwright: ejecuta scripts/setup_browser.sh'); process.exitCode = 1; });
child.on('exit', code => { process.exitCode ||= code ?? 1; });
process.on('SIGINT', () => child.stdin.end());
process.on('SIGTERM', () => child.stdin.end());
const output = createInterface({ input: child.stdout });
output.on('line', line => {
  const response = JSON.parse(line);
  if (response.error) { console.error(response.error); process.exitCode = 1; child.stdin.end(); }
  else console.log('Gennie Web abierto con su perfil persistente.');
});
child.stdin.write('{"method":"open"}\n');
