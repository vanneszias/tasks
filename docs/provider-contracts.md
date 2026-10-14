# Provider contracts — milestone-0 findings

Checked 2026-10-14 against public provider OpenAPI schemas and official authentication documentation. [Schema identities and SHA-256](evidence/milestone-0/tested.json) are durable evidence; `scripts/inspect_provider_contracts.py` fetches schemas into ignored `.spike`. These are documentation/schema findings, not authenticated account or write tests.

## Jira Cloud

- Manually supplied Atlassian account API token with account email, sent as HTTP Basic. Account passwords are not API credentials. Unscoped-token site calls use `https://SITE.atlassian.net`; scoped tokens require the documented gateway/site-ID setup, which still needs an authenticated test before claiming support.
- Schema classic scopes: `read:jira-work` for search, issue reads and available transitions; `write:jira-work` for transitioning. Granular alternatives differ per endpoint: transition reads include `read:issue.transition:jira`, `read:status:jira`, `read:field-configuration:jira`; writes include `write:issue:jira`, `write:issue.property:jira`. Do not confuse OAuth scope names with confirmed scoped-token setup. Browse Projects, issue security and Transition Issues permissions remain necessary regardless of token.
- Use enhanced `GET`/`POST /rest/api/3/search/jql` with configurable JQL, requested fields, `maxResults` and opaque `nextPageToken`. Inspect completion fields and continue until the response indicates completion; legacy `/search` is being removed. POST avoids long query URLs.
- Read `/rest/api/3/issue/{issueIdOrKey}`; Jira v3 rich description is Atlassian Document Format, so Copy context needs a readable conversion.
- Read `/rest/api/3/issue/{issueIdOrKey}/transitions?expand=transitions.fields`. Select currently available transition IDs, not status names. Inspect each field's `required`, schema and allowed values; report missing required-field configuration instead of inventing values.
- Transition with `POST .../transitions`, body `{"transition":{"id":"ID"}}` plus explicitly configured required fields where supported. After timeouts, re-read current state before retrying. Provider workflow permissions can change between lookup and write.

Source: [Jira Basic auth](https://developer.atlassian.com/cloud/jira/platform/basic-auth-for-rest-apis/) and the Jira schema URL in the manifest.

## GitHub

- Manually supplied fine-grained PAT is the intended initial token type; classic PATs remain a documented option with broader `repo` access for private repositories. Send Bearer token and GitHub JSON Accept header. Pin and test an API version explicitly rather than relying on a server default; current docs advertise `2026-03-10`, while earlier clients commonly pin `2022-11-28`.
- Fine-grained repository permissions to validate: Issues read/write, Actions read, Checks read; Contents read if querying combined commit statuses. Restrict to selected repositories. Organization approval/SSO and the user's own permissions may further limit access.
- `GET /repos/{owner}/{repo}/issues`: configurable `state`, `assignee`, comma-separated `labels`, optional `since`, sort/direction. Exclude every item containing `pull_request`. Follow `Link` pagination; `per_page` maximum 100. Do not treat a page containing only PRs as list exhaustion.
- Read `GET .../issues/{issue_number}`. Add existing configured progress label with `POST .../issues/{issue_number}/labels`, body `{"labels":["in-progress"]}`. Avoid replacing the full label set. On successful completion remove only that label using URL-encoded `DELETE .../labels/{name}`.
- Close with `PATCH .../issues/{issue_number}`, body `{"state":"closed","state_reason":"completed"}`. Re-read on ambiguous timeout or conflicting state. Archiving and upstream completion have independent outcomes.
- Commit-specific Actions: `GET /repos/{owner}/{repo}/actions/runs?head_sha=SHA`; retain workflow ID, attempt, status/conclusion, `head_sha` and `html_url`, and paginate. Check runs: `GET /repos/{owner}/{repo}/commits/{SHA}/check-runs`; paginate and decide explicitly whether to request all runs rather than only latest. Combined statuses, if needed, are `GET .../commits/{SHA}/status`, not a substitute for Actions/check attempts.
- Branch filters are supplementary: verify returned SHA equals the observed target SHA. Missing checks, incomplete pages and old green results are absent/unknown/stale, not passing.

Sources: [GitHub issues REST reference](https://docs.github.com/en/rest/issues/issues), [PAT permissions](https://docs.github.com/en/rest/authentication/permissions-required-for-fine-grained-personal-access-tokens), and GitHub schema in the manifest. Permission/API-version combinations still require test-account verification.

## Bitbucket Cloud

- Scoped Atlassian API token with account email via Basic authentication; current documentation also supports Bearer API-token authentication. Repository/project/workspace access tokens are separate Bearer credential types with different resource boundaries. Do not assume a Jira token grants Bitbucket access or request Git credentials for CI reads.
- Inspected endpoint scopes: `read:repository:bitbucket` for repository/commit statuses, `read:pipeline:bitbucket` for pipeline lists and logs. No pipeline-write/admin scope is needed for observational CI. User/repository access still applies.
- Read `GET /2.0/repositories/{workspace}/{repo_slug}`. Pipeline lookup: `GET .../pipelines?target.commit.hash=SHA`, optionally branch filters. Schema explicitly supports the commit filter. Verify the response's target commit SHA and retain each pipeline UUID/run, state, timestamps and links.
- Read commit statuses using `GET .../commit/{SHA}/statuses`, with optional `refname`, `q`, `sort`. `/statuses/build` is the write collection and is not the GET list endpoint.
- Follow `next` pagination URLs and supported `pagelen`; do not reconstruct pagination from assumed total counts. Follow only the expected provider origin and never forward credentials to arbitrary links.
- Step logs: `GET .../pipelines/{pipeline_uuid}/steps/{step_uuid}/log`; prefer returned run/log links for user navigation. Multiple pipelines/statuses remain separate; no results do not imply success.

Sources: [Using API tokens](https://support.atlassian.com/bitbucket-cloud/docs/using-api-tokens/) and Bitbucket schema in the manifest. Live scoped-token/access-token coverage is still untested.

## Adapter verification still required

Fixtures must exercise pagination, PR exclusion, preserving unrelated labels, Jira required fields, expired credentials, permission errors, rate limiting, ambiguous write timeouts, upstream conflicts, multiple runs/attempts and stale SHAs. Account validation and writes must use opt-in test accounts/issues; the real SCLAPP and SMOG examples are not mutation fixtures. Credentials belong in the OS credential store, not SQLite, reports or request logs.
