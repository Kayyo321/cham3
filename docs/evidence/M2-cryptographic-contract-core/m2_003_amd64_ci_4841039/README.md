# M2-003 — GitHub Actions evidence at 4841039

This is the second amd64 run of candidate 4841039d772ca923e09a55f1505f119317e73a91 (push event). The author retrieved it through the GitHub MCP connector; it is not an independent review.

| Workflow run | Jobs | Result |
|---|---|---|
| [m2_checks 38319998393](https://github.com/Kayyo321/cham3/actions/runs/38319998393) | `qualify_amd64` (115045530322): GitHub-hosted amd64 runner, pinned Debian 13.7 container, `CHAM3_M2_REQUIRE_CLEAN=1` | success; `m2 qualification: PASS`, with `suite_unit` (including `format_differential`) and `suite_harness` PASS; summary lines the same as the VM run |
| [m1_checks 38319998397](https://github.com/Kayyo321/cham3/actions/runs/38319998397) | static_and_desktop (115045530658), privileged_capture (115045530808), package_build (115045530429) | success, every step |
| [m0_checks 38319998384](https://github.com/Kayyo321/cham3/actions/runs/38319998384) | image_build (115045530288), package_build (115045530522), static_checks (115045530578) | success, every step |

- m2 artifact metadata: `m2_qualification_amd64`, id 11778127974, 89 588 bytes, `sha256:17bd802388903bd47c11aee48e71a68a828279705df813a09a9056e50f96acf9`, head 4841039 (the job log reports the same digest).
- **NOT_RUN:** download of the artifact ZIP bytes; `gh` is not authenticated in the author's cloud session. The owner can download it with `gh run download 38319998393 -n m2_qualification_amd64`.
- The base ecc8213 (the docs-only commits that recorded the owner decisions) also passed m2/m1/m0: runs 38316885003, 38316884917 and 38316884957, all success.
