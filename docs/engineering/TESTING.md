# Testing Strategy

Testing verifies the observable requirements in `PROJECT.md`; a configuration file or successful build alone is not evidence of enforcement. This strategy does not assert that the repository currently has a test harness.

## Result labels

- **PASS:** stated requirement demonstrated in the stated environment with artifacts.
- **FAIL:** observed behavior violates acceptance criteria.
- **NOT_RUN:** not executed; give reason.
- **BLOCKED:** execution prevented by a dependency or unsafe/unavailable environment; identify it.

Always record commit, environment, command, result, artifacts, and limits. Distinguish unit, mocked, namespace/container, VM, and physical-hardware evidence.

## Layers

1. **Static checks:** formatting, lint, type checks, dependency/license checks, secret scanning and configuration validation as each is introduced.
2. **Unit tests:** deterministic CBOR/COSE handling, signature and key-authority rules, contract parsing, policy decisions, expiration, revocation, revision and delegation.
3. **Component integration:** vault authorization, registry consistency, authenticated service IPC, launcher attribution, brokers, namespaces, AppArmor/seccomp and lifecycle behavior.
4. **Negative security tests:** malformed/tampered contracts; unknown keys; wrong owner/app/resource; expired/revoked/stale grants; unauthorized IPC/signing; helper process attempts; policy conflicts.
5. **Network tests:** Direct policy; Tor-only egress with packet observation; gateway unavailable/restart; no direct fallback; DNS, IPv4, IPv6, UDP and unsupported protocols; Offline external and proxy/host-service attempts; firewall reload and mode-transition socket teardown. Include a positive network control.
6. **Desktop tests:** KDE/Wayland Super+Click attribution, spoofed titles/windows, inspector read-only behavior, authenticated approval, cancel behavior, and display of confirmed backend state.
7. **Distribution tests:** Debian package build/install/upgrade, image build, boot in a VM, application launch, update signature and recovery. A VM result is not a hardware compatibility result.
8. **Compatibility tests:** representative Debian GTK/Qt/KDE packages, Flatpak, standalone executables, helper processes, browsers, communications apps and portals; document unsupported behavior instead of relaxing mandatory restrictions.
9. **Fuzzing/reliability:** parsers, IPC boundaries, protocol handling, crash/restart, interrupted transitions, power/session changes. Fuzz only where the interface and harness exist.
10. **Release verification:** clean source revision, reproducible-build evidence where supported, artifact signatures, security findings, supported hardware, upgrade/recovery, and release gate.

## Test quality

Pair security negative controls with positive controls proving the target path works absent the restriction. Prefer external observation for routing claims. Test failure and recovery as well as steady state. Validate test sensitivity by introducing a controlled defect in a disposable copy and confirming the test fails. Never call a simulated result full-system proof. Review false positives, environment dependencies, and blind spots independently.

Security-sensitive changes require independent review of the test design and results. Required tests that cannot run remain NOT_RUN or BLOCKED and prevent gates that require them from passing.
