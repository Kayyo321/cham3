"""M2-004 harness sensitivity: identityd and vaultd tests must detect defects.

Seeded defects in a disposable copy of the workspace (never the checkout), one at a
time. Each removes or weakens one rule of ADR 0005 / ADR 0006 section 10 or the
M2-004 key-validation obligation, and the named Rust test must then fail (not merely
fail to build); the unmodified copy passes the same tests.
"""

import os
import shutil
import tempfile
import unittest

from m2_harness_support import RepoRoot, run

# (name, file, [(original, replacement)], package, test target, test filter)
SeededDefects = [
    ("minimum_key_epoch_not_enforced", "core/identityd/src/index.rs",
     [("    if resolution.key_epoch < resolution.min_key_epoch {\n"
       "        return Err(authority(authority_code_t::key_epoch_stale));\n"
       "    }\n", "")],
     "cham3_identityd", "journal_scenarios", "key_authority_scenarios_match_the_oracle"),
    # Layer 6 checks retirement both in the key's scope and explicitly; the defect
    # removes both (removing one alone is not observable).
    ("retirement_ignored", "core/identityd/src/index.rs",
     [("    if resolution.retired {\n"
       "        return Err(authority(authority_code_t::key_retired));\n"
       "    }\n", ""),
      ("            if key.retired {\n"
       "                return key_scope_t::denied(authority_code_t::key_retired);\n"
       "            }\n", "")],
     "cham3_identityd", "journal_scenarios", "key_authority_scenarios_match_the_oracle"),
    ("certified_point_not_validated", "core/identityd/src/index.rs",
     [("        if !public_key_is_valid(&certificate.subject_public_key) {\n",
       "        if false && !public_key_is_valid(&certificate.subject_public_key) {\n")],
     "cham3_identityd", "service_operations", "certified_keys_must_be_canonical_points_of_large_order"),
    ("temporary_window_unbounded", "core/identityd/src/index.rs",
     [("pub const MaxTemporaryCertificateSeconds: i64 = 7 * 24 * 60 * 60;",
       "pub const MaxTemporaryCertificateSeconds: i64 = 70 * 24 * 60 * 60;")],
     "cham3_identityd", "service_operations", "temporary_certificates_need_a_window_of_at_most_seven_days"),
    # R01 (M2-004 review): a weaker durable invalidation counts as covering a pending
    # compromise, which is then lost.
    ("pending_compromise_cleared_by_weaker_record", "core/identityd/src/service.rs",
     [("        (Some(_), invalidation_reason_t::compromised) => false,\n",
       "        (Some(_), invalidation_reason_t::compromised) => true,\n")],
     "cham3_identityd", "service_operations",
     "a_pending_compromise_is_kept_until_a_durable_compromise_covers_it"),
    ("invalidation_not_denied_before_durable", "core/identityd/src/service.rs",
     [("                self.deny_in_memory(&payload.target_key_id, payload.reason);\n", ""),
      ("            identity_effect_t::invalidate => {\n"
       "                self.deny_in_memory(&target, payload.reason);\n",
       "            identity_effect_t::invalidate => {\n")],
     "cham3_identityd", "service_operations", "invalidation_denies_in_memory_before_it_is_durable"),
    # M2-004 revision-2 review, M2-004-R2-01: pending ids reserve their retained slot.
    ("pending_proposals_not_counted_as_retained", "core/vaultd/src/service.rs",
     [("                .filter(|proposal| !pending_only || proposal.state == proposal_state_t::pending)\n",
       "                .filter(|proposal| (proposal.state == proposal_state_t::pending) == pending_only)\n")],
     "cham3_vaultd", "vault_operations", "pending_proposals_count_against_the_retained_id_bounds"),
    ("approval_digest_not_compared", "core/vaultd/src/service.rs",
     [("            if !bool::from(proposal.approval_digest.ct_eq(approval_digest)) {\n",
       "            if false && !bool::from(proposal.approval_digest.ct_eq(approval_digest)) {\n")],
     "cham3_vaultd", "vault_operations", "approval_must_match_the_registered_proposal"),
    ("proposal_reusable", "core/vaultd/src/service.rs",
     [("            proposal.retire(proposal_state_t::used);\n", "            let _ = proposal;\n")],
     "cham3_vaultd", "vault_operations", "an_exact_proposal_is_signed_once_after_digest_approval"),
    # R04: used and cancelled ids are dropped by the next registration's cleanup.
    ("used_proposal_ids_forgotten", "core/vaultd/src/service.rs",
     [("            .filter(|(_, proposal)| self.expired(proposal))\n",
       "            .filter(|(_, proposal)| {\n"
       "                self.expired(proposal) || proposal.state != proposal_state_t::pending\n"
       "            })\n")],
     "cham3_vaultd", "vault_operations", "proposal_ids_stay_single_use_for_their_lifetime"),
    # R02: only the payload timestamp is checked against the window.
    ("signing_window_not_checked", "core/vaultd/src/service.rs",
     [("        check_signing_window(&resolution, self.clock.utc_seconds())?;\n", "")],
     "cham3_vaultd", "vault_operations", "a_live_proposal_does_not_extend_the_signing_keys_window"),
    # R03: authority is checked only before the derivation.
    ("authority_not_rechecked_after_derivation", "core/vaultd/src/service.rs",
     [("        // which clears it).\n        self.check_signing_key(uid, &parsed)?;\n",
       "        // which clears it).\n")],
     "cham3_vaultd", "vault_operations",
     "authority_changes_during_the_derivation_prevent_the_signature"),
    ("ceremony_not_rechecked_after_derivation", "core/vaultd/src/service.rs",
     [("        self.open_ceremony(uid, recovery_id)?;\n"
       "        self.identity_key(uid, in_ceremony)?;\n"
       "        let mut objects", "        let mut objects")],
     "cham3_vaultd", "vault_recovery", "a_ceremony_that_ends_during_the_derivation_gets_no_certificates"),
    ("attempt_limit_ignored", "core/vaultd/src/service.rs",
     [("        if self.attempts(uid)?.locked_at(self.utc_now_unsigned()) {\n",
       "        if false && self.attempts(uid)?.locked_at(self.utc_now_unsigned()) {\n")],
     "cham3_vaultd", "vault_operations", "failed_passphrases_are_limited_and_the_limit_survives_restart"),
    ("metadata_not_authenticated", "core/vaultd/src/key_object.rs",
     [("        let aad = metadata.associated_data();\n", "        let aad: Vec<u8> = Vec::new();\n"),
      ("        let aad = self.metadata.associated_data();\n", "        let aad: Vec<u8> = Vec::new();\n")],
     "cham3_vaultd", "vault_operations", "tampered_or_substituted_key_objects_never_sign"),
    ("seed_not_checked_against_key_id", "core/vaultd/src/key_object.rs",
     [("            != self.metadata.key_id\n", "            == [0xa5; 32]\n")],
     "cham3_vaultd", "vault_operations", "tampered_or_substituted_key_objects_never_sign"),
    ("kdf_bounds_unchecked", "core/vaultd/src/kdf.rs",
     [("    params.check(locked_budget_bytes)?;\n", "")],
     "cham3_vaultd", "vault_operations", "tampered_or_substituted_key_objects_never_sign"),
    ("kdf_workspace_not_cleared", "core/vaultd/src/kdf.rs",
     [("        self.clear();\n        if result.is_err() {\n", "        if result.is_err() {\n")],
     "cham3_vaultd", "--lib", "kdf::tests::workspace_is_clear_after_success"),
    # The approval checks key status and scope before and after the derivation; the
    # defect removes both (removing one alone is not observable here).
    ("scope_not_checked_at_approval", "core/vaultd/src/service.rs",
     [("        let parsed = parse_commit(&payload, &key_id)?;\n"
       "        self.check_signing_key(uid, &parsed)?;\n",
       "        let parsed = parse_commit(&payload, &key_id)?;\n"),
      ("        // which clears it).\n        self.check_signing_key(uid, &parsed)?;\n",
       "        // which clears it).\n")],
     "cham3_vaultd", "vault_operations", "invalidated_keys_never_sign_even_from_a_restored_object"),
    ("unknown_status_erases_held_keys", "core/vaultd/src/service.rs",
     [("        let status = self.registry.recovery_status(uid, &recovery_id)?;\n",
       "        let status = match self.registry.recovery_status(uid, &recovery_id) {\n"
       "            Ok(status) => status,\n"
       "            Err(error) => {\n"
       "                self.finish_bundle(uid, &bundle.new_keys)?;\n"
       "                return Err(error);\n"
       "            }\n"
       "        };\n")],
     "cham3_vaultd", "vault_recovery", "an_unavailable_status_keeps_every_object_and_a_retry_resumes"),
    # R05: a definitive naming error erases the held keys without asking the status.
    ("definitive_naming_error_erases_held_keys", "core/vaultd/src/service.rs",
     [("                let reconciled = self\n",
       "                if !transient(&error) {\n"
       "                    self.finish_bundle(uid, &bundle.new_keys)?;\n"
       "                    return Err(error);\n"
       "                }\n"
       "                let reconciled = self\n")],
     "cham3_vaultd", "vault_recovery", "a_definitive_naming_error_is_reconciled_from_the_status"),
]


class seeded_identity_vault_defects_t(unittest.TestCase):
    """Runs the named Rust tests in a disposable workspace copy with one defect each."""

    @classmethod
    def setUpClass(cls):
        cls.workspace = tempfile.mkdtemp(prefix="cham3_m2_identity_vault_defect_")
        for name in ("Cargo.toml", "Cargo.lock"):
            shutil.copy2(os.path.join(RepoRoot, name), os.path.join(cls.workspace, name))
        shutil.copytree(os.path.join(RepoRoot, ".cargo"), os.path.join(cls.workspace, ".cargo"))
        shutil.copytree(os.path.join(RepoRoot, "core"), os.path.join(cls.workspace, "core"))
        # The identity scenario test reads the published vectors relative to the crate.
        shutil.copytree(os.path.join(RepoRoot, "tests", "m2", "vectors"),
                        os.path.join(cls.workspace, "tests", "m2", "vectors"))
        cls.environment = dict(os.environ, CARGO_TARGET_DIR=os.path.join(cls.workspace, "target"))
        cls.environment.pop("RUSTFLAGS", None)

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.workspace, ignore_errors=True)

    def cargo_test(self, package, target, test_filter):
        selector = ["--lib"] if target == "--lib" else ["--test", target]
        return run(["cargo", "test", "--frozen", "-p", package] + selector + ["--", "--exact", test_filter],
                   cwd=self.workspace, env=self.environment)

    def test_unmodified_copy_passes(self):
        for name, _, _, package, target, test_filter in SeededDefects:
            with self.subTest(control=name):
                status, output = self.cargo_test(package, target, test_filter)
                self.assertEqual(status, 0, output[-2000:])
                self.assertIn("test result: ok. 1 passed", output)

    def test_each_seeded_defect_is_detected(self):
        for name, relative, edits, package, target, test_filter in SeededDefects:
            with self.subTest(defect=name):
                path = os.path.join(self.workspace, relative)
                with open(path, encoding="utf-8") as handle:
                    source = handle.read()
                changed = source
                for original, replacement in edits:
                    self.assertEqual(changed.count(original), 1, "defect anchor for %s" % name)
                    changed = changed.replace(original, replacement)
                try:
                    with open(path, "w", encoding="utf-8") as handle:
                        handle.write(changed)
                    status, output = self.cargo_test(package, target, test_filter)
                finally:
                    with open(path, "w", encoding="utf-8") as handle:
                        handle.write(source)
                self.assertNotEqual(status, 0, "%s was not detected" % name)
                # The defect must compile and be caught by the test, not by the build.
                self.assertIn("test %s ... FAILED" % test_filter, output, output[-2000:])


if __name__ == "__main__":
    unittest.main()
