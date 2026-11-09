#!/usr/bin/env python3
import sqlite3
import tempfile
from pathlib import Path
import unittest
import pilot_gennie_real


class GennieProfileTest(unittest.TestCase):
    def test_quality_requires_exact_mailto_values_and_successful_execution(self):
        output={'state':'completed','output':{'status':'ok','data':{
            'emails':['mailto:ana@example.test','mailto:luis@example.test']}}}
        self.assertTrue(pilot_gennie_real.quality('simple',output)['passed'])
        output['output']['data']['emails'][0]='ana@example.test'
        self.assertEqual(pilot_gennie_real.quality('simple',output)['reason'],'output_mismatch')
        output['state']='waiting_user'
        self.assertEqual(pilot_gennie_real.quality('simple',output)['reason'],'builder_not_completed')

    def test_reads_requested_profile_without_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'fixture.sqlite3'
            with sqlite3.connect(path) as connection:
                connection.execute('CREATE TABLE llm_architects(id TEXT,model TEXT,effort TEXT,read_files INTEGER,write_files INTEGER,run_commands INTEGER,local_actions INTEGER)')
                connection.execute('CREATE TABLE llm_architect_prompts(architect_id TEXT,personality TEXT,instructions TEXT)')
                connection.execute("INSERT INTO llm_architects VALUES('plugin_builder','model-x','medium',1,1,1,1)")
                connection.execute("INSERT INTO llm_architect_prompts VALUES('plugin_builder','persona','instrucciones')")
            before=path.read_bytes()
            result=pilot_gennie_real.settings(path)
            self.assertEqual(result['instructions'],'instrucciones')
            self.assertEqual(result['model'],'model-x')
            self.assertEqual(path.read_bytes(),before)


if __name__=='__main__':
    unittest.main()
