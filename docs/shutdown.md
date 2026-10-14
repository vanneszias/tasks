# Coordinated shutdown spike — issue #1

This is implementation/evidence guidance, not an amendment to the authoritative
specification or a declaration that milestone 0 is accepted.

## Implemented contract

- Cmd+Q, the Quit control, window close and the existing fixture timer enter one
  coordinator. The native AppKit delegate guard cancels application/Dock Quit and
  queues the same coordinator before GPUI's 100 ms application teardown begins.
  A window-close request retains the window so failure stays visible.
- Input and PTY writes, including delayed fixture input and VT-generated replies,
  are disabled before either terminal begins stopping. The Cancel Quit control
  cancels exit while bounded cleanup continues. Failed/canceled Quit keeps both
  terminals read-only; Retry Quit repeats cleanup without starting new work.
- `OwnedProcess` retains the unreaped PTY child and live ownership identities.
  It observes ancestry/session membership during execution every 100 ms and at
  shutdown, including descendant sessions captured before reparenting. A session
  must still have a captured process birth identity; a stored SID/PID alone does
  not authorize adoption or signaling. Process birth is obtained from macOS
  `proc_pidinfo` or Linux `/proc/PID/stat`, then checked immediately before signals.
- Root HUP gets 1,500 ms for graceful standalone cleanup. Remaining owned groups
  get TERM (750 ms), then KILL (750 ms). Each `ps` observation has a 400 ms limit;
  root acknowledgement uses nonblocking `try_wait` with a 400 ms limit. Snapshot
  collection is serialized with the ownership monitor. There is no blocking PTY
  child `wait()` in normal shutdown or terminal Drop. The two terminals are stopped
  sequentially on the background executor, and both are attempted on failure.
- Observation, signal and child-acknowledgement errors cancel Quit visibly and
  preserve ownership for retry. A non-clean standalone agent-root exit or actual
  TERM/KILL of that root latches persistence uncertainty across retry, even after
  reaping. The UI does not offer a bypass that turns this into persistence proof.
  Inspect/recover the exact saved conversation separately; restarting/force-exiting
  the spike is not evidence that its history was safely persisted.
- Direct OpenCode launches require `--standalone`; shutdown does not find or stop
  the user's shared service. Shell/standalone classification is based on the
  launched executable, not rendered terminal output. No persisted PID is loaded.

Stage budgets bound waiting rather than promise an exact wall-clock deadline on
an arbitrarily descheduled host. Observation scheduling adds bounded snapshot
time to each stage. The runnable tests require ordinary shutdown attempts to
return within 6 seconds per PTY and the two-terminal cleanup within 12 seconds.

## Runnable checks and observed results

```sh
cargo test --locked --features pty
cargo fmt --check
CARGO_TARGET_DIR=/Users/zias/dev/tasks/target \
GHOSTTY_SOURCE=/Users/zias/dev/tasks/.spike/ghostty \
  cargo check --locked --features native --bins
CARGO_TARGET_DIR=/Users/zias/dev/tasks/target \
GHOSTTY_SOURCE=/Users/zias/dev/tasks/.spike/ghostty \
  cargo test --locked --features native --bin terminal-spike
```

The headless PTY checks use disposable Python processes, private temporary
directories, actual controlling PTYs, distinct foreground/background process
groups, a detached descendant session whose supervisor exits, and a separate
unrelated session. They exercise TERM/KILL escalation, confirm every owned process
stops, wait beyond each tool's delayed-write deadline, and confirm the unrelated
session survives. A stubborn root's forced status survives a fast retry.

Fault coverage injects observation/signaling errors only into `cfg(test)` code and
acknowledgement errors through the existing `Child` interface in a test wrapper.
There is no production environment-variable switch to bypass shutdown checks.
A stale birth identity is also tested against a live disposable PTY process:
neither session adoption nor SIGKILL occurs despite matching PID/group numbers.

macOS headless PTY checks, native compilation and the retained Enter/Tab routing
regression pass. Native compilation reports the existing upstream future-
incompatibility notices for `block` and `proc-macro-error2`.

## Proof still required / known limits

- Actual native app lifecycle tests, screenshots, Cmd+Q/control/window/Dock Quit,
  visible failure/cancel, exact OpenCode session/location/history resume, and
  unrelated active OpenCode execution must be verified sequentially after merge.
  No native GUI lifecycle test was run in this isolated worktree. Existing
  `scripts/check_native_sessions.py` remains the disposable exact-resume fixture;
  use the merged root's relative `.spike`/`target/debug` paths.
- Clean root exit plus absence of owned execution is process acknowledgement,
  **not an OpenCode structured persistence acknowledgement**. Standalone remains
  provisional until the native exact-history checks and ownership contract are
  demonstrated. Forced exit never counts as that demonstration.
- Unix ancestry polling cannot capture a process that double-forks into a new
  session and loses all observable ancestry between snapshots. It is not a tested
  parent-death/no-orphan strategy. Crash, SIGKILL of the app, credential-changing
  children, and intentionally escaped descendants require further investigation.
  Unverifiable observed ownership cancels Quit rather than authorizing a signal.
- Birth/session/group revalidation narrows reuse races but Unix `kill(-PGID, …)`
  has no atomic compare-birth operation. A process/group can exit between the final
  check and signaling. Do not describe this spike as a kernel-enforced ownership
  guarantee. Processes never observed as owned are never deliberately targeted.
- Linux has a birth-identity implementation, but this change's executable evidence
  was collected on macOS; Linux PTY execution/native UI acceptance remains open.

The original specification, architecture, decision log, handoff, milestone report
and evidence manifest are intentionally not modified by this worktree.
