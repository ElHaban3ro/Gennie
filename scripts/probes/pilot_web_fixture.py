#!/usr/bin/env python3
"""Sitio HTTP local y temporal compartido por las variantes del piloto."""
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import threading
from urllib.parse import urlsplit
from urllib.error import HTTPError, URLError
from urllib.request import urlopen

import pilot_fixture


def inspect(origin, source):
    """Inspecciona únicamente el servidor de pruebas en loopback."""
    parsed=urlsplit(origin)
    if parsed.scheme != 'http' or parsed.hostname != '127.0.0.1' or not parsed.port or parsed.path not in ('','/') or parsed.query or parsed.fragment or source not in ('main','backup'):
        raise ValueError('invalid_fixture_origin')
    try:
        with urlopen(origin.rstrip('/')+'/'+source,timeout=5) as response:
            body=response.read(100_001)
            if len(body)>100_000:
                raise ValueError('fixture_response_too_large')
            return {'status':'ok','html':body.decode('utf-8')}
    except HTTPError as error:
        if error.code == 503:
            return {'status':'unavailable','message':'La vía principal no responde; la interfaz alternativa backup está disponible.'}
        if error.code == 404:
            return {'status':'not_found'}
        raise ValueError('fixture_http_error') from None
    except (URLError,UnicodeError):
        raise ValueError('fixture_unavailable') from None


@contextmanager
def serve(case):
    if case not in pilot_fixture.CASES:
        raise ValueError('invalid_case')
    accesses = []

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def do_GET(self):
            path = urlsplit(self.path).path
            source = {'/main':'main','/backup':'backup'}.get(path)
            accesses.append({'path':path})
            if source is None:
                status, body = 404, 'Página inexistente'
            else:
                result = pilot_fixture.inspect(case,source)
                status = {'ok':200,'unavailable':503,'not_found':404}[result['status']]
                body = result.get('html',result.get('message','Página inexistente'))
            encoded = ('<!doctype html><html lang="es"><head><meta charset="utf-8">'
                       '<title>Directorio ficticio</title></head><body>'+body+'</body></html>').encode('utf-8')
            self.send_response(status)
            self.send_header('Content-Type','text/html; charset=utf-8')
            self.send_header('Content-Length',str(len(encoded)))
            self.end_headers()
            self.wfile.write(encoded)

    server = ThreadingHTTPServer(('127.0.0.1',0),Handler)
    thread = threading.Thread(target=server.serve_forever,name='gennie-pilot-http',daemon=True)
    thread.start()
    try:
        yield {'origin':f'http://127.0.0.1:{server.server_port}','accesses':accesses}
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)
        if thread.is_alive():
            raise RuntimeError('fixture_server_shutdown_failed')
