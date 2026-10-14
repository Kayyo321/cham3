# The reviewer's controls, reproduced unchanged (stage B1 revision 3)

**Sources:** the original B1 snippets (`policy_reviewer_snippet.rs` `c33ade32…b25b`, `registry_reviewer_snippet.rs` `014db1de…a47`) and the re-review's three supplemental snippets (`r2_policy_controls.rs` `8fe26726…5e1d`, `r2_registry_controls.rs` `de0eb57e…c043`, `r2_startup_control.rs` `bbb3b9ad…1287`).
- Extracted byte-identically from the records at 792f5f0 and 9b5afa2; the digests equal the records'.

**Scripts:**
- `run_reviewer_b1.sh` composes `reviewer_b1.rs` as the first record's C6 does: the fixture, a newline, then the snippet.
- `run_rereview_r2.sh` composes `rereview_r2.rs` as the second record's C6 does: the fixture, a newline, the original snippet, then the supplemental snippet(s).
- Both run debug and release in `cham3-m2-dev` as UID 1001, in fresh clean clones.

| Log | Commit | policyd | registryd |
|---|---|---|---|
| `r2_at_77a3d0e.log` | 77a3d0e (reviewed) | 1/1 | 3 passed, **1 failed** (`rereview_startup_sample_closes_the_resumed_episode`: the two in-range cases, `startup_mark=2`) |
| `r2_at_5ea2407.log` | 5ea2407 (revision 3) | **1/1** | **4/4** |
| `b1_at_5ea2407.log` | 5ea2407 | **5/5** | **9/9** |

Both profiles agree.
