# M2-001 / M2-002 revision 4.1 — GitHub Actions evidence at 28a8595

This is the second amd64 run of candidate 28a8595e627bcd092acaff4b8161522cf5d38f7f (push event). It was retrieved by the author through the GitHub MCP connector and is not an independent review.

| Workflow run | Jobs | Result |
|---|---|---|
| [m2_checks 38312981989](https://github.com/Kayyo321/cham3/actions/runs/38312981989) | `qualify_amd64` (115022869104): GitHub-hosted amd64 runner, pinned Debian 13.7 container, `CHAM3_M2_REQUIRE_CLEAN=1` | success; `m2 qualification: PASS`; summary identical to the VM run |
| [m1_checks 38312982028](https://github.com/Kayyo321/cham3/actions/runs/38312982028) | static_and_desktop, privileged_capture, package_build | success |
| [m0_checks 38312982051](https://github.com/Kayyo321/cham3/actions/runs/38312982051) | image_build, package_build, static_checks | success |

- m2 artifact metadata: `m2_qualification_amd64`, id 11775706466, 86 368 bytes, `sha256:6b95c08b22272d120c2334d83ac4751cb583d8696a38c90fdf7533fd8615f345`, head 28a8595.
- **NOT_RUN:** download of the artifact ZIP bytes. `gh` is not authenticated in the author's cloud session. The owner can run `gh run download 38312981989 -n m2_qualification_amd64` on their desktop.
- The evidence commit before this candidate (70c431c, docs only) also passed m2/m1/m0: runs 38300276222, 38300276203 and 38300276242.
