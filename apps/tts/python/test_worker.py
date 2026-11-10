"""Pruebas sin modelo ni dispositivo de audio para el inicio de reproducción."""
import sys
import types
import unittest
import io
from unittest.mock import patch

import worker


class StreamingTest(unittest.TestCase):
    def test_inworld_long_blocks_preserve_text_and_complete_tags_with_utf16_limit(self):
        text = "[warmly] " + ("😀 " * 1100) + '<break time="0.5s"/> [curious] Final.'
        chunks = list(worker.inworld_chunks(text))
        self.assertGreater(len(chunks), 1)
        self.assertEqual(''.join(chunks), text)
        self.assertTrue(all(len(chunk.encode('utf-16-le')) <= 4000 for chunk in chunks))
        self.assertTrue(all(chunk.count('[') == chunk.count(']') and chunk.count('<') == chunk.count('>') for chunk in chunks))

    def test_inworld_normal_answer_is_one_block(self):
        text = '[warmly] Hola. <break time="0.5s"/> [curious] ¿Cómo estás?'
        self.assertEqual(list(worker.inworld_chunks(text)), [text])

    def test_inworld_reports_ready_before_audio_finishes(self):
        output = io.StringIO()
        line = '{"text":"Hola","voice":"Ashley","speed":1.0,"provider":"inworld"}'
        requests = io.StringIO(line + "\n")
        events = []
        class Remote:
            def __init__(self, req): self.config = self.options(req)
            @staticmethod
            def options(req): return req["voice"]
            def speak(self, text): events.append(output.getvalue())
            def close(self): pass
        with patch.object(sys, "stdin", requests), patch.object(sys, "stdout", output), patch.object(worker, "Inworld", Remote):
            worker.main()
        self.assertIn('"event": "loaded"', events[0])

    def test_load_prepares_model_without_playback_and_reuses_it(self):
        created = []

        class Model:
            def close(self):
                pass

            def list_voices(self):
                return ["em_alex"]

        class KokoroTTS:
            @staticmethod
            def from_pretrained():
                created.append(True)
                return Model()

        kokoro = types.ModuleType("kokoro_mlx")
        kokoro.KokoroTTS = KokoroTTS
        output = io.StringIO()
        requests = io.StringIO('{"command":"load"}\n{"text":"Hola","voice":"em_alex","speed":1.0}\n')
        with patch.dict(sys.modules, {"kokoro_mlx": kokoro}), patch.object(sys, "stdin", requests), patch.object(sys, "stdout", output), patch.object(worker, "speak") as speak:
            worker.main()

        self.assertEqual(created, [True])
        self.assertEqual([line.strip() for line in output.getvalue().splitlines()], ['{"event": "loaded"}', '{"ok": true}', '{"ok": true}'])
        speak.assert_called_once()

    def test_inworld_reuses_connection_and_streams_audio(self):
        import json
        import base64
        sent = []
        played = []
        output_settings = []
        class Socket:
            def __init__(self): self.messages = iter([{"result":{"contextCreated":{}}}, {"result":{"audioChunk":{"audioContent":base64.b64encode(b"abcd").decode()}}}, {"result":{"flushCompleted":{}}}])
            def send(self, msg): sent.append(json.loads(msg))
            def recv(self, timeout): return json.dumps(next(self.messages))
            def close(self): pass
        class Output:
            def __init__(self, **kwargs): output_settings.append(kwargs)
            def __enter__(self): return self
            def __exit__(self, *args): pass
            def write(self, chunk): played.append(chunk)
        sd = types.ModuleType("sounddevice"); sd.RawOutputStream = Output
        client = types.ModuleType("websockets.sync.client"); client.connect = lambda *args, **kwargs: Socket()
        config = {"voice":"Ashley", "speed":1.0, "inworld_model":"inworld-tts-2", "inworld_language":"es-CO", "inworld_delivery":"CREATIVE", "inworld_normalization":"OFF"}
        with patch.dict(sys.modules, {"sounddevice": sd, "websockets": types.ModuleType("websockets"), "websockets.sync": types.ModuleType("websockets.sync"), "websockets.sync.client": client}), patch.dict("os.environ", {"INWORLD_API_KEY":"sample"}):
            remote = worker.Inworld(config)
            remote.speak("Hola")
            remote.close()
        self.assertEqual(played, [b"abcd"])
        self.assertEqual(sent[0]["create"]["language"], "es-CO")
        self.assertEqual(sent[0]["create"]["audioConfig"]["sampleRateHertz"], 48000)
        self.assertEqual(output_settings[0]["samplerate"], 48000)
        self.assertEqual(sent[0]["create"]["deliveryMode"], "CREATIVE")
        self.assertEqual(sent[1]["sendText"]["text"], "Hola")

    def test_long_text_reaches_playback_before_later_synthesis(self):
        events = []

        class Model:
            def generate_stream(self, phrase, voice, speed):
                events.append(("generate", phrase))
                yield phrase

        def play_stream(chunks):
            for chunk in chunks:
                events.append(("play", chunk))

        playback = types.ModuleType("kokoro_mlx.playback")
        playback.play_stream = play_stream
        previous = sys.modules.get("kokoro_mlx.playback")
        sys.modules["kokoro_mlx.playback"] = playback
        try:
            worker.speak(
                Model(),
                "Esta primera frase contiene suficientes palabras para iniciar la reproducción pronto. Luego continúa la respuesta con otro fragmento.",
                "em_alex",
                1.0,
            )
        finally:
            if previous is None:
                del sys.modules["kokoro_mlx.playback"]
            else:
                sys.modules["kokoro_mlx.playback"] = previous

        self.assertEqual(
            [kind for kind, _ in events],
            ["generate", "play", "generate", "play"],
        )

    def test_very_long_sentence_starts_with_short_complete_words(self):
        text = " ".join(["palabra"] * 100)
        chunks = list(worker.text_chunks(text))
        self.assertGreater(len(chunks), 1)
        self.assertTrue(all(len(chunk) <= worker.MAX_CHUNK_CHARS for chunk in chunks))
        self.assertEqual(" ".join(chunks), text)


if __name__ == "__main__":
    unittest.main()
