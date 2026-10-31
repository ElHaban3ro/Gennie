"""Guardia de procesos QA con ejecutables temporales, sin ventanas o modelos."""
import shutil
import subprocess
import tempfile
from pathlib import Path
import unittest

import native_ui


class NativeCleanupTests(unittest.TestCase):
    def test_cleanup_stops_same_fixture_and_preserves_other_path(self):
        with tempfile.TemporaryDirectory(prefix='gennie-cleanup-test-') as temporary:
            root=Path(temporary)
            binaries=[]
            for name in ('own','other'):
                app=root/name/'runner';app.parent.mkdir()
                binaries.append(app)
            subprocess.run(['/usr/bin/clang','-x','c','-o',str(binaries[0]),'-'],
                           input='#include <unistd.h>\nint main(void) { sleep(30); return 0; }\n',
                           text=True,capture_output=True,check=True,timeout=15)
            shutil.copyfile(binaries[0],binaries[1]);binaries[1].chmod(0o700)
            children=[subprocess.Popen([str(app)]) for app in binaries]
            try:
                result=native_ui.stop_qa_residuals(binaries[0].resolve())
                self.assertEqual(result['found'],[children[0].pid])
                self.assertEqual(result['remaining'],[])
                self.assertNotEqual(children[0].wait(timeout=3),0)
                self.assertIsNone(children[1].poll())
                self.assertEqual(native_ui.qa_processes(binaries[0]),[])
            finally:
                for child in children:
                    if child.poll() is None:child.terminate()
                    child.wait(timeout=3)

    def test_residual_cleanup_does_not_count_as_clean_ui_quit(self):
        manifest={'stop':'already_exited','exit_code':0,'integrity':'ok','foreign_keys':[],
                  'qa_residual_processes':{'found':[123],'remaining':[]}}
        with self.assertRaises(SystemExit):native_ui.require_clean_quit(manifest,'')


if __name__=='__main__':unittest.main()
