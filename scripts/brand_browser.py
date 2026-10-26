#!/usr/bin/env python3
"""Prepara una copia privada de Chromium con identidad Gennie Web, sin tocar perfiles."""
import hashlib
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile


def prepare(runtime: Path, executable: Path) -> None:
    source = executable.resolve().parents[2]
    if source.suffix != '.app' or not source.is_dir():
        raise RuntimeError('No se encuentra el paquete macOS de Chromium')
    icon = Path(__file__).resolve().parents[1] / 'apps/browser/assets/app-icon.icns'
    source_info = plistlib.loads((source / 'Contents/Info.plist').read_bytes())
    target = runtime / 'Gennie Web.app'
    stamp = {'version': 1, 'chromium': source_info['CFBundleVersion'],
             'icon': hashlib.sha256(icon.read_bytes()).hexdigest()}
    stamp_path = target / 'Contents/Resources/gennie-brand.json'
    if stamp_path.is_file() and json.loads(stamp_path.read_text()) == stamp:
        subprocess.run(['codesign', '--verify', '--deep', '--strict', str(target)], check=True)
        print('Gennie Web ya está preparado.')
        return
    # No sustituir un bundle cargado por el navegador abierto.
    processes = subprocess.check_output(['ps', '-axo', 'command='], text=True)
    if str(target / 'Contents/MacOS') + '/' in processes:
        raise RuntimeError('Cierra Gennie Web antes de actualizar su paquete; el perfil se conserva')
    runtime.mkdir(parents=True, exist_ok=True, mode=0o700)
    with tempfile.TemporaryDirectory(prefix='.gennie-web-', dir=runtime) as temporary:
        staged = Path(temporary) / target.name
        # Clonación APFS: copia independiente sin duplicar los bloques sin cambios.
        result = subprocess.run(['/bin/cp', '-cR', str(source), str(staged)],
                                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if result.returncode:
            if staged.exists():
                shutil.rmtree(staged)
            subprocess.run(['ditto', str(source), str(staged)], check=True)
        info_path = staged / 'Contents/Info.plist'
        info = plistlib.loads(info_path.read_bytes())
        info.update(CFBundleName='Gennie Web', CFBundleDisplayName='Gennie Web',
                    CFBundleIdentifier='com.gennie.web', CFBundleIconFile='gennie-web.icns')
        info.pop('CFBundleIconName', None)
        info_path.write_bytes(plistlib.dumps(info))
        resources = staged / 'Contents/Resources'
        shutil.copyfile(icon, resources / 'gennie-web.icns')
        # Algunas versiones localizan el nombre del paquete; conservar copyright y permisos.
        for localized in resources.glob('*.lproj/InfoPlist.strings'):
            data = subprocess.check_output(['plutil', '-convert', 'xml1', '-o', '-', str(localized)])
            strings = plistlib.loads(data)
            strings.update(CFBundleName='Gennie Web', CFBundleDisplayName='Gennie Web')
            localized.write_bytes(plistlib.dumps(strings))
        (resources / 'gennie-brand.json').write_text(json.dumps(stamp))
        subprocess.run(['codesign', '--force', '--deep', '--sign', '-',
                        '--preserve-metadata=entitlements,flags', str(staged)], check=True)
        subprocess.run(['codesign', '--verify', '--deep', '--strict', str(staged)], check=True)
        # Guardar la versión anterior si una actualización falla al colocar el nuevo paquete.
        backup = Path(temporary) / 'previous.app'
        if target.exists():
            target.rename(backup)
        try:
            staged.rename(target)
        except OSError:
            if backup.exists():
                backup.rename(target)
            raise
    print('Gennie Web preparado con nombre e icono propios.')


if __name__ == '__main__':
    if sys.platform != 'darwin' or len(sys.argv) != 3:
        raise SystemExit('Uso en macOS: brand_browser.py RUNTIME EJECUTABLE_CHROMIUM')
    os.umask(0o077)
    prepare(Path(sys.argv[1]).resolve(), Path(sys.argv[2]))
