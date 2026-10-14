"""Read public provider schemas; no account credentials and no provider writes."""
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import urllib.request

SOURCES = {
    "jira": ("https://dac-static.atlassian.com/cloud/jira/platform/swagger-v3.v3.json",
             ["/rest/api/3/search/jql", "/rest/api/3/issue/{issueIdOrKey}",
              "/rest/api/3/issue/{issueIdOrKey}/transitions"]),
    "github": ("https://raw.githubusercontent.com/github/rest-api-description/main/descriptions/api.github.com/api.github.com.json",
               ["/repos/{owner}/{repo}/issues", "/repos/{owner}/{repo}/issues/{issue_number}",
                "/repos/{owner}/{repo}/issues/{issue_number}/labels",
                "/repos/{owner}/{repo}/issues/{issue_number}/labels/{name}",
                "/repos/{owner}/{repo}/actions/runs", "/repos/{owner}/{repo}/commits/{ref}/check-runs"]),
    "bitbucket": ("https://dac-static.atlassian.com/cloud/bitbucket/swagger.v3.json",
                  ["/repositories/{workspace}/{repo_slug}", "/repositories/{workspace}/{repo_slug}/pipelines",
                   "/repositories/{workspace}/{repo_slug}/commit/{commit}/statuses",
                   "/repositories/{workspace}/{repo_slug}/pipelines/{pipeline_uuid}/steps/{step_uuid}/log"]),
}


def inspect(entry):
    provider, (url, paths) = entry
    with urllib.request.urlopen(url, timeout=30) as response:
        raw = response.read()
    source = json.loads(raw)
    result = {"source": url, "sha256": hashlib.sha256(raw).hexdigest(),
              "version": source.get("info", {}).get("version"), "paths": {}}
    for path in paths:
        # Bitbucket's documented path templates sometimes retain trailing slashes.
        actual = path if path in source["paths"] else path + "/"
        methods = source["paths"].get(actual)
        if methods is None:
            result["paths"][path] = {"error": "path not present; do not infer supported endpoint"}
            continue
        result["paths"][actual] = {method: {key: value for key, value in spec.items() if key in
              ("summary", "parameters", "requestBody", "security", "x-atlassian-oauth2-scopes", "x-github")}
              for method, spec in methods.items() if method in ("get", "post", "patch", "delete")}
    Path(f".spike/{provider}-openapi.json").write_bytes(raw)
    return provider, result


if __name__ == "__main__":
    with ThreadPoolExecutor(3) as pool:
        results = dict(pool.map(inspect, SOURCES.items()))
    Path(".spike/provider-contract-evidence.json").write_text(json.dumps(results, indent=2))
    for provider, result in results.items():
        print(provider, result["version"], result["sha256"])
        for path, methods in result["paths"].items():
            print(path, ", ".join(methods))
