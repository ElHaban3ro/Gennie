"""Pruebas de política y efectos simulados; no controlan aplicaciones reales."""
import importlib.util
import contextlib
import io
import json
from pathlib import Path
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('worker', Path(__file__).with_name('worker.py'))
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)


def context():
    return {'installed': {bundle: True for bundle in worker.BUNDLES.values()},
            'running': ['com.spotify.client'], 'browser': 'com.apple.Safari',
            'players': {'com.spotify.client': {'state': 'playing', 'track': 'one'}}}


class Agent:
    def __init__(self, route, action='open_chrome'):
        self.route, self.action = route, action

    def predict(self, state, questions):
        name = next(iter(questions))
        criteria = questions[name]['criteria']
        if name == 'operation':
            key = self.route if self.route!='complete' else (
                self.action if self.action in worker.MUSIC_ACTIONS or self.action not in worker.CATALOG else 'open_app')
        else:
            key = worker.ROUTE_KEYS[self.route] if name == 'route' else worker.ACTION_KEYS.get(self.action, self.action)
        return {'answers': {name: {'choice': key, 'probabilities': {k: float(k == key) for k in criteria}}}}


class PolicyTests(unittest.TestCase):
    def test_music_operation_cannot_be_overridden_by_application_match(self):
        ctx = context()
        ctx['applications'] = {
            'com.apple.Music': {'name':'Music','path':'/fixture/Music.app','aliases':['Music','Música']},
            'com.spotify.client': {'name':'Spotify','path':'/fixture/Spotify.app','aliases':['Spotify']},
        }
        class ConflictingAgent:
            def predict(self, state, questions):
                name = next(iter(questions))
                criteria = questions[name]['criteria']
                key = {'operation':'pause_music','application_request':'open_app','speech_act':'command'}[name]
                return {'answers':{name:{'choice':key,'probabilities':{k:float(k==key) for k in criteria}}}}
        for previous in (None, 'com.apple.Music', 'com.spotify.client'):
            request = {'text':'Pausa la música.'}
            if previous:
                request.update(last_action='open_app',last_target=previous)
            with self.subTest(previous=previous):
                result = worker.decide(ConflictingAgent(), request, ctx)
                self.assertEqual((result['action'],result['target']), ('pause_music','com.spotify.client'))

    def test_generic_music_does_not_name_apple_music(self):
        ctx = context()
        ctx['applications'] = {'com.apple.Music':{'name':'Music','aliases':['Music','Música']}}
        intent = {'route':'complete','action':'pause_music','probability':1.0}
        result = worker.bind_intent(intent, Agent('complete'), {'text':'Pausa la música.'}, ctx)
        self.assertEqual(result['target'], 'com.spotify.client')

    def test_two_players_never_select_apple_by_generic_alias(self):
        ctx = context()
        ctx['applications'] = {'com.apple.Music':{'name':'Music','aliases':['Music','Música']}}
        intent = {'route':'complete','action':'pause_music','probability':1.0}
        ctx['players']['com.apple.Music'] = {'state':'playing','track':'two'}
        result = worker.bind_intent(intent,Agent('complete'),{'text':'Pausa la música.'},ctx)
        self.assertEqual(result['availability']['reason'],'choose_player')
        self.assertIsNone(result['target'])

    def test_uncertain_operation_cannot_fall_back_to_an_application(self):
        with patch.object(worker,'choice',return_value=('pause_music',.8729)), \
                patch.object(worker,'application_intent') as application:
            result = worker.decide(Agent('complete'),{'text':'Una petición incierta.'},context())
        self.assertEqual(result['route'],'ambiguous')
        self.assertIsNone(result['target'])
        application.assert_not_called()

    def test_opening_and_music_disagreement_does_not_execute(self):
        with patch.object(worker,'infer_operation',return_value={'route':'complete','action':'open_app','probability':1.0}), \
                patch.object(worker,'application_intent',return_value=None), \
                patch.object(worker,'infer_intent',return_value={'route':'complete','action':'pause_music','probability':1.0}):
            result = worker.decide(Agent('complete'),{'text':'otra frase'},context())
        self.assertEqual(result['route'],'ambiguous')
        self.assertIsNone(result['target'])

    def test_pause_protocol_sends_music_command_once_without_opening_application(self):
        ctx = context()
        ctx['applications'] = {'com.apple.Music':{'name':'Music','aliases':['Music','Música']}}
        with patch.object(worker,'run') as run, \
                patch.object(worker,'player_snapshot',return_value={'state':'paused','track':'one'}), \
                patch.object(worker.time,'sleep'):
            replies = self.run_protocol(
                [{'op':'decide','text':'Pausa la música.'},
                 {'op':'execute','action':'pause_music','target':'com.spotify.client'}],
                system_context={'return_value':ctx},
                choice={'side_effect':[('idle',1.0),('pause_music',1.0)]})
        self.assertEqual(replies[0]['action'],'pause_music')
        self.assertEqual(replies[0]['target'],'com.spotify.client')
        self.assertIs(replies[1]['verified'],True)
        run.assert_called_once()
        self.assertEqual(run.call_args.args[0],['/usr/bin/osascript','-e','tell application "Spotify" to pause'])

    def test_explicit_veto_cannot_be_overridden_by_model_confidence(self):
        for text in ('No pauses la música.','Nunca pauses Spotify.','Ayer pausé la música.',
                     '¿Qué significa pausar la música?','Mañana abre Discord.',
                     '«Pausa Spotify»','No, mejor abre Música.'):
            with self.subTest(text=text), patch.object(worker,'infer_operation') as operation:
                result = worker.decide(Agent('complete'),{'text':text},context())
                self.assertIsNone(result['action'])
                self.assertIsNone(result['target'])
                operation.assert_not_called()

    def run_protocol(self, requests, **patches):
        output = io.StringIO()
        with contextlib.ExitStack() as stack:
            stack.enter_context(patch.object(worker.sys,'stdin',io.StringIO('\n'.join(json.dumps(row) for row in requests))))
            stack.enter_context(patch.object(worker.sys,'argv',['worker','/fixture']))
            stack.enter_context(contextlib.redirect_stdout(output))
            stack.enter_context(patch.object(worker,'load_local_agent',return_value=object()))
            stack.enter_context(patch.object(worker,'choice',return_value=('idle',1.0)))
            for name,value in patches.items():
                stack.enter_context(patch.object(worker,name,**value))
            worker.main()
        return [json.loads(line) for line in output.getvalue().splitlines()]

    def test_failed_prediction_revokes_previous_execution_plan(self):
        plan = {'route':'complete','action':'open_app','target':'com.hnc.Discord'}
        with patch.object(worker,'execute') as execute:
            replies = self.run_protocol(
                [{'op':'decide','text':'abre Discord'}, {'op':'decide','text':'otra frase'},
                 {'op':'execute','action':'open_app','target':'com.hnc.Discord'}],
                system_context={'return_value':context()}, decide={'side_effect':[plan,ValueError('failure')]})
            self.assertIn('error',replies[-1])
            execute.assert_not_called()

    def test_failed_execution_revalidation_consumes_plan(self):
        plan = {'route':'complete','action':'open_app','target':'com.hnc.Discord'}
        with patch.object(worker,'execute') as execute:
            replies = self.run_protocol(
                [{'op':'decide','text':'abre Discord'},
                 {'op':'execute','action':'open_app','target':'com.hnc.Discord'},
                 {'op':'execute','action':'open_app','target':'com.hnc.Discord'}],
                system_context={'side_effect':[context(),ValueError('failure'),context()]},
                decide={'return_value':plan},bind_intent={'return_value':plan})
            self.assertIn('error',replies[-1])
            execute.assert_not_called()

    def test_uncertain_incomplete_prediction_does_not_claim_missing_words(self):
        class Uncertain(Agent):
            def predict(self, state, questions):
                name = next(iter(questions))
                keys = list(questions[name]['criteria'])
                probabilities = {key:.4485 if key=='incomplete' else .5515/(len(keys)-1) for key in keys}
                return {'answers':{name:{'choice':'incomplete','probabilities':probabilities}}}
        result = worker.decide(Uncertain('incomplete'),{'text':'Abre Safari.'},context())
        self.assertEqual(result['route'],'ambiguous')
        self.assertIsNone(result['action'])

    def test_five_routes_preserve_wait_clarify_and_no_order(self):
        for route in ('incomplete', 'ambiguous', 'no_order', 'god_call'):
            result = worker.decide(Agent(route), {'text': 'frase final'}, context())
            self.assertEqual(result['route'], route)
            self.assertIsNone(result['action'])
        result = worker.decide(Agent('complete'), {'text': 'abre Chrome'}, context())
        self.assertEqual(result['route'], 'complete')
        self.assertEqual(result['action'], 'open_chrome')

    def test_two_players_require_clarification_for_music(self):
        ctx = context()
        ctx['players']['com.apple.Music'] = {'state': 'playing', 'track': 'two'}
        result = worker.decide(Agent('complete', 'next_track'), {'text': 'siguiente canción'}, ctx)
        self.assertEqual(result['route'], 'ambiguous')
        self.assertIsNone(result['action'])

    def test_unknown_model_action_is_rejected(self):
        with self.assertRaises(ValueError):
            worker.decide(Agent('complete', 'delete_files'), {'text': 'borra'}, context())

    def test_unavailable_application_does_not_change_the_interpreted_intent(self):
        ctx = context()
        ctx['installed']['com.google.Chrome'] = False
        result = worker.decide(Agent('complete', 'open_chrome'), {'text': 'abre Chrome'}, ctx)
        self.assertEqual(result['route'], 'complete')
        self.assertEqual(result['action'], 'open_chrome')
        self.assertEqual(result['availability']['status'], 'unavailable')
        self.assertIsNone(result['target'])

    def test_domain_intent_is_not_restricted_by_a_phrase_whitelist(self):
        agent = Agent('complete')
        class Adapted:
            def infer(self, requests):
                return [{'route':'complete','action':'open_chrome','probability':.99}]
        agent.domain_adapter = Adapted()
        result = worker.decide(agent, {'text': 'trae delante el navegador de Google que necesito consultarlo'}, context())
        self.assertEqual(result['action'], 'open_chrome')

    def test_explicit_player_is_preserved_even_when_another_is_playing(self):
        ctx = context()
        ctx['players']['com.apple.Music'] = {'state':'playing','track':'two'}
        result = worker.decide(Agent('complete','pause_music'), {'text':'pausa Spotify'}, ctx)
        self.assertEqual(result['target'], 'com.spotify.client')

    def test_base_application_questions_restore_adaptation_after_error(self):
        class Model:
            def __init__(self): self.loaded = []
            def load_state_dict(self,state,strict): self.loaded.append(state)
        class Adapter:
            original_tensors = 'base'
            adapted_tensors = 'adapted'
        agent = Agent('complete')
        agent.model = Model()
        adapter = Adapter()
        agent.domain_adapter = adapter
        with patch.object(worker,'infer_application_intent',side_effect=ValueError('failure')):
            with self.assertRaises(ValueError): worker.application_intent(agent,{'text':'Discord'},context())
        self.assertIs(agent.domain_adapter,adapter)
        self.assertEqual(agent.model.loaded,['base','adapted'])

    def test_application_removed_before_effect_is_not_opened(self):
        with patch.object(worker,'system_context',return_value=context()),patch.object(worker,'run') as run:
            result = worker.execute({'action':'open_app','target':'com.fixture.editor'})
            self.assertFalse(result['verified'])
            run.assert_not_called()

    def test_workspace_preserves_missing_foreground_as_unknown(self):
        with patch.object(worker, 'run', return_value=json.dumps({'running':['com.spotify.client']})):
            observed = worker.workspace()
            self.assertIsNone(observed['frontmost'])
            self.assertIsNone(observed['frontmost_pid'])

    def test_missing_foreground_during_activation_recovers_without_reopening(self):
        unknown = json.dumps({'running':['com.spotify.client']})
        foreground = json.dumps({'running':['com.spotify.client'], 'frontmost':'com.spotify.client'})
        with patch.object(worker, 'run', side_effect=['', unknown, foreground]) as run, \
                patch.object(worker.time, 'sleep'):
            result = worker.open_application(['/usr/bin/open', '-b', 'com.spotify.client'], 'com.spotify.client')
            self.assertTrue(result['verified'])
            self.assertEqual(sum(call.args[0][0] == '/usr/bin/open' for call in run.call_args_list), 1)

    def test_opened_application_survives_transient_verification_error(self):
        ctx = context()
        ctx['applications'] = {'com.spotify.client': {'name':'Spotify', 'path':'/fixture/Spotify.app', 'aliases':['Spotify']}}
        observed = {**ctx, 'frontmost':'com.spotify.client'}
        for action in ('open_app', 'open_spotify'):
            with self.subTest(action=action), patch.object(worker, 'run') as run, \
                    patch.object(worker, 'workspace', side_effect=[
                        worker.subprocess.CalledProcessError(1, 'osascript'), observed]), \
                    patch.object(worker.time, 'sleep'):
                plan = {'route':'complete', 'action':action, 'target':'com.spotify.client'}
                replies = self.run_protocol(
                    [{'op':'decide', 'text':'Por favor, abre Spotify.'},
                     {'op':'execute', 'action':action, 'target':'com.spotify.client'}],
                    system_context={'return_value':ctx}, decide={'return_value':plan},
                    bind_intent={'return_value':plan})
                self.assertIs(replies[1].get('verified'), True, replies[1])
                self.assertEqual(run.call_count, 1, 'No repetir la apertura')

    def test_open_command_error_still_checks_whether_application_opened(self):
        ctx = context()
        ctx['applications'] = {'com.spotify.client': {'name':'Spotify', 'path':'/fixture/Spotify.app', 'aliases':['Spotify']}}
        observed = {**ctx, 'frontmost':'com.spotify.client'}
        with patch.object(worker, 'system_context', return_value=ctx), \
                patch.object(worker, 'run', side_effect=worker.subprocess.TimeoutExpired('open', 8)) as run, \
                patch.object(worker, 'workspace', return_value=observed):
            result = worker.execute({'action':'open_app', 'target':'com.spotify.client'})
            self.assertTrue(result['verified'])
            self.assertEqual(run.call_count, 1, 'No repetir la apertura tras timeout')

    def test_persistent_verification_error_is_uncertain_without_reopening(self):
        with patch.object(worker, 'run') as run, \
                patch.object(worker, 'workspace', side_effect=worker.subprocess.TimeoutExpired('osascript', 1)) as workspace, \
                patch.object(worker.time, 'sleep'):
            result = worker.open_application(['/usr/bin/open', '-b', 'com.spotify.client'], 'com.spotify.client')
            self.assertFalse(result['verified'])
            self.assertTrue(result['uncertain'])
            self.assertEqual(json.loads(result['evidence'])['reason'], 'observation_unavailable')
            self.assertEqual(run.call_count, 1)
            self.assertEqual(workspace.call_count, 12)
            self.assertTrue(all(call.kwargs['timeout'] <= 1.0 for call in workspace.call_args_list))

    def test_verification_deadline_bounds_repeated_reads(self):
        with patch.object(worker, 'run') as run, \
                patch.object(worker, 'workspace', side_effect=worker.subprocess.TimeoutExpired('osascript', 1)) as workspace, \
                patch.object(worker.time, 'monotonic', side_effect=[0, 0, 4, 4]), \
                patch.object(worker.time, 'sleep') as sleep:
            result = worker.open_application(['/usr/bin/open', '-b', 'com.spotify.client'], 'com.spotify.client')
            self.assertTrue(result['uncertain'])
            self.assertEqual(run.call_count, 1)
            self.assertEqual(workspace.call_count, 1)
            sleep.assert_not_called()

    def test_running_application_without_foreground_is_not_verified(self):
        with patch.object(worker, 'run') as run, \
                patch.object(worker, 'workspace', return_value={'running':['com.spotify.client'], 'frontmost':'com.apple.finder'}), \
                patch.object(worker.time, 'sleep'):
            result = worker.open_application(['/usr/bin/open', '-b', 'com.spotify.client'], 'com.spotify.client')
            self.assertFalse(result['verified'])
            self.assertTrue(result['uncertain'])
            self.assertEqual(json.loads(result['evidence'])['reason'], 'application_not_observed')
            self.assertEqual(run.call_count, 1)

    def test_failed_spawn_does_not_claim_command_was_sent(self):
        with patch.object(worker, 'run', side_effect=FileNotFoundError('open')) as run, \
                patch.object(worker, 'workspace') as workspace:
            result = worker.open_application(['/usr/bin/open', '-b', 'com.spotify.client'], 'com.spotify.client')
            self.assertFalse(result['verified'])
            self.assertFalse(result.get('uncertain', False))
            self.assertEqual(json.loads(result['evidence']), {'stage':'application_open', 'reason':'os_unavailable'})
            self.assertEqual(run.call_count, 1)
            workspace.assert_not_called()

    def test_named_music_operation_is_preserved_when_application_is_in_inventory(self):
        ctx = context()
        ctx['applications'] = {'com.spotify.client':{'name':'Spotify','path':'/Applications/Spotify.app','aliases':['Spotify']}}
        class MusicAgent(Agent):
            def predict(self,state,questions):
                name = next(iter(questions))
                criteria = questions[name]['criteria']
                key = {'operation':'pause_music','application_request':'god_call','speech_act':'command','route':'complete','action':'pause_music'}[name]
                return {'answers':{name:{'choice':key,'probabilities':{k:float(k==key) for k in criteria}}}}
        result = worker.decide(MusicAgent('complete','pause_music'),{'text':'Pausa Spotify.'},ctx)
        self.assertEqual(result['action'],'pause_music')
        self.assertEqual(result['target'],'com.spotify.client')

    def test_context_change_does_not_execute(self):
        with patch.object(worker, 'system_context', return_value=context()), patch.object(worker, 'run') as run:
            result = worker.execute({'action': 'open_chrome', 'target': 'com.apple.Safari'})
            self.assertFalse(result['verified'])
            run.assert_not_called()

    def test_non_commands_cannot_fall_back_to_application_or_music(self):
        ctx = context()
        ctx['applications'] = {'com.spotify.client':{'name':'Spotify','path':'/fixture/Spotify.app','aliases':['Spotify']}}
        for speech in ('no_order','incomplete'):
            with patch.object(worker,'choice',return_value=(speech,1.0)), \
                    patch.object(worker,'infer_intent') as legacy, patch.object(worker,'application_intent') as application:
                result = worker.decide(Agent('complete'),{'text':'No pauses Spotify'},ctx)
                self.assertIsNone(result['target'])
                legacy.assert_not_called()
                application.assert_not_called()

    def test_music_binding_preserves_explicit_and_verified_player(self):
        ctx = context()
        ctx['applications'] = {'com.apple.Music':{'name':'Music','path':'/fixture/Music.app','aliases':['Music','Música']}}
        ctx['players']['com.apple.Music'] = {'state':'paused','track':'apple-track'}
        intent = {'route':'complete','action':'play_music','probability':1.0}
        for request in ({'text':'Reanúdala.','last_action':'pause_music','last_target':'com.apple.Music'},
                        {'text':'Reanuda en Music.'},
                        {'text':'Sí, reanúdala.','pending':'Reanuda Apple Music.','pending_action':'play_music'}):
            result = worker.bind_intent(intent,Agent('complete'),request,ctx)
            self.assertEqual(result['target'],'com.apple.Music')
        result = worker.bind_intent(intent,Agent('complete'),{'text':'Reanuda Spotify.','last_action':'pause_music','last_target':'com.apple.Music'},ctx)
        self.assertEqual(result['target'],'com.spotify.client')

    def test_boolean_confidences_are_invalid(self):
        class BooleanAgent:
            def predict(self,*args):
                return {'answers':{'question':{'choice':'yes','probabilities':{'yes':True,'no':False}}}}
        with self.assertRaises(ValueError):
            worker.choice(BooleanAgent(),{},'question',{'yes':'sí','no':'no'},'Pregunta')

    def test_asr_punctuation_can_resolve_uncertain_command_without_lowering_threshold(self):
        ctx = context()
        ctx['applications'] = {'com.hnc.Discord':{'name':'Discord','path':'/Applications/Discord.app','aliases':['Discord']}}
        with patch.object(worker, 'choice', side_effect=[('open_app',1.0),('open_app',.9996),('command',.9298),('command',.9788)]) as choice:
            result = worker.decide(Agent('complete'),{'text':'Abre Discord.','pending':'Abre Discord.','pending_action':'open_app'},ctx)
        self.assertEqual(result['target'],'com.hnc.Discord')
        self.assertEqual(choice.call_args_list[2].args[1],{'utterance':'Abre Discord.'})
        self.assertEqual(choice.call_args_list[3].args[1],{'utterance':'Abre Discord'})

    def test_punctuation_recheck_does_not_override_negation_or_accept_uncertainty(self):
        ctx = context()
        ctx['applications'] = {'com.hnc.Discord':{'name':'Discord','path':'/Applications/Discord.app','aliases':['Discord']}}
        with patch.object(worker,'choice',side_effect=[('open_app',1.0),('open_app',.9996),('no_order',.8)]) as choice:
            result = worker.decide(Agent('complete'),{'text':'La frase dice abre Discord.'},ctx)
            self.assertIsNone(result['target'])
            self.assertEqual(choice.call_count,3)
        with patch.object(worker,'choice',side_effect=[('open_app',1.0),('open_app',.9996),('command',.9298),('command',.94)]):
            result = worker.decide(Agent('complete'),{'text':'Abre Discord.'},ctx)
            self.assertIsNone(result['target'])

    def test_unverified_effect_is_not_repeated(self):
        ctx = context()
        with patch.object(worker, 'system_context', return_value=ctx), patch.object(worker, 'run') as run, \
                patch.object(worker, 'player_snapshot', return_value=ctx['players']['com.spotify.client']), \
                patch.object(worker.time, 'sleep'):
            result = worker.execute({'action': 'next_track', 'target': 'com.spotify.client'})
            self.assertFalse(result['verified'])
            self.assertEqual(run.call_count, 1)

    def test_track_change_is_real_evidence_for_skip(self):
        with patch.object(worker, 'system_context', return_value=context()), patch.object(worker, 'run') as run, \
                patch.object(worker, 'player_snapshot', return_value={'state': 'playing', 'track': 'two'}):
            result = worker.execute({'action': 'next_track', 'target': 'com.spotify.client'})
            self.assertTrue(result['verified'])
            self.assertEqual(run.call_count, 1)
            self.assertIn('two', result['evidence'])


if __name__ == '__main__':
    unittest.main()
