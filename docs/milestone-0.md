# Milestone 0 — integration foundation

Status: **gate open**, tested 2026-10-14. Runnable spike and evidence are available; milestones 1–4 have not started. The product specification, architecture, decisions and handoff remain the governing contracts.

## Tested revisions

| Component | Tested version |
| --- | --- |
| Rust / Cargo | 1.98.0; exact compiler build IDs in manifest |
| GPUI | crates.io 0.2.2, default features disabled, `font-kit` enabled |
| GPUI crate VCS metadata | `69e2130295c2649963eb639fc70b4f2ee8ea1624`, upstream metadata reports `dirty: true`; Cargo.lock pins the published crate checksum |
| Ghostty VT | `291fd050b9e9c293c8adb6a140742649b7e42d3a` |
| Zig | 0.16.0, download checksum verified by bootstrap |
| portable-pty | 0.9.0 |
| OpenCode | v2.0.10; installed binary SHA-256 in manifest |
| Zed | 1.23.3, `/opt/homebrew/bin/zed` → `/Applications/Zed.app` |
| Host | Apple Silicon, macOS 26.6.2, Xcode 27.0 (27A266a) |

[Evidence manifest](evidence/milestone-0/tested.json) records source SHA-256 values, tool identities, fixture session IDs and check results. There is no repository commit yet; these hashes identify the tested working files. Ignored `.spike` fixtures contain private server logs and raw databases; they are not deliverables.

## Results and limits

| Foundation | Evidence | Acceptance remaining |
| --- | --- | --- |
| GPUI + Ghostty VT | Native GPUI paints real OpenCode TUI cells; retained terminal entities, PTYs and C bridge build/run | Physical keyboard, clipboard, selection, IME, resize and tab-switch acceptance; mouse protocol and accessibility coverage |
| VT behavior | Sanitizer-backed executable checks: wide/combining Unicode, colors, selection/copy, Ctrl+C, Kitty key encoding, bracketed paste, alternate screen, resize, scrollback, independent states | These are C/VT checks, not proof of native input routing |
| OpenCode private API | Real agent-loop shell interrupted; same session/history survives server restart; unrelated private server remains alive | Direct API `/shell` does not have the same interrupt semantics |
| Standalone + actual Quit | Latest native run passes all 12 checks, including exact-session resume, no automatic launch prompt, unrelated active execution completing, and no delayed owned-tool write | Bounded failure handling, complete shell-group teardown, application acknowledgement before exit, crash behavior |
| Zed | Isolated disposable worktree launch exits 0 in 0.579 seconds without `--wait` | Visually confirm opened path; executable correction UI comes later |
| Providers | Public OpenAPI schemas inspected and hashed; [contract findings](provider-contracts.md) | Authenticated test-account and fixture adapter tests in later milestones |
| Linux | `cargo test --locked --lib` passes in pinned Rust Bookworm image | Minimal library has zero tests and no lifecycle/storage/provider implementation; native Linux UI not built |

### Native rendering

[Inspected screenshot](evidence/milestone-0/embedded-opencode.png) shows native spike controls, OpenCode conversation title, prompt field, model/agent selector and checkout status. The test supplies a fixture prompt directly to the PTY; it does not exercise a physical keystroke. Initial invisible text was caused by disabling GPUI defaults without enabling `font-kit`, which selected a placeholder text system. `gpui-text-smoke` reproduces text rendering independently of Ghostty.

The bridge uses external `libghostty-vt`, not Ghostty's internal AppKit surface API. Rust owns PTY I/O, GPUI rendering/input and terminal entities; C isolates Ghostty's unstable ABI. Hidden terminals continue parsing bounded output batches but do not paint. A 10,000-line VT parsing check took 0.874 ms on this host; this is not end-to-end rendering latency, hidden-view CPU profiling, or an interaction benchmark. GPUI currently uses fixed 9×18 pixel cells and Menlo 14px; complex shaping, protocol completeness and accessibility still need acceptance.

Manual feedback confirmed text entry but found Enter did not submit. GPUI's macOS backend supplies `key_char` for Enter (`\n`) and Tab (`\t`); the prototype incorrectly deferred every unmodified `key_char` to committed-text handling. It now defers printable text only, allowing control keys through Ghostty encoding. A Rust routing regression check and sanitizer-backed C checks verify Enter produces `\r` and Tab produces `\t`. Native build and all 12 native session checks pass after this fix (`native-session-tdpfjrdl`, session `ses_ec61689fdffefOdK5v1V2c1T0C`); physical Enter/Tab re-acceptance is still pending. The regenerated screenshot was inspected for native OpenCode rendering.

### Session contract observed in OpenCode v2.0.10

- Interactive invocation: `opencode --standalone --session SESSION_ID` with explicit cwd. Do not use `--continue` to identify an issue conversation.
- A private `opencode serve --hostname 127.0.0.1 --port PORT` can create and inspect sessions sharing the fixture SQLite store while standalone clients run. This demonstrates the tested fixture concurrency only.
- `POST /api/session`: `{"title":"…","model":{"providerID":"fixture","id":"local"},"location":{"directory":"ABSOLUTE_PATH"}}`. Session responses wrap the object under `data`.
- `GET /api/session/{sessionID}` and `GET /api/session/{sessionID}/message` provide identity/location and persisted history. `POST /api/session/{sessionID}/prompt` sends `{"text":"…"}` for the test agent loop.
- `POST /api/session/{sessionID}/interrupt` stops the tested agent-loop shell and returns `{"interrupted":true}`. A direct `/shell` request returned `{"interrupted":false}` and continued: it is not an appropriate agent interruption fixture.
- Installed `serve` requires HTTP Basic username `opencode` and its generated password, despite OpenAPI advertising `security: []`. The fixture captures passwords only in mode-0600 ignored logs.
- The local OpenAI-compatible model requires capabilities `{"tools":true,"input":["text"],"output":["text"]}`. Fixtures deny all other providers and use separate HOME, XDG paths, database, temporary files and Git config. They do not reuse production credentials.

Standalone is **provisional**, not a finalized production lifecycle strategy. The private API's interrupt acknowledgement and standalone's graceful shutdown need a reliable ownership/persistence contract before Start can ship.

### Shutdown blocker and failed run

Current `src/terminal.rs` takes the owned child on GPUI Quit, calls portable-pty `kill()` (SIGHUP on the tested Unix backend), then `wait()` on a background executor. GPUI's quit-future budget is 100 ms. Failure is logged; the UI does not hold Quit or present actionable interruption failure. `Drop` also performs a blocking kill/wait. This does not satisfy the architecture's bounded failure contract or prove all owned shell process groups stop.

The first rerun in this continuation (`native-session-bsoyowyj`, session `ses_ec6291a52ffels233M8kJi2IB2`) failed immediate `tool_process_stopped` and exact history equality after resume; it prevented the delayed write and left unrelated work running. The PID was absent at subsequent inspection. The harness now allows **at most two seconds after app exit** for descendant teardown and saves before/after history snapshots. A subsequent run (`native-session-g9vm_f2n`, session `ses_ec627d879ffeeWXzM28v12cnnM`) passed, including strict full history equality. This is consistent with asynchronous teardown, but is not a proven root-cause fix. The production shutdown code has not changed, and passing after an external wait is not proof that the app acknowledged completion before exiting.

Before closing the gate: coordinate shutdown before `cx.quit()`, verify session interruption/persistence, define a bounded deadline and visible failure, and prove owned shell-group cleanup with stubborn processes and concurrent agents. Avoid signaling stored PIDs after restart or terminating the user's shared OpenCode service. Crash/no-orphan guarantees are unproven.

## Build and run

Prerequisites: Rust via rustup, Python 3, Git, Xcode command-line tools, installed OpenCode v2.0.10 and Zed, internet for pinned dependencies. Native bootstrap currently supports Apple Silicon macOS and the two listed Linux architectures; Intel macOS is not configured. Minimum supported macOS is **not established** by testing only this host.

Run from repository root:

```sh
python3 scripts/bootstrap_ghostty.py
cargo build --locked --features native --bins
cargo fmt --check
cargo test --locked --features native --bin terminal-spike
python3 scripts/check_vt.py
python3 scripts/check_sessions.py
TASKS_CAPTURE_NATIVE=1 python3 scripts/check_native_sessions.py
python3 scripts/check_zed.py
python3 scripts/inspect_provider_contracts.py
python3 scripts/record_evidence.py
```

Run native checks sequentially: they open windows and screenshot a fixed screen rectangle. The fixture can take about a minute. Screenshot capture requires an available desktop/screen capture; inspect every new capture. `record_evidence.py` copies available results, including failures; it does not certify acceptance or automatically inspect screenshots. Cargo currently reports upstream future-incompatibility warnings for `block` and `proc-macro-error2`.

For an interactive shell without user repository changes:

```sh
mkdir -p .spike/manual-terminal
target/debug/terminal-spike "$PWD/.spike/manual-terminal" /bin/zsh -f
```

Two tabs are retained; the selected one is painted. Quit through **Quit spike**, Cmd+Q, or closing the last window. For a local fixture OpenCode run, use `check_native_sessions.py`; it constructs isolated storage and a local deterministic provider automatically. Plain `terminal-spike PATH opencode --standalone --session ID` uses the inherited OpenCode configuration, so use only a deliberately configured test session for mutation acceptance.

### Linux boundary reproduction

The default feature set excludes GPUI and portable-pty; `build.rs` skips the native build. Reproduce the current minimal core boundary with Docker:

```sh
docker run --rm -v "$PWD:/source:ro" \
  rust:1.98.0-bookworm@sha256:82150a52ec202c1b14d7817e14516c392bb7f5cfebd88f1ed531cb37ebd39922 \
  sh -c 'cp -R /source /work && cd /work && CARGO_TARGET_DIR=/build cargo test --locked --lib'
```

This does not validate future workflow modules, Ghostty on Linux or GPUI Wayland/X11. The external VT/PTY architecture avoids an AppKit terminal dependency; Linux UI build/input, fonts, clipboard and Secret Service remain follow-up work.

## Manual native acceptance — user-selected next step

macOS returned `false` for System Events UI automation. The user chose manual checks. Record host/build hashes, steps, actual results and screenshots; an unperformed check remains open.

1. Launch the disposable shell above. Click Terminal 1; type `printf 'hello é 界 e\u0301\n'` and press Return. Check single committed input, wide-cell alignment and combining marks. Test dead-key composition and an installed CJK IME; compose/cancel/commit without duplicated or prematurely sent characters.
2. Type `sleep 30`; press Ctrl+C. Check prompt returns. Exercise arrows, Home/End, Option-modified keys, Tab and backspace. Cmd+Q must reach app Quit, not become terminal input.
3. Drag-select Unicode output, Cmd+C, and paste into a separate text editor. Copy text from that editor and Cmd+V into the shell. Check exact text, multiline handling and bracketed paste in OpenCode. Test selection at viewport edges.
4. Run `seq 1 10000`; scroll backward/forward. Resize narrow/wide and run `stty size`; verify PTY dimensions and redraw, cursor and alternate-screen behavior. Record responsiveness during output.
5. In Terminal 1, start `for i in 1 2 3 4 5; do echo first-$i; sleep 1; done`. Switch to Terminal 2 and run `echo second`; return to Terminal 1. Check output/state retained and input stays in the selected terminal. Check focus after clicking tabs and window deactivation/reactivation.
6. Inspect OpenCode's keyboard navigation, clipboard, resize and mouse interactions using a disposable provider/session. Current mouse handlers select/scroll locally; forwarding application mouse protocols is not implemented. Record unsupported behavior explicitly.
7. Check visible focus and labeled controls with macOS Accessibility Inspector/VoiceOver. Terminal cells are custom canvas paint; screen-reader text/navigation is not implemented or accepted.
8. Run the native session fixture. Inspect its screenshot, strict history check and unrelated-agent result. Check Cmd+Q and **Quit spike** manually as well as the fixture timer's actual `cx.quit()` path. A passing ordinary tool test does not resolve stubborn-child/failure handling.
9. Run `check_zed.py`; confirm Zed shows the exact evidence `worktree` path. Leave unrelated editor windows intact.

**Exit decision:** retain accepted technologies and report these gaps. Native manual results, shutdown coordination/failure proof, terminal protocol/accessibility findings and supported OS validation are required before proceeding to milestone 1.
