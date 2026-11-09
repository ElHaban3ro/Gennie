#!/usr/bin/env python3
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import pilot_mcp


class PilotMcpTests(unittest.TestCase):
    def test_stdio_and_trace(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'trace.jsonl'
            env = dict(os.environ,GENNIE_PILOT_CASE='simple',GENNIE_PILOT_TRACE=str(path))
            requests = [
                {'jsonrpc':'2.0','id':1,'method':'initialize'},
                {'jsonrpc':'2.0','id':2,'method':'tools/list'},
                {'jsonrpc':'2.0','id':3,'method':'tools/call','params':{'name':'inspect_page','arguments':{'source':'main'}}},
            ]
            result = subprocess.run([sys.executable,pilot_mcp.__file__],env=env,
                                    input=''.join(json.dumps(v)+'\n' for v in requests),
                                    capture_output=True,text=True,check=True,timeout=5)
            responses = [json.loads(row) for row in result.stdout.splitlines()]
            self.assertEqual(responses[1]['result']['tools'],pilot_mcp.TOOLS)
            self.assertEqual(responses[2]['result']['structuredContent']['status'],'ok')
            self.assertEqual(json.loads(path.read_text())['tool'],'inspect_page')

    def test_invalid_tool_or_source_is_rejected(self):
        with self.assertRaises(ValueError): pilot_mcp.invoke('simple','unknown',{})
        with self.assertRaises(ValueError): pilot_mcp.invoke('simple','inspect_page',{'source':'external'})


if __name__ == '__main__':
    unittest.main()
