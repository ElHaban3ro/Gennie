#!/usr/bin/env python3
import io
import json
import unittest
import tempfile
from pathlib import Path
from unittest.mock import patch
import pilot_openai_decisions as decisions


class DecisionsTest(unittest.TestCase):
    def test_request_and_usage(self):
        def opener(request, timeout):
            payload = json.loads(request.data)
            self.assertEqual(request.full_url, decisions.ENDPOINT)
            self.assertEqual(payload['model'], 'gpt-6-luna')
            self.assertEqual(payload['questions'][0]['choices'], [{'value':'web'}, {'value':'desktop'}])
            self.assertEqual(timeout, 20)
            return io.BytesIO(json.dumps({'model':'gpt-6-luna', 'answers':[
                {'name':'route', 'type':'choice', 'choice':'web'}],
                'usage':{'input_tokens':10,'output_tokens':0,'total_tokens':10}}).encode())
        result = decisions.choose('fixture', 'Selecciona un ingeniero', ['web','desktop'],
                                  api_key='fake', opener=opener)
        self.assertEqual(result['choice'], 'web')
        self.assertEqual(result['usage']['total_tokens'], 10)

    def test_no_key_and_unknown_choice_fail_closed(self):
        with patch.dict('os.environ', {'OPENAI_API_KEY': ''}, clear=True):
            with self.assertRaisesRegex(decisions.DecisionError, 'missing_openai_api_key'):
                decisions.choose('x', 'y', ['a','b'])
        def opener(*args, **kwargs):
            return io.BytesIO(b'{"model":"gpt-6-luna","answers":[{"type":"choice","name":"route","choice":"c"}]}')
        with self.assertRaisesRegex(decisions.DecisionError, 'unexpected_answer'):
            decisions.choose('x', 'y', ['a','b'], api_key='fake', opener=opener)

    def test_file_and_environment_precedence(self):
        with tempfile.TemporaryDirectory() as root:
            env = Path(root) / '.env'
            env.write_text('OTHER=untouched\nOPENAI_API_KEY="file-key"\n')
            with patch.dict('os.environ', {}, clear=True):
                self.assertEqual(decisions.load_key(env), 'file-key')
            with patch.dict('os.environ', {'OPENAI_API_KEY': 'env-key'}, clear=True):
                self.assertEqual(decisions.load_key(env), 'env-key')
            with patch.dict('os.environ', {'OPENAI_API_KEY': ''}, clear=True):
                self.assertEqual(decisions.load_key(env), '')


if __name__ == '__main__':
    unittest.main()
