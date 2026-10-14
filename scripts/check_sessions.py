"""Executable V2 contract evidence; never uses the user's service or credentials."""
from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import subprocess
import tempfile
import time

from inspect_opencode import isolated_env, request, start_server
from fixture_model import start_model, Model


def wait_for(predicate, label, timeout=10):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.05)
    raise AssertionError(label)


def main():
    root = Path(tempfile.mkdtemp(prefix="session-check-", dir=Path(".spike").resolve()))
    env = isolated_env(root)
    subprocess.run(["git", "init", "-q", str(root / "repo")], check=True, env=env)
    model = start_model(root, env)
    processes = []
    pool = ThreadPoolExecutor(2)
    try:
        server, url, schema, headers = start_server(root, env)
        processes.append(server)
        assert request(url + "/api/info", headers=headers)["version"] == "2.0.10", "Retest the API contract for this installed version"
        other, other_url, _, other_headers = start_server(root, env)
        processes.append(other)
        def api(path, method="GET", body=None):
            return request(url + path, method, body, headers)
        first = api("/api/session", "POST", {"title": "owned fixture",
                    "model": {"providerID": "fixture", "id": "local"},
                    "location": {"directory": str(root / "repo")}})["data"]
        second = request(other_url + "/api/session", "POST", {"title": "unrelated fixture",
                         "location": {"directory": str(root / "repo")}}, other_headers)["data"]
        assert first["id"] != second["id"]
        # A real agent loop with a deterministic local model initiating a shell tool.
        api(f'/api/session/{first["id"]}/prompt', "POST", {"text": "Run the disposable shutdown fixture"})
        try:
            wait_for(lambda: (root / "repo/started").exists(), "agent tool did not start", timeout=20)
        except AssertionError:
            print("fixture provider requests:", json.dumps(Model.requests))
            print("messages:", api(f'/api/session/{first["id"]}/message'))
            raise
        interruption = api(f'/api/session/{first["id"]}/interrupt', "POST")
        print("interrupt response:", interruption)
        assert interruption["interrupted"] is True
        wait_for(lambda: first["id"] not in api("/api/session/active")["data"], "agent stayed active")
        assert not (root / "repo/escaped").exists()
        assert first["id"] not in api("/api/session/active")["data"]
        messages = api(f'/api/session/{first["id"]}/message')
        server.terminate()
        server.wait(timeout=10)
        assert other.poll() is None
        assert request(other_url + f'/api/session/{second["id"]}', headers=other_headers)["data"]["id"] == second["id"]
        resumed, url, _, headers = start_server(root, env)
        processes.append(resumed)
        restored = api(f'/api/session/{first["id"]}')["data"]
        assert restored["id"] == first["id"]
        assert restored["location"] == first["location"]
        assert api(f'/api/session/{first["id"]}/message') == messages
        evidence = {"version": "2.0.10", "session": first["id"], "location": first["location"],
                    "interruption": interruption, "history_preserved": True,
                    "unrelated_private_server_alive": True,
                    "scope": "private serve API; standalone TUI and actual GPUI Quit still require testing"}
        Path(".spike/session-evidence.json").write_text(json.dumps(evidence, indent=2))
        print(json.dumps(evidence, indent=2))
    finally:
        for process in processes:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=10)
        pool.shutdown(wait=True)
        model.shutdown()


if __name__ == "__main__":
    main()
