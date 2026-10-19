"""Pruebas sin modelo ni dispositivo de audio para el inicio de reproducción."""
import sys
import types
import unittest
import io
from unittest.mock import patch

import worker


class StreamingTest(unittest.TestCase):
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
