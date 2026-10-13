"""Pruebas puras del contexto y del corte por confianza; sin cargar pesos."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('domain', Path(__file__).with_name('domain.py'))
domain = importlib.util.module_from_spec(spec)
spec.loader.exec_module(domain)


class Probabilities:
    def __init__(self, index, score):
        self.index,self.score = index,score
    def argmax(self):
        return self.index
    def __getitem__(self, index):
        return self.score if index==self.index else 0


class DomainTests(unittest.TestCase):
    def test_corrupt_thresholds_cannot_authorize_actions(self):
        for value in [-1, 2, float('nan'), '0.9', True]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                domain.validate_thresholds({'threshold':.9,'action_thresholds':{'open_files':value}})
        with self.assertRaises(ValueError):
            domain.validate_thresholds({'threshold':.9,'disabled_actions':['delete_files']})

    def test_semantic_guard_restores_adapted_weights_on_failure(self):
        class Model:
            def __init__(self):
                self.loaded = []
            def load_state_dict(self, state, strict):
                self.loaded.append(state)
        class Agent:
            def __init__(self):
                self.model = Model()
            def predict(self, state, questions):
                raise RuntimeError('inference failed')
        adapter = object.__new__(domain.DomainAdapter)
        adapter.metadata = {'version':2}
        adapter.agent = Agent()
        adapter.original_tensors,adapter.adapted_tensors = 'original','adapted'
        with self.assertRaises(RuntimeError):
            adapter.check_semantics({'text':'abre Finder'},'open_files')
        self.assertEqual(adapter.agent.model.loaded,['original','adapted'])

    def test_low_score_preserves_pending_action_without_authorizing_execution(self):
        adapter = object.__new__(domain.DomainAdapter)
        adapter.threshold = .9
        adapter.metadata = {'action_thresholds':{'open_chrome':.95}}
        result = adapter.route_probabilities([Probabilities(2,.92)])[0]
        self.assertEqual(result['route'],'ambiguous')
        self.assertEqual(result['requested_action'],'open_chrome')
        self.assertIsNone(result['action'])

    def test_confirmed_context_and_pending_context_are_distinct(self):
        text = domain.semantic_text({'text':'Spotify','last_action':'open_files','pending':'pausa la música','pending_action':'pause_music'})
        self.assertIn('Última acción comprobada: Abrir Finder',text)
        self.assertIn('Acción pendiente: Pausar música',text)

    def test_non_command_label_never_has_executable_action(self):
        adapter = object.__new__(domain.DomainAdapter)
        adapter.threshold = .5
        adapter.metadata = {}
        result = adapter.route_probabilities([Probabilities(14,.99)])[0]
        self.assertEqual(result['route'],'no_order')
        self.assertIsNone(result['action'])


if __name__ == '__main__':
    unittest.main()
