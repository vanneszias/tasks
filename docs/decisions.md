# Architecture and product decision log

## D001 — Personal, local-first application

Status: accepted.

The first version serves one person and maintains its development environment locally. Shared team state and remote execution requirements have not been established.

## D002 — macOS first, future Linux support

Status: accepted.

Initial delivery targets macOS. Platform-specific integration must be isolated so Linux support can be added. The exact portability boundary requires design and dependency validation.

## D003 — Workspace means project

Status: accepted.

A workspace groups a project's issues and corresponding repository/CI integration. It is not a per-issue checkout. Repository cardinality remains open.

## D004 — Native UI, agent terminals, external editor

Status: user-selected direction; technical integration pending validation.

Use GPUI for the app, Ghostty for OpenCode terminals, OpenCode for agents, and Zed for manual editing. Validate available integration APIs, process ownership, terminal embedding, and macOS/Linux support before finalizing architecture.

## D005 — Single repository initially

Status: accepted.

Each workspace has one repository in the first release. Multi-repository workspaces are a future upgrade.

## D006 — Explicit user prompt starts agent work

Status: accepted.

Starting an issue prepares its branch/worktree, updates its issue state, launches OpenCode, and opens Zed for that issue. OpenCode waits for a user prompt. Starting is an explicit in-app action; upstream state transitions only refresh displayed state.

## D007 — Manual local merge workflow initially

Status: accepted.

The user handles local integration manually for now. The app's responsibility for observing merges, exposing push actions, and updating issue states still needs definition.

## D008 — Future project setup command

Status: accepted direction; deferred capability.

Account for a project-specific setup command in the design. Running setup commands is not required in the first release. Its future configuration, execution environment, and failure semantics remain to be specified.

## D009 — Repository-hosted documentation

Status: accepted.

Keep specifications, decision records, and developer handoff documents in this repository.

## D010 — Existing local repositories and filtered issue lists

Status: accepted.

Workspace onboarding selects an existing local repository and configures its issue integration. Cloning/new repository setup is deferred. Issue lists support configurable filters; filter defaults are not yet decided.

## D011 — Terminal-first issue view and resumable agents

Status: accepted direction.

Show interactive OpenCode in an embedded Ghostty terminal, alongside issue details and Git/CI status. One session per issue initially. Stop the agent process when the app quits, preserve its saved conversation and worktree, and resume later. Provide an explicit Finish/Archive lifecycle with separate worktree/branch cleanup.

## D012 — Provider-specific progress mapping

Status: accepted.

Use configurable Jira workflow transitions and GitHub issue labels to represent progress. GitHub Projects integration is not required initially.

## D013 — Explicit completion and observational Git integration

Status: accepted.

The user performs commits, pushes, and local merges in Zed or a terminal. The app shows branch, changed-file summary, ahead/behind counts, merge status, and upstream CI status with a logs link, and can open Zed or an additional shell terminal.

Finish shows merge status and latest target-branch CI, offers the appropriate upstream completion action, and archives the issue locally. Removing the worktree/branch is a separate explicit cleanup action. Automatic completion on merge detection is not part of the initial workflow.

## D014 — Explicit context transfer and recoverable partial startup

Status: accepted.

Provide Copy issue context rather than sending an automatic agent prompt. Permit local startup from cached issue details when issue-provider requests fail; expose failed synchronization with Retry. Preserve prepared worktrees on OpenCode/Zed launch failure and retry the failed step.

## D015 — Existing developer tools and token authentication

Status: accepted.

Reuse installed Git, OpenCode, and Zed and their existing authentication/configuration. Store manually supplied API credentials in the OS credential store. Browser-based sign-in is deferred.

## D016 — Workspace-first navigation

Status: accepted.

Provide workspace/active-issue navigation, filtered workspace issue lists, and a terminal-centered issue detail view with a Git/CI/issue side panel. Defer the all-workspaces home overview.

## D017 — Naming and SCLAPP environment baseline

Status: accepted.

Use a configurable root with workspace/issue directories, e.g. `/Users/zias/dev/worktrees/sclapp/sclapp-41/`. Default branch examples are `feature/sclapp-41-short-description` and `feature/issue-4-short-description`. SCLAPP targets `develop`; its existing `master` instruction is outdated. Runtime/test environment setup stays manual initially.

## D018 — Concurrent issues and conservative cleanup

Status: accepted.

Switching views leaves issue processes running. Quit stops app-owned execution; resume issues individually after reopening. Existing-worktree adoption is deferred. Cleanup blocks dirty or unverified-integrated work; archiving does not require deletion.

## D019 — Rust desktop monolith and local SQLite

Status: proposed engineering default.

Use one Rust desktop application, with GPUI views, asynchronous I/O, SQLite persistence and small integration modules. Use installed Git via subprocesses. No custom daemon, plugin framework, event-sourced system or generic workflow engine is needed. Future multi-repository support can migrate the single-repository schema when implemented.

## D020 — Ghostty integration must be demonstrated

Status: technical gate.

Current Ghostty source distinguishes its macOS-oriented internal full embedder API from the external `libghostty-vt` API. The latter provides terminal state/input handling but needs a renderer and PTY host; it is also marked unstable. Prefer evaluating VT plus GPUI rendering for portability. Do not assume an existing supported full Ghostty widget can be dropped into GPUI. Pin tested revisions after a proof of concept.

## D021 — OpenCode session ownership must be explicit

Status: technical gate.

OpenCode V2 defaults to a shared background service. Terminal exit is insufficient evidence that execution stopped. Validate exact session identification, interruption, child-process handling and resume semantics. Evaluate documented standalone mode first for bounded process ownership, while verifying persistence and compatibility with existing configuration. Never stop the user's entire shared service to implement app quit.
