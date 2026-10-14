# Architecture and workflow specification

Status: proposed engineering design supporting confirmed product requirements. Validate the marked technical gates before committing to integration APIs. Research checked 2026-10-14.

## 1. Shape

One Rust desktop application:

```text
GPUI workspace / issue / terminal views
             |
Application operations: Start, Resume, Finish, Cleanup, Refresh
             |
SQLite + Git CLI + Jira/GitHub/Bitbucket HTTP adapters
             |
Ghostty terminal host + OpenCode session integration + Zed launcher
             |
Platform services: credential store, clipboard, URLs, process handling
```

Start with modules in one app crate; isolate Ghostty FFI if its build requires another crate. Keep blocking Git, database work and HTTP off the UI thread. Pin tested Rust/GPUI/Ghostty/Zig versions and retain lockfiles. Do not use floating `main` dependencies in the delivered app.

Use SQLite for durable app state and migrations. OpenCode owns conversation content; persist its session identifier, not a second conversation database. Secrets live in macOS Keychain, with a future Linux Secret Service implementation. Normal settings contain credential references only. Avoid speculative interfaces except real provider/platform boundaries.

## 2. Domain and persistence

| Entity | Essential fields / ownership |
| --- | --- |
| Workspace | Stable ID, name, local repository path, canonical Git common-directory identity, remote name/URL, target branch, worktree root, branch pattern, issue-source binding, code-provider binding, filter, progress/completion mappings |
| Account | Provider, host/site identity, account label, OS credential reference; reusable across workspaces |
| Issue cache | Workspace ID + immutable provider issue ID, display key/number, URL, title, description, upstream status/labels, assignee, refreshed time |
| Environment | ID, issue link, exact branch/path, base commit, target captured at creation, app ownership marker, lifecycle, created/archive timestamps |
| Agent binding | Environment ID, exact OpenCode session ID and compatible server/data context, last known state; ephemeral process handles are not reusable after restart |
| Operation | ID, environment, kind, completed steps, intended branch/path/base, desired upstream change, error and retry state |
| CI cache | Provider/repository, run ID, commit SHA, branch, workflow/pipeline identity, attempt, normalized state, URL, refreshed time |

Enforce one environment per issue in v1 and one OpenCode session binding per environment. Keep archived environment records after cleanup. A future repository table can replace workspace repository columns; do not build unused multi-repo orchestration now.

Glossary: a **workspace** is a project; an **environment** is the local work for an issue; a **worktree** is its Git checkout; an **agent session** is a persisted conversation, not a terminal process; a **terminal** is a process I/O surface; **archive** hides finished local work without deleting it; **cleanup** removes verified disposable Git resources.

## 3. Separate state dimensions

| Dimension | States / source of truth |
| --- | --- |
| Issue | Jira workflow state or GitHub open/closed + labels; provider-owned |
| Environment | Not started → preparing → active → archived; preparation can be partial/failed; cleanup tracked separately |
| Agent | Not started, launching, connected, stopped, failed; busy/waiting only when verified through OpenCode structured signals |
| Synchronization | Current, pending, failed, stale, conflict |
| Git | Branch/path existence, dirty files, tracking divergence, target ancestry; local Git observations |
| CI | Unknown, absent, queued, running, passed, failed, canceled, skipped; provider-owned and commit-specific |

No single “done” flag spans these dimensions. An archived item may have failed upstream completion. A running terminal is not proof an agent is generating. If structured activity is unavailable, show connected/unknown rather than infer state from terminal text.

## 4. Start operation

1. Serialize Start for the issue and Git mutations for its repository. Persist intended operation before side effects. If app-owned environment exists, open/resume it rather than duplicate it.
2. Validate repository, configured remote/target, branch name, worktree destination, and issue identity. Sanitize path components, use `git check-ref-format`, and pass process arguments as arrays. Do not interpolate issue titles into shell commands.
3. Fetch the configured remote and resolve the remote-tracking target commit. Snapshot its SHA. Source API unavailability is independent of Git connectivity. Proposed default: if fetch fails, stop preparation with Retry rather than silently branch from a stale target.
4. Create branch and worktree from that SHA without modifying the original checkout. Record created resources and reconcile Git's result before proceeding. A conflicting existing branch/directory is a visible error, not permission to adopt or overwrite it.
5. Attempt configured in-progress synchronization. If unavailable, record failure and continue local launch. Re-read state before retrying; do not reapply an old transition after the issue was independently completed.
6. Launch interactive OpenCode with this worktree as its working directory, bind the exact session, and launch Zed at the same path. Track these results independently. No prompt is sent. Offer Copy issue context.
7. Mark local environment active when Git preparation succeeds; show launch/sync failures separately with step-specific Retry. Do not delete successful preparation to simulate rollback.

Naming is chosen once and persisted. Title changes do not rename branches. Branch/path collisions require the user to edit the proposed name; never force replacement. Repository instructions/config are read by OpenCode normally; the app does not rewrite them.

## 5. Sessions, editing, and shutdown

Multiple issue terminals may run concurrently. Only view visibility changes when selecting another issue. Zed and OpenCode share the same checkout; the app observes edits without file locking or arbitration. The user can interrupt the agent in the TUI before overlapping edits. Do not create an automatic commit/stash mechanism.

OpenCode V2 normally uses a shared server. Validate standalone mode as the initial ownership strategy, including whether multiple private servers safely share persistence and how exact session resume is selected. If using a shared service instead, interrupt only bound sessions through the supported API, verify execution ended, and terminate only app-owned clients/children. Never kill all OpenCode processes or change global service settings.

On normal Quit, stop accepting launches, interrupt active app-owned agent work, wait for acknowledgement/persistence, close owned terminal processes and shell process groups, persist session references, and exit. Define bounded shutdown behavior in the spike and expose interruption failure instead of falsely reporting success. Do not terminate external Zed windows. Crash recovery must reconcile live ownership; do not signal a process using a persisted PID alone. Strong no-orphan behavior after a crash requires a tested parent-death strategy, not an assumed guarantee.

On restart show saved environments as stopped or requiring reconciliation. Resume reuses the exact session and path, without applying Start transitions again. Missing worktree/session is an actionable error; do not silently create a replacement conversation. Shell scrollback persistence is not required; OpenCode conversation history is.

## 6. Provider integration contracts

Use normal provider HTTPS APIs from the app; MCP tools used during development are not runtime dependencies.

| Provider | v1 operations |
| --- | --- |
| Jira Cloud | Validate account/site; query issues using configurable JQL; read issue; list currently available transitions; apply configured progress/completion transition |
| GitHub | Query repository issues with configurable state/assignee/label filters; exclude PRs; read issue; add progress label; close issue; read Actions/check results and run links |
| Bitbucket Cloud | Validate repository/account; read pipeline/build status and run/log links; Git transport remains installed Git's responsibility |

Verify exact endpoints, current token types/scopes, pagination and Jira transition-required fields against provider docs during implementation. Do not assume Jira and Bitbucket share credentials. Expose unsupported required transition fields as actionable configuration failures, not fabricated values. Proposed label policy: require/select an existing progress label and preserve unrelated labels; remove the configured progress label on successful GitHub completion. Validate this default during account configuration.

Provider data wins on refresh. App actions express desired transitions; they do not force external state to follow local archival. If current source state changed since the action, refresh and explain the conflict rather than replaying stale intent. Persist failed requested updates for explicit Retry, including for archived issues. Read requests may retry with bounded backoff; handle pagination, rate limits, expired tokens and permission errors distinctly. Do not blindly replay writes after an ambiguous timeout: re-read first.

Proposed polling defaults: selected issue and CI every 30 seconds while active; workspace list every 2 minutes; refresh on app focus and manual request. Respect rate-limit guidance, pause unnecessary background polling, coalesce identical repository requests, and label cached timestamps. No webhook server is necessary for v1.

## 7. Git and CI observation

Use Git CLI machine-readable output, including NUL-delimited file names. Inspect worktree status, branch tracking, common-directory identity and registered worktree metadata. Changed-file summary is observational; no integrated diff editor is required.

Show separately: issue branch divergence from its configured tracking branch; whether issue HEAD is an ancestor of local target HEAD; whether it is an ancestor of last-fetched remote target HEAD. Missing tracking branches have an explicit “not tracking” state. Never label a local merge as pushed merely because it is locally integrated.

CI must be keyed by commit SHA and workflow/pipeline/attempt. Show the target commit and runs checked; a green run for a previous commit is stale, not passing for the current target. Multiple runs remain visible. No runs means absent/unknown, not success. Initial Finish displays this evidence but does not impose an unrequested passing-CI merge gate.

Normal ancestry proves merge integration. Squash/cherry-pick integration is unverified unless a future reliable strategy is implemented; do not use similar commit subjects as proof. Worktree cleanup blocks on unverified integration.

## 8. Finish and cleanup

Finish refreshes observations, displays merge/CI evidence, offers the configured Jira completion transition or GitHub closure, and archives locally on confirmation. Upstream failure remains visible/retryable; local archive can still succeed. Stop the issue's owned execution before marking it archived. No merge, push, or branch deletion is implied.

Cleanup revalidates app ownership, canonical path, Git registration, stopped processes, and dirty state immediately before mutation. Include tracked/untracked files; if ignored local files exist, block deletion or require the user to remove them manually rather than discard environment data. Refuse main-checkout removal. Proposed conservative default: require ancestry into the freshly fetched remote target before deletion, so locally merged but unpushed work remains protected. If remote verification is unavailable, block cleanup, not archive.

Use ordinary Git worktree removal and non-forced branch deletion, verifying the configured target explicitly. If Git refuses branch deletion because its own upstream checks differ, retain the branch and report partial cleanup. Never use force deletion/reset or recursive directory deletion to bypass a refusal. Track checkout removal and branch removal separately; retries inspect what still exists. Retain issue/session metadata after cleanup.

## 9. Ghostty / GPUI technical gates

GPUI supports macOS and Linux backends but is pre-1.0. Current Ghostty `include/ghostty.h` explicitly describes an internal macOS-oriented API and directs external embedders to `libghostty-vt`. The VT header supplies terminal state, input encoding, render-state access and scrollback, but declares its API unstable. It is not a ready-made GPUI terminal widget.

Preferred investigation: Ghostty VT core through C FFI, GPUI terminal renderer/input layer, and a PTY host. Demonstrate Unicode/wide/combining characters, colors, selection, scrollback, alternate screen, bracketed paste, keyboard/mouse protocols used by OpenCode, input composition, resize, cursor/focus and multiple terminals. Assess accessibility and clipboard handling. Record rendering cost while hidden and under heavy output.

Compare effort with the internal full-surface approach only if needed; document its platform constraints and upstream maintenance burden. No silent substitution with a different emulator or an external terminal window: that changes the accepted embedded Ghostty requirement. If neither route is viable, report the concrete blocker for a product decision.

Isolate FFI/native handles and teardown order. Keep domain/provider/storage code free of AppKit types. Build core tests on Linux early; the Linux terminal UI may follow later, but the chosen rendering/PTY architecture must have a credible Linux implementation path.

## 10. Future setup-command boundary

Reserve a documented insertion point after worktree creation and before tool launch. Do not build a hook framework now. A future command will need cwd, explicit environment, output/exit status, cancellation, retry semantics and per-worktree runtime identity. It must not automatically reuse SCLAPP's database/services across concurrent checkouts. Add the schema and UI when the capability ships.

## 11. Evidence and unresolved technical contracts

Official sources inspected:

- [GPUI site](https://www.gpui.rs/) and [current README](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md): Rust, platform backends, pre-1.0 status.
- [Ghostty architecture](https://ghostty.org/docs/about): shared core and native frontends.
- [Ghostty internal header](https://github.com/ghostty-org/ghostty/blob/main/include/ghostty.h): internal API and platform constraints.
- [Ghostty VT header](https://github.com/ghostty-org/ghostty/blob/main/include/ghostty/vt.h): external terminal API scope and instability.
- [OpenCode V2 CLI](https://opencode.ai/v2/docs/cli): interactive invocation, shared service, standalone option.
- [OpenCode V2 API](https://opencode.ai/v2/docs/api): session creation/lookup/interruption and events; API described as experimental.

These are documentation/source checks, not proof of a working integrated prototype. Pin source revisions in the milestone-0 report. Exact OpenCode session-selection CLI options, API payload/location/auth contract, service ownership, persistence under standalone mode, Zed launch flags and provider API contracts still need validation against the installed/pinned versions. Do not infer them from old OpenCode V1 examples.
