# Implementation backlog

GitHub is the active task tracker: [vanneszias/tasks issues](https://github.com/vanneszias/tasks/issues). Issues were created using `gh` from the repository handoff on 2026-10-14. Each contains scope, acceptance criteria and dependencies; an implementation issue is not complete until its checks pass. Milestone exit gates remain sequential.

## Milestone 0 — integration foundation

- [#1 Bounded Quit and owned-process shutdown](https://github.com/vanneszias/tasks/issues/1)
- [#2 Native terminal protocols, focus, IME and accessibility](https://github.com/vanneszias/tasks/issues/2)
- [#3 Linux boundaries and supported macOS baseline](https://github.com/vanneszias/tasks/issues/3)
- [#4 Scoped provider credentials and API contracts](https://github.com/vanneszias/tasks/issues/4)
- [#5 Acceptance decision and tested evidence](https://github.com/vanneszias/tasks/issues/5) — depends on #1–#4

## Milestone 1 — local lifecycle

- [#6 SQLite state and existing-repository onboarding](https://github.com/vanneszias/tasks/issues/6) — after #5
- [#7 Durable, idempotent branch/worktree Start](https://github.com/vanneszias/tasks/issues/7)
- [#8 Exact OpenCode binding, Zed, context and shell](https://github.com/vanneszias/tasks/issues/8)
- [#9 Workspace-first fixture UI and lifecycle gate](https://github.com/vanneszias/tasks/issues/9)

## Milestone 2 — issue integrations

- [#10 Credential-store accounts and provider configuration](https://github.com/vanneszias/tasks/issues/10) — after #9
- [#11 Paginated issue caches and independent sync](https://github.com/vanneszias/tasks/issues/11)
- [#12 Recoverable Jira transitions and GitHub labels/closure](https://github.com/vanneszias/tasks/issues/12)
- [#13 Both provider combinations and integration gate](https://github.com/vanneszias/tasks/issues/13)

## Milestone 3 — Git, CI and completion

- [#14 Dirty files, divergence and local/remote integration](https://github.com/vanneszias/tasks/issues/14) — after #13
- [#15 Commit-specific CI and run/log links](https://github.com/vanneszias/tasks/issues/15) — after #13
- [#16 Explicit Finish, completion and archive](https://github.com/vanneszias/tasks/issues/16)
- [#17 Conservative ownership-verified cleanup](https://github.com/vanneszias/tasks/issues/17)
- [#18 Git/CI/Finish/cleanup acceptance gate](https://github.com/vanneszias/tasks/issues/18)

## Milestone 4 — recovery and usable release

- [#19 Interrupted operations, missing resources and outages](https://github.com/vanneszias/tasks/issues/19) — after #18
- [#20 Native interaction and accessible UI states](https://github.com/vanneszias/tasks/issues/20)
- [#21 macOS packaging and reproducible release](https://github.com/vanneszias/tasks/issues/21)
- [#22 All product acceptance scenarios and release limits](https://github.com/vanneszias/tasks/issues/22)

Mutation checks use disposable repositories and test-provider fixtures. Native terminal/session gates need actual executable/manual evidence; passing core tests alone cannot close them. Future lifecycle implementation preserves the original checkout and user credentials/configuration.
