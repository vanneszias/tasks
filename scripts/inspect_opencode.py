"""Inspect the installed API using private, disposable OpenCode storage."""
import base64
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time
import urllib.request


def isolated_env(root):
    env = {"PATH": os.environ["PATH"], "HOME": str(root / "home"),
           "TERM": "xterm-256color", "OPENCODE_DB": str(root / "sessions.sqlite"),
           "TMPDIR": str(root / "tmp"), "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null"}
    (root / "tmp").mkdir(exist_ok=True)
    for kind in ("CONFIG", "DATA", "STATE", "CACHE"):
        env[f"XDG_{kind}_HOME"] = str(root / kind.lower())
    for name in ("HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME"):
        Path(env[name]).mkdir(parents=True, exist_ok=True)
    return env


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def request(url, method="GET", body=None, headers=None, timeout=10):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(url, data=data, method=method,
                                 headers={"Content-Type": "application/json", **(headers or {})})
    with urllib.request.urlopen(req, timeout=timeout) as response:
        data = response.read()
        return json.loads(data) if data else None


def start_server(root, env):
    port = free_port()
    log = open(root / f"server-{port}.log", "wb")
    process = subprocess.Popen([shutil.which("opencode"), "serve", "--hostname", "127.0.0.1",
                                "--port", str(port)], cwd=root, env=env,
                               stdout=log, stderr=log, start_new_session=True)
    log.close()
    os.chmod(root / f"server-{port}.log", 0o600)
    url = f"http://127.0.0.1:{port}"
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline and process.poll() is None:
        try:
            lines = (root / f"server-{port}.log").read_text().splitlines()
            password = next(line.removeprefix("server password ") for line in lines
                            if line.startswith("server password "))
            headers = {"Authorization": "Basic " + base64.b64encode(
                ("opencode:" + password).encode()).decode()}
            return process, url, request(url + "/openapi.json", headers=headers), headers
        except (OSError, ValueError, StopIteration):
            time.sleep(0.1)
    process.terminate()
    process.wait(timeout=10)
    raise RuntimeError(f"Private server did not start; inspect {root}/server-{port}.log")


if __name__ == "__main__":
    root = Path(tempfile.mkdtemp(prefix="tasks-opencode-", dir=Path(".spike").resolve()))
    process, url, schema, headers = start_server(root, isolated_env(root))
    try:
        destination = Path(".spike/installed-openapi.json")
        destination.write_text(json.dumps(schema, indent=2))
        print("Isolated fixture:", root)
        print("API info:", request(url + "/api/info", headers=headers))
        for path, methods in schema["paths"].items():
            if path in ("/api/session", "/api/session/{sessionID}",
                        "/api/session/{sessionID}/shell", "/api/session/{sessionID}/interrupt",
                        "/api/session/{sessionID}/message", "/api/session/active"):
                print(path, json.dumps(methods, indent=2))
        for name in ("Location.PublicRef", "Session.Info", "SessionInterruptResponse", "SessionActive"):
            print(name, json.dumps(schema["components"]["schemas"].get(name), indent=2))
    finally:
        process.terminate()
        process.wait(timeout=10)
