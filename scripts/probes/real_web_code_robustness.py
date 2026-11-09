#!/usr/bin/env python3
"""Pruebas metamórficas locales de las soluciones obtenidas en la web real."""
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
import json
from pathlib import Path
import threading
import uuid
from urllib.request import urlopen

import assignment_evaluation as common
import real_web_code_sandbox
import real_web_oracle
import real_web_read_proxy

ROOT=Path(__file__).resolve().parents[2]
SAMPLES={'gennie':'fbf692c7-2db2-4b57-a902-03ab660156fd',
         'direct':'381961b9-40e9-4041-bd4e-5160cf474226',
         'native':'b60dfad9-39b8-4459-9b22-d9941721b27f'}


def transform(text,kind):
    if kind=='attribute_order':
        return text.replace('<span class="text" itemprop="text">','<span itemprop="text" class="text">').replace('<small class="author" itemprop="author">','<small itemprop="author" class="author">')
    if kind=='extra_class':
        for name in ('quote','text','author','tag'):
            text=text.replace('class="'+name+'"','class="'+name+' extra"')
        return text
    if kind=='single_quotes':
        for name in ('quote','text','author','tag'):
            text=text.replace('class="'+name+'"',"class='"+name+"'")
        return text
    raise ValueError('invalid_mutation')


def parse(pages):
    rows=[]
    for page in pages:
        parser=real_web_oracle.Parser();parser.feed(page);rows.extend(parser.rows)
    return rows


def main():
    directory=ROOT/'.verification/real-web-code'/('robustness-'+str(uuid.uuid4()));directory.mkdir(parents=True,mode=0o700)
    with real_web_read_proxy.serve() as proxy:
        pages=[]
        for page in (1,2):
            with urlopen(f'http://127.0.0.1:{proxy["port"]}/page/{page}',timeout=12) as response:
                pages.append(response.read().decode('utf-8'))
    expected=parse(pages)
    if len(expected)!=20:raise RuntimeError('unexpected_source')
    baselines={arm:json.loads((ROOT/'.verification/real-web-code'/id/'result.json').read_text()) for arm,id in SAMPLES.items()}
    if any(result['site_snapshot_sha256']!=__import__('hashlib').sha256(json.dumps(expected,ensure_ascii=False,sort_keys=True).encode()).hexdigest() for result in baselines.values()):
        raise RuntimeError('source_changed_since_experiment')
    results=[]
    for kind in ('attribute_order','extra_class','single_quotes'):
        changed=[transform(page,kind) for page in pages]
        if changed==pages or parse(changed)!=expected:raise RuntimeError('mutation_not_equivalent')
        class Handler(BaseHTTPRequestHandler):
            def log_message(self,*args):pass
            def do_GET(self):
                index={'/page/1':0,'/page/2':1}.get(self.path)
                if index is None:self.send_error(403);return
                data=changed[index].encode();self.send_response(200);self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
        server=ThreadingHTTPServer(('127.0.0.1',0),Handler);thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
        try:
            for arm,id in SAMPLES.items():
                source=(ROOT/'.verification/real-web-code'/id/'solution.js').read_text()
                result=real_web_code_sandbox.execute(source,server.server_port)
                results.append({'mutation':kind,'arm':arm,'passed':result['exit_code']==0 and result['output']==expected,
                                'exit_code':result['exit_code'],'elapsed_ms':result['elapsed_ms']})
        finally:server.shutdown();server.server_close();thread.join(timeout=5)
    common.save(directory/'result.json',{'cases':results,'purpose':'semantic_equivalence_mutations','source_rows':len(expected)})
    print(json.dumps({'directory':str(directory),'results':results}))


if __name__=='__main__':main()
