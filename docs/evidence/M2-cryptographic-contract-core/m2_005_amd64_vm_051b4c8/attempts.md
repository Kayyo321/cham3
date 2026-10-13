# Attempts

| Commit | Run | Outcome |
|---|---|---|
| 4f7080b091adef38d3c8b1a36fb975db53ba52b7 (the implementation) | Clean-clone qualification in this VM, started 15:23 UTC | Every gate before the suites PASS (source identity, pins and locks, crate sources, dependency inventory, rustfmt, clippy, naming, profiles, debug and release builds, backends, valgrind); the run was **stopped by the author before its suites completed** when 051b4c8 was pushed, so that the evidence is taken at the final candidate. Not a failure. GitHub Actions at 4f7080b: m2 38421605190, m1 38421605215, m0 38421605053, all success (including the protection and full-filesystem steps). |
| 051b4c8aacdeecfabb23625f93dd16615aec39a0 (the candidate: adds three R03 seeded defects to the harness) | This directory | PASS |

The pinned image `cham3-m2-dev` used for both runs was built once in this VM from the Dockerfile of the working tree, which is byte-identical to the one at 24a35c1, 4f7080b and 051b4c8 (`git diff --exit-code 24a35c1 051b4c8 -- build` exits 0; the first build attempt met a Docker Hub rate limit, HTTP 429, and succeeded on the second retry).
