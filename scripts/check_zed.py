"""Launch a disposable Git worktree in isolated Zed user data, without --wait."""
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
from inspect_opencode import isolated_env

root = Path(tempfile.mkdtemp(prefix="zed-launch-", dir=Path(".spike").resolve()))
env = isolated_env(root)
repo = root / "repo"
subprocess.run(["git", "init", "-q", "-b", "develop", str(repo)], check=True, env=env)
subprocess.run(["git", "-C", str(repo), "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                "-c", "commit.gpgsign=false", "commit", "-q", "--allow-empty", "-m", "fixture"], check=True, env=env)
worktree = root / "worktree"
subprocess.run(["git", "-C", str(repo), "worktree", "add", "-q", "-b", "feature/fixture-1", str(worktree)], check=True, env=env)
zed = shutil.which("zed")
assert zed, "Installed Zed executable missing"
version = subprocess.check_output([zed, "--version"], text=True).strip()
started = time.monotonic()
result = subprocess.run([zed, "--user-data-dir", str(root / "zed-data"), "--new", str(worktree)],
                        capture_output=True, text=True, timeout=10, env=env)
evidence = {"executable": zed, "version": version, "worktree": str(worktree),
            "cli_exit_code": result.returncode, "elapsed_seconds": time.monotonic() - started,
            "separate_user_data": True, "window_path_confirmation": "manual check required"}
Path(".spike/zed-evidence.json").write_text(json.dumps(evidence, indent=2))
print(json.dumps(evidence, indent=2))
assert result.returncode == 0, result.stderr
