#!/usr/bin/env python3
import json
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
from pathlib import Path
import tempfile
import threading
import unittest

import real_web_code_sandbox
import real_web_oracle


class Handler(BaseHTTPRequestHandler):
    def log_message(self,*args):pass
    def do_GET(self):
        body=b'fixture';self.send_response(200);self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)


class SandboxTest(unittest.TestCase):
    def test_code_cannot_access_files_environment_processes_or_other_domains(self):
        server=ThreadingHTTPServer(('127.0.0.1',0),Handler)
        thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
        try:
            with tempfile.TemporaryDirectory() as temp:
                secret=Path(temp)/'private';secret.write_text('fixture')
                target=Path(temp)/'unexpected'
                code='''let out={};
for(const [name,fn] of Object.entries({read:()=>Deno.readTextFile(READ),write:()=>Deno.writeTextFile(WRITE,"x"),env:()=>Promise.resolve(Deno.env.get("HOME")),run:()=>new Deno.Command("/bin/true").output(),other:()=>fetch("https://example.com/")})){try{await fn();out[name]=true}catch(e){out[name]=false}}
out.site=(await fetch("https://quotes.toscrape.com/")).status;console.log(JSON.stringify(out));'''.replace('READ',json.dumps(str(secret))).replace('WRITE',json.dumps(str(target)))
                result=real_web_code_sandbox.execute(code,server.server_port)
                self.assertEqual(result['exit_code'],0)
                self.assertEqual(result['output'],{'read':False,'write':False,'env':False,'run':False,'other':False,'site':200})
                self.assertFalse(target.exists())
        finally:
            server.shutdown();server.server_close();thread.join(timeout=5)

    def test_oracle_parser_preserves_fields_and_html_entities(self):
        parser=real_web_oracle.Parser()
        parser.feed('<div class="quote"><span class="text">A &amp; B</span><small class="author">C</small><div class="tags"><a class="tag">x</a></div></div>')
        self.assertEqual(parser.rows,[{'text':'A & B','author':'C','tags':['x']}])


if __name__=='__main__':unittest.main()
