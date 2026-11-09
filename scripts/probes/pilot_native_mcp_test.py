#!/usr/bin/env python3
import unittest
import tempfile
import json
import os
import subprocess
import sys
from pathlib import Path
import pilot_native_mcp as mcp


class NativeMcpTest(unittest.TestCase):
    def test_decision_and_usage_are_forwarded(self):
        received = []
        def choose(evidence,instructions,choices):
            received.append((evidence,instructions,choices))
            return {'choice':'web','usage':{'total_tokens':8}}
        result = mcp.invoke('simple','decide',{'evidence':'t','instructions':'i','choices':['web','desktop']},choose)
        self.assertEqual(result['usage']['total_tokens'],8)
        self.assertEqual(received,[('t','i',['web','desktop'])])

    def test_rejects_wrong_arguments(self):
        with self.assertRaises(ValueError):
            mcp.invoke('simple','decide',{'choices':[]})

    def test_plan_requires_distinct_proposals_and_error_lists(self):
        plan={'proposals':['principal','alternativa'],'static_errors':['inaccesible'],'dynamic_errors':[]}
        self.assertEqual(mcp.invoke('simple','record_plan',plan)['proposal_count'],2)
        plan['proposals']=['principal','principal']
        with self.assertRaises(ValueError):
            mcp.invoke('simple','record_plan',plan)

    def test_budget_shared_by_file_and_fails_closed(self):
        with tempfile.TemporaryDirectory() as root:
            trace = str(Path(root)/'trace')
            self.assertTrue(mcp.reserve(trace,'recipe_tests',2))
            self.assertTrue(mcp.reserve(trace,'recipe_tests',2))
            self.assertFalse(mcp.reserve(trace,'recipe_tests',2))

    def test_recipe_budget_is_independent_per_verified_thread(self):
        with tempfile.TemporaryDirectory() as root:
            trace=str(Path(root)/'trace')
            first={'threadId':'00000000-0000-4000-8000-000000000001'}
            second={'threadId':'00000000-0000-4000-8000-000000000002'}
            self.assertEqual(mcp.reserve_recipe(trace,{}),'thread_identity_unavailable')
            for _ in range(4):
                self.assertIsNone(mcp.reserve_recipe(trace,first))
            self.assertEqual(mcp.reserve_recipe(trace,first),'executor_attempt_budget_exhausted')
            self.assertIsNone(mcp.reserve_recipe(trace,second))

    def test_injected_failures_are_bounded_and_plan_is_required(self):
        with tempfile.TemporaryDirectory() as root:
            env=os.environ.copy()
            env.update(GENNIE_PILOT_CASE='simple',GENNIE_PILOT_TRACE=str(Path(root)/'trace'),GENNIE_PILOT_RECIPE_FAULTS='2')
            recipe={'source':'main','fields':{'emails':{'selector':'a.mail','attribute':'href'}}}
            plan={'proposals':['principal','alternativa'],'static_errors':['inaccesible'],'dynamic_errors':[]}
            calls=[('test_recipe',{'recipe':recipe}),('record_plan',plan)]+[('test_recipe',{'recipe':recipe})]*3
            messages=[{'jsonrpc':'2.0','id':i,'method':'tools/call','params':{'name':name,'arguments':args,
                      '_meta':{'threadId':'00000000-0000-4000-8000-000000000001'}}}
                      for i,(name,args) in enumerate(calls)]
            process=subprocess.run([sys.executable,mcp.__file__],input=''.join(json.dumps(x)+'\n' for x in messages),
                                   text=True,capture_output=True,env=env,timeout=5,check=True)
            outputs=[json.loads(line)['result']['structuredContent']['status'] for line in process.stdout.splitlines()]
            self.assertEqual(outputs,['plan_required','plan_recorded','injected_transient_failure',
                                      'injected_transient_failure','verified'])

    def test_real_plugin_mode_does_not_expose_fixture_recipe(self):
        with tempfile.TemporaryDirectory() as root:
            env=os.environ.copy()
            env.update(GENNIE_PILOT_CASE='simple',GENNIE_PILOT_TRACE=str(Path(root)/'trace'),GENNIE_PILOT_PLUGIN_MODE='1')
            message={'jsonrpc':'2.0','id':1,'method':'tools/list','params':{}}
            process=subprocess.run([sys.executable,mcp.__file__],input=json.dumps(message)+'\n',
                                   text=True,capture_output=True,env=env,timeout=5,check=True)
            names={item['name'] for item in json.loads(process.stdout)['result']['tools']}
            self.assertEqual(names,{'record_plan','decide'})


if __name__ == '__main__':
    unittest.main()
