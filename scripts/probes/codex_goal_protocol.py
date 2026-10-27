#!/usr/bin/env python3
"""Comprueba Goals por stdio sin credenciales, inferencia ni datos del usuario."""

import json
import os
import selectors
import subprocess
import tempfile
import time

binary = os.environ.get(
    "GENNIE_TEST_CODEX",
    "/Applications/ChatGPT.app/Contents/Resources/codex-cli/CodexCLI.app/Contents/MacOS/codex",
)
with tempfile.TemporaryDirectory(prefix="gennie-goal-probe-") as directory:
    process = subprocess.Popen(
        [binary, "app-server", "--listen", "stdio://"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        env={**os.environ, "CODEX_HOME": directory},
        cwd=directory,
        bufsize=0,
    )
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ)
    buffer = b""

    def send(message):
        process.stdin.write((json.dumps(message) + "\n").encode())
        process.stdin.flush()

    def receive(deadline):
        global buffer
        while time.monotonic() < deadline:
            if b"\n" in buffer:
                line, buffer = buffer.split(b"\n", 1)
                return json.loads(line)
            if selector.select(max(0, deadline - time.monotonic())):
                data = os.read(process.stdout.fileno(), 65536)
                if not data:
                    raise RuntimeError("El app-server terminó antes de responder")
                buffer += data
        return None

    def response(request_id):
        deadline = time.monotonic() + 8
        while event := receive(deadline):
            if event.get("id") == request_id:
                if "error" in event:
                    raise RuntimeError(f"Operación rechazada: {event['error'].get('code')}")
                return event["result"]
        raise RuntimeError("El app-server no respondió a tiempo")

    try:
        send({"id": 1, "method": "initialize", "params": {"clientInfo": {"name": "gennie-goal-probe", "version": "1"}, "capabilities": {"experimentalApi": True}}})
        response(1)
        send({"method": "initialized", "params": {}})
        send({"id": 2, "method": "thread/start", "params": {"cwd": directory, "model": "gpt-6-luna", "approvalPolicy": "never", "sandbox": "read-only"}})
        thread = response(2)["thread"]["id"]
        objective = "Comprueba un resultado ficticio sin realizar ninguna acción."
        send({"id": 3, "method": "thread/goal/set", "params": {"threadId": thread, "origin": "user", "objective": objective, "status": "paused"}})
        assert response(3)["goal"]["status"] == "paused"
        send({"id": 4, "method": "thread/goal/get", "params": {"threadId": thread}})
        goal = response(4)["goal"]
        assert goal["objective"] == objective and goal["status"] == "paused"
        deadline = time.monotonic() + 1
        while event := receive(deadline):
            assert event.get("method") != "turn/started", "Se inició un turno inesperado"
        send({"id": 5, "method": "thread/goal/set", "params": {"threadId": thread, "origin": "automatic", "status": "complete"}})
        assert response(5)["goal"]["status"] == "complete"
        print("Goal creado, consultado y completado; sin continuación automática ni inferencia.")
    finally:
        selector.close()
        process.stdin.close()
        process.terminate()
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
