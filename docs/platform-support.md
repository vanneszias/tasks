# Platform boundaries and baseline evidence

Issue: [#3](https://github.com/vanneszias/tasks/issues/3). Checked 2026-10-14 against
`657c7143479448eafc34d5f860c87df6b960ff7d` plus the verification script in this change.
This supplements the [milestone report](milestone-0.md); it does not certify the
remaining native acceptance gates.

## Reproduce the Linux checks

Prerequisites: Python 3 (verified with 3.9.6), Git, Docker with a running Linux
engine, and network access to the pinned image and crates.io. From repository root:

```sh
python3 scripts/check_linux.py
python3 scripts/check_linux.py --pty --platform linux/arm64
python3 scripts/check_linux.py --pty --platform linux/amd64
```

The first command selects the engine's architecture and checks the default
library. `--pty` additionally checks Unix PTY library and integration-test targets:

```sh
cargo test --locked --lib
cargo test --locked --features pty --lib --tests
```

Both paths check `cargo tree --locked` and fail if GPUI enters the dependency
graph. The PTY command includes future `tests/process_shutdown.rs` tests without
requiring the native binaries, Ghostty, Zig, Xcode, or a desktop session.

The image is fixed to:

```text
rust:1.98.0-bookworm@sha256:82150a52ec202c1b14d7817e14516c392bb7f5cfebd88f1ed531cb37ebd39922
```

The script streams an archive of tracked and non-ignored untracked build inputs
(`Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `build.rs`, `src/`, `tests/`,
`native/`) into a disposable container. It prints the archive's SHA-256. The
archive uses stable timestamps and ownership; its hash identifies the actual
source snapshot, including uncommitted edits. Symlinks/non-files are rejected.
There is no host source mount, and no copy of `.git`, `.cargo`, `target/`, `.spike/`,
HOME, Git configuration, provider accounts, or credential stores. Sources are
extracted to `/work`, Cargo outputs go to `/build`, and `--rm` removes the container.
No image rebuild, floating Rust install, host dependency cache, or system package
installation is needed. Bookworm already supplies `ps` (procps-ng 4.0.2), which the
Unix process implementation uses.

### Executed results

Source archive SHA-256:
`b0bdd16930f82a50635ded5c0b968d3f5d6255a48964d9e82d764afce1aa5d5b`.
Cargo.lock SHA-256:
`de9ec1b44d93c9c76b1e853dfe31cfd482bb72ece01efeae459325b17c4b3cf8`.

| Command / environment | Actual result |
| --- | --- |
| `python3 scripts/check_linux.py` | Exit 0; engine-default Linux aarch64; default library passed with **0 tests**; dependency graph excludes GPUI |
| `python3 scripts/check_linux.py --pty --platform linux/arm64` | Exit 0; Linux aarch64; default library and PTY library/integration targets passed; **0 tests** on this source snapshot; both dependency graphs exclude GPUI |
| `python3 scripts/check_linux.py --pty --platform linux/amd64` | Exit 0; Linux x86_64 **under QEMU emulation** on the ARM64 engine; both checks passed with **0 tests**; both dependency graphs exclude GPUI. jemalloc reports the expected QEMU `MADV_DONTNEED` fallback. This is not physical x86-64 host evidence. |
| `python3 scripts/check_linux.py --help` | Exit 0 on Python 3.9.6 |
| `git diff --check` | Exit 0 |

Container Rust: `1.98.0 (88d9e12ae 2026-08-18)`, full revision
`88d9e12ae178fab0fb5cc050a94da85685d449ea`, LLVM 22.1.8.
Container Cargo: `1.98.0 (797e8a9bc 2026-08-05)`.
Docker engine: Colima Linux/ARM64, Docker 29.5.2; client 29.9.0 on macOS ARM64.

These checks prove the current compilation boundary. They do not yet exercise
process shutdown: the checked revision has no library or integration tests.
Rerun `--pty` after integrating issue #1's process tests and retain its new source
hash/results. Core lifecycle, storage and provider behavior still need their own
tests as implemented. Linux UI execution is not part of these checks.

## Feature and target boundaries

- Default features are empty; the library exports a revision constant. `build.rs`
  returns before native compilation unless `native` is enabled. Its unconditional
  `cc` build dependency is a Rust build helper, not an AppKit dependency.
- `pty` enables only portable-pty 0.9.0 and libc 0.2.190. `src/lib.rs` exports
  `process` only under `cfg(all(feature = "pty", unix))`.
- `native` adds GPUI 0.2.2 and `pty`; both binaries require `native`. AppKit and
  Metal remain in the native dependency path. The Cocoa shutdown bridge is built
  only for target OS `macos`; the Ghostty C bridge is native-feature gated.
- GPUI is pinned with defaults disabled and only `font-kit` enabled. Linux
  Wayland/X11 features are consequently absent. A usable Linux UI requires
  selecting a backend, its system dependencies, and testing fonts/input,
  clipboard, rendering and teardown. A successful default/PTY check is not a
  successful Linux native build or UI test.
- Bootstrap lists Zig packages for macOS ARM64 and Linux ARM64/x86-64. macOS Intel
  has no bootstrap entry and no execution evidence. Unix PTY gating does not
  establish Windows support.

## macOS minimum: documented candidate versus tested host

**macOS 13.0 is the dependency-backed baseline candidate, not an established
supported runtime.** The only executed native host remains Apple Silicon
macOS 26.6.2 (25G83), Xcode 27.0 (27A266a), SDK 27.0, Rust 1.98.0. Native execution
evidence is in the existing milestone report; no native GUI checks were run for
this portability change.

| Inspected requirement | Finding and limit |
| --- | --- |
| [Pinned Ghostty `src/build/Config.zig`](https://github.com/ghostty-org/ghostty/blob/291fd050b9e9c293c8adb6a140742649b7e42d3a/src/build/Config.zig#L787-L813) | `osVersionMin(.macos)` declares 13.0.0; `osVersionMinLibVt` delegates to it for macOS. The VT target selects this default when its query has no explicit OS minimum (lines 143–149). This is a source requirement, not old-host execution evidence. |
| Pinned Zig 0.16.0 `aarch64-macos` executable | `otool -l` reports `LC_BUILD_VERSION minos 13.0`, SDK 26.4. The compiler itself therefore also carries a 13.0 deployment minimum. Bootstrap verifies the archive checksum `b23d70deaa879b5c2d486ed3316f7eaa53e84acf6fc9cc747de152450d401489`. |
| [Published GPUI 0.2.2 build script](https://docs.rs/crate/gpui/0.2.2/source/build.rs) | Metal shader compilation requests `-mmacosx-version-min=10.15.7` (line 210). This shader flag does not establish the whole application's minimum. GPUI's README requires Xcode and the Metal tooling but supplies no precise Xcode minimum. |
| [Rust macOS target documentation](https://doc.rust-lang.org/rustc/platform-support/apple-darwin.html) | Documents Rust minima of 11.0 ARM64 and 10.12 x86-64, and `MACOSX_DEPLOYMENT_TARGET`. Installed Rust 1.98.0 reports `MACOSX_DEPLOYMENT_TARGET=11.0` with `rustc --print=deployment-target`; a Rust-only default does not override Ghostty's 13.0 requirement. |
| [Pinned Ghostty HACKING.md](https://github.com/ghostty-org/ghostty/blob/291fd050b9e9c293c8adb6a140742649b7e42d3a/HACKING.md#L50-L68) | Full Ghostty macOS-app development requires Xcode 26/macOS 26 SDK and states Xcode 26 can run on macOS 15. This is a build-host/tooling requirement, separate from deployment. This project builds the VT library, not the upstream Swift app; the full-app requirement is not proof that VT runtime requires macOS 26. |

Source identities: Ghostty revision
`291fd050b9e9c293c8adb6a140742649b7e42d3a`;
its `src/build/Config.zig` SHA-256
`e15abd51f767a70c92faac2bc8bf86fbb096db6680bca3de7388b9a4568fd739`.
GPUI 0.2.2 package checksum in Cargo.lock:
`979b45cfa6ec723b6f42330915a1b3769b930d02b2d505f9697f8ca602bee707`;
its inspected `build.rs` SHA-256
`d29a19372417cbb88381324dc9f1a32916b85f238baaf67668a2ac12197e6962`.
Its crate VCS metadata records `69e2130295c2649963eb639fc70b4f2ee8ea1624`
with `dirty: true`, so the published package checksum is authoritative.
Ghostty's VT header explicitly warns that its API is unstable; GPUI is pre-1.0.

### Concrete baseline-host blockers

1. No macOS 13 Apple Silicon host/VM or older-host execution evidence is available
   in this environment. macOS 13 cannot be claimed supported until a native
   binary actually runs there, including the sequential terminal and shutdown
   acceptance checks. Building on 26.6.2 or setting a deployment flag is not that
   execution evidence.
2. The existing native binary in the main worktree reports `minos 11.0`, SDK 27.0
   (`otool -l target/debug/terminal-spike`). That records an existing artifact,
   not a fresh build or 11.0 compatibility: the repository does not explicitly
   align all Rust/C/Zig deployment settings to the 13.0 dependency candidate.
   A baseline-validation build should set `MACOSX_DEPLOYMENT_TARGET=13.0` for
   Rust/C and explicitly select Ghostty's `aarch64-macos.13.0` Zig target, then
   inspect the final Mach-O and execute it on the baseline. Packaging/build-code
   alignment remains work for the native owner; this change owns verification.
3. No Intel macOS host was tested; bootstrap's missing Intel package entry is
   a concrete additional build blocker. Linux ARM64 and emulated x86-64 core/PTY
   compilation do not establish macOS Intel, older macOS, or native Linux UI
   support.

Read-only requirement inspection commands used on the available host:

```sh
sw_vers
xcodebuild -version
xcrun --sdk macosx --show-sdk-version
rustc -Vv
rustc --print=deployment-target
otool -l /Users/zias/dev/tasks/.spike/zig-aarch64-macos-0.16.0/zig
otool -l /Users/zias/dev/tasks/target/debug/terminal-spike
```
