"""Fetch and build pinned Ghostty/Zig under .spike; no global installation."""
import hashlib
import platform
from pathlib import Path
import subprocess
import tarfile
import urllib.request

REVISION = "291fd050b9e9c293c8adb6a140742649b7e42d3a"
ZIG = "0.16.0"
PACKAGES = {
    ("Darwin", "arm64"): ("aarch64-macos", "b23d70deaa879b5c2d486ed3316f7eaa53e84acf6fc9cc747de152450d401489"),
    ("Linux", "x86_64"): ("x86_64-linux", "70e49664a74374b48b51e6f3fdfbf437f6395d42509050588bd49abe52ba3d00"),
    ("Linux", "aarch64"): ("aarch64-linux", "ea4b09bfb22ec6f6c6ceac57ab63efb6b46e17ab08d21f69f3a48b38e1534f17"),
}


def main():
    root = Path(".spike").resolve()
    root.mkdir(exist_ok=True)
    target, checksum = PACKAGES[(platform.system(), platform.machine())]
    name = f"zig-{target}-{ZIG}"
    archive = root / (name + ".tar.xz")
    zig = root / name / "zig"
    if not zig.exists():
        urllib.request.urlretrieve(f"https://ziglang.org/download/{ZIG}/{name}.tar.xz", archive)
        assert hashlib.sha256(archive.read_bytes()).hexdigest() == checksum, "Zig checksum mismatch"
        with tarfile.open(archive) as tar:
            for member in tar.getmembers():
                destination = (root / member.name).resolve()
                assert root in destination.parents, "Unsafe archive path"
            tar.extractall(root)
    assert subprocess.check_output([zig, "version"], text=True).strip() == ZIG
    source = root / "ghostty"
    if not source.exists():
        subprocess.run(["git", "init", "-q", str(source)], check=True)
        subprocess.run(["git", "-C", str(source), "fetch", "--depth", "1",
                        "https://github.com/ghostty-org/ghostty.git", REVISION], check=True)
        subprocess.run(["git", "-C", str(source), "checkout", "--detach", "FETCH_HEAD"], check=True)
    actual = subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
    assert actual == REVISION, f"Refusing to modify existing Ghostty checkout at {actual}"
    subprocess.run([zig, "build", "-Demit-lib-vt", "-Doptimize=ReleaseFast"], cwd=source, check=True)
    print(f"Built Ghostty {REVISION} with Zig {ZIG} ({target})")


if __name__ == "__main__":
    main()
