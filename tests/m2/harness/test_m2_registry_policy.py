"""M2-005 harness sensitivity: registryd, policyd and contractd tests must detect defects.

Seeded defects in a disposable copy of the workspace (never the checkout), one at a
time. Each removes or weakens one admission rule (wire format sections 6.5-6.12, 7),
one journal, durability or reserve rule (ADR 0006 section 1), one current-authorization
rule (ADR 0007 section 1) or one submission check, and the named Rust test must then fail
(not merely fail to build); the unmodified copy passes the same tests.
"""

import os
import shutil
import tempfile
import unittest

from m2_harness_support import RepoRoot, run

Lineage = "core/registryd/src/lineage.rs"
Registry = "core/registryd/src/registry.rs"
Evaluate = "core/policyd/src/evaluate.rs"

# (name, [(file, original, replacement)], package, test target, test filter)
SeededDefects = [
    # Layer 7 (wire format 6.11): a parent that is no longer a live temporary head.
    ("ended_branch_extended", [(Lineage,
      "        if lineage.branches.contains_key(parent) && extends {\n",
      "        if extends {\n")],
     "cham3_registryd", "admission_scenarios", "admission_invalid_vectors_match_the_oracle"),
    ("base_state_not_checked", [(Lineage,
      "        if base.state_hash() != payload.base_state_hash {\n",
      "        if false && base.state_hash() != payload.base_state_hash {\n")],
     "cham3_registryd", "admission_scenarios", "admission_invalid_vectors_match_the_oracle"),
    ("result_state_not_checked", [(Lineage,
      "        if encode(&result.to_value()) != encode(&payload.result_state.to_value()) {\n",
      "        if false && encode(&result.to_value()) != encode(&payload.result_state.to_value()) {\n")],
     "cham3_registryd", "admission_scenarios", "admission_invalid_vectors_match_the_oracle"),
    ("grant_ids_reusable", [(Lineage,
      "            .any(|grant| self.grants.contains_key(&grant.grant_id))\n",
      "            .any(|grant| grant.grant_id == [0xa5; 16])\n")],
     "cham3_registryd", "admission_scenarios", "admission_invalid_vectors_match_the_oracle"),
    # M2-004 carried obligation: admission rejects duplicate request ids.
    ("request_ids_reusable", [(Lineage,
      "        if self.requests.contains(&payload.authorization.request_id) {\n",
      "        if false && self.requests.contains(&payload.authorization.request_id) {\n")],
     "cham3_registryd", "registry_operations", "duplicate_request_ids_are_rejected_across_the_users_lineages"),
    # Wire format 6.7 (R12): a restore matches temporary history.
    ("restore_matches_temporary_history", [(Lineage,
      "            .filter(|old| old.lifetime.kind == lifetime_kind_t::persistent)\n",
      "            .filter(|old| old.lifetime.kind == old.lifetime.kind)\n")],
     "cham3_registryd", "admission_scenarios", "admission_scenarios_match_the_oracle"),
    # Wire format 6.9 (R06): any subject may follow the genesis.
    ("subject_not_bound", [(Lineage,
      "        } else if payload.subject != *authoritative {\n",
      "        } else if false && payload.subject != *authoritative {\n")],
     "cham3_registryd", "admission_scenarios", "admission_invalid_vectors_match_the_oracle"),
    ("branch_revocation_unbounded", [(Lineage,
      "            if !allowed {\n                return reject(rule_code_t::branch_rule);\n",
      "            if false && !allowed {\n                return reject(rule_code_t::branch_rule);\n")],
     "cham3_registryd", "admission_scenarios", "admission_scenarios_match_the_oracle"),
    # Wire format 6.12 (R07-r2): the network slot counted per state only.
    ("network_slot_per_state_only", [(Lineage,
      "                self.occupying_network_grants(others.chain([&payload.result_state]))\n",
      "                self.occupying_network_grants(others.take(0).chain([&payload.result_state]))\n")],
     "cham3_registryd", "admission_scenarios", "admission_scenarios_match_the_oracle"),
    ("premature_expiry_accepted", [(Lineage,
      "                if time < expiration.as_str() {\n",
      "                if false && time < expiration.as_str() {\n")],
     "cham3_registryd", "admission_scenarios", "admission_scenarios_match_the_oracle"),
    ("delegated_grants_admitted", [(Lineage,
      "            .any(|grant| grant.delegation.parent_grant_id.is_some())\n",
      "            .any(|grant| grant.delegation.parent_grant_id == Some([0xa5; 16]))\n")],
     "cham3_registryd", "registry_operations", "delegated_grants_are_refused_until_their_admission_rules_exist"),
    # Service-level admission and the journal.
    # (beyond the model) the signing key's window at registry time.
    ("signing_window_not_checked_at_admission", [(Registry,
      "        check_signing_window(&resolution, self.clock.utc_seconds())?;\n", "")],
     "cham3_registryd", "registry_operations", "admission_requires_the_signing_keys_window_at_registry_time"),
    ("policy_epoch_not_checked", [(Registry,
      "        if payload.policy.epoch != self.config.policy_epoch {\n",
      "        if false && payload.policy.epoch != self.config.policy_epoch {\n")],
     "cham3_registryd", "registry_operations", "stale_epochs_lineage_limits_and_unknown_users_are_refused"),
    ("admission_uses_the_reserve", [(Registry,
      "        self.check_quota(&slot.journal, record.len(), false)?;\n",
      "        self.check_quota(&slot.journal, record.len(), true)?;\n")],
     "cham3_registryd", "registry_operations", "the_byte_quota_keeps_its_reserve_for_restrictive_records"),
    ("admit_open_to_every_role", [(Registry,
      "        require(caller, &[Contractd])?;\n",
      "        let _ = caller;\n")],
     "cham3_registryd", "registry_operations", "operations_are_limited_to_their_roles"),
    ("replay_ignores_recorded_placement", [(Registry,
      "                    || admission.placement != placement\n", "")],
     "cham3_registryd", "registry_operations", "a_correctly_chained_record_must_still_pass_every_check_on_replay"),
    ("failed_append_not_remembered", [("core/registryd/src/journal.rs",
      "            self.failed = true;\n", "")],
     "cham3_registryd", "registry_operations", "failed_appends_change_nothing_and_deny_until_restart"),
    # M2-005 review R01: directory entries durable in their parents, on every attempt.
    ("parent_directory_not_synced_on_retry", [("core/registryd/src/journal.rs",
      "        ensure_directory(&directory, self.owner)?;\n        io.sync_directory(&self.users_directory())\n            .map_err(io_error)?;\n",
      "        if ensure_directory(&directory, self.owner)? {\n            io.sync_directory(&self.users_directory())\n                .map_err(io_error)?;\n        }\n")],
     "cham3_registryd", "registry_operations", "a_failed_parent_directory_sync_is_repeated_before_a_retry_is_acknowledged"),
    ("new_users_directory_not_synced_in_root", [("core/registryd/src/journal.rs",
      "        io.sync_directory(root).map_err(io_error)?;\n", "")],
     "cham3_registryd", "registry_operations", "every_created_directory_entry_is_durable_before_the_enrollment_is_acknowledged"),
    # M2-005 review R2-01: a directory's existence is no proof of an earlier successful
    # sync; the syncs are repeated at every start and every reopen.
    ("store_open_trusts_existing_directories", [("core/registryd/src/journal.rs",
      "        ensure_directory(root, owner)?;\n        let parent = root.parent().ok_or(store_error_t::unsafe_path)?;\n        io.sync_directory(parent).map_err(io_error)?;\n        ensure_directory(&root.join(\"users\"), owner)?;\n        io.sync_directory(root).map_err(io_error)?;\n",
      "        if ensure_directory(root, owner)? {\n            let parent = root.parent().ok_or(store_error_t::unsafe_path)?;\n            io.sync_directory(parent).map_err(io_error)?;\n        }\n        if ensure_directory(&root.join(\"users\"), owner)? {\n            io.sync_directory(root).map_err(io_error)?;\n        }\n")],
     "cham3_registryd", "registry_operations", "a_failed_startup_directory_sync_is_repeated_at_the_next_start"),
    ("reopened_user_directory_not_synced", [("core/registryd/src/journal.rs",
      "        io.sync_directory(&directory).map_err(io_error)?;\n        Ok(journal_file_t {\n            file,\n            length,\n",
      "        Ok(journal_file_t {\n            file,\n            length,\n")],
     "cham3_registryd", "registry_operations", "a_failed_creation_sync_is_repeated_when_the_journal_is_reopened"),
    # M2-005 review R03 (ADR 0006 section 1): the preallocated restrictive reserve.
    ("reserve_not_preallocated", [("core/registryd/src/journal.rs",
      "        io.allocate(&mut reserve, allocated, ReserveBytes)\n            .map_err(io_error)?;\n", "")],
     "cham3_registryd", "registry_operations", "the_preallocated_reserve_is_used_only_by_restrictive_records"),
    # M2-005 review R2-02: the reserve's length never stands in for its sync.
    ("existing_reserve_length_trusted_at_creation", [("core/registryd/src/journal.rs",
      "        io.allocate(&mut reserve, allocated, ReserveBytes)\n            .map_err(io_error)?;\n",
      "        if allocated < ReserveBytes {\n            io.allocate(&mut reserve, allocated, ReserveBytes)\n                .map_err(io_error)?;\n        }\n")],
     "cham3_registryd", "registry_operations", "a_reserve_written_without_a_completed_sync_is_never_trusted_for_its_length"),
    ("replenishment_trusts_the_file_length", [("core/registryd/src/journal.rs",
      "        io.allocate(&mut self.reserve, length, ReserveBytes)\n            .map_err(|_| store_error_t::reserve_depleted)?;\n",
      "        let _ = io.allocate(&mut self.reserve, length, ReserveBytes);\n")],
     "cham3_registryd", "registry_operations", "a_reserve_written_without_a_completed_sync_is_never_trusted_for_its_length"),
    ("reopened_reserve_trusted_by_length", [("core/registryd/src/journal.rs",
      "    if length == ReserveBytes && io.allocate(reserve, length, ReserveBytes).is_ok() {\n",
      "    if length == ReserveBytes {\n")],
     "cham3_registryd", "registry_operations", "a_reserve_written_without_a_completed_sync_is_never_trusted_for_its_length"),
    ("admission_proceeds_into_a_depleted_reserve", [("core/registryd/src/journal.rs",
      "        if !restrictive {\n            self.replenish_reserve(io)?;\n        }\n", "")],
     "cham3_registryd", "registry_operations", "the_preallocated_reserve_is_used_only_by_restrictive_records"),
    ("restrictive_append_never_releases_the_reserve", [("core/registryd/src/journal.rs",
      "            if restrictive && out_of_space(error) {\n",
      "            if false && restrictive && out_of_space(error) {\n")],
     "cham3_registryd", "registry_operations", "a_torn_restrictive_write_on_a_full_filesystem_is_truncated_back_and_retried"),
    # M2-005 review R02 (ADR 0007 section 6): an ended session never becomes live again.
    ("ended_sessions_reusable_at_admission", [(Lineage,
      "                .is_some_and(|session_id| self.ended_sessions.contains(session_id))\n",
      "                .is_some_and(|session_id| *session_id == [0xa5; 16])\n")],
     "cham3_registryd", "admission_scenarios", "admission_scenarios_match_the_oracle"),
    ("ended_sessions_forgotten", [(Lineage,
      "                self.ended_sessions.insert(*session_id);\n", "")],
     "cham3_registryd", "registry_operations", "an_ended_session_refuses_every_later_grant_bound_to_it"),
    ("chain_not_verified", [("core/registryd/src/record.rs",
      "        if stored != expected {\n", "        if false && stored != expected {\n")],
     "cham3_registryd", "--lib", "record::tests::frames_chain_and_detect_defects"),
    # Layer 8 (ADR 0007 section 1). The service also checks mandatory policy before
    # reading any snapshot; the defect removes both checks.
    ("mandatory_policy_ignored", [
        (Evaluate,
         "    if snapshot.policy.forbids(\n        identity.owner_uid,\n        request.category,\n",
         "    if false && snapshot.policy.forbids(\n        identity.owner_uid,\n        request.category,\n"),
        ("core/policyd/src/service.rs",
         "        if self.policy.forbids(\n            identity.owner_uid,\n            request.category,\n",
         "        if false && self.policy.forbids(\n            identity.owner_uid,\n            request.category,\n")],
     "cham3_policyd", "current_authorization", "mandatory_policy_and_emergency_restriction_win_over_grants"),
    ("expiration_ignored", [(Evaluate,
      "        if snapshot.now >= end {\n", "        if false && snapshot.now >= end {\n")],
     "cham3_policyd", "current_authorization", "eligible_grants_pass_and_every_rule_denies_despite_valid_signatures"),
    ("ended_session_ignored_by_evaluation", [(Evaluate,
      "        if snapshot.view.ended_sessions.contains(session_id) {\n",
      "        if false && snapshot.view.ended_sessions.contains(session_id) {\n")],
     "cham3_policyd", "current_authorization", "a_durably_ended_session_denies_every_grant_bound_to_it_whatever_the_attestation"),
    ("session_attestation_age_unbounded", [(Evaluate,
      "                    .is_some_and(|age| age <= MaxSessionAttestationSeconds)\n",
      "                    .is_some()\n")],
     "cham3_policyd", "current_authorization", "session_grants_need_this_live_session_and_end_with_their_branch"),
    ("compromised_key_ignored", [(Evaluate,
      "    if origin.key_invalidated == compromised || current.invalidated == compromised {\n",
      "    if false && (origin.key_invalidated == compromised || current.invalidated == compromised) {\n")],
     "cham3_policyd", "current_authorization", "a_compromised_signing_key_ends_its_grants_and_a_rotated_one_does_not"),
    ("pending_compromise_ignored", [(Evaluate,
      "    if origin.key_invalidated == compromised || current.invalidated == compromised {\n",
      "    if origin.key_invalidated == compromised {\n")],
     "cham3_policyd", "current_authorization", "conservative_evaluation_of_inconsistent_snapshots"),
    ("policy_epoch_floor_ignored", [(Evaluate,
      "    if origin.policy_epoch < snapshot.view.min_valid_policy_epoch {\n",
      "    if false && origin.policy_epoch < snapshot.view.min_valid_policy_epoch {\n")],
     "cham3_policyd", "current_authorization", "a_raised_minimum_policy_epoch_ends_older_grants"),
    ("runtime_identity_not_matched", [(Evaluate,
      "        subject.owner_uid == identity.owner_uid\n",
      "        true || subject.owner_uid == identity.owner_uid\n")],
     "cham3_policyd", "current_authorization", "eligible_grants_pass_and_every_rule_denies_despite_valid_signatures"),
    ("network_conflict_resolved_by_order", [(Evaluate,
      "    let [grant] = eligible.as_slice() else {\n",
      "    let Some(grant) = eligible.first() else {\n")],
     "cham3_policyd", "current_authorization", "conservative_evaluation_of_inconsistent_snapshots"),
    ("user_state_from_identity_ignored", [(Evaluate,
      "        .find(|state| *state != user_state_t::normal)\n",
      "        .find(|state| *state != *state)\n")],
     "cham3_policyd", "current_authorization", "conservative_evaluation_of_inconsistent_snapshots"),
    # Submission: the owner binding of contractd.
    ("submission_owner_not_bound", [("core/contractd/src/submit.rs",
      "        if payload.actor.uid != caller_uid {\n",
      "        if false && payload.actor.uid != caller_uid {\n")],
     "cham3_contractd", "submission", "submission_verifies_every_layer_before_admission"),
]


class seeded_registry_policy_defects_t(unittest.TestCase):
    """Runs the named Rust tests in a disposable workspace copy with one defect each."""

    @classmethod
    def setUpClass(cls):
        cls.workspace = tempfile.mkdtemp(prefix="cham3_m2_registry_policy_defect_")
        for name in ("Cargo.toml", "Cargo.lock"):
            shutil.copy2(os.path.join(RepoRoot, name), os.path.join(cls.workspace, name))
        shutil.copytree(os.path.join(RepoRoot, ".cargo"), os.path.join(cls.workspace, ".cargo"))
        shutil.copytree(os.path.join(RepoRoot, "core"), os.path.join(cls.workspace, "core"))
        # The admission differential reads the published vectors relative to the crate.
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
        for name, _, package, target, test_filter in SeededDefects:
            with self.subTest(control=name):
                status, output = self.cargo_test(package, target, test_filter)
                self.assertEqual(status, 0, output[-2000:])
                self.assertIn("test result: ok. 1 passed", output)

    def test_each_seeded_defect_is_detected(self):
        for name, edits, package, target, test_filter in SeededDefects:
            with self.subTest(defect=name):
                originals = {}
                try:
                    for relative, original, replacement in edits:
                        path = os.path.join(self.workspace, relative)
                        if path not in originals:
                            with open(path, encoding="utf-8") as handle:
                                originals[path] = handle.read()
                        with open(path, encoding="utf-8") as handle:
                            source = handle.read()
                        self.assertEqual(source.count(original), 1, "defect anchor for %s" % name)
                        with open(path, "w", encoding="utf-8") as handle:
                            handle.write(source.replace(original, replacement))
                    status, output = self.cargo_test(package, target, test_filter)
                finally:
                    for path, source in originals.items():
                        with open(path, "w", encoding="utf-8") as handle:
                            handle.write(source)
                self.assertNotEqual(status, 0, "%s was not detected" % name)
                # The defect must compile and be caught by the test, not by the build.
                self.assertIn("test %s ... FAILED" % test_filter, output, output[-2000:])


if __name__ == "__main__":
    unittest.main()
