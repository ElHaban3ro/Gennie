#!/usr/bin/env python3
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import codex_comparison_mcp as fixture


class McpFixtureTests(unittest.TestCase):
    def test_stdio_contract_and_isolated_trace(self):
        with tempfile.TemporaryDirectory() as directory:
            trace = Path(directory)/'trace.jsonl'
            env = dict(os.environ, CONTINUITY_COMPARISON_TRACE=str(trace))
            requests = [
                {'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-03-26'}},
                {'jsonrpc':'2.0','id':2,'method':'tools/list'},
                {'jsonrpc':'2.0','id':3,'method':'tools/call','params':{'name':'probe_read','arguments':{'target':'alpha'}}},
            ]
            process = subprocess.run([sys.executable,fixture.__file__],input=''.join(json.dumps(v)+'\n' for v in requests),
                                     capture_output=True,text=True,env=env,timeout=5,check=True)
            responses = [json.loads(v) for v in process.stdout.splitlines()]
            self.assertEqual(responses[1]['result']['tools'][0]['inputSchema'], fixture.SCHEMA)
            self.assertEqual(responses[2]['result']['structuredContent']['value'],'LIMA-42')
            self.assertEqual([json.loads(v)['event'] for v in trace.read_text().splitlines()],['mcp_initialize','mcp_tools_list','call','observation'])


if __name__ == '__main__':
    unittest.main()
