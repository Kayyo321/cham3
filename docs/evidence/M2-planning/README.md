# M2 planning evidence

Selected **upstream-produced** evidence inspected on 2026-10-12, retained from [Stasha Linux qualification run 38219130719](https://github.com/Kayyo321/stasha/actions/runs/38219130719), exact source `5ada5de229df1255cd9b64ab0e28e94ae3d754b1`. Artifact: `linux-compiler-qualification-5ada5de229df1255cd9b64ab0e28e94ae3d754b1`, ID `11734852747` (downloaded before expiry).

- `upstream_run.json`, `upstream_jobs.json`: GitHub API snapshots; completed success at the exact source.
- `RESULT.txt`, `STATUS.txt`, `compiler-version.txt`, `compiler.sha256`, `toolchain.json`: selected unmodified artifact receipts. Runner paths are upstream provenance, not paths on this host.
- `upstream_abi_report.json`: selected unmodified `abi/report.json` from that artifact.
- `release_snapshot.json`: latest-release API snapshot at research time; macOS-only published assets, not a Debian compiler.

Acquisition command and limits: [research record](../../research/M2_stasha_readiness.md). The raw artifact was downloaded outside the repository and not executed. Full compiler bytes and all raw suite traces are not retained here; the recorded compiler digest is upstream evidence, not an independent local hash of compiler bytes. These files establish neither Debian compatibility nor M2 security. No production credentials or private keys were used.
