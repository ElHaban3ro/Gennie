"""Inventario local de bundles y resolución de entidades; sin gramática de órdenes."""
import os
from pathlib import Path
import plistlib
import re
import subprocess
import unicodedata
from xml.parsers.expat import ExpatError


def normalized_name(value):
    value = unicodedata.normalize('NFKD', value.casefold())
    return ' '.join(re.findall(r'[^\W_]+', ''.join(c for c in value if not unicodedata.combining(c))))


def localized_aliases(path):
    aliases = set()
    table = Path(path)/'Contents/Resources/InfoPlist.loctable'
    if table.is_file():
        try:
            values = plistlib.loads(table.read_bytes())
            if isinstance(values,dict):
                for translation in values.values():
                    if isinstance(translation,dict):
                        aliases.update(value for key in ('CFBundleDisplayName','CFBundleName') if isinstance(value:=translation.get(key),str) and value)
        except (OSError, ValueError, ExpatError):
            pass
    for file in (Path(path)/'Contents/Resources').glob('*.lproj/InfoPlist.strings'):
        try:
            data = file.read_bytes()
            try:
                values = plistlib.loads(data)
            except plistlib.InvalidFileException:
                text = data.decode('utf-16' if data.startswith((b'\xff\xfe',b'\xfe\xff')) else 'utf-8-sig')
                values = dict(re.findall(r'"?(CFBundle(?:DisplayName|Name))"?\s*=\s*"([^"\\]*)"\s*;',text))
            if isinstance(values,dict):
                aliases.update(value for key in ('CFBundleDisplayName','CFBundleName') if isinstance(value:=values.get(key),str) and value)
        except (OSError, UnicodeError, ValueError, ExpatError):
            continue
    return aliases


def discover_applications(roots=None, spotlight=True):
    paths = set()
    if roots is None:
        roots = [Path('/Applications'), Path('/System/Applications'), Path('/System/Library/CoreServices'), Path.home()/'Applications']
    for root in roots:
        if not root.is_dir():
            continue
        for directory, folders, _ in os.walk(root):
            for folder in list(folders):
                if folder.casefold().endswith('.app'):
                    paths.add(str(Path(directory)/folder))
                    folders.remove(folder)
    # Spotlight añade aplicaciones fuera de las carpetas estándar, sin abrirlas.
    try:
        output = subprocess.run(['/usr/bin/mdfind','kMDItemContentType == "com.apple.application-bundle"'],capture_output=True,text=True,check=True,timeout=8).stdout if spotlight else ''
        paths.update(path for path in output.splitlines() if path.casefold().endswith('.app'))
    except (OSError, subprocess.SubprocessError):
        pass  # El recorrido de carpetas no depende de que Spotlight esté indexado.
    applications = {}
    ordered = sorted(paths,key=lambda path:(not path.startswith('/Applications/'),not path.startswith('/System/Applications/'),len(path),path))
    for path in ordered:
        if '/.Trash/' in path or '/Contents/' in path:
            continue
        try:
            with (Path(path)/'Contents/Info.plist').open('rb') as file:
                info = plistlib.load(file)
            if not isinstance(info,dict):
                continue
            bundle = info.get('CFBundleIdentifier')
            executable = info.get('CFBundleExecutable')
            if not isinstance(bundle,str) or not bundle.strip() or not isinstance(executable,str):
                continue
            if not executable or Path(executable).name != executable or executable in ('.','..'):
                continue
            if not (Path(path)/'Contents/MacOS'/executable).is_file():
                continue
            # Los agentes internos sin interfaz no son aplicaciones que se puedan presentar.
            if info.get('LSBackgroundOnly'):
                continue
            aliases = {Path(path).stem}
            aliases.update(value for key in ('CFBundleName','CFBundleDisplayName') if isinstance(value:=info.get(key),str) and value)
            short_aliases = set(aliases)
            aliases.update(localized_aliases(path))
            name = next((value for key in ('CFBundleDisplayName','CFBundleName')
                         if isinstance(value:=info.get(key),str) and normalized_name(value)),Path(path).stem)
            if bundle in applications:
                applications[bundle]['aliases'] = sorted(set(applications[bundle]['aliases'])|aliases)
                applications[bundle]['short_aliases'] = sorted(set(applications[bundle]['short_aliases'])|short_aliases)
            else:
                applications[bundle] = {'name':name,'path':path,'aliases':sorted(aliases),'short_aliases':sorted(short_aliases)}
        except (OSError, ValueError, plistlib.InvalidFileException, ExpatError):
            continue
    return applications


def matching_applications(text, applications):
    normalized = ' '+normalized_name(text)+' '
    token_owners = {}
    for bundle, app in applications.items():
        # Solo títulos originales para abreviaturas: una preposición de una traducción no es una entidad.
        for alias in app.get('short_aliases',app['aliases']):
            for token in normalized_name(alias).split():
                if len(token)>=3:
                    token_owners.setdefault(token,set()).add(bundle)
    spans = {}
    for bundle, app in applications.items():
        names = {normalized_name(alias):True for alias in app['aliases']}
        for token,owners in token_owners.items():
            if owners=={bundle}:
                names.setdefault(token,False)
        found = []
        for name,complete in names.items():
            if not name:
                continue
            start = normalized.find(' '+name+' ')
            while start>=0:
                found.append((start+1,start+1+len(name),complete))
                start = normalized.find(' '+name+' ',start+1)
        if found:
            spans[bundle] = found
    # Un título mayor gana al fragmento; a igual texto, un alias completo gana a una abreviatura.
    # Dos alias completos iguales siguen siendo ambiguos y las menciones separadas se conservan.
    return [(bundle,applications[bundle]) for bundle,positions in spans.items() if any(
        not any(other!=bundle and left<=start and end<=right
                and (right-left>end-start or (other_complete and not complete))
                for other,ranges in spans.items() for left,right,other_complete in ranges)
        for start,end,complete in positions)]
