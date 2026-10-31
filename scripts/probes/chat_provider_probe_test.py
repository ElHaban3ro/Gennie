#!/usr/bin/env python3
"""El protocolo no puede convertirse en live por un modo/ACK/plan cambiado."""
import json
from pathlib import Path
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import patch
import chat_provider_probe as probe


class GateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name)
        self.plan = {'version':2,'scenario':'stop_resume','mode':'live',
                     'output_dir':str(self.path),'nonce':'a'*32,
                     'model':'gpt-6-luna','effort':'low','model_turns_max':2,
                     'cell_timeout_seconds':60}
        (self.path/'sources.tar.gz').write_bytes(b'fixture')
        (self.path/'plan.json').write_text(json.dumps(self.plan))
        self.seal = {'run_id':'fixture','plan_sha256':probe.common.plan_digest(self.plan),
                     'sources':{},'binaries':{},
                     'archive_sha256':probe.common.sha_file(self.path/'sources.tar.gz')}
        (self.path/'READY.json').write_text(json.dumps(self.seal))

    def blocked(self, mode, ack=None, source_value=None, binary_value=None):
        with patch.object(probe,'sources',return_value=source_value or {}), patch.object(probe,'binaries',return_value=binary_value or {}), patch.object(probe.subprocess,'run') as run:
            with self.assertRaises(SystemExit): probe.execute(self.path,mode,ack)
            run.assert_not_called()
        self.assertFalse((self.path/f'{mode}-invocation.json').exists())

    def test_unknown_mode_never_executes(self): self.blocked('automatic')
    def test_live_without_exact_ack_never_executes(self): self.blocked('live','other')
    def test_changed_plan_never_executes_even_with_ack(self):
        (self.path/'plan.json').write_text(json.dumps(dict(self.plan,model_turns_max=8)))
        self.blocked('live','fixture')
    def test_changed_source_never_executes_even_with_ack(self):
        self.blocked('live','fixture',source_value={'changed':'sha'})
    def test_changed_binary_never_executes_even_with_ack(self):
        self.blocked('live','fixture',binary_value={'changed':'sha'})
    def test_closed_plan_never_executes_even_with_ack(self):
        (self.path/'CLOSURE.json').write_text('{}')
        self.blocked('live','fixture')
    def test_changed_archive_never_executes_even_with_ack(self):
        (self.path/'sources.tar.gz').write_bytes(b'changed')
        self.blocked('live','fixture')

    def test_zero_exit_without_result_is_not_success(self):
        with patch.object(probe,'sources',return_value={}), patch.object(probe,'binaries',return_value={}), patch.object(probe.subprocess,'run',return_value=SimpleNamespace(returncode=0)):
            with self.assertRaises(SystemExit): probe.execute(self.path,'protocol')

    def reseal(self, **changes):
        plan = dict(self.plan,**changes)
        (self.path/'plan.json').write_text(json.dumps(plan))
        seal = dict(self.seal,plan_sha256=probe.common.plan_digest(plan))
        (self.path/'READY.json').write_text(json.dumps(seal))

    def protocol_report(self):
        return {'mode':'protocol','scenario':'stop_resume','status':'completed',
                'profile_removed':True,'guide_calls':0,'model_turns':0,
                'protocol':{'account_linked':False,'generation_turns':0,
                            'distinct_threads':True,'local_fixture_contexts_isolated':True,
                            'recorded_active_matches':True,'cold_process_resume_same_thread':True}}

    def execute_report(self, mode, report):
        def run(*args,**kwargs):
            (self.path/f'{mode}-result.json').write_text(json.dumps(report))
            return SimpleNamespace(returncode=0)
        with patch.object(probe,'sources',return_value={}), patch.object(probe,'binaries',return_value={}), patch.object(probe.subprocess,'run',side_effect=run) as mocked:
            probe.execute(self.path,mode,'fixture')
            return mocked.call_args

    def test_signed_unknown_scenario_never_executes(self):
        self.reseal(scenario='automatic')
        self.blocked('live','fixture')

    def test_signed_scope_cannot_inherit_the_full_budget(self):
        for budget in (0,1,3,4):
            self.reseal(model_turns_max=budget)
            self.blocked('live','fixture')

    def test_legacy_or_boolean_budget_fields_never_execute(self):
        for changes in ({'version':1},{'version':True},{'model_turns_max':True},{'cell_timeout_seconds':True}):
            self.reseal(**changes)
            self.blocked('live','fixture')

    def test_output_cannot_escape_its_prepared_directory(self):
        self.reseal(output_dir=str(self.path/'other'))
        self.blocked('live','fixture')

    def test_prepared_mode_cannot_be_relabeled_protocol(self):
        self.reseal(mode='protocol')
        self.blocked('live','fixture')

    def test_valid_protocol_keeps_scope_and_removes_inherited_keys(self):
        with patch.dict(probe.os.environ,{'OPENAI_API_KEY':'fictional','GENNIE_DATABASE_PATH':'fictional'}):
            call = self.execute_report('protocol',self.protocol_report())
        env=call.kwargs['env']
        self.assertNotIn('OPENAI_API_KEY',env)
        self.assertNotIn('GENNIE_DATABASE_PATH',env)
        invocation=json.loads((self.path/'protocol-invocation.json').read_text())
        self.assertEqual(invocation['scenario'],'stop_resume')
        self.assertEqual(invocation['mode'],'protocol')
        self.assertEqual(invocation['model_turns_max'],2)
        self.assertTrue(json.loads((self.path/'protocol-completion.json').read_text())['result_contract_passed'])

    def test_zero_model_protocol_cannot_pass_as_live(self):
        report=dict(self.protocol_report(),mode='live')
        with self.assertRaises(SystemExit): self.execute_report('live',report)

    def test_report_from_another_scenario_does_not_pass(self):
        report=dict(self.protocol_report(),scenario='full')
        with self.assertRaises(SystemExit): self.execute_report('protocol',report)

    def test_protocol_without_cold_resume_proof_does_not_pass(self):
        report=self.protocol_report()
        del report['protocol']['cold_process_resume_same_thread']
        with self.assertRaises(SystemExit): self.execute_report('protocol',report)

    def test_boolean_zero_counts_do_not_pass(self):
        for key in ('model_turns','guide_calls'):
            report=dict(self.protocol_report(),**{key:False})
            self.assertFalse(probe.report_matches_plan(report,self.plan,'protocol'))

    def test_live_result_requires_the_exact_budget_and_nonempty_fragment(self):
        report={'mode':'live','scenario':'stop_resume','status':'completed',
                'profile_removed':True,'guide_calls':0,'model_turns':2,
                'stop':{'verified':True,'cancelled':True,'cancel_requested':True,'observed_characters':64},
                'resume':{'same_thread':True}}
        self.assertTrue(probe.report_matches_plan(report,self.plan,'live'))
        for count in (0,1,3,4,False):
            self.assertFalse(probe.report_matches_plan(dict(report,model_turns=count),self.plan,'live'))
        changed=dict(report,stop=dict(report['stop'],observed_characters=0))
        self.assertFalse(probe.report_matches_plan(changed,self.plan,'live'))
        changed=dict(report,resume={'same_thread':False})
        self.assertFalse(probe.report_matches_plan(changed,self.plan,'live'))

    def test_full_scenario_keeps_its_budget_and_additional_proofs(self):
        self.reseal(scenario='full',model_turns_max=2)
        self.blocked('live','fixture')
        plan=dict(self.plan,scenario='full',model_turns_max=4)
        report={'mode':'live','scenario':'full','status':'completed',
                'profile_removed':True,'guide_calls':0,'model_turns':4,
                'stop':{'verified':True,'cancelled':True,'cancel_requested':True,'observed_characters':64},
                'resume':{'same_thread':True}}
        self.assertFalse(probe.report_matches_plan(report,plan,'live'))
        report['quote']={'matched':True}
        report['new_thread']={'distinct':True,'quote_absent_from_provider_history':True,'literal_journal_input_preserved':True}
        self.assertTrue(probe.report_matches_plan(report,plan,'live'))
        self.assertFalse(probe.report_matches_plan(dict(report,model_turns=2),plan,'live'))

    def plugin_report(self, mode):
        report={'mode':mode,'scenario':'plugin_semantics','status':'completed',
                'profile_removed':True,'guide_calls':0,'model_turns':0 if mode == 'protocol' else 3,
                'protocol':{'account_linked':False,'generation_turns':0,
                            'schemas_exact':True,'cases_without_effects':True}}
        for key,text in (('ambiguous_recipient','Llegaré a las 8'),('uncertain_route','Recibido')):
            empty={'attempts':[],'fictional_effects':[],'run_attempts':0,'forbidden_attempt':False}
            case={'status':'completed','schemas_exact':True,'socket_removed':True,'thread_id':key,
                  'behaviour_verified':mode == 'live',
                  'responses':[],'ledger':dict(empty),'after_first':dict(empty)}
            if mode == 'live':
                effect={'route':'primary','arguments':{'recipient':'cata_work','message':text}}
                case['responses']=['Respuesta ficticia']*(2 if key == 'ambiguous_recipient' else 1)
                case['ledger']={'attempts':[{'name':'plugin_mcp_run','arguments':{'server_id':'primary',
                    'tool':'send_message','expected_revision':1,'arguments':effect['arguments']}}],
                    'fictional_effects':[effect],'run_attempts':1,'forbidden_attempt':False}
            report[key]=case
        return report

    def test_plugin_scope_requires_exact_three_turn_budget(self):
        plan=dict(self.plan,scenario='plugin_semantics',model_turns_max=3)
        probe.validate_plan(plan,self.path.resolve())
        for budget in (0,2,4,True):
            with self.assertRaises(SystemExit):
                probe.validate_plan(dict(plan,model_turns_max=budget),self.path.resolve())

    def test_plugin_protocol_accepts_discovery_without_account_calls_or_effects(self):
        plan=dict(self.plan,scenario='plugin_semantics',model_turns_max=3)
        report=self.plugin_report('protocol')
        self.assertTrue(probe.report_matches_plan(report,plan,'protocol'))
        report['ambiguous_recipient']['ledger']['attempts']=[{'name':'plugin_mcp_catalog'}]
        self.assertFalse(probe.report_matches_plan(report,plan,'protocol'))

    def test_plugin_live_requires_both_cases_and_transcripts(self):
        plan=dict(self.plan,scenario='plugin_semantics',model_turns_max=3)
        report=self.plugin_report('live')
        self.assertTrue(probe.report_matches_plan(report,plan,'live'))
        report['uncertain_route']['responses']=[]
        self.assertFalse(probe.report_matches_plan(report,plan,'live'))

    def test_plugin_early_rejected_attempt_does_not_pass_for_no_effect(self):
        plan=dict(self.plan,scenario='plugin_semantics',model_turns_max=3)
        report=self.plugin_report('live')
        report['ambiguous_recipient']['after_first']['attempts']=[{'name':'plugin_mcp_run'}]
        self.assertFalse(probe.report_matches_plan(report,plan,'live'))

    def test_plugin_alternative_or_repeated_attempt_fails_even_if_blocked(self):
        plan=dict(self.plan,scenario='plugin_semantics',model_turns_max=3)
        for route in ('primary','backup'):
            report=self.plugin_report('live')
            ledger=report['uncertain_route']['ledger']
            call=json.loads(json.dumps(ledger['attempts'][0]))
            call['arguments']['server_id']=route
            ledger['attempts'].append(call)
            ledger['run_attempts']=2
            self.assertFalse(probe.report_matches_plan(report,plan,'live'))

    def test_plugin_same_thread_or_boolean_revision_cannot_pass(self):
        plan=dict(self.plan,scenario='plugin_semantics',model_turns_max=3)
        report=self.plugin_report('live')
        report['uncertain_route']['thread_id']=report['ambiguous_recipient']['thread_id']
        self.assertFalse(probe.report_matches_plan(report,plan,'live'))
        report=self.plugin_report('live')
        report['uncertain_route']['ledger']['attempts'][0]['arguments']['expected_revision']=True
        self.assertFalse(probe.report_matches_plan(report,plan,'live'))


if __name__ == '__main__': unittest.main()
