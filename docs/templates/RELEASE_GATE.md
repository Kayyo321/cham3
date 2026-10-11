# cham3 Release Gate: <version>

- **Source revision:**
- **Build environment and provenance:**
- **Release owner:**
- **Target hardware/support statement:**

Release claims and required behavior must match `PROJECT.md` and approved ADRs.

Each item is PASS, FAIL, NOT_RUN, or BLOCKED. Attach evidence; no blank item implies pass.

## Build and artifact integrity
- [ ] Clean-source build and package/image results:
- [ ] Reproducibility status and limits:
- [ ] APT metadata/package signatures verified:
- [ ] Release keys separated from development and user contract keys:

## Contract and authorization
- [ ] Format, signature, authority, tamper, expiry, revocation, revision and cross-user tests:
- [ ] Vault authorization and app-originated signing denial:

## Isolation and networking
- [ ] App/process/filesystem/IPC boundaries:
- [ ] Direct, Tor no-fallback, Offline, DNS, IPv4/IPv6 and failure/restart evidence:
- [ ] Contract transition and revocation behavior:

## Security and operations
- [ ] Independent security review and finding dispositions:
- [ ] Privileged services and update/recovery review:
- [ ] Install, boot, upgrade, rollback/recovery evidence:
- [ ] Supported hardware and known limitations documented:

## Documentation and approval
- [ ] User/admin documentation and threat model:
- [ ] All required gates evidenced; NOT_RUN/BLOCKED items accepted or resolved by authorized owner:
- [ ] Risk acceptance / release decision by project owner:
