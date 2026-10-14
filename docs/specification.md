# Development control center — product specification

Status: v0.1 handoff specification. Confirmed requirements below reflect the interview. Proposed engineering defaults are identified in the architecture document; feasibility gates remain before implementation commitments.

## Goal

A native app for managing project issues and their development lifecycle across local worktrees, branches, OpenCode agents, manual work in Zed, upstream code review, and CI.

## Confirmed requirements

- Personal, local-first application initially.
- Initial platform: macOS. Architecture must allow future Linux support.
- GPUI for the application UI.
- Ghostty for terminals used by OpenCode; embedding feasibility and integration method require investigation.
- Zed is the preferred external editor for manual work.
- A workspace represents a project: its collection of issues and corresponding upstream repository/CI.
- Issue sources: Jira or GitHub Issues. Repository/CI providers: Bitbucket or GitHub.
- Each issue being worked on receives its own worktree and branch(es).
- Users can move an issue into progress, work with an agent, inspect/edit manually, merge into a target branch, push, and see CI status.
- Status changes should be reflected in the relevant connected services; exact mappings and triggers are unresolved.
- First release: one repository per workspace. Multi-repository workspaces are a future upgrade.
- Starting an issue fetches the repository, creates an issue branch/worktree from the target branch, moves the issue to the configured in-progress state, launches OpenCode, and opens that worktree in Zed. Detailed ordering and failure handling remain to be specified.
- OpenCode waits for the user's prompt; it does not automatically plan or implement the issue.
- Initial integration uses the user's manual local merge flow. Automated merging and pull-request workflows are not initial requirements.
- Project setup commands are a future capability to account for in the design, not a first-release requirement.
- Specifications and handoff documents remain in this repository.
- Add a workspace from an existing local repository. Cloning/new repository setup is a future capability.
- Issue lists support configurable filters. Exact defaults and supported filter fields remain to be defined.
- Only the in-app Start action creates the local environment and launches tools. Upstream issue transitions refresh displayed state without launching anything.
- The active issue view contains an embedded Ghostty terminal running interactive OpenCode, alongside issue details and Git/CI status.
- One OpenCode session per issue initially. Stop its process when the app quits; resume the saved conversation in the same worktree later.
- Preserve worktrees and session history until explicit cleanup.
- Configure Jira workflow transitions and GitHub issue labels for in-progress state. GitHub Projects integration is not required initially.
- Finish is an explicit user action: show merge status against the target branch and latest target-branch CI result; let the user choose whether to transition Jira to its configured completed state or close the GitHub issue; archive locally. Worktree and branch removal is a separate explicit cleanup action.
- Initial Git features are observational: branch, changed-file summary, ahead/behind counts, merge status, and upstream CI status with a logs link. Offer opening Zed or an additional shell terminal. Users perform commits, pushes, and merges in their editor/terminal.
- Provide Copy issue context with title, description, URL, and target branch. The user pastes it into OpenCode and supplies instructions.
- Allow starting from cached issue details when the issue provider is unavailable; surface failed upstream updates and offer Retry. Preserve successful local preparation when tool launches fail and retry the failed step.
- Reuse existing Git credentials/SSH configuration and existing OpenCode and Zed installations/configuration. API integrations initially use manually supplied tokens stored in the OS credential store. Browser-based sign-in is a future enhancement.
- Initial layout: workspace/active-issue navigation at left; selected issue's OpenCode terminal and additional shell tab centrally; issue and Git/CI details in a side panel; a filtered issue list at workspace level. An all-workspaces home overview is deferred.
- Worktree path and branch naming are configurable. Accepted examples: `/Users/zias/dev/worktrees/sclapp/sclapp-41/`, `feature/sclapp-41-short-description`, and `feature/issue-4-short-description` for GitHub. The root directory is user-selected.
- SCLAPP targets `develop`; its `master` instruction is outdated. Initial worktrees support editing/agents; application runtime and test environment setup remain manual.
- Attaching existing worktrees is deferred. Reopening an environment already created by this app is supported.
- Switching issues preserves running sessions. Multiple issues may run concurrently, with one OpenCode session per issue. On app reopen, sessions are resumed individually rather than all automatically restarted.
- Cleanup refuses removal if there are uncommitted changes or commits not verified as integrated into the target. Archiving remains possible without cleanup.

## Concrete reference workflows

| Workspace | Issue | Repository | Target branch |
| --- | --- | --- | --- |
| SCLAPP | https://statik.atlassian.net/browse/SCLAPP-41 | https://bitbucket.org/statikbe/sclapp | develop |
| SMOG | https://github.com/vanneszias/smog/issues/4 | https://github.com/vanneszias/smog | develop |

These URLs are user-provided examples. Their contents have not yet been inspected.

### Local SCLAPP findings (2026-10-14)

Read-only inspection of `/Users/zias/dev/sclapp` found:

- Current checkout is `develop`; `git worktree list` registers only the main checkout.
- Local branches include `feature/legacy-import`, `WIP`, `acceptance`, and `master`; this does not establish a consistent issue-branch naming convention.
- Recent commit subjects commonly use `SCLAPP-39: feat: ...`, `SCLAPP-56: docs: ...`, etc., with occasional lowercase issue keys.
- `/Users/zias/dev/worktrees/sclapp/golden-cabin/` exists, but is not listed as a registered worktree of this checkout. Do not treat that directory as an app-owned or valid worktree without validation.
- `AGENTS.md` lines 117–122 name `master` as trunk and say worktrees cannot run the application because DDEV configuration, database, certificates, and `.env` are tied to the main checkout. The user confirmed `develop` and accepted manual runtime setup initially. Do not rewrite repository instructions automatically; copied issue context must identify the configured target.
- Bitbucket Pipelines configuration is present.

## Domain and workflow contracts

See [Architecture](architecture.md) for entities, independent state dimensions, operation ordering, provider mapping, recovery, and technical validation gates.

## Screens and interactions

### Workspace onboarding/settings

Choose an existing repository; validate Git identity and remote; configure workspace name, issue provider/account/project, code provider/repository, remote and target branch, worktree root, branch pattern, issue filter, and progress/completion mapping. Discover installed OpenCode/Zed and allow executable paths to be corrected. Test each service connection independently. Credentials go to the OS store, never plain settings.

### Workspace issues

Show key/number, title, upstream status/labels, assignee, and local environment state. Support refresh, editable filter, pagination, loading, empty results, invalid filter, and cached/offline states with last-refresh time. Active issues remain in navigation even when excluded by the filter. Start is per issue; existing app-owned environments offer Open/Resume.

### Issue detail

Header: issue key/title and source link, Start/Open/Resume as applicable, Open in Zed, Copy issue context, Finish. Center: OpenCode terminal and shell tab. Side panel: description, source status, worktree path, branch and target, changed-file summary, tracking ahead/behind counts, local/remote integration evidence, CI runs and logs links, synchronization failures with Retry.

Selection changes must not restart processes or discard terminal state. Keyboard focus stays predictable between navigation and the terminal; terminal shortcuts are not swallowed by app shortcuts. Copy/paste, text selection, resize, scrolling, Unicode and input composition are required terminal interactions. Provide labeled controls and visible focus; accessibility feasibility is part of terminal validation.

### Finish/archive/cleanup

Show current integration evidence and target-commit CI. Offer upstream completion explicitly; local archival and upstream success have separate results. Archived items remain accessible and retain failed synchronization actions. Cleanup is separate, names the exact checkout/branch, and explains why removal is blocked. Session history survives checkout cleanup; resuming requires that its worktree still exists.

## Deferred scope

Multi-repository workspaces; cloning/new project setup; setup commands and automatic DDEV/container provisioning; attaching existing worktrees; multiple agent sessions per issue; agents continuing after app exit; browser sign-in; global home overview; GitHub Projects; app-driven commit/push/merge or PR lifecycle; remote execution and shared team state.

## Outstanding decisions

Product name and visual styling can be chosen during implementation. The first release is not blocked on either. Ghostty integration route, OpenCode lifecycle/session binding, exact dependency versions, provider credential requirements, and supported minimum OS versions must be resolved in milestone 0. See the handoff for explicit gates.

## Acceptance criteria

1. Add SCLAPP and SMOG from local repositories and show their filtered issue lists with independent credentials and status mappings.
2. Starting an issue creates exactly one branch/worktree from the fetched configured target, opens that path in Zed, launches interactive OpenCode without sending a prompt, and attempts the configured provider progress update.
3. Clicking Start twice or retrying after a partial failure does not create duplicate branches, checkouts, sessions, or labels.
4. Copy issue context includes the title, readable description, issue URL, and configured target without modifying repository files.
5. Two issues can run independently; switching between them retains their terminal sessions and output.
6. Quit stops app-owned agent execution. Reopen and Resume restore the exact saved conversation and worktree without disturbing unrelated OpenCode sessions.
7. Source-side status changes refresh the app but never create worktrees or launch tools.
8. Issue API failure permits cached local work and exposes retryable synchronization failure; launch failure preserves the worktree. Git fetch failure never silently claims a fresh base.
9. Git/CI observations show commit identity and freshness; missing/stale CI is never displayed as passing.
10. Finish offers upstream completion and archives locally; failed completion remains retryable. Cleanup never deletes dirty or unverified work, the main checkout, or an unrelated directory.
11. A crash between Git creation and local persistence can be reconciled using recorded ownership and Git metadata without deleting or adopting unrelated work.
12. Core workflow/storage/Git/provider code is platform-independent; macOS-specific terminal, credential, and launch code is isolated, and the Linux build strategy is documented.
