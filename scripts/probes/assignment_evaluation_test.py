"""Controles del registro y del protocolo; sin modelos, cuentas o red."""
import contextlib
import csv
import io
import json
import os
from pathlib import Path
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import assignment_evaluation as driver
import assignment_evaluation_worker as worker


class ProtocolTests(unittest.TestCase):
    def test_schema_inventory_compares_received_contracts_without_capturing_auth(self):
        expected = {'assignments_propose': {'type':'object','additionalProperties':False}}
        server = {'tools': {'assignments_propose': {'inputSchema':dict(reversed(list(expected['assignments_propose'].items()))), 'description':'NO_CAPTURE'}}, 'auth':'SECRET'}
        evidence = worker.schema_inventory(server, expected)
        self.assertTrue(evidence['assignments_propose']['matches'])
        self.assertNotIn('SECRET', json.dumps(evidence))
        self.assertNotIn('NO_CAPTURE', json.dumps(evidence))
        server['tools']['assignments_propose']['inputSchema']['additionalProperties'] = 0
        self.assertFalse(worker.schema_inventory(server, expected)['assignments_propose']['matches'])
        del server['tools']['assignments_propose']['inputSchema']
        self.assertFalse(worker.schema_inventory(server, expected)['assignments_propose']['matches'])
        with self.assertRaisesRegex(worker.Failure, 'expected_schema_unavailable'):
            worker.schema_inventory(server, {})

    def test_live_schema_mismatch_stops_before_generation(self):
        methods=[]
        def call(identifier, method, params):
            methods.append(method)
            responses={
                'initialize':{}, 'account/read':{'account':{'type':'chatgpt'}},
                'model/list':{'data':[{'model':'gpt-6-luna','supportedReasoningEfforts':[{'reasoningEffort':'low'}]}]},
                'thread/start':{'thread':{'id':'fictitious'}},
                'mcpServerStatus/list':{'data':[{'name':'gennie','tools':{'assignments_propose':{'inputSchema':{'type':'string'}}}}]},
            }
            return responses[method]
        fake=SimpleNamespace(call=call,send=lambda _:None,thread_id=None,turn_started=False,
            usage=None,messages={},tool_types=[],retries=0,selector=SimpleNamespace(close=lambda:None))
        with tempfile.TemporaryDirectory() as directory:
            spec={'mode':'live','base':str(Path(directory)/'provider'),'model':'gpt-6-luna','effort':'low',
                'timeout_seconds':1,'mcp_config':'fixture','instructions':'fixture','tool_names':['assignments_propose'],
                'expected_tool_schemas':{'assignments_propose':{'type':'object'}}}
            process=SimpleNamespace(poll=lambda:0)
            with patch.dict(os.environ, {'CODEX_HOME':directory}), patch.object(worker.subprocess,'Popen',return_value=process), patch.object(worker,'Client',return_value=fake):
                result=worker.run(spec)
        self.assertEqual(result['status'],'mcp_schema_mismatch')
        self.assertEqual(result['generation_turns'],0)
        self.assertNotIn('turn/start',methods)

    def test_usage_preserves_reported_zero_but_not_missing_or_sensitive_fields(self):
        result=worker.usage_snapshot({'total':{'inputTokens':0,'outputTokens':4,'cachedInputTokens':True,'totalTokens':-1,'api_key':'SECRET'}})
        self.assertEqual(result,{'total':{'inputTokens':0,'outputTokens':4}})
        self.assertIsNone(worker.usage_snapshot({'total':{'auth':'SECRET'}}))
        self.assertIsNone(worker.usage_snapshot(None))

    def test_buffered_frames_preserve_early_completion_and_ignore_foreign_text(self):
        read_fd,write_fd=os.pipe()
        output=os.fdopen(read_fd,'rb',buffering=0)
        class Process:
            stdout=output
            stdin=io.BytesIO()
        client=worker.Client(Process(),time.monotonic()+1)
        client.thread_id='own';client.generation_requested=True
        frames=[{'method':'turn/completed','params':{'threadId':'own','turn':{'id':'t1','status':'completed'}}},
                {'id':6,'result':{'turn':{'id':'t1'}}},
                {'method':'item/completed','params':{'threadId':'foreign','item':{'type':'agentMessage','text':'DO_NOT_CAPTURE'}}},
                {'method':'item/completed','params':{'threadId':'own','item':{'id':'a1','type':'agentMessage','text':'Respuesta ficticia'}}}]
        try:
            os.write(write_fd,b'\n'.join(json.dumps(row).encode() for row in frames)+b'\n')
            self.assertEqual(client.call(6,'turn/start',{}),{'turn':{'id':'t1'}})
            client.next();client.next()
            self.assertEqual(client.completed_turns,{'t1':'completed'})
            self.assertEqual(list(client.messages.values()),['Respuesta ficticia'])
        finally:
            client.selector.close();output.close();os.close(write_fd)

    def test_pre_request_text_and_unavailable_actions_are_not_accepted(self):
        read_fd,write_fd=os.pipe();output=os.fdopen(read_fd,'rb',buffering=0)
        class Process:
            stdout=output
            stdin=io.BytesIO()
        client=worker.Client(Process(),time.monotonic()+1)
        try:
            client.observe({'method':'item/completed','params':{'item':{'type':'agentMessage','text':'DO_NOT_STORE'}}})
            self.assertTrue(client.unexpected_output);self.assertFalse(client.messages)
            os.write(write_fd,json.dumps({'id':99,'method':'requestApproval','params':{}}).encode()+b'\n')
            with self.assertRaisesRegex(worker.Failure,'unavailable_action_requested'):client.next()
            self.assertIn(b'-32601',Process.stdin.getvalue())
        finally:
            client.selector.close();output.close();os.close(write_fd)


class ReportTests(unittest.TestCase):
    def test_explicit_cases_limit_budgets_and_preserve_corpus_order(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);corpus=root/'corpus.json'
            corpus.write_text(json.dumps({'version':1,'seed':1,'cases':[
                {'id':'already_measured','family':'proposals'},
                {'id':'missing_time','family':'proposals'},
                {'id':'notification_noise','family':'proposals'},
                {'id':'stale_context','family':'contract'}]}))
            args=SimpleNamespace(scope='full',cases=['notification_noise','missing_time'],with_guide=True,
                                 guide_env=None,repetitions=1,budget_minutes=20)
            with patch.object(driver,'ROOT',root),patch.object(driver,'CORPUS',corpus),patch.object(driver,'fingerprint',return_value={}),patch.object(driver,'binaries',return_value={}),contextlib.redirect_stdout(io.StringIO()):
                driver.prepare(args)
            plan=driver.read(next(root.glob('.verification/assignment-evaluation/*/plan.json')))
            self.assertEqual(plan['cases'],['missing_time','notification_noise'])
            self.assertEqual(plan['metadata']['model_cells_max'],8)
            self.assertEqual(plan['metadata']['guide_calls_max'],4)
            self.assertFalse(plan['metadata']['stop_on_failed_model_cell'])
            self.assertFalse(driver.read(next(root.glob('.verification/assignment-evaluation/*/READY.json')))['execution_started'])

    def test_invalid_case_selection_creates_no_executable_plan(self):
        for scope, selected in [('pilot',['known']),('full',['unknown']),('full',['control']),('full',['known','known'])]:
            with self.subTest(scope=scope,selected=selected),tempfile.TemporaryDirectory() as temporary:
                root=Path(temporary);corpus=root/'corpus.json'
                corpus.write_text(json.dumps({'version':1,'seed':1,'cases':[
                    {'id':'known','family':'proposals'},{'id':'control','family':'contract'}]}))
                args=SimpleNamespace(scope=scope,cases=selected,with_guide=False,guide_env=None,
                                     repetitions=1,budget_minutes=1)
                with patch.object(driver,'ROOT',root),patch.object(driver,'CORPUS',corpus),patch.object(driver,'fingerprint',return_value={}),patch.object(driver,'binaries',return_value={}),contextlib.redirect_stdout(io.StringIO()):
                    with self.assertRaises(SystemExit):driver.prepare(args)
                self.assertFalse((root/'.verification').exists())

    def test_submission_preparation_uses_one_turn_and_one_guide_call_without_comparison(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);corpus=root/'corpus.json'
            corpus.write_text(json.dumps({'version':1,'seed':1,'cases':[{'id':'duplicate_scope','family':'proposals'}]}))
            args=SimpleNamespace(scope='submission',with_guide=True,guide_env=None,repetitions=1,budget_minutes=2)
            with patch.object(driver,'ROOT',root),patch.object(driver,'CORPUS',corpus),patch.object(driver,'fingerprint',return_value={}),patch.object(driver,'binaries',return_value={}),contextlib.redirect_stdout(io.StringIO()):
                driver.prepare(args)
            plan=driver.read(next(root.glob('.verification/assignment-evaluation/*/plan.json')))
            self.assertEqual(plan['cases'],['duplicate_scope'])
            self.assertEqual(plan['representations'],['flat_v1'])
            self.assertEqual(plan['strategies'],['local_rules','bot_guide'])
            self.assertEqual(plan['metadata']['model_cells_max'],1)
            self.assertEqual(plan['metadata']['guide_calls_max'],1)
            self.assertTrue(plan['metadata']['stop_on_failed_model_cell'])
            self.assertEqual(plan['total_timeout_seconds'],120)

    def test_smoke_preparation_limits_to_one_case_four_turns_and_two_guide_calls(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);corpus=root/'corpus.json'
            corpus.write_text(json.dumps({'version':1,'seed':1,'cases':[{'id':'duplicate_scope','family':'proposals'}]}))
            args=SimpleNamespace(scope='smoke',with_guide=True,guide_env=None,repetitions=1,budget_minutes=5)
            with patch.object(driver,'ROOT',root),patch.object(driver,'CORPUS',corpus),patch.object(driver,'fingerprint',return_value={}),patch.object(driver,'binaries',return_value={}),contextlib.redirect_stdout(io.StringIO()):
                driver.prepare(args)
            plan=driver.read(next(root.glob('.verification/assignment-evaluation/*/plan.json')))
            self.assertEqual(plan['cases'],['duplicate_scope'])
            self.assertEqual(plan['metadata']['model_cells_max'],4)
            self.assertEqual(plan['metadata']['guide_calls_max'],2)
            self.assertTrue(plan['metadata']['stop_on_failed_model_cell'])
            self.assertEqual(plan['total_timeout_seconds'],300)

    def test_seal_tracks_nested_source_additions_edits_and_removals_without_reading_env(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            corpus=root/'corpus.json';corpus.write_text('{}')
            for name in ('Cargo.toml','Cargo.lock'): (root/name).write_text('fixture')
            crate=root/'kernel/mcp';(crate/'src/nested').mkdir(parents=True)
            (crate/'Cargo.toml').write_text('fixture')
            source=crate/'src/nested/io.rs';source.write_text('first')
            (root/'.env').write_text('PRIVATE_NOT_PART_OF_SEAL')
            with patch.object(driver,'ROOT',root),patch.object(driver,'CORPUS',corpus):
                before=driver.fingerprint()
                source.write_text('second');edited=driver.fingerprint()
                self.assertNotEqual(before['kernel/mcp/src/nested/io.rs'],edited['kernel/mcp/src/nested/io.rs'])
                self.assertNotIn('.env',edited)
                added=crate/'src/added.rs';added.write_text('new')
                self.assertIn('kernel/mcp/src/added.rs',driver.fingerprint())
                added.unlink();self.assertEqual(edited,driver.fingerprint())

    def fixture(self,directory):
        corpus=directory/'corpus.json'
        corpus.write_text(json.dumps({'cases':[{'id':'case1','family':'proposals','request':'Petición ficticia','rubric':['Criterio declarado']}]}))
        plan={'version':1,'mode':'live','metadata':{'run_id':'sample','source_sha256':{}},'cases':['case1']}
        driver.save(directory/'plan.json',plan)
        driver.save(directory/'READY.json',{'plan_sha256':driver.plan_digest(plan)})
        row={'case_id':'case1','family':'proposals','strategy':'bot','representation':'flat_v1','repetition':1,'status':'completed','model_turns':1,'proposals':[{'candidate':{'questions':[]}}],'machine':{'checks':{'plans_unchanged':True},'semantic_quality':None}}
        (directory/'live.jsonl').write_text(json.dumps(row)+'\n')
        return corpus,row

    def report(self,directory,corpus):
        with patch.object(driver,'CORPUS',corpus),contextlib.redirect_stdout(io.StringIO()):driver.report(directory)

    def test_blind_ids_and_existing_ratings_survive_later_protocol_and_model_rows(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory=Path(temporary);corpus,row=self.fixture(directory);self.report(directory,corpus)
            first=driver.read(directory/'review-packet.json')[0]['blind_id']
            with (directory/'ratings.csv').open(newline='') as file:reader=csv.DictReader(file);columns=reader.fieldnames;ratings=list(reader)
            ratings[0].update(reviewer='reviewer1',reviewer_kind='human',usefulness_0_to_2='2',clarity_0_to_2='1',questions_0_to_2='2',scope_preserved='yes')
            with (directory/'ratings.csv').open('w',newline='') as file:writer=csv.DictWriter(file,fieldnames=columns);writer.writeheader();writer.writerows(ratings)
            (directory/'protocol.jsonl').write_text(json.dumps({'case_id':'case1','family':'proposals','strategy':'bot','status':'protocol_verified','model_turns':0})+'\n')
            row['representation']='grouped_v1'
            with (directory/'live.jsonl').open('a') as file:file.write(json.dumps(row)+'\n')
            self.report(directory,corpus)
            packet=driver.read(directory/'review-packet.json')
            self.assertEqual(len(packet),2);self.assertIn(first,[r['blind_id'] for r in packet])
            with (directory/'ratings.csv').open(newline='') as file:after=list(csv.DictReader(file))
            self.assertEqual(len(after),2);self.assertEqual(after[0]['usefulness_0_to_2'],'2')
            summary=driver.read(directory/'summary.json')
            self.assertEqual(summary['ratings']['submitted'],1)
            self.assertIsNone(summary['semantic_quality']);self.assertIsNone(summary['monetary_cost_total'])
            self.assertFalse(summary['ratings']['reviewer_identity_verified'])

    def test_live_requires_exact_plan_and_ack_before_any_subprocess(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory=Path(temporary);self.fixture(directory)
            with patch.object(driver,'fingerprint',return_value={}),patch.object(driver.subprocess,'run') as run:
                with self.assertRaises(SystemExit):driver.execute(directory,'live','wrong')
                run.assert_not_called()
                plan=driver.read(directory/'plan.json');plan['cases'].append('unapproved')
                (directory/'plan.json').write_text(json.dumps(plan))
                with self.assertRaises(SystemExit):driver.execute(directory,'live','sample')
                run.assert_not_called()

    def test_malformed_rows_are_reported_without_counting_missing_data_as_success(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory=Path(temporary);corpus,_=self.fixture(directory)
            with (directory/'live.jsonl').open('a') as file:file.write('{partial\n')
            self.report(directory,corpus)
            summary=driver.read(directory/'summary.json')
            self.assertEqual(summary['malformed_rows'],[{'file':'live.jsonl','line':2}])
            self.assertIsNone(summary['semantic_quality'])


class ExecutionArtifactTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory=Path(self.temp.name)
        self.corpus=self.directory/'corpus.json'
        self.corpus.write_text(json.dumps({'cases':[{'id':'case1','family':'proposals'}]}))
        self.plan={'mode':'protocol','cases':['case1'],'repetitions':1,'strategies':['bot'],
                   'representations':['flat_v1'],'metadata':{'fixture':True}}
        self.manifest={'version':1,'mode':'protocol','metadata':self.plan['metadata'],'sequence_count':1}
        self.rows=[{'case_id':'case1','strategy':'bot','representation':'flat_v1','repetition':1,
                    'status':'protocol_verified','model_turns':0,'guide_calls':0,
                    'provider':{'status':'protocol_verified','generation_turns':0,
                                'schema_inventory':{'fixture':{'matches':True}}}}]

    def valid(self):
        with patch.object(driver,'CORPUS',self.corpus):
            return driver.result_artifacts_match(self.directory,self.plan)

    def write(self):
        (self.directory/'protocol-manifest.json').write_text(json.dumps(self.manifest))
        (self.directory/'protocol.jsonl').write_text(''.join(json.dumps(row)+'\n' for row in self.rows))

    def test_missing_empty_or_partial_artifacts_do_not_pass(self):
        self.assertFalse(self.valid())
        self.write();self.assertTrue(self.valid())
        (self.directory/'protocol.jsonl').write_text('')
        self.assertFalse(self.valid())
        (self.directory/'protocol.jsonl').write_text('{partial\n')
        self.assertFalse(self.valid())

    def test_duplicate_missing_or_foreign_cells_do_not_pass(self):
        for rows in ([],self.rows*2,[dict(self.rows[0],case_id='other')]):
            self.rows=rows;self.write();self.assertFalse(self.valid())

    def test_protocol_generation_and_boolean_counts_do_not_pass(self):
        for change in ({'model_turns':1},{'guide_calls':1},{'model_turns':False},{'repetition':True}):
            original=self.rows[0]
            self.rows=[dict(original,**change)];self.write();self.assertFalse(self.valid())
            self.rows=[original]

    def test_manifest_mode_metadata_and_counts_must_match(self):
        for change in ({'mode':'live'},{'metadata':{}},{'sequence_count':0},{'sequence_count':True}):
            original=self.manifest
            self.manifest=dict(original,**change);self.write();self.assertFalse(self.valid())
            self.manifest=original

    def test_live_failed_and_not_run_cells_are_preserved_as_observations(self):
        self.plan['mode']='live';self.manifest['mode']='live'
        for status,count in (('provider_failed',None),('not_run_budget',0),('not_run_after_technical_failure',0)):
            row=dict(self.rows[0],status=status,model_turns=count)
            (self.directory/'live-manifest.json').write_text(json.dumps(self.manifest))
            (self.directory/'live.jsonl').write_text(json.dumps(row)+'\n')
            self.assertTrue(self.valid())

    def test_closed_or_unknown_mode_never_launches(self):
        with patch.object(driver.subprocess,'run') as run:
            with self.assertRaises(SystemExit):driver.execute(self.directory,'automatic')
            (self.directory/'CLOSURE.json').write_text('{}')
            with self.assertRaises(SystemExit):driver.execute(self.directory,'protocol')
            run.assert_not_called()


if __name__=='__main__':unittest.main()
