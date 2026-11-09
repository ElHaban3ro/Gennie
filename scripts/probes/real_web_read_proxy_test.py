#!/usr/bin/env python3
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
import threading
import unittest
from unittest.mock import patch
from urllib.error import HTTPError
from urllib.request import Request,urlopen

import real_web_read_proxy


class Upstream(BaseHTTPRequestHandler):
    def log_message(self,*args):pass
    def do_GET(self):
        body=b'fixture';self.send_response(200);self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)


class ProxyTest(unittest.TestCase):
    def test_only_get_to_whitelisted_routes_and_bounded_requests(self):
        server=ThreadingHTTPServer(('127.0.0.1',0),Upstream)
        thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
        try:
            with patch.dict(real_web_read_proxy.URLS,{'/page/1':f'http://127.0.0.1:{server.server_port}/','/page/2':f'http://127.0.0.1:{server.server_port}/'}):
                with real_web_read_proxy.serve() as proxy:
                    base=f'http://127.0.0.1:{proxy["port"]}'
                    with self.assertRaises(HTTPError) as error:urlopen(base+'/other',timeout=2)
                    self.assertEqual(error.exception.code,403)
                    with self.assertRaises(HTTPError) as error:urlopen(Request(base+'/page/1',data=b'x',method='POST'),timeout=2)
                    self.assertEqual(error.exception.code,405)
                    for _ in range(40):
                        with urlopen(base+'/page/1',timeout=2) as response:self.assertEqual(response.read(),b'fixture')
                    with self.assertRaises(HTTPError) as error:urlopen(base+'/page/1',timeout=2)
                    self.assertEqual(error.exception.code,429)
        finally:
            server.shutdown();server.server_close();thread.join(timeout=5)


if __name__=='__main__':unittest.main()
