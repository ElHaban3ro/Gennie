"""Regresión: conservar el intérprete del venv aunque sea un enlace simbólico."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class RuntimeConfigTests(unittest.TestCase):
    def test_configuration_preserves_python_symlink_path(self):
        root = Path(__file__).resolve().parents[3]
        with tempfile.TemporaryDirectory(prefix='soc-runtime-config-') as directory:
            path = Path(directory)
            python = path/'venv-python'
            python.symlink_to(Path(sys.executable).resolve())
            model = path/'multilingual'
            model.mkdir()
            (model/'rl_agent_config.json').write_text(json.dumps({'encoder': 'jhu-clsp/mmBERT-base'}))
            (model/'model.safetensors').touch()
            output = path/'runtime.json'
            subprocess.run([sys.executable, str(root/'scripts/configure_laya.py'), '--python', str(python), '--model', str(model), '--output', str(output)], check=True, capture_output=True)
            self.assertEqual(json.loads(output.read_text())['python'], str(python.absolute()))


if __name__ == '__main__':
    unittest.main()
