#!/usr/bin/env python3
"""Variantes equivalentes locales para validar código sobre HTML público real."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import threading
from urllib.request import urlopen

import real_web_code_robustness as mutations
import real_web_code_sandbox as sandbox


def check(source, port, expected):
    pages = []
    for number in (1, 2):
        with urlopen(f'http://127.0.0.1:{port}/page/{number}', timeout=12) as response:
            pages.append(response.read(100_001).decode('utf-8'))
    baseline = mutations.parse(pages)
    if len(baseline) != 20 or baseline != expected['quotes']:
        raise ValueError('site_changed')
    verdict = {}
    for kind in ('attribute_order', 'extra_class', 'single_quotes'):
        changed = [mutations.transform(page, kind) for page in pages]
        if changed == pages or mutations.parse(changed) != baseline:
            raise ValueError('invalid_transformation')

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass
            def do_GET(self):
                index = {'/page/1': 0, '/page/2': 1}.get(self.path)
                if index is None:
                    self.send_error(403)
                    return
                body = changed[index].encode('utf-8')
                self.send_response(200)
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)

        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            result = sandbox.execute(source, server.server_port)
            verdict[kind] = result['exit_code'] == 0 and result['output'] == expected
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)
            if thread.is_alive():
                raise RuntimeError('variant_server_shutdown_failed')
    return verdict
