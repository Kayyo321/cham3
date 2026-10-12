"""Regression controls for the independent reviews' reproduced holes.

Each case from the reviewer's schema_holes.py (review 9cf8825, evidence bundle: R06-R08)
and r2_probes.py (re-review 336f13b appendix: R07-r2, R12) is replayed separately
against the repaired oracle and must now be rejected with the designated layer/code;
positive controls show the unmodified or legitimately varied objects still pass. The
journal-rule sensitivity tests inject a controlled defect into a copy of the oracle's
journal model and require the published scenarios to detect it. Revision 4 (R14,
re-review f451f2e) adds the recovery key-naming interface: the IPC specification's
registryd table must route the naming record, agree with the oracle's interface model,
and the model's role, durability and reconciliation rules are defect-tested. The time
counterexamples (R09-r2, R15) are in test_m2_trusted_time.py; R01-r2 in
test_m2_source_identity.py.
"""

import copy
import hashlib
import importlib.util
import json
import os
import re
import unittest

from m2_harness_support import RepoRoot

OraclePath = os.path.join(RepoRoot, "tests", "m2", "oracle", "ecr_oracle.py")
IpcSpecPath = os.path.join(RepoRoot, "docs", "specs", "m2_service_ipc_v1.md")
RoleToken = re.compile(r"^(service:[a-z*]+|trusted_client|admin|fixture_consumer)$")
VectorsPath = os.path.join(RepoRoot, "tests", "m2", "vectors", "ecr_v1", "vectors.json")


def load_oracle():
    spec = importlib.util.spec_from_file_location("cham3_ecr_oracle", OraclePath)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class review_regressions_t(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.oracle = load_oracle()
        with open(VectorsPath, "r", encoding="utf-8") as handle:
            cls.vectors = json.load(handle)
        cls.keys = {item["label"]: cls.oracle.key_t(item["label"]) for item in cls.vectors["keys"]}
        cls.resolver = cls.oracle.resolver_t(list(cls.keys.values()))
        cls.valid = {item["name"]: bytes.fromhex(item["envelope"]) for item in cls.vectors["valid"]}
        cls.signer = cls.keys["user_1001_persistent"]

    def payload(self, name):
        return self.oracle.verify(self.valid[name], "commit", self.resolver)

    def assert_rejected(self, function, layer, code):
        with self.assertRaises(self.oracle.oracle_error_t) as caught:
            function()
        self.assertEqual((caught.exception.layer, caught.exception.code), (layer, code))

    def test_positive_control_lineage_admits(self):
        chain = [self.valid[name] for name in self.vectors["lineage"]["commits"]]
        self.assertEqual(len(self.oracle.admit_chain(chain, self.resolver)), len(chain))

    def test_r06_ordinary_revision_cannot_substitute_identity(self):
        revision = copy.deepcopy(self.payload("commit_revision"))
        revision["subject"]["package_identity"] = "deb:attacker:amd64"
        revision["subject"]["executable_sha256"] = b"\x42" * 32
        envelope, _ = self.oracle.sign_commit(self.signer, revision)
        self.assert_rejected(lambda: self.oracle.admit_chain([self.valid["commit_genesis"], envelope], self.resolver),
                             "admission", "subject_binding_mismatch")

    def test_r07_top_level_network_mode_is_not_representable(self):
        genesis = copy.deepcopy(self.payload("commit_genesis"))
        genesis["delta"] = {"add_grants": [], "remove_grant_ids": [], "network_mode": {"from": "offline", "to": "direct"}}
        genesis["result_state"] = {"grants": [], "network_mode": "direct"}
        envelope, _ = self.oracle.sign_commit(self.signer, genesis)
        self.assert_rejected(lambda: self.oracle.verify(envelope, "commit", self.resolver), "schema", "unknown_field")

    def test_r07_restore_cannot_introduce_network_absent_from_snapshot(self):
        names = self.vectors["lineage"]["commits"]
        case = next(item for item in self.vectors["admission_invalid"] if item["name"] == "restore_network_not_in_snapshot")
        chain = [self.valid[name] for name in names[: case["prefix"]]]
        self.assert_rejected(lambda: self.oracle.admit_chain(chain + [bytes.fromhex(case["envelope"])], self.resolver),
                             "admission", "restore_rule")

    def test_r08_maximum_object_fits_wrappers_and_one_more_byte_fails(self):
        limits = self.vectors["limits"]
        boundary = self.valid["commit_boundary_max_size"]
        self.assertEqual(len(boundary), limits["max_envelope_bytes"])
        self.oracle.admit_chain([boundary], self.resolver)
        wrapped = self.oracle.encode({"op": "submit_signed", "request_id": b"\x00" * 16,
                                      "args": {"proposal_id": b"\x00" * 16, "envelope": boundary}})
        self.assertLessEqual(len(wrapped), limits["max_wrapper_bytes"])
        self.assertLessEqual(len(wrapped) - len(boundary), limits["max_wrapper_metadata_bytes"])
        over = next(item for item in self.vectors["invalid"] if item["name"] == "boundary_one_byte_over")
        self.assertEqual(len(bytes.fromhex(over["envelope"])), limits["max_envelope_bytes"] + 1)
        self.assert_rejected(lambda: self.oracle.verify(bytes.fromhex(over["envelope"]), "commit", self.resolver),
                             "envelope_decode", "input_too_large")

    # ------------------------------------------------------------ re-review r2 (336f13b)

    def reviewer_siblings(self):
        """The r2_probes construction: Direct and Tor timed temporary siblings, both rooted
        at the current Offline genesis, both signed by the persistent key."""
        genesis = self.payload("commit_genesis")
        envelopes = []
        for mode in ("direct", "tor"):
            sibling = copy.deepcopy(genesis)
            sibling["commit_type"] = "temporary_grant"
            sibling["parents"] = [hashlib.sha256(self.valid["commit_genesis"]).digest()]
            sibling["base_state_hash"] = self.oracle.state_hash(genesis["result_state"])
            sibling["timestamp"] = self.oracle.Times[1]
            sibling["authorization"]["request_id"] = self.oracle.synthetic_id("review sibling " + mode)
            grant = self.oracle.make_grant("review network " + mode, "network", "network_mode", [mode], kind="timed",
                                           start=self.oracle.Times[1], expiration=self.oracle.Times[11])
            sibling["delta"] = {"add_grants": [grant], "remove_grant_ids": []}
            sibling["result_state"] = self.oracle.apply_delta(genesis["result_state"], sibling["delta"])
            envelopes.append(self.oracle.sign_commit(self.signer, sibling)[0])
        return envelopes

    def test_r07r2_one_network_branch_is_admitted(self):
        direct, tor = self.reviewer_siblings()
        self.oracle.admit_chain([self.valid["commit_genesis"], direct], self.resolver)
        self.oracle.admit_chain([self.valid["commit_genesis"], tor], self.resolver)

    def test_r07r2_live_sibling_network_branches_conflict(self):
        direct, tor = self.reviewer_siblings()
        for first, second in ((direct, tor), (tor, direct)):
            self.assert_rejected(lambda: self.oracle.admit_chain([self.valid["commit_genesis"], first, second], self.resolver),
                                 "admission", "network_grant_conflict")

    def reviewer_restore(self, grant):
        names = self.vectors["lineage"]["commits"]
        restore = copy.deepcopy(self.payload("commit_restore"))
        restore["restore_reference"] = hashlib.sha256(self.valid["commit_temporary_session"]).digest()
        restore["authorization"]["request_id"] = self.oracle.synthetic_id("review temporary restore")
        restore["delta"] = {"add_grants": [grant], "remove_grant_ids": []}
        restore["result_state"] = self.oracle.apply_delta(self.payload("commit_revocation")["result_state"], restore["delta"])
        envelope, _ = self.oracle.sign_commit(self.signer, restore)
        return [self.valid[name] for name in names[:6]] + [envelope]

    def test_r12_ancestral_session_grant_cannot_restore_as_persistent(self):
        old = self.payload("commit_temporary_session")["delta"]["add_grants"][0]
        restored = copy.deepcopy(old)
        restored["grant_id"] = self.oracle.synthetic_id("review restored session as persistent")
        restored["lifetime"] = {"kind": "persistent", "effective_start": self.oracle.Times[6]}
        self.assert_rejected(lambda: self.oracle.admit_chain(self.reviewer_restore(restored), self.resolver), "admission", "restore_rule")

    def test_r12_positive_control_persistent_grant_of_same_ancestor_restores(self):
        clipboard = next(grant for grant in self.payload("commit_temporary_session")["result_state"]["grants"]
                         if grant["resource"]["category"] == "clipboard")
        restored = copy.deepcopy(clipboard)
        restored["grant_id"] = self.oracle.synthetic_id("review restored clipboard from session ancestor")
        restored["lifetime"]["effective_start"] = self.oracle.Times[6]
        self.assertEqual(len(self.oracle.admit_chain(self.reviewer_restore(restored), self.resolver)), 7)

    # ------------------------------------------------------------ journal-rule sensitivity

    def scenario_failures(self, names):
        valid_by_name = {item["name"]: (item["kind"], bytes.fromhex(item["envelope"])) for item in self.vectors["valid"]}
        failures = []
        for scenario in self.vectors["journal_scenarios"]:
            if scenario["name"] in names:
                failures.extend(self.oracle.run_scenario(scenario, valid_by_name, self.resolver))
        return failures

    def with_defect(self, name, replacement, scenarios):
        original = getattr(self.oracle.journal_t, name)
        setattr(self.oracle.journal_t, name, replacement)
        try:
            return self.scenario_failures(scenarios)
        finally:
            setattr(self.oracle.journal_t, name, original)

    def test_published_journal_scenarios_pass(self):
        names = {scenario["name"] for scenario in self.vectors["journal_scenarios"]}
        self.assertGreaterEqual(len(names), 15)
        self.assertEqual(self.scenario_failures(names), [])

    def test_defect_per_state_network_rule_only_is_detected(self):
        # Revision-2 behavior: no cross-branch network slot.
        failures = self.with_defect("occupying_network_grants", lambda journal, states: set(),
                                    {"network_sibling_branches_conflict", "network_sibling_after_durable_expiry"})
        self.assertTrue(any("network_sibling_branches_conflict" in failure for failure in failures), failures)

    def test_defect_clock_based_slot_release_is_detected(self):
        # A slot released by lifetime arithmetic instead of durable terminal records.
        def occupying(journal, states):
            return {grant["grant_id"] for state in states for grant in state["grants"]
                    if self.oracle.is_network_grant(grant) and grant["lifetime"].get("expiration", "9999") > self.oracle.Times[2]
                    and grant["grant_id"] not in journal.ended}
        failures = self.with_defect("occupying_network_grants", occupying, {"network_sibling_non_overlapping_lifetimes_conflict"})
        self.assertTrue(failures)

    def test_defect_revision_2_restore_predicate_is_detected(self):
        def revision_2_restore(journal, payload):
            reference = journal.commits.get(payload["restore_reference"])
            if reference is None or reference["lineage_id"] != payload["lineage_id"]:
                self.oracle.admission_error("restore_rule", "reference not in lineage")
            for grant in payload["delta"]["add_grants"]:
                if not any((self.oracle.encode(old["resource"]), self.oracle.encode(old["delegation"]))
                           == (self.oracle.encode(grant["resource"]), self.oracle.encode(grant["delegation"]))
                           for old in reference["result_state"]["grants"]):
                    self.oracle.admission_error("restore_rule", "restored grant not in referenced state")
        failures = self.with_defect("check_restore", revision_2_restore, {"restore_matches_only_persistent_history"})
        self.assertGreaterEqual(len(failures), 3, failures)

    def test_defect_recovery_structure_unchecked_is_detected(self):
        failures = self.with_defect("check_recovery_commit", lambda journal, lineage, payload, position, base: None,
                                    {"recovery_rejects_bypass"})
        self.assertGreaterEqual(len(failures), 5, failures)

    def test_defect_containment_not_enforced_on_keys_is_detected(self):
        original = self.oracle.journal_t.check_key_authority

        def lenient(journal, payload):
            state, journal.state = journal.state, "normal"
            try:
                original(journal, payload)
            finally:
                journal.state = state
        failures = self.with_defect("check_key_authority", lenient,
                                    {"recovery_c1_complete", "recovery_rejects_deleted_user", "recovery_quarantine_requires_admin_first"})
        self.assertTrue(any("recovery_c1_complete" in failure for failure in failures), failures)
        self.assertTrue(any("recovery_rejects_deleted_user" in failure for failure in failures), failures)

    def test_defect_old_keys_not_retired_is_detected(self):
        def keep_old_keys(journal, record):
            original(journal, record)
            journal.min_key_epoch = 0
            for key in journal.keys.values():
                key.retired = False
        original = self.oracle.journal_t.admit_naming
        failures = self.with_defect("admit_naming", keep_old_keys,
                                    {"recovery_rejects_stale_key", "recovery_rejects_abandoned_key"})
        self.assertTrue(any("recovery_rejects_stale_key" in failure for failure in failures), failures)
        self.assertTrue(any("recovery_rejects_abandoned_key" in failure for failure in failures), failures)


    # ------------------------------------------------------------ R14: key-naming route

    def registry_table(self):
        with open(IpcSpecPath, "r", encoding="utf-8") as handle:
            text = handle.read()
        table = text.split("### cham3-registryd")[1].split("## 4.")[0]
        rows = {}
        for operations, roles in re.findall(r"^\| (.*?) \| (.*?) \|", table, re.M):
            for operation in re.findall(r"`([a-z_]+)`", operations):
                rows[operation] = {token for token in re.findall(r"`([^`]+)`", roles) if RoleToken.match(token)}
        return rows

    def test_r14_reviewer_static_gap_is_closed(self):
        # The re-review's static reproduction asserted the gap; it must now find a
        # registryd naming operation authorized for vaultd.
        rows = self.registry_table()
        self.assertTrue(any("recovery_keys" in operation for operation in rows), rows)
        self.assertTrue(any("service:vaultd" in roles for roles in rows.values()), rows)

    def test_r14_specification_matches_interface_model(self):
        rows = self.registry_table()
        self.assertEqual(rows["name_recovery_keys"], {"service:vaultd"})
        self.assertEqual(rows["append_identity"], {"service:identityd"})
        for operation, roles in self.oracle.RegistryModelOperations.items():
            self.assertEqual(rows.get(operation), roles, operation)
        vaultd_operations = {operation for operation, roles in rows.items() if "service:vaultd" in roles}
        self.assertEqual(vaultd_operations, {"name_recovery_keys", "recovery_status"},
                         "vaultd reaches registryd only for key naming and its reconciliation")
        # No registryd operation appends a caller-chosen record.
        self.assertEqual(self.oracle.OperationArguments["name_recovery_keys"],
                         {"owner_uid", "recovery_id", "persistent_key_id", "temporary_key_id"})

    def test_r14_positive_scenarios_name_keys_only_through_the_interface(self):
        for scenario in self.vectors["journal_scenarios"]:
            for step in scenario["steps"]:
                self.assertNotEqual(step.get("record", {}).get("record"), "recovery_keys", scenario["name"])
        journal = self.oracle.journal_t(self.resolver, key_authority=True)
        self.assert_rejected(lambda: journal.admit_record({"record": "recovery_keys"}), "admission", "lifecycle_rule")

    def with_service_defect(self, name, replacement, scenarios):
        original = getattr(self.oracle.registry_service_t, name)
        setattr(self.oracle.registry_service_t, name, replacement)
        try:
            return self.scenario_failures(scenarios)
        finally:
            setattr(self.oracle.registry_service_t, name, original)

    def test_defect_naming_open_to_every_role_is_detected(self):
        operations = self.oracle.RegistryModelOperations
        original = operations["name_recovery_keys"]
        operations["name_recovery_keys"] = {"service:vaultd", "service:identityd", "service:contractd", "service:policyd",
                                            "trusted_client", "admin", "fixture_consumer"}
        try:
            failures = self.scenario_failures({"recovery_naming_interface"})
        finally:
            operations["name_recovery_keys"] = original
        self.assertGreaterEqual(len(failures), 6, failures)

    def test_defect_unanchored_naming_served_is_detected(self):
        original = self.oracle.registry_service_t.op_name_recovery_keys

        def apply_before_anchor(service, args, anchor_fault):
            return original(service, args, False)
        failures = self.with_service_defect("op_name_recovery_keys", apply_before_anchor,
                                            {"recovery_naming_anchor_failure_restart", "recovery_naming_anchor_failure_retry"})
        self.assertTrue(any("recovery_naming_anchor_failure_restart" in failure for failure in failures), failures)

    def test_defect_naming_effective_before_anchor_with_preserved_responses_is_detected(self):
        # I04 (re-review 3a3b5d1): the reviewer's mutant applies the naming scope, epoch
        # floor and retirement before the anchor update, yet keeps every response
        # (storage_unavailable), the pending-tail block and the retry result. Only the
        # scenarios' direct key-state observations can detect it.
        oracle = self.oracle
        original = oracle.registry_service_t.op_name_recovery_keys

        def early(service, args, anchor_fault):
            if not anchor_fault:
                return original(service, args, False)
            original(service, args, False)
            service.journal.sequence -= 1
            service.pending = {"record": "recovery_keys", "recovery_id": args["recovery_id"],
                               "persistent_key_id": args["persistent_key_id"], "temporary_key_id": args["temporary_key_id"]}
            oracle.ipc_error("storage_unavailable")

        def settle(service, anchor_fault=False):
            if service.pending is None:
                return
            if anchor_fault:
                oracle.ipc_error("storage_unavailable")
            service.pending = None
            service.journal.sequence += 1
        names = {"recovery_naming_anchor_failure_restart", "recovery_naming_anchor_failure_retry"}
        saved = (oracle.registry_service_t.op_name_recovery_keys, oracle.registry_service_t.settle)
        oracle.registry_service_t.op_name_recovery_keys, oracle.registry_service_t.settle = early, settle
        try:
            failures = self.scenario_failures(names)
        finally:
            oracle.registry_service_t.op_name_recovery_keys, oracle.registry_service_t.settle = saved
        for name in names:
            self.assertTrue(any(name in failure and "key state" in failure for failure in failures), failures)
        self.assertFalse(any("expected" in failure and "key state" not in failure for failure in failures),
                         "responses and record counts are preserved by this mutant: %s" % failures)

    def test_defect_lost_durable_tail_is_detected(self):
        def drop_tail(service, anchor_fault=False):
            if anchor_fault and service.pending is not None:
                self.oracle.ipc_error("storage_unavailable")
            service.pending = None
        failures = self.with_service_defect("settle", drop_tail, {"recovery_naming_anchor_failure_restart"})
        self.assertTrue(failures)

    def test_defect_non_idempotent_retry_is_detected(self):
        original = self.oracle.registry_service_t.op_name_recovery_keys

        def strict(service, args, anchor_fault):
            if args["recovery_id"] in service.journal.named_keys:
                self.oracle.record_error("recovery_rule", "already named")
            return original(service, args, anchor_fault)
        failures = self.with_service_defect("op_name_recovery_keys", strict,
                                            {"recovery_naming_interface", "recovery_naming_anchor_failure_restart"})
        self.assertGreaterEqual(len(failures), 2, failures)

    def test_r14_vault_reconciliation_never_erases_named_or_unknown_keys(self):
        held = (b"\x01" * 32, b"\x02" * 32)
        other = (b"\x03" * 32, b"\x04" * 32)

        def status(named, ceremony_open=True, state="contained"):
            return {"user_state": state, "ceremony_open": ceremony_open,
                    "persistent_key_id": named[0] if named else None, "temporary_key_id": named[1] if named else None}
        decide = self.oracle.vault_reconcile
        self.assertEqual(decide(None, held), "keep_and_retry")
        self.assertEqual(decide(status(held), held), "keep_named")
        self.assertEqual(decide(status(held, False, "normal"), held), "keep_named", "closed ceremony: these are the current keys")
        self.assertEqual(decide(status(held, False, "contained"), held), "keep_named", "aborted ceremony: retired only by a later naming")
        self.assertEqual(decide(status(other), held), "erase_retired")
        self.assertEqual(decide(status(None), held), "resume_naming")
        self.assertEqual(decide(status(None, False, "deleted"), held), "erase_unnamed")

    def test_r14_in_flight_request_after_vault_restart_reconciles(self):
        # vaultd restarts while its naming request is still queued at registryd. It
        # resumes with the keys it holds (never regenerates while they are nameable), so
        # the late request and the resumed one name the same keys.
        valid_by_name = {item["name"]: (item["kind"], bytes.fromhex(item["envelope"])) for item in self.vectors["valid"]}
        scenario = next(item for item in self.vectors["journal_scenarios"] if item["name"] == "recovery_naming_interface")
        journal = self.oracle.journal_t(self.resolver, key_authority=True)
        service = self.oracle.registry_service_t(journal)
        steps = scenario["steps"]
        first_naming = next(index for index, step in enumerate(steps)
                            if step.get("call", {}).get("operation") == "name_recovery_keys" and step["expect"] == "admitted")
        for step in steps[:first_naming]:
            if "valid" in step:
                kind, data = valid_by_name[step["valid"]]
                if kind == "commit":
                    service.admit(data)
                else:
                    service.call("service:identityd", "append_identity", {"kind": kind, "object": data})
            elif "commit" in step and step["expect"] == "admitted":
                service.admit(bytes.fromhex(step["commit"]))
            elif "identity" in step and step["expect"] == "admitted":
                service.call("service:identityd", "append_identity", {"kind": step["kind"], "object": bytes.fromhex(step["identity"])})
            elif "record" in step and step["expect"] == "admitted":
                service.record(self.oracle.decode_record(step["record"]))
        args = self.oracle.decode_record(steps[first_naming]["call"]["args"])
        held = (args["persistent_key_id"], args["temporary_key_id"])
        status_args = {"owner_uid": 1001, "recovery_id": args["recovery_id"]}
        self.assertEqual(self.oracle.vault_reconcile(service.call("service:vaultd", "recovery_status", status_args), held), "resume_naming")
        late = service.call("service:vaultd", "name_recovery_keys", dict(args))
        resumed = service.call("service:vaultd", "name_recovery_keys", dict(args))
        self.assertEqual((late["already_named"], resumed["already_named"]), (False, True))
        self.assertEqual(self.oracle.vault_reconcile(service.call("service:vaultd", "recovery_status", status_args), held), "keep_named")


if __name__ == "__main__":
    unittest.main()
