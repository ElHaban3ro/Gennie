"""Kokoro local. Una línea JSON por solicitud y respuesta; texto solo en memoria."""
import itertools
import json
import re
import sys


MAX_CHUNK_CHARS = 160


def text_chunks(text):
    """Entrega frases breves sin partir palabras ni esperar una frase enorme."""
    words = text.split()
    chunk = []
    length = 0
    for word in words:
        next_length = length + len(word) + bool(chunk)
        if chunk and next_length > MAX_CHUNK_CHARS:
            yield " ".join(chunk)
            chunk = []
            length = 0
        chunk.append(word)
        length += len(word) + (len(chunk) > 1)
        if length >= 70 and re.search(r"[.!?;:][\"'’”)]?$", word):
            yield " ".join(chunk)
            chunk = []
            length = 0
    if chunk:
        yield " ".join(chunk)


def speak(model, text, voice, speed):
    from kokoro_mlx.playback import play_stream

    audio_chunks = (
        audio
        for phrase in text_chunks(text)
        for audio in model.generate_stream(phrase, voice=voice, speed=speed)
    )
    first = next(audio_chunks, None)
    if first is not None:
        # Abrir la salida después del primer fragmento evita arrancar con silencio.
        play_stream(itertools.chain((first,), audio_chunks))


def send(value):
    sys.stdout.write(json.dumps(value, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def main():
    model = None
    for line in sys.stdin:
        try:
            request = json.loads(line)
            if model is None:
                from kokoro_mlx import KokoroTTS
                model = KokoroTTS.from_pretrained()
            voice = request["voice"]
            if voice not in model.list_voices():
                send({"ok": False, "error": "La voz seleccionada no existe en el modelo instalado"})
                continue
            speak(model, request["text"], voice, request["speed"])
            send({"ok": True})
        except Exception as exc:
            # No enviar texto ni traceback del usuario a la interfaz o los registros.
            send({"ok": False, "error": type(exc).__name__})

    if model is not None:
        model.close()


if __name__ == "__main__":
    main()
