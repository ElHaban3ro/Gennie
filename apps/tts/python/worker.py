"""Kokoro local. Una línea JSON por solicitud y respuesta; texto solo en memoria."""
import itertools
import os
import urllib.request
import json
import re
import sys
import urllib.error


MAX_CHUNK_CHARS = 160
INWORLD_MAX_UTF16 = 2000
INWORLD_SAMPLE_RATE = 48000


def inworld_chunks(text):
    """Respeta el límite UTF-16 sin partir etiquetas ni descartar caracteres."""
    start = 0
    while start < len(text):
        units = 0
        tag = None
        safe = start
        whitespace = start
        end = start
        while end < len(text):
            ch = text[end]
            count = 2 if ord(ch) > 0xFFFF else 1
            if units + count > INWORLD_MAX_UTF16:
                break
            units += count
            if tag is None and ch in "[<":
                tag = "]" if ch == "[" else ">"
            elif tag == ch:
                tag = None
            end += 1
            if tag is None:
                safe = end
                if ch.isspace():
                    whitespace = end
        if end == len(text):
            yield text[start:end]
            break
        cut = whitespace if whitespace > start and whitespace - start >= (safe - start) // 2 else safe
        if cut == start:
            raise ValueError("Etiqueta de voz demasiado larga")
        yield text[start:cut]
        start = cut


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


class Inworld:
    """Una conexión y contexto persistentes; no guarda audio ni texto."""
    def __init__(self, request):
        from websockets.sync.client import connect
        key = os.environ.get("INWORLD_API_KEY", "").strip()
        if not key:
            raise RuntimeError("Falta INWORLD_API_KEY")
        self.config = self.options(request)
        self.ws = connect("wss://api.inworld.ai/tts/v1/voice:streamBidirectional", additional_headers={"Authorization": "Basic " + key}, open_timeout=10, close_timeout=1, max_size=4194304, compression=None)
        create = {"voiceId": request["voice"], "modelId": request["inworld_model"],
                  "audioConfig": {"audioEncoding": "PCM", "sampleRateHertz": INWORLD_SAMPLE_RATE, "speakingRate": request["speed"]},
                  "autoMode": False, "timestampType": "TIMESTAMP_TYPE_UNSPECIFIED"}
        if request.get("inworld_language"):
            create["language"] = request["inworld_language"]
        if request["inworld_model"] == "inworld-tts-2":
            create["deliveryMode"] = request["inworld_delivery"]
        if request["inworld_normalization"] != "AUTO":
            create["applyTextNormalization"] = request["inworld_normalization"]
        self.ws.send(json.dumps({"contextId": "gennie", "create": create}))
        result = self.receive()
        if "contextCreated" not in result:
            self.close()
            raise RuntimeError("Inworld no confirmó la conexión")

    @staticmethod
    def options(request):
        return tuple(request.get(k) for k in ("voice", "speed", "inworld_model", "inworld_language", "inworld_delivery", "inworld_normalization"))

    def receive(self):
        message = json.loads(self.ws.recv(timeout=30))
        if "error" in message:
            raise RuntimeError("Error de Inworld")
        result = message.get("result", {})
        if result.get("status", {}).get("code", 0):
            raise RuntimeError("Error de Inworld")
        return result

    def speak(self, text):
        for chunk in inworld_chunks(text):
            self.speak_chunk(chunk)

    def speak_chunk(self, text):
        import base64
        import sounddevice as sd
        if len(text.encode("utf-16-le")) > 4000:
            raise ValueError("Texto demasiado largo para Inworld")
        self.ws.send(json.dumps({"contextId": "gennie", "sendText": {"text": text}}))
        self.ws.send(json.dumps({"contextId": "gennie", "flushContext": {}}))
        wrote = False
        with sd.RawOutputStream(samplerate=INWORLD_SAMPLE_RATE, channels=1, dtype="int16") as output:
            while True:
                result = self.receive()
                chunk = result.get("audioChunk", {}).get("audioContent")
                if chunk:
                    audio = base64.b64decode(chunk, validate=True)
                    if len(audio) % 2:
                        raise ValueError("Audio PCM incompleto")
                    output.write(audio)
                    wrote = True
                if "flushCompleted" in result:
                    break
                if "contextClosed" in result:
                    raise RuntimeError("Conexión de voz cerrada")
        if not wrote:
            raise RuntimeError("Inworld no devolvió audio")

    def close(self):
        try:
            self.ws.close()
        except Exception:
            pass


def send(value):
    sys.stdout.write(json.dumps(value, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def main():
    model = None
    remote = None
    for line in sys.stdin:
        try:
            request = json.loads(line)
            if request.get("provider") == "inworld":
                if remote is None or remote.config != Inworld.options(request):
                    if remote is not None:
                        remote.close()
                    remote = Inworld(request)
                    send({"event": "loaded"})
                if request.get("command") != "load":
                    remote.speak(request["text"])
                send({"ok": True})
                continue
            if model is None:
                from kokoro_mlx import KokoroTTS
                model = KokoroTTS.from_pretrained()
                send({"event": "loaded"})
            if request.get("command") == "load":
                send({"ok": True})
                continue
            voice = request["voice"]
            if voice not in model.list_voices():
                send({"ok": False, "error": "La voz seleccionada no existe en el modelo instalado"})
                continue
            speak(model, request["text"], voice, request["speed"])
            send({"ok": True})
        except urllib.error.HTTPError as exc:
            # No reenviar el cuerpo, que puede contener texto o datos del proveedor.
            send({"ok": False, "error": f"Inworld respondió HTTP {exc.code}"})
        except Exception as exc:
            # No enviar texto ni traceback del usuario a la interfaz o los registros.
            if remote is not None:
                remote.close()
                remote = None
            send({"ok": False, "error": type(exc).__name__})

    if remote is not None:
        remote.close()
    if model is not None:
        model.close()


if __name__ == "__main__":
    main()
