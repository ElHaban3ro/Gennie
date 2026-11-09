#!/usr/bin/env python3
"""Proxy GET acotado: solo dos páginas públicas reales del sitio de pruebas."""
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
import threading
from urllib.error import HTTPError,URLError
from urllib.request import HTTPRedirectHandler,build_opener

URLS={'/page/1':'https://quotes.toscrape.com/','/page/2':'https://quotes.toscrape.com/page/2/'}


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self,*args,**kwargs):
        return None


@contextmanager
def serve():
    accesses=[]
    lock=threading.Lock()
    permits=threading.BoundedSemaphore(4)
    class Handler(BaseHTTPRequestHandler):
        def log_message(self,*args): pass
        def do_POST(self):
            self.send_error(405)
        def do_GET(self):
            if self.path not in URLS:
                accesses.append({'allowed':False})
                self.send_error(403);return
            with lock:
                forwarded=sum(1 for item in accesses if item.get('page') in URLS)
                if forwarded>=40:
                    self.send_error(429);return
                accesses.append({'page':self.path,'status':'started'})
                entry=accesses[-1]
            if not permits.acquire(blocking=False):
                entry['status']='busy';self.send_error(429);return
            try:
                with build_opener(NoRedirect()).open(URLS[self.path],timeout=10) as response:
                    data=response.read(1_000_001)
                    if len(data)>1_000_000: raise ValueError('response_too_large')
                    self.send_response(response.status)
                    self.send_header('Content-Type','text/html; charset=utf-8')
                    self.send_header('Content-Length',str(len(data)))
                    self.end_headers();self.wfile.write(data)
                    entry.update(status=response.status,bytes=len(data))
            except (HTTPError,URLError,TimeoutError,ValueError,OSError):
                entry['status']='failed'
                try:self.send_error(502)
                except OSError:pass
            finally:permits.release()
    server=ThreadingHTTPServer(('127.0.0.1',0),Handler)
    server.daemon_threads=True
    thread=threading.Thread(target=server.serve_forever,name='gennie-readonly-web-proxy',daemon=True)
    thread.start()
    try:yield {'port':server.server_port,'accesses':accesses}
    finally:
        server.shutdown();server.server_close();thread.join(timeout=5)
        if thread.is_alive():raise RuntimeError('proxy_shutdown_failed')
