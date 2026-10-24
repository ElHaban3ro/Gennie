#!/usr/bin/env python3
"""Prueba explícita de apertura con SQLite temporal y restauración del foco."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=Path, default=root/'target/debug/gennie')
parser.add_argument('--output', type=Path, default=root/'docs/verification/decisions-native.json')
parser.add_argument('--command', default='Abre Finder.')
parser.add_argument('--bundle', default='com.apple.finder')
args = parser.parse_args()
spec = importlib.util.spec_from_file_location('worker', root/'apps/decisions/python/worker.py')
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)
before_context = worker.workspace()
report = {'input': args.command, 'expected_bundle': args.bundle, 'context': 'native', 'database': 'temporary'}
with tempfile.TemporaryDirectory(prefix='gennie-command-smoke-') as directory:
    try:
        database = Path(directory)/'review.sqlite3'
        result = subprocess.run([str(args.binary.resolve()), '--command', args.command], env={**os.environ, 'GENNIE_DATABASE_PATH': str(database)}, capture_output=True, text=True, timeout=140)
        report.update(exit_code=result.returncode, stdout=result.stdout.strip(), stderr=result.stderr.strip())
        if result.stdout.strip():
            report['outcome'] = json.loads(result.stdout)
        if database.exists():
            with sqlite3.connect(database) as db:
                report['journal'] = db.execute('SELECT action,target,status,evidence FROM decision_attempts').fetchall()
        after = worker.workspace()
        report['after'] = {'frontmost':after.get('frontmost'), 'frontmost_pid':after.get('frontmost_pid')}
        report['application_frontmost'] = after.get('frontmost') == args.bundle
    except Exception as error:
        # La sonda conserva stdout/diario aunque falle su lectura posterior.
        report['probe_error'] = type(error).__name__
    finally:
        try:
            if before_context.get('frontmost_pid'):
                # Activar el proceso existente; no relanzar un agente privado como WindowManager.
                pid = int(before_context['frontmost_pid'])
                script = f"ObjC.import('AppKit'); let a=$.NSRunningApplication.runningApplicationWithProcessIdentifier({pid}); !a.isNil() && a.activateWithOptions($.NSApplicationActivateIgnoringOtherApps);"
                restored = subprocess.run(['/usr/bin/osascript','-l','JavaScript','-e',script],capture_output=True,text=True,timeout=8)
                report['foreground_restore_requested'] = restored.returncode==0 and restored.stdout.strip()=='true'
                report['foreground_restored'] = worker.workspace().get('frontmost_pid') == pid
        except Exception as error:
            report['restore_error'] = type(error).__name__
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2)+'\n')
print(json.dumps(report, ensure_ascii=False))
assert report.get('exit_code') == 0 and report.get('outcome',{}).get('verified'), 'Fallo de orden real; consultar informe'
assert report.get('application_frontmost'), 'No se observó la aplicación en primer plano'
assert len(report['journal']) == 1, 'Debe existir exactamente un intento'
assert tuple(report['journal'][0][0:3]) == ('open_app', args.bundle, 'verified'), 'No se confirmó el resultado en SQLite'
