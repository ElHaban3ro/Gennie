"""Pruebas sin modelo ni dispositivo de audio para el inicio de reproducción."""
import sys
import types
import unittest

import worker


class StreamingTest(unittest.TestCase):
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
