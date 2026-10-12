# M2-001 / M2-002 revision 4: GitHub Actions evidence at 9b1aa2b

This is the second amd64 run of the revision-4 candidate 9b1aa2b9d1435fb33c303ca7fe01ce7afcb2964a (push event on `codex/m2-cryptographic-contract-core`). The author retrieved it; it is not an independent review.

| Workflow run | Jobs | Result |
|---|---|---|
| [m2_checks 38299143273](https://github.com/Kayyo321/cham3/actions/runs/38299143273) | `qualify_amd64` (114977492780): GitHub-hosted amd64 runner, kernel 6.17.0-1022-azure, the pinned Debian 13.7 container built on the runner from the unchanged Dockerfile, UID 1001, `CHAM3_M2_REQUIRE_CLEAN=1` | success; `m2 qualification: PASS` |
| [m1_checks 38299143245](https://github.com/Kayyo321/cham3/actions/runs/38299143245) | `static_and_desktop` (114977492842), `privileged_capture` (114977493295), `package_build` (114977493453) | success (all jobs and steps) |
| [m0_checks 38299143276](https://github.com/Kayyo321/cham3/actions/runs/38299143276) | `image_build` (114977492654), `package_build` (114977493305), `static_checks` (114977493327) | success (all jobs and steps) |

- `ci_runs.json`: run, job and step conclusions as returned by the GitHub MCP connector (`actions_get get_workflow_run`, `actions_list list_workflow_jobs`), all `success`. It also holds the m2 artifact metadata: `m2_qualification_amd64`, id 11770401811, 86 294 bytes, `sha256:b2b3501d65e8c9a004419dbfaaecdd91a85c6f1cac2265b3a689bcf61783977f`, head 9b1aa2b.
- `qualify_amd64_log_excerpt.txt`: the identity block and the qualification summary, copied from the decoded job log obtained through the connector (`get_job_logs`). The summary is identical to the VM run's `summary.txt` (checked with `diff`).
- **NOT_RUN:** download and inspection of the artifact ZIP bytes. This session's `gh` token is not authenticated, and in revision 3 the blob-storage redirect was refused by the egress policy. Only the artifact metadata is recorded.
