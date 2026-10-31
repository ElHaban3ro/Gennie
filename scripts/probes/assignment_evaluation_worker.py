#!/usr/bin/env python3
"""Un turno experimental cerrado; protocolo local sin cuenta o live explícito."""
import json
import hashlib
import os
from pathlib import Path
import selectors
import shutil
import signal
import subprocess
import sys
import time


class Failure(Exception):
    pass


def binary():
    candidates = [Path('/Applications/ChatGPT.app/Contents/Resources/codex-cli/CodexCLI.app/Contents/MacOS/codex'), Path.home()/'.local/bin/codex', Path('/opt/homebrew/bin/codex'), Path('/usr/local/bin/codex')]
    return str(next((p.resolve() for p in candidates if p.is_file()), Path(shutil.which('codex') or 'codex')))


def usage_snapshot(value):
    """Conserva contadores reportados, sin sumar instantáneas acumulativas."""
    allowed = ('inputTokens', 'cachedInputTokens', 'outputTokens', 'reasoningOutputTokens', 'totalTokens')
    if not isinstance(value, dict):
        return None
    result = {}
    for group in ('last', 'total'):
        source = value.get(group)
        if isinstance(source, dict):
            counts = {k:v for k,v in source.items() if k in allowed and type(v) is int and v >= 0}
            if counts:
                result[group] = counts
    return result or None


def schema_inventory(server, expected):
    """Compara los esquemas publicados por el fixture con los descubiertos por Codex."""
    if not isinstance(expected, dict) or not expected or any(not isinstance(s, dict) for s in expected.values()):
        raise Failure('expected_schema_unavailable')
    def digest(value):
        return hashlib.sha256(json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    tools = server.get('tools', {})
    evidence = {}
    for name, schema in expected.items():
        definition = tools.get(name)
        received = definition.get('inputSchema') if isinstance(definition, dict) else None
        a, b = digest(schema), digest(received)
        evidence[name] = {'expected_sha256':a, 'received_sha256':b, 'matches':a == b}
    return evidence


class Client:
    def __init__(self, process, until):
        self.process, self.until = process, until
        self.selector = selectors.DefaultSelector()
        self.selector.register(process.stdout, selectors.EVENT_READ)
        self.buffer = bytearray()
        self.events = 0
        self.turn_started = False
        self.messages = {}
        self.usage = None
        self.tool_types = []
        self.retries = 0
        self.completed_turns = {}
        self.thread_id = None
        self.generation_requested = False
        self.unexpected_output = False

    def send(self, value):
        self.process.stdin.write(json.dumps(value, ensure_ascii=False).encode()+b'\n')
        self.process.stdin.flush()

    def next(self):
        while b'\n' not in self.buffer:
            remaining = self.until-time.monotonic()
            if remaining <= 0 or not self.selector.select(remaining):
                raise Failure('timeout')
            data = os.read(self.process.stdout.fileno(), 65536)
            if not data:
                raise Failure('provider_closed')
            self.buffer.extend(data)
            if len(self.buffer) > 8*1024*1024:
                raise Failure('protocol_budget_exceeded')
        line, _, tail = self.buffer.partition(b'\n')
        self.buffer = bytearray(tail)
        try:
            value = json.loads(line)
        except (ValueError, UnicodeError) as error:
            raise Failure('protocol_invalid') from error
        if not isinstance(value, dict):
            raise Failure('protocol_invalid')
        self.events += 1
        if self.events > 50000:
            raise Failure('protocol_budget_exceeded')
        if 'id' in value and 'method' in value:
            self.send({'id':value['id'], 'error':{'code':-32601,'message':'Action unavailable in evaluation'}})
            raise Failure('unavailable_action_requested')
        self.observe(value)
        return value

    def observe(self, value):
        method, params = value.get('method'), value.get('params', {})
        if self.thread_id is not None and params.get('threadId') not in (None, self.thread_id):
            return
        if method == 'turn/started':
            self.turn_started = True
        if method == 'turn/completed':
            turn = params.get('turn', {})
            if isinstance(turn, dict) and isinstance(turn.get('id'), str):
                self.completed_turns[turn['id']] = turn.get('status')
        if method == 'thread/tokenUsage/updated':
            self.usage = usage_snapshot(params.get('tokenUsage'))
        if method == 'error' and params.get('willRetry') is True:
            self.retries += 1
        item = params.get('item', {})
        if method in ('item/started', 'item/completed'):
            kind = item.get('type')
            if kind in ('webSearch', 'webSearchCall', 'commandExecution', 'fileChange', 'computerUse', 'browserUse'):
                raise Failure('closed_scope_violation')
            if method == 'item/completed' and kind == 'agentMessage':
                if not self.generation_requested:
                    self.unexpected_output = True
                    return
                text = item.get('text')
                if isinstance(text, str):
                    if len(self.messages) >= 128 or sum(map(len, self.messages.values()))+len(text) > 64000:
                        raise Failure('output_budget_exceeded')
                    self.messages[item.get('id', str(len(self.messages)))] = text
            if method == 'item/completed' and kind == 'mcpToolCall':
                self.tool_types.append(str(item.get('tool', 'unknown'))[:200])

    def call(self, identifier, method, params):
        self.send({'id':identifier,'method':method,'params':params})
        while True:
            value = self.next()
            if value.get('id') == identifier:
                if 'error' in value:
                    raise Failure('rpc_failed_'+method.replace('/', '_'))
                return value.get('result', {})


def run(spec):
    if spec['mode'] not in ('protocol','live'):
        raise Failure('invalid_mode')
    base = Path(spec['base'])
    base.mkdir(mode=0o700, parents=True, exist_ok=False)
    profile, workspace = base/'cli-home', base/'workspace'
    profile.mkdir(mode=0o700)
    workspace.mkdir(mode=0o700)
    if spec['mode'] == 'live':
        source = Path(os.environ.get('CODEX_HOME', str(Path.home()/'.codex')))/'auth.json'
        if source.is_file():
            (profile/'auth.json').symlink_to(source)
    env = os.environ.copy()
    env['CODEX_HOME'] = str(profile)
    for key in ('OPENAI_API_KEY','CODEX_API_KEY','OPENROUTER_API_KEY','GENNIE_OPENROUTER_API_KEY','GENNIE_DATABASE_PATH','ACCESS_TOKEN','CHATGPT_ACCESS_TOKEN'):
        env.pop(key, None)
    executable = binary()
    configuration = [
        'default_permissions="gennie_eval"',
        'permissions.gennie_eval.filesystem={":root"="deny",":minimal"="read",'+json.dumps(executable)+'="read"}',
        'permissions.gennie_eval.network.enabled=false',
        'features.plugins=false','features.apps=false','features.shell_tool=false','features.unified_exec=false',
        'features.computer_use=false','features.browser_use=false','features.browser_use_external=false',
        'web_search="disabled"', spec['mcp_config'],
    ]
    command = [executable,'app-server','--listen','stdio://']
    for item in configuration:
        command.extend(['-c',item])
    started = time.monotonic()
    process = subprocess.Popen(command, cwd=workspace, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, bufsize=0)
    client = Client(process, started+spec['timeout_seconds'])
    result = {'status':'not_started','generation_turns':0,'cost_reported':None,'usage_reported':None}
    try:
        client.call(1,'initialize',{'clientInfo':{'name':'gennie_evaluation','version':'1'},'capabilities':{'experimentalApi':True}})
        client.send({'method':'initialized','params':{}})
        if spec['mode'] == 'live':
            account = client.call(2,'account/read',{'refreshToken':False})
            if (account.get('account') or {}).get('type') != 'chatgpt':
                raise Failure('chatgpt_session_unavailable')
            entries = client.call(3,'model/list',{'includeHidden':False}).get('data', [])
            chosen = next((m for m in entries if m.get('model') == spec['model']), None)
            if chosen is None or spec['effort'] not in [v.get('reasoningEffort') for v in chosen.get('supportedReasoningEfforts', [])]:
                raise Failure('model_or_effort_unavailable')
        params = {'cwd':str(workspace),'ephemeral':True,'approvalPolicy':'never','permissions':'gennie_eval','developerInstructions':spec['instructions']}
        if spec['mode'] == 'live':
            params['model'] = spec['model']
        thread = client.call(4,'thread/start',params)['thread']['id']
        client.thread_id = thread
        inventory = client.call(5,'mcpServerStatus/list',{'threadId':thread,'serverName':'gennie','detail':'toolsAndAuthOnly'})
        server = next((item for item in inventory.get('data', []) if item.get('name') == 'gennie'), None)
        if server is None or server.get('toolsError') is not None or not set(spec['tool_names']).issubset(server.get('tools', {})):
            raise Failure('mcp_inventory_unavailable')
        result['schema_inventory'] = schema_inventory(server, spec.get('expected_tool_schemas'))
        if not all(e['matches'] for e in result['schema_inventory'].values()):
            raise Failure('mcp_schema_mismatch')
        connected = time.monotonic()
        if spec['mode'] == 'protocol':
            client.call(7,'thread/unsubscribe',{'threadId':thread})
            if client.turn_started or client.completed_turns or client.unexpected_output:
                raise Failure('unexpected_generation')
            result['status'] = 'protocol_verified'
        else:
            result['generation_turns'] = None
            client.generation_requested = True
            turn = client.call(6,'turn/start',{'threadId':thread,'cwd':str(workspace),'approvalPolicy':'never','permissions':'gennie_eval','model':spec['model'],'effort':spec['effort'],'input':[{'type':'text','text':spec['request']} ]})['turn']['id']
            result['generation_turns'] = 1
            while turn not in client.completed_turns:
                client.next()
            if client.completed_turns[turn] != 'completed':
                raise Failure('generation_not_completed')
            result['status'] = 'completed'
        result['connection_ms'] = round((connected-started)*1000)
        result['turn_ms'] = round((time.monotonic()-connected)*1000)
    except Failure as error:
        result['status'] = str(error)
    except Exception:
        result['status'] = 'worker_exception'
    finally:
        if client.turn_started:
            result['generation_turns'] = 1
        result['usage_reported'] = client.usage
        result['messages'] = list(client.messages.values())
        result['mcp_tools_completed'] = client.tool_types
        result['provider_retries_observed'] = client.retries
        result['elapsed_ms'] = round((time.monotonic()-started)*1000)
        client.selector.close()
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
    return result


def main():
    spec = json.loads(Path(sys.argv[1]).read_text())
    result = run(spec)
    target = Path(spec['output'])
    with target.open('x', encoding='utf-8') as file:
        os.chmod(target, 0o600)
        json.dump(result, file, ensure_ascii=False, indent=2)
    return 0 if result['status'] in ('completed','protocol_verified') else 1


if __name__ == '__main__':
    raise SystemExit(main())
