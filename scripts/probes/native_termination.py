#!/usr/bin/env python3
"""Comprueba el AppDelegate real con ventana oculta, sin engine ni backend."""
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]


def main():
    frameworks = ROOT / 'frontend/build/macos/Build/Products/Release/Gennie.app/Contents/Frameworks'
    development_frameworks = ROOT / 'frontend/build/macos/Build/Products/Release'
    if not (development_frameworks / 'FlutterMacOS.framework/Modules/module.modulemap').is_file():
        raise SystemExit('Compila release con scripts/build.sh antes de la prueba nativa.')
    source = (ROOT / 'frontend/macos/Runner/AppDelegate.swift').read_text()
    assert source.count('@main\n') == 1
    with tempfile.TemporaryDirectory(prefix='gennie-termination-test-') as directory:
        directory = Path(directory)
        # Misma clase; solo se retira su entrypoint para usar el del test.
        delegate = directory / 'AppDelegate.swift'
        delegate.write_text(source.replace('@main\n', '', 1))
        binary = directory / 'termination-test'
        subprocess.run(['xcrun', 'swiftc', '-parse-as-library', '-F', str(development_frameworks),
                        '-framework', 'Cocoa', '-framework', 'FlutterMacOS',
                        '-Xlinker', '-rpath', '-Xlinker', str(frameworks),
                        str(delegate), str(ROOT / 'scripts/probes/native_termination_test.swift'),
                        '-o', str(binary)], check=True, timeout=30)
        subprocess.run([str(binary)], check=True, timeout=10)


if __name__ == '__main__':
    main()
