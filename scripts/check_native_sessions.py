"""Run the actual GPUI Quit path with standalone OpenCode and a local model."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

from check_sessions import wait_for
from fixture_model import start_model
from inspect_opencode import isolated_env, request, start_server


def alive(pid):
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False


def main():
    root = Path(tempfile.mkdtemp(prefix="native-session-", dir=Path(".spike").resolve()))
    env = isolated_env(root)
    subprocess.run(["git", "init", "-q", str(root / "repo")], check=True, env=env)
    model = start_model(root, env)
    processes = []
    evidence = {"opencode": "2.0.10", "fixture": str(root), "checks": {}}
    try:
        server, url, _, headers = start_server(root, env)
        processes.append(server)
        assert request(url + "/api/info", headers=headers)["version"] == "2.0.10", "Retest the native contract for this installed version"
        api = lambda path, method="GET", body=None: request(url + path, method, body, headers)
        session = api("/api/session", "POST", {"title": "native standalone fixture",
                      "model": {"id": "local", "providerID": "fixture"},
                      "location": {"directory": str(root / "repo")}})["data"]
        subprocess.run(["git", "init", "-q", str(root / "external-repo")], check=True, env=env)
        shutil.copyfile(root / "repo/opencode.json", root / "external-repo/opencode.json")
        external = api("/api/session", "POST", {"title": "unrelated fixture",
                       "model": {"id": "local", "providerID": "fixture"},
                       "location": {"directory": str(root / "external-repo")}})["data"]
        api(f'/api/session/{external["id"]}/prompt', "POST", {"text": "Run the disposable shutdown fixture"})
        wait_for(lambda: (root / "external-repo/started").exists(), "unrelated agent did not start", timeout=20)
        log = open(root / "native.log", "wb")
        launch_env = {**env, "TASKS_SPIKE_QUIT_AFTER": "10",
                      "TASKS_SPIKE_TEST_INPUT": "Run the disposable shutdown fixture\r"}
        app = subprocess.Popen([str(Path("target/debug/terminal-spike").resolve()), str(root / "repo"),
                                shutil.which("opencode"), "--standalone", "--session", session["id"]],
                               env=launch_env, stdout=log, stderr=log)
        processes.append(app)
        log.close()
        time.sleep(1)
        evidence["checks"]["launch_sent_no_prompt"] = not any(
            message["type"] == "user" for message in api(f'/api/session/{session["id"]}/message')["data"])
        wait_for(lambda: (root / "repo/started").exists() or app.poll() is not None,
                 "standalone tool did not start", timeout=20)
        if (root / "repo/started").exists():
            evidence["checks"]["agent_tool_started"] = True
            pid = int((root / "repo/tool-pid").read_text())
            evidence["tool_pid"] = pid
        else:
            evidence["checks"]["agent_tool_started"] = False
            print((root / "native.log").read_text())
            raise AssertionError("GPUI/OpenCode startup failed; fixture log retained")
        if os.environ.get("TASKS_CAPTURE_NATIVE") == "1":
            capture = subprocess.run(["screencapture", "-x", "-R120,120,1000,650", str(root / "terminal.png")])
            evidence["visual_capture"] = "captured; requires inspection" if capture.returncode == 0 else "blocked: screencapture could not create image from rect"
        app.wait(timeout=20)
        evidence["checks"]["gpui_quit_returned"] = app.returncode == 0
        # Observe bounded teardown, including tool descendants, after the native process exits.
        deadline = time.monotonic() + 2
        while alive(pid) and time.monotonic() < deadline:
            time.sleep(0.05)
        evidence["checks"]["tool_process_stopped"] = not alive(pid)
        evidence["checks"]["unrelated_server_alive"] = server.poll() is None
        evidence["checks"]["unrelated_execution_still_active"] = external["id"] in api("/api/session/active")["data"]
        evidence["checks"]["unrelated_session_preserved"] = api(f'/api/session/{external["id"]}')["data"]["id"] == external["id"]
        messages = api(f'/api/session/{session["id"]}/message')
        (root / "history-before-resume.json").write_text(json.dumps(messages, indent=2))
        evidence["session"] = session["id"]
        evidence["checks"]["history_recorded"] = any(item["type"] == "user" for item in messages["data"])
        # Resume the same ID, with no prompt and no fixture input on this launch.
        with open(root / "resume.log", "wb") as log:
            resumed = subprocess.Popen([str(Path("target/debug/terminal-spike").resolve()), str(root / "repo"),
                                        shutil.which("opencode"), "--standalone", "--session", session["id"]],
                                       env={**env, "TASKS_SPIKE_QUIT_AFTER": "5"}, stdout=log, stderr=log)
            processes.append(resumed)
            resumed.wait(timeout=15)
        restored = api(f'/api/session/{session["id"]}')["data"]
        evidence["checks"]["exact_session_location"] = restored["location"] == session["location"]
        resumed_messages = api(f'/api/session/{session["id"]}/message')
        (root / "history-after-resume.json").write_text(json.dumps(resumed_messages, indent=2))
        evidence["checks"]["history_preserved"] = resumed_messages == messages
        # Observe the original tool's delayed side effect, not just its parent's PID.
        time.sleep(22)
        evidence["checks"]["no_late_tool_write"] = not (root / "repo/escaped").exists()
        evidence["checks"]["unrelated_execution_completed"] = (root / "external-repo/escaped").exists()
    finally:
        for process in reversed(processes):
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=10)
        model.shutdown()
        Path(".spike/native-session-evidence.json").write_text(json.dumps(evidence, indent=2))
        print(json.dumps(evidence, indent=2))
    assert all(evidence["checks"].values()), "Milestone 0 native session gate FAILED"


if __name__ == "__main__":
    main()
