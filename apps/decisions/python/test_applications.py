"""Inventario real de bundles temporales, localizaciones y nombres sin efectos."""
import importlib.util
from pathlib import Path
import plistlib
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('applications',Path(__file__).with_name('applications.py'))
applications = importlib.util.module_from_spec(spec)
spec.loader.exec_module(applications)


class ApplicationTests(unittest.TestCase):
    def make_bundle(self, root, name='Fixture.app', bundle='com.fixture.app', **metadata):
        app = Path(root)/name
        contents = app/'Contents'
        (contents/'MacOS').mkdir(parents=True)
        (contents/'MacOS/Fixture').write_text('fixture')
        info = {'CFBundleIdentifier':bundle,'CFBundleName':'Fixture','CFBundleExecutable':'Fixture'}
        info.update(metadata)
        (contents/'Info.plist').write_bytes(plistlib.dumps(info))
        return app

    def test_malformed_localization_does_not_abort_inventory(self):
        with tempfile.TemporaryDirectory() as directory:
            file = Path(directory)/'Contents/Resources/en.lproj/InfoPlist.strings'
            file.parent.mkdir(parents=True)
            file.write_bytes(b'<?xml version="1.0" encoding="UTF-16"?><plist><dict/></plist>')
            self.assertEqual(applications.localized_aliases(directory),set())

    def test_bundle_inventory_includes_localized_names_and_excludes_agents(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name,background in [('Calculator',False),('InternalAgent',True)]:
                app = root/(name+'.app')/'Contents'
                (app/'MacOS').mkdir(parents=True)
                (app/'MacOS'/name).write_text('fixture')
                (app/'Info.plist').write_bytes(plistlib.dumps({'CFBundleIdentifier':'com.fixture.'+name,'CFBundleName':name,'CFBundleExecutable':name,'LSBackgroundOnly':background}))
            strings = root/'Calculator.app/Contents/Resources/es.lproj/InfoPlist.strings'
            strings.parent.mkdir(parents=True)
            strings.write_text('CFBundleDisplayName = "Calculadora";')
            table = root/'Calculator.app/Contents/Resources/InfoPlist.loctable'
            table.write_bytes(plistlib.dumps({'fr':{'CFBundleDisplayName':'Calculatrice'}}))
            inventory = applications.discover_applications([root],spotlight=False)
            self.assertEqual(set(inventory),{'com.fixture.Calculator'})
            matches = applications.matching_applications('Por favor abre la calculadora.',inventory)
            self.assertEqual([bundle for bundle,_ in matches],['com.fixture.Calculator'])
            self.assertEqual([bundle for bundle,_ in applications.matching_applications('ouvre Calculatrice',inventory)],['com.fixture.Calculator'])

    def test_shared_name_preserves_ambiguity_and_words_are_not_substrings(self):
        inventory = {'one':{'aliases':['Editor Pro']},'two':{'aliases':['Editor Lite']}}
        self.assertEqual(applications.matching_applications('abre editor',inventory),[])
        self.assertEqual([key for key,_ in applications.matching_applications('abre Editor Pro',inventory)],['one'])
        self.assertEqual(applications.matching_applications('abre el editor profesional',inventory),[])

    def test_filler_is_not_a_partial_localized_application_name(self):
        inventory = {'discord':{'aliases':['Discord'],'short_aliases':['Discord']},
                     'bluetooth':{'aliases':['Bluetooth File Exchange','Intercambio por Bluetooth'],'short_aliases':['Bluetooth File Exchange']}}
        self.assertEqual([key for key,_ in applications.matching_applications('Por favor, abre Discord.',inventory)],['discord'])

    def test_complete_title_disambiguates_contained_alias_but_keeps_two_mentions(self):
        inventory = {'normal':{'aliases':['Firefox']},'dev':{'aliases':['Firefox','Firefox Developer Edition']}}
        self.assertEqual([key for key,_ in applications.matching_applications('abre Firefox Developer Edition',inventory)],['dev'])
        self.assertEqual(len(applications.matching_applications('abre Firefox y Firefox Developer Edition',inventory)),2)

    def test_aliases_are_data_even_when_utterance_is_negated(self):
        inventory = {'app':{'aliases':['Visual Studio Code','Code']}}
        self.assertEqual([key for key,_ in applications.matching_applications('No abras Code.',inventory)],['app'])

    def test_complete_localized_name_beats_derived_abbreviation(self):
        inventory = {'utility':{'aliases':['Audio MIDI Setup'],'short_aliases':['Audio MIDI Setup']},
                     'recorder':{'aliases':['Recorder','Audio'],'short_aliases':['Recorder']}}
        self.assertEqual([key for key,_ in applications.matching_applications('Abre Audio.',inventory)],['recorder'])
        for text in ('Abre Audio MIDI Setup y Audio.', 'Abre Audio y Audio MIDI Setup.'):
            with self.subTest(text=text):
                self.assertEqual({key for key,_ in applications.matching_applications(text,inventory)}, {'utility','recorder'})

    def test_shared_complete_localized_alias_remains_ambiguous(self):
        inventory = {'one':{'aliases':['Recorder','Audio'],'short_aliases':['Recorder']},
                     'two':{'aliases':['Transcriber','Audio'],'short_aliases':['Transcriber']}}
        self.assertEqual({key for key,_ in applications.matching_applications('Abre Audio.',inventory)}, {'one','two'})

    def test_unicode_punctuation_preserves_boundaries_and_accents(self):
        inventory = {'music':{'aliases':['Música']}, 'code':{'aliases':['Code']}}
        for text in ('«MÚSICA»', 'Música…', 'mu\u0301sica', 'Música\u00a0'):
            with self.subTest(text=text):
                self.assertEqual([key for key,_ in applications.matching_applications(text,inventory)],['music'])
        for text in ('musical', 'codec', 'decode', 'codebase'):
            with self.subTest(text=text):
                self.assertEqual(applications.matching_applications(text,inventory),[])

    def test_inventory_recognizes_case_insensitive_bundle_extension(self):
        with tempfile.TemporaryDirectory() as directory:
            self.make_bundle(directory,name='Fixture.APP')
            self.assertEqual(set(applications.discover_applications([Path(directory)],spotlight=False)),{'com.fixture.app'})

    def test_invalid_display_name_keeps_valid_bundle_name(self):
        with tempfile.TemporaryDirectory() as directory:
            self.make_bundle(directory,CFBundleName='Good Name',CFBundleDisplayName=['invalid'])
            inventory = applications.discover_applications([Path(directory)],spotlight=False)
            self.assertEqual(inventory['com.fixture.app']['name'],'Good Name')
            self.assertEqual([key for key,_ in applications.matching_applications('Good Name',inventory)],['com.fixture.app'])

    def test_invisible_display_name_keeps_visible_bundle_name(self):
        with tempfile.TemporaryDirectory() as directory:
            self.make_bundle(directory,CFBundleName='Good Name',CFBundleDisplayName='\u200e')
            inventory = applications.discover_applications([Path(directory)],spotlight=False)
            self.assertEqual(inventory['com.fixture.app']['name'],'Good Name')

    def test_bundle_executable_must_belong_to_bundle(self):
        with tempfile.TemporaryDirectory() as directory:
            external = Path(directory)/'external'
            external.write_text('fixture')
            self.make_bundle(directory,CFBundleExecutable=str(external))
            self.assertEqual(applications.discover_applications([Path(directory)],spotlight=False),{})

    def test_missing_and_removed_executables_are_not_available(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            app = self.make_bundle(root)
            self.assertEqual(len(applications.discover_applications([root],spotlight=False)),1)
            (app/'Contents/MacOS/Fixture').unlink()
            self.assertEqual(applications.discover_applications([root],spotlight=False),{})

    def test_malformed_bundle_does_not_hide_other_applications(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.make_bundle(root)
            malformed = self.make_bundle(root,name='Malformed.app',bundle='com.fixture.bad')
            (malformed/'Contents/Info.plist').write_bytes(b'<?xml version="1.0"?><plist><dict>')
            self.assertEqual(set(applications.discover_applications([root],spotlight=False)),{'com.fixture.app'})

    def test_spotlight_stale_and_nested_paths_are_excluded(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            app = self.make_bundle(root)
            nested = self.make_bundle(app/'Contents',name='Helper.app',bundle='com.fixture.helper')
            stale = root/'Deleted.app'
            output = '\n'.join(map(str,(app,nested,stale)))
            with patch.object(applications.subprocess,'run',return_value=type('Result',(),{'stdout':output})()):
                self.assertEqual(set(applications.discover_applications([],spotlight=True)),{'com.fixture.app'})


if __name__=='__main__':
    unittest.main()
