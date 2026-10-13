"""M2-004: the vault refuses to run without its process protection (ADR 0005 §5).

The release-built cham3_vault_protection_probe applies vaultd's start-up protection.
With a memlock limit below the locked KDF budget plus headroom (here 8 MiB, the
qualification container's default) it must report protection_unavailable and exit 3
before deriving anything; the hardening applied first (no core dumps, non-dumpable)
stays. The positive path needs a larger limit and runs separately
(tests/m2/protection/check_vault_protection.sh).
"""

import os
import resource
import subprocess
import unittest

Probe = os.environ.get("CHAM3_M2_VAULT_PROTECTION_PROBE", "")
LowLimit = 8 << 20


def lower_memlock():
    resource.setrlimit(resource.RLIMIT_MEMLOCK, (LowLimit, LowLimit))


class vault_protection_t(unittest.TestCase):
    def test_insufficient_memlock_refuses_service(self):
        self.assertTrue(os.path.isfile(Probe), "probe not built: %r" % Probe)
        completed = subprocess.run([Probe], capture_output=True, text=True, check=False,
                                   preexec_fn=lower_memlock, timeout=600)
        self.assertEqual(completed.returncode, 3, completed.stdout + completed.stderr)
        self.assertEqual(completed.stdout.strip(), "protection_unavailable memlock_too_low")
        self.assertNotIn("derivation_ms", completed.stdout)


if __name__ == "__main__":
    unittest.main()
