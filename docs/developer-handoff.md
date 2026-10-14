# Developer handoff — native project control center

Status: v0.1, ready for technical validation and phased implementation. Product scope is agreed; marked engineering proposals may be refined with evidence. No application code has been built in this discovery task.

## Read first

1. [Product specification and acceptance criteria](specification.md)
2. [Architecture and workflow contracts](architecture.md)
3. [Decision log](decisions.md)

## Product in one paragraph

Build a personal, local-first macOS app in Rust/GPUI, structured for a future Linux port. A workspace is an existing local repository plus its Jira/GitHub issues and Bitbucket/GitHub CI. Start creates an issue branch/worktree from the configured target, updates upstream progress, opens Zed and interactive OpenCode in embedded Ghostty, and waits for the user's prompt. The user commits, merges and pushes manually. The app observes Git/CI, supports explicit Finish/archive and conservative cleanup, and stops/resumes issue agents across app exits.

## First integration examples

- SCLAPP: Jira `SCLAPP-41`, Bitbucket `statikbe/sclapp`, target `develop`. Local repo `/Users/zias/dev/sclapp`; example worktree `/Users/zias/dev/worktrees/sclapp/sclapp-41/`. Runtime setup remains manual; do not assume DDEV works in new checkouts.
- SMOG: GitHub `vanneszias/smog`, issue `4`, target `develop`. User selects local repo during onboarding.

Use disposable repositories and test issues for mutation tests. Do not use these real issues as automated test fixtures or transition them just to demonstrate connectivity.

## Milestone 0 — prove the integration foundation

Deliver a small runnable technical prototype and record exact dependency/tool versions and evidence.

1. **GPUI + Ghostty:** render an embedded interactive terminal, run OpenCode, demonstrate input/clipboard/resize/scrollback and two retained terminals. Evaluate external Ghostty VT + GPUI rendering first. Document macOS and Linux implications.
2. **OpenCode ownership:** create/bind a specific conversation, quit during work, verify its execution actually stops, reopen and resume the same session. Prove unrelated OpenCode sessions are unaffected. Inspect installed V2 help/OpenAPI for exact supported invocation and payloads. Test standalone persistence/concurrency before choosing it.
3. **Native launch:** open a selected worktree in installed Zed without blocking the app; confirm launch behavior and executable discovery.
4. **Provider contract check:** document current supported token types, scopes, endpoints, filtering/pagination, Jira transition fields, GitHub labels/closure, and Bitbucket/GitHub commit-specific CI lookups. Real account reads may validate access; writes use test fixtures/accounts.
5. **Portability:** core crate/modules build on Linux; platform-specific dependencies are gated. Document minimum macOS/toolchain versions after validating dependencies.

Exit: technical evidence that embedded terminals, exact resume, and stop-on-quit can meet the product contract. If blocked, report findings before building the full UI around an assumed API. Do not silently replace Ghostty or weaken shutdown semantics.

## Milestone 1 — local lifecycle

- GPUI application shell, SQLite migrations, workspace onboarding from existing repositories.
- Branch/worktree creation with configurable naming, durable operation steps, and safe retry.
- OpenCode session binding, copy context, Zed launch, shell tab, concurrent issue selection, quit/resume.
- Use a minimal fixture issue source during this milestone; it is a development aid, not an additional production feature.

Exit: a fixture issue runs through Start → edit → quit → Resume, including a launch failure and retry, without duplicate resources or changes to the original checkout.

## Milestone 2 — issue integrations

- Credential-store backed accounts; configurable Jira JQL and GitHub issue filters.
- Issue caches, pagination, source links and independent sync status.
- Jira progress transitions, GitHub progress labels, explicit Retry and conflict handling.
- Verify both SCLAPP-shaped and SMOG-shaped provider combinations using fixtures/test accounts.

Exit: source-side transitions never launch anything; cached work remains usable during provider outages; replaying a Start retry is idempotent.

## Milestone 3 — Git, CI and completion

- Changed-file/tracking/integration status with local vs remote evidence.
- Commit-specific Bitbucket Pipelines and GitHub CI observations with logs links.
- Finish, optional upstream completion, archive, archived-item access and pending-sync recovery.
- Conservative cleanup with ownership, dirty-state and target-integration verification.

Exit: manual merge/push is reflected correctly, stale green CI is not misrepresented, and dirty/unmerged/unpushed or unrelated work cannot be deleted by cleanup.

## Milestone 4 — recovery and usable release

- Reconcile interrupted operations, missing files, expired credentials and network failures.
- Keyboard/focus/terminal polish, empty/loading/error states, actionable diagnostics.
- macOS app packaging and a documented development/build process; assess signing/distribution needs based on personal use.
- Re-run the product acceptance scenarios and document known limitations.

Exit: both supported provider flows satisfy the specification with reproducible evidence. Deferred features remain deferred.

## Verification strategy

Use actual disposable Git repositories for lifecycle tests: main checkout dirty, branch/path collision, duplicate Start, crash between Git and SQLite steps, ignored/untracked files, worktree missing, local merge not pushed, normal pushed merge, squash/cherry-pick ambiguity, cleanup partial failure. Assert that original checkout contents and unrelated branches survive.

Use fixture HTTP responses for Jira/GitHub/Bitbucket pagination, auth expiry, rate limiting, required transition fields, timeouts after a successful write, state conflicts, label preservation, multiple CI runs and stale SHA results. Add a small opt-in live contract smoke test against test accounts.

Terminal/session integration needs executable/manual acceptance evidence: multiple terminals, Unicode/input composition, keyboard routing, resize, large output, switching issues, exact-session resume, shutdown during an active tool, and an unrelated external OpenCode session remaining alive. Test actual app Quit, not only terminal closure.

Do not invent agent busy/idle semantics by scraping rendered text. Do not use passing unit tests as a substitute for proving the native rendering/process lifecycle.

## Implementation guardrails

- App owns its created environments and bound sessions; providers own upstream issue/CI facts; Git owns checkout truth.
- Run Git and tools with explicit cwd and argument arrays; preserve user's tool config and repository instructions.
- Keep secrets out of SQLite/plain configuration/logs. Failed operations report the affected step without including tokens.
- Avoid an app daemon, generic workflow engine, provider plugin system, speculative multi-repo schema and automatic environment provisioning.
- No Git mutations beyond the requested worktree/branch creation and explicit conservative cleanup. Commit/push/merge remain manual.
- Unknown or stale observations must remain visibly unknown/stale.
- Never silently adopt existing worktrees or delete paths to recover from startup errors.

## Suggested prompt for the implementing agent

> Read `docs/specification.md`, `docs/architecture.md`, and `docs/decisions.md`. Implement milestone 0 from `docs/developer-handoff.md` first. Record tested revisions and runnable evidence for embedded GPUI/Ghostty terminals and OpenCode exact-session resume/stop-on-quit. Keep accepted scope intact; flag concrete blockers rather than substituting technologies. Then implement the milestones sequentially with the stated acceptance checks. Preserve existing user repositories and credentials, and use disposable Git/test-provider fixtures for mutation tests.
