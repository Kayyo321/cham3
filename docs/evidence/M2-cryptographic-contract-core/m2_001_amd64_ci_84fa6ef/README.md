# M2-001 / M2-002 revision 3 — GitHub Actions evidence at 84fa6ef

Second amd64 run of the revision-3 candidate 84fa6ef172b3fad21bb079549776252bb65624d6 (push event on `codex/m2-cryptographic-contract-core`). Author-retrieved; not independent review.

| Workflow run | Jobs | Result |
|---|---|---|
| [m2_checks 38292641158](https://github.com/Kayyo321/cham3/actions/runs/38292641158) | `qualify_amd64` (114956138804): GitHub-hosted amd64 runner, kernel 6.17.0-1022-azure, pinned Debian 13.7 container built on the runner from the unchanged Dockerfile, UID 1001, `CHAM3_M2_REQUIRE_CLEAN=1` | success; `m2 qualification: PASS` |
| [m1_checks 38292641153](https://github.com/Kayyo321/cham3/actions/runs/38292641153) | `static_and_desktop`, `package_build`, `privileged_capture`, `image_build` | success (all jobs and steps) |
| [m0_checks 38292641431](https://github.com/Kayyo321/cham3/actions/runs/38292641431) | `package_build`, `static_checks` | success (all jobs and steps) |

- `ci_runs.json`: run, job and step conclusions from `gh api repos/Kayyo321/cham3/actions/runs/<id>` and `/jobs` (95 conclusions, all `success`), and the m2 artifact metadata (`m2_qualification_amd64`, id 11766997724, 85 633 bytes, `sha256:20d8c277f175c1309c57275917c06f8b26e796a5002083fe494560a4a6554374`, head 84fa6ef).
- `qualify_amd64_log_excerpt.txt`: identity and qualification summary copied from the decoded job log obtained through the GitHub MCP connector (`get_job_logs`). Its 25-line summary is identical to the native VM run's `summary.txt` (checked with `diff`).
- **BLOCKED:** `gh run download 38292641158 -n m2_qualification_amd64` and the raw job-log download (`gh api …/jobs/<id>/logs`) both redirect to `productionresultssa14.blob.core.windows.net`, which this session's egress policy refuses (HTTP 403 on CONNECT, recorded by the agent proxy). No artifact bytes were inspected; the lock checks are evidenced by the decoded log's PASS lines and by the VM run's empty diffs.
