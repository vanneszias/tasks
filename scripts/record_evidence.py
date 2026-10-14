"""Record tool revisions and tested source hashes; exclude credentials/raw server logs."""
import datetime
import hashlib
import json
from pathlib import Path
import shutil
import subprocess


def command(*args):
    return subprocess.check_output(args, text=True).strip()


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


destination = Path("docs/evidence/milestone-0")
destination.mkdir(parents=True, exist_ok=True)
native = json.loads(Path(".spike/native-session-evidence.json").read_text())
provider = json.loads(Path(".spike/provider-contract-evidence.json").read_text())
gpui = next(Path.home().glob(".cargo/registry/src/*/gpui-0.2.2/.cargo_vcs_info.json"))
report = {
    "recorded_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "source_commit": command("git", "rev-parse", "HEAD"),
    "source_worktree_status": command("git", "status", "--porcelain"),
    "gate": "OPEN: native event/IME/accessibility acceptance and bounded shutdown-failure coverage incomplete",
    "tools": {"rustc": command("rustc", "--version"), "cargo": command("cargo", "--version"),
              "opencode": command("opencode", "--version"), "opencode_binary_sha256": digest(shutil.which("opencode")),
              "zed": command("zed", "--version"), "macos": command("sw_vers", "-productVersion"),
              "xcode": command("xcodebuild", "-version"),
              "ghostty_revision": command("git", "-C", ".spike/ghostty", "rev-parse", "HEAD"),
              "zig": command(".spike/zig-aarch64-macos-0.16.0/zig", "version"),
              "gpui": "0.2.2 + font-kit", "gpui_source": json.loads(gpui.read_text()),
              "portable_pty": "0.9.0"},
    "source_sha256": {str(path): digest(path) for path in
                       [Path("Cargo.toml"), Path("Cargo.lock"), Path("build.rs"), Path("rust-toolchain.toml"),
                        *sorted(Path("src").rglob("*.rs")), *sorted(Path("native").glob("*.c")),
                        *sorted(Path("native").glob("*.m")), *sorted(Path("tests").rglob("*.rs")),
                        *sorted(Path("scripts").glob("*.py"))]},
    "native_sessions": native,
    "native_evidence_source_matches": all(
        Path(path).exists() and digest(path) == expected
        for path, expected in native["tested_source_sha256"].items()
    ) if "tested_source_sha256" in native else None,
    "api_sessions": json.loads(Path(".spike/session-evidence.json").read_text()),
    "vt": json.loads(Path(".spike/vt-evidence.json").read_text()),
    "zed_launch": json.loads(Path(".spike/zed-evidence.json").read_text()),
    "provider_schemas": {key: {field: value[field] for field in ("source", "version", "sha256")}
                         for key, value in provider.items()},
    "linux": {"image": "rust:1.98.0-bookworm@sha256:82150a52ec202c1b14d7817e14516c392bb7f5cfebd88f1ed531cb37ebd39922",
              "command": "cargo test --locked --lib", "result": "PASS: core boundary builds; library has no lifecycle tests yet"},
    "native_input_automation": command("osascript", "-e", 'tell application "System Events" to get UI elements enabled'),
}
(destination / "tested.json").write_text(json.dumps(report, indent=2) + "\n")
image = Path(native["fixture"]) / "terminal.png"
if image.exists():
    shutil.copyfile(image, destination / "embedded-opencode.png")
print(destination / "tested.json")
