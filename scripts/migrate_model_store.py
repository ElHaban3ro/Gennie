#!/usr/bin/env python3
"""Copia los modelos locales a una caché de usuario compartida, sin borrar el origen."""

import hashlib
import json
import os
from pathlib import Path
import subprocess


HOME = Path.home()
SHARED = HOME / "Library/Application Support/AIModels"
SOC = HOME / "Library/Application Support/soc"
NEMOTRON = "nemotron-3.5-asr-streaming-0.6b-Q8_0.gguf"
NEMOTRON_SHA256 = "b94545b313b3223fda7b2857a52681da813935c2127643d1e9ff0c23d988089c"


def clone_missing(source: Path, target: Path) -> None:
    if source.is_symlink():
        if target.is_symlink() and os.readlink(target) == os.readlink(source):
            return
        if target.exists() or target.is_symlink():
            raise RuntimeError(f"Conflicto en destino: {target}")
        target.parent.mkdir(parents=True, exist_ok=True)
        target.symlink_to(os.readlink(source))
    elif source.is_dir():
        if not target.exists():
            target.parent.mkdir(parents=True, exist_ok=True)
            subprocess.run(["cp", "-cR", str(source), str(target)], check=True)
        else:
            if not target.is_dir():
                raise RuntimeError(f"Conflicto en destino: {target}")
            for child in source.iterdir():
                clone_missing(child, target / child.name)
    elif source.is_file():
        if target.exists():
            if target.stat().st_size != source.stat().st_size or sha256(target) != sha256(source):
                raise RuntimeError(f"Conflicto en destino: {target}")
            return
        target.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(["cp", "-cp", str(source), str(target)], check=True)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def source_hf_home(model: Path) -> Path:
    for ancestor in model.parents:
        if ancestor.name == "hub":
            return ancestor.parent
    raise RuntimeError("El checkpoint de Laya no está dentro de una caché Hugging Face")


def main() -> None:
    old_gguf = SOC / "models" / NEMOTRON
    new_gguf = SHARED / "gguf" / NEMOTRON
    if old_gguf.is_file():
        if sha256(old_gguf) != NEMOTRON_SHA256:
            raise RuntimeError("El Nemotron original no supera SHA-256")
        clone_missing(old_gguf, new_gguf)
        if sha256(new_gguf) != NEMOTRON_SHA256:
            raise RuntimeError("El Nemotron compartido no supera SHA-256")
        old_marker = old_gguf.with_suffix(".sha256")
        if old_marker.is_file():
            clone_missing(old_marker, new_gguf.with_suffix(".sha256"))
        print(f"Nemotron: {new_gguf}")

    tts_cache = SOC / "tts/hf"
    if tts_cache.is_dir():
        clone_missing(tts_cache, SHARED / "huggingface")

    decisions = SOC / "decisions"
    runtime = decisions / "runtime-v2.json"
    if not runtime.is_file():
        runtime = decisions / "runtime.json"
    if runtime.is_file():
        config = json.loads(runtime.read_text())
        old_model = Path(config["model"])
        old_cache = source_hf_home(old_model)
        shared_cache = SHARED / "huggingface"
        if old_cache != shared_cache:
            clone_missing(old_cache, shared_cache)
            new_model = shared_cache / old_model.relative_to(old_cache)
            for name in ("rl_agent_config.json", "model.safetensors"):
                if not (new_model / name).is_file():
                    raise RuntimeError(f"Falta {name} en el checkpoint compartido")
            config["model"] = str(new_model)
            backup = runtime.with_name(runtime.name + ".before-shared-models")
            if not backup.exists():
                backup.write_bytes(runtime.read_bytes())
            temporary = runtime.with_name(runtime.name + ".shared-models-tmp")
            temporary.write_text(json.dumps(config, indent=2) + "\n")
            os.replace(temporary, runtime)
        print(f"Laya: {config['model']}")
    print(f"Caché Hugging Face compartida: {SHARED / 'huggingface'}")


if __name__ == "__main__":
    main()
