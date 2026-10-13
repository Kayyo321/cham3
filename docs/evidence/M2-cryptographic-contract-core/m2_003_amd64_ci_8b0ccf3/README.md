# M2-003 revision 2 — GitHub Actions evidence at 8b0ccf3

This is the second amd64 run of candidate 8b0ccf37f2437cae0a9668e012ddf55e9952e2aa (push event). The author retrieved it through the GitHub MCP connector; it is not an independent review.

| Workflow run | Jobs | Result |
|---|---|---|
| [m2_checks 38327808145](https://github.com/Kayyo321/cham3/actions/runs/38327808145) | `qualify_amd64` (115070439635): GitHub-hosted amd64 runner, pinned Debian 13.7 container, `CHAM3_M2_REQUIRE_CLEAN=1` | success, every step |
| [m1_checks 38327808083](https://github.com/Kayyo321/cham3/actions/runs/38327808083) | static_and_desktop (115070439174), privileged_capture (115070439420), package_build (115070439495) | success, every step |
| [m0_checks 38327808082](https://github.com/Kayyo321/cham3/actions/runs/38327808082) | static_checks (115070439223), image_build (115070439367), package_build (115070439518) | success, every step |

- m2 artifact metadata: `m2_qualification_amd64`, id 11782306047, 90 097 bytes, `sha256:0e77977e49149b51d95b2eec23e5a38f2adc51122437ff6f931fa42bf66d0eb8`, head 8b0ccf3.
- **NOT_RUN:** download of the artifact ZIP bytes (author `gh` unauthenticated). Download with `gh run download 38327808145 -n m2_qualification_amd64`.
