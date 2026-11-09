#!/usr/bin/env python3
import tempfile
from pathlib import Path
import unittest
import pilot_architecture_runner as runner


class RunnerTests(unittest.TestCase):
    def test_roles_require_observed_tools_and_aggregate_tokens_once(self):
        recipe = {'source':'main','fields':{'emails':{'selector':'a.mail','attribute':'href'}}}
        def fake(case,role,payload,**kwargs):
            result = {'status':'completed','usage_reported':{'total':{'inputTokens':10,'outputTokens':2,'totalTokens':12}}}
            response = {'architect':{'objective':'extraer enlaces'},'decision':{'engineer':'web'},'planner':{'proposals':['a','b']},
                        'executor':{'recipe':recipe,'verdict':'done'},'parent_validator':{'accepted':True}}[role]
            tools = {'planner':'inspect_page','executor':'test_recipe','parent_validator':'test_recipe'}
            return {'result':result,'response':response,'calls':[{'tool':tools[role]}] if role in tools else [],'temporary_removed':True}
        with tempfile.TemporaryDirectory() as directory:
            result = runner.run_cell('simple',mode='live',directory=Path(directory)/'run',session=fake)
            self.assertEqual(result['outcome']['status'],'completed')
            self.assertEqual(result['roles'],5)
            self.assertEqual(result['usage_total']['totalTokens'],60)
            self.assertIsNone(result['usage_total']['cachedInputTokens'])
            self.assertTrue(result['usage_complete'])

    def test_missing_tool_prevents_false_success(self):
        def fake(case,role,payload,**kwargs):
            return {'result':{'status':'completed','usage_reported':None},
                    'response':{'objective':'extraer enlaces'} if role=='architect' else {'engineer':'web'} if role=='decision' else {'proposals':['a','b']},'calls':[],'temporary_removed':True}
        with tempfile.TemporaryDirectory() as directory:
            result = runner.run_cell('simple',mode='live',directory=Path(directory)/'run',session=fake)
            self.assertEqual(result['outcome']['status'],'required_tool_not_observed')
            self.assertFalse(result['usage_complete'])
            self.assertIsNone(result['usage_total']['totalTokens'])


if __name__ == '__main__':
    unittest.main()
