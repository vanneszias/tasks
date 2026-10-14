"""Exercise the same C/FFI boundary used by GPUI; not a visual acceptance test."""
from pathlib import Path
import subprocess

root = Path(".spike")
subprocess.run(["cc", "-std=c11", "-Wall", "-Wextra", "-Werror", "-fsanitize=address,undefined",
                "-I.spike/ghostty/include", "native/ghostty_bridge.c", "native/check_vt.c",
                ".spike/ghostty/zig-out/lib/libghostty-vt.a", "-lpthread", "-lm",
                "-o", str(root / "check-vt")], check=True)
result = subprocess.check_output([str(root / "check-vt")], text=True)
(root / "vt-evidence.json").write_text(result)
print(result, end="")
