#!/usr/bin/env python3
"""Evalúa Nemotron→Laya con voz sintética de macOS, sin micrófono ni efectos."""
import argparse
import array
import json
import os
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--nemotron', type=Path, required=True)
parser.add_argument('--probe', type=Path, default=root/'target/debug/examples/streaming_probe')
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--voice', default='Monica')
args = parser.parse_args()
cases = [('abre Safari', 'open_safari'), ('me abres el Finder un momentico', 'open_files'), ('por favor inicia Discord', 'open_discord'),
         ('no abras Chrome', 'no_order'), ('abre el', 'incomplete'), ('hazlo', 'ambiguous'), ('crea una lista de reproducción', 'god_call')]
records = []
with tempfile.TemporaryDirectory(prefix='soc-synthetic-voice-') as directory:
    directory = Path(directory)
    for index,(text,expected) in enumerate(cases):
        audio,raw = directory/f'{index}.aiff', directory/f'{index}.f32'
        subprocess.run(['/usr/bin/say','-v',args.voice,'-o',str(audio),text], check=True, timeout=30)
        subprocess.run(['/opt/homebrew/bin/ffmpeg','-v','error','-y','-i',str(audio),'-af','adelay=300:all=1,apad=pad_dur=1.5','-ar','16000','-ac','1','-f','f32le',str(raw)], check=True, timeout=30)
        samples = array.array('f',raw.read_bytes())
        peak = max((abs(value) for value in samples),default=0)
        if len(samples)<1600 or peak<.0001:
            raise RuntimeError('La síntesis no produjo voz válida; no atribuir el silencio al ASR')
        result = subprocess.run([str(args.probe),str(args.nemotron),str(raw),'cpu'], capture_output=True,text=True,check=True,timeout=120)
        final = next(line for line in result.stdout.splitlines() if line.startswith('final='))
        transcription = final[6:].split(' updates=',1)[0]
        records.append({'source_text':text,'expected':expected,'transcription':transcription,'source':'synthetic_file',
                        'audio_seconds':len(samples)/16000,'audio_peak':peak})
        print(f'Nemotron {index+1}/{len(cases)}',flush=True)
args.output.parent.mkdir(parents=True,exist_ok=True)
args.output.write_text(json.dumps({'audio_source':'synthetic','microphone':False,'execution':False,'records':records},ensure_ascii=False,indent=2)+'\n')
