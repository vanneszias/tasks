"""Check portable Rust boundaries in a disposable, pinned Linux container."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tarfile
import tempfile

IMAGE = "rust:1.98.0-bookworm@sha256:82150a52ec202c1b14d7817e14516c392bb7f5cfebd88f1ed531cb37ebd39922"
ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pty", action="store_true", help="also run Unix PTY library/integration tests")
    parser.add_argument("--platform", choices=["linux/arm64", "linux/amd64"],
                        help="Docker platform (defaults to the Docker engine's architecture)")
    args = parser.parse_args()
    # Only build inputs: no .git, target, .spike, host Cargo config or credentials.
    paths = subprocess.check_output([
        "git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--",
        "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "build.rs", "src", "tests", "native",
    ], cwd=ROOT).split(b"\0")
    command = """
set -eu
mkdir /work
tar -xf - -C /work
cd /work
export CARGO_TARGET_DIR=/build
uname -sm
rustc -Vv
cargo -V
cargo tree --locked --prefix none > /tmp/dependencies
cat /tmp/dependencies
if grep -Eq '^gpui ' /tmp/dependencies; then exit 1; fi
cargo test --locked --lib
"""
    if args.pty:
        command += """
cargo tree --locked --features pty --prefix none > /tmp/dependencies
cat /tmp/dependencies
if grep -Eq '^gpui ' /tmp/dependencies; then exit 1; fi
cargo test --locked --features pty --lib --tests
"""
    with tempfile.TemporaryFile() as archive:
        with tarfile.open(fileobj=archive, mode="w") as tar:
            for name in sorted(set(p.decode() for p in paths if p)):
                path = ROOT / name
                if path.is_symlink() or not path.is_file():
                    raise ValueError(f"Expected a regular source file: {name}")
                info = tar.gettarinfo(str(path), arcname=name)
                info.uid = info.gid = info.mtime = 0
                info.uname = info.gname = ""
                with path.open("rb") as source:
                    tar.addfile(info, source)
        archive.seek(0)
        print(f"Source archive SHA-256: {hashlib.sha256(archive.read()).hexdigest()}", flush=True)
        archive.seek(0)
        docker = ["docker", "run", "--rm", "-i"]
        if args.platform:
            docker += ["--platform", args.platform]
        docker += [IMAGE, "sh", "-c", command]
        print(f"Image: {IMAGE}; platform: {args.platform or 'engine default'}; PTY: {args.pty}", flush=True)
        subprocess.run(docker, stdin=archive, check=True)


if __name__ == "__main__":
    main()
