#!/usr/bin/env python3
"""Crea un paquete local con descripción del uso del micrófono, sin instalarlo."""
from pathlib import Path
import argparse
import plistlib
import shutil
import subprocess

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--binary', type=Path, default=root/'target/debug/soc')
parser.add_argument('--output', type=Path, default=root/'target/debug/soc.app')
parser.add_argument('--identifier', default='dev.fer.soc')
args = parser.parse_args()
contents = args.output.resolve()/'Contents'
(contents/'MacOS').mkdir(parents=True, exist_ok=True)
shutil.copy2(args.binary, contents/'MacOS/soc')
info = {
    'CFBundleExecutable': 'soc', 'CFBundleIdentifier': args.identifier,
    'CFBundleName': 'soc', 'CFBundlePackageType': 'APPL',
    'CFBundleVersion': '1', 'CFBundleShortVersionString': '0.1.0',
    'LSUIElement': True, 'NSHighResolutionCapable': True,
    'NSMicrophoneUsageDescription': 'soc transcribe tu voz localmente mientras activas el atajo. No guarda audio.',
    'NSAppleEventsUsageDescription': 'soc comprueba y controla Música o Spotify cuando activas el control por voz.',
}
with (contents/'Info.plist').open('wb') as output:
    plistlib.dump(info, output)
subprocess.run(['codesign', '--force', '--sign', '-', str(args.output.resolve())], check=True)
print(args.output.resolve())
