"""Published vectors of the trusted-time rule (ADR 0007 section 5, section 8 option B).

The expected values come from tests/m2/oracle/trusted_time_model.py, the executable
reference of the rule. The cargo test core/registryd/tests/trusted_time_model.rs
replays every case through registryd's clock (core/registryd/src/clock.rs) and compares
each value. The cases are the model's named reproductions (the review counterexample,
same-boot restart from floors, a new boot, a peer's offset), plus seeded random sequences
of samples, adopted offsets, floors and restarts in the same boot and in a new boot.

Usage: trusted_time_vectors.py --write <path>   writes the document
       trusted_time_vectors.py --check <path>   exits 1 unless the stored file equals it

Every value is an integer number of seconds. A case's floors are the persisted floors
the first start reads; a "floor" step records what the running clock persists, and a
"restart" step starts a new clock from every floor so far (in the same boot, or in a
new one) and a fresh sample.
"""

import importlib.util
import json
import os
import random
import sys

Here = os.path.dirname(os.path.abspath(__file__))
Seed = 20261014
RandomCases = 160
StepsPerCase = 30
Hour = 3600
Noon = 1_791_806_400  # 2026-10-12T12:00:00Z


def load_model():
    spec = importlib.util.spec_from_file_location("cham3_trusted_time_model", os.path.join(Here, "trusted_time_model.py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


Model = load_model()


class case_builder_t:
    """Runs the model and records each operation with the values it produces."""

    def __init__(self, name, boot_id, realtime, boottime, floors=()):
        self.name = name
        self.boot_id = boot_id
        self.floors = [dict(floor) for floor in floors]
        self.steps = []
        self.start = {"realtime": realtime, "boottime": boottime}
        self.begin(boot_id, realtime, boottime)
        self.expect_start = {"trusted": self.trusted.now(boottime), "observed": self.observed.now(boottime)}

    def begin(self, boot_id, realtime, boottime):
        self.current_boot = boot_id
        self.boottime = boottime
        trusted_floors = [Model.floor_t(floor["boot_id"], floor["boottime"], floor["trusted"]) for floor in self.floors]
        observed_floors = [Model.floor_t(floor["boot_id"], floor["boottime"], floor["observed"]) for floor in self.floors]
        self.trusted = Model.trusted_clock_t(boot_id, realtime, boottime, trusted_floors)
        self.observed = Model.observed_clock_t(boot_id, boottime, observed_floors)

    def sample(self, realtime, boottime):
        assert boottime >= self.boottime
        self.boottime = boottime
        anomalies = len(self.trusted.anomalies)
        trusted = self.trusted.sample(realtime, boottime)
        self.steps.append({"op": "sample", "realtime": realtime, "boottime": boottime, "trusted": trusted,
                           "observed": self.observed.now(boottime), "anomaly": len(self.trusted.anomalies) > anomalies})

    def adopt(self, offset):
        self.trusted.adopt(offset)
        self.steps.append({"op": "adopt", "offset": offset, "trusted": self.trusted.now(self.boottime)})

    def floor(self):
        floor = {"boot_id": self.current_boot, "boottime": self.boottime,
                 "trusted": self.trusted.floor(self.boottime).trusted, "observed": self.observed.floor(self.boottime).trusted}
        self.floors.append(floor)
        self.steps.append(dict(floor, op="floor"))

    def restart(self, boot_id, realtime, boottime):
        self.begin(boot_id, realtime, boottime)
        self.steps.append({"op": "restart", "boot_id": boot_id, "realtime": realtime, "boottime": boottime,
                           "trusted": self.trusted.now(boottime), "observed": self.observed.now(boottime)})

    def document(self, initial_floors):
        return {"name": self.name, "boot_id": self.boot_id, "start": self.start, "floors": initial_floors,
                "expect_start": self.expect_start, "steps": self.steps}


def named_cases():
    cases = []
    # The re-review's counterexample (R09-r2): a forward step, a correction back, an idle hour.
    case = case_builder_t("forward_step_then_correction_then_idle", "boot-a", Noon, 0)
    case.sample(Noon + 2 * Hour, 60)
    case.sample(Noon + Hour, 61)
    case.sample(Noon + Hour, 61 + Hour)
    cases.append(case.document([]))
    # Same-boot restart from the larger of two floors, with the wall clock set back.
    floors = [{"boot_id": "boot-a", "boottime": 100, "trusted": Noon + 5000, "observed": 90},
              {"boot_id": "boot-a", "boottime": 50, "trusted": Noon + 4000, "observed": 60}]
    case = case_builder_t("same_boot_restart_from_the_largest_floor", "boot-a", Noon, 120, floors)
    case.sample(Noon, 180)
    cases.append(case.document(floors))
    # A new boot starts no earlier than the last persisted trusted value; the observed
    # counter resumes from its last value (powered-off time never counts).
    floors = [{"boot_id": "boot-a", "boottime": 7200, "trusted": Noon + 7200, "observed": 7000}]
    case = case_builder_t("new_boot_from_the_last_floor", "boot-b", Noon - Hour, 5, floors)
    case.sample(Noon - Hour + 10, 15)
    case.floor()
    case.restart("boot-b", Noon - 2 * Hour, 20)
    cases.append(case.document(floors))
    # A peer's larger offset is adopted; a smaller one changes nothing.
    case = case_builder_t("peer_offsets_only_raise", "boot-a", Noon, 10)
    case.adopt(Noon + 100 - 10)
    case.adopt(Noon - 500)
    case.sample(Noon + 20, 30)
    cases.append(case.document([]))
    # An anomaly episode: more than 300 s behind, then within 300 s, then behind again.
    case = case_builder_t("anomaly_episodes", "boot-a", Noon, 0)
    case.sample(Noon - 301, 0)
    case.sample(Noon - 300, 0)
    case.sample(Noon + 10 - 400, 10)
    cases.append(case.document([]))
    return cases


def random_cases(generator):
    cases = []
    for index in range(RandomCases):
        boot_number = 0
        boot_id = "boot-r%d-%d" % (index, boot_number)
        realtime = Noon + generator.randint(-5 * Hour, 5 * Hour)
        boottime = generator.randint(0, 600)
        initial = []
        if generator.random() < 0.3:
            initial.append({"boot_id": "boot-earlier-%d" % index, "boottime": generator.randint(0, 9000),
                            "trusted": Noon + generator.randint(-6 * Hour, 6 * Hour), "observed": generator.randint(0, 50_000)})
        case = case_builder_t("seeded_%03d" % index, boot_id, realtime, boottime, initial)
        for _ in range(StepsPerCase):
            action = generator.random()
            if action < 0.6:
                boottime = case.boottime + generator.randint(0, 3 * Hour)
                around = boottime + case.trusted.offset
                case.sample(around + generator.randint(-Hour, Hour), boottime)
            elif action < 0.7:
                case.adopt(case.trusted.offset + generator.randint(-Hour, Hour))
            elif action < 0.85:
                case.floor()
            elif action < 0.95:
                boottime = case.boottime + generator.randint(0, 600)
                case.restart(case.current_boot, Noon + generator.randint(-6 * Hour, 6 * Hour), boottime)
            else:
                boot_number += 1
                case.restart("boot-r%d-%d" % (index, boot_number), Noon + generator.randint(-6 * Hour, 6 * Hour),
                             generator.randint(0, 600))
        cases.append(case.document(initial))
    return cases


def document():
    generator = random.Random(Seed)
    return {
        "description": "Trusted-time vectors from tests/m2/oracle/trusted_time_model.py (ADR 0007 section 5, "
                       "section 8 option B); replayed by core/registryd/tests/trusted_time_model.rs",
        "seed": Seed,
        "anomaly_seconds": Model.AnomalySeconds,
        "cases": named_cases() + random_cases(generator),
    }


def serialize(value):
    """One case per line, keys sorted: stable and reviewable as a diff."""
    cases = value["cases"]
    head = dict(value, cases=[])
    lines = [json.dumps(case, sort_keys=True, separators=(",", ":")) for case in cases]
    text = json.dumps(head, sort_keys=True, indent=1)
    return text.replace('"cases": []', '"cases": [\n' + ",\n".join(lines) + "\n ]") + "\n"


def main(arguments):
    if len(arguments) != 2 or arguments[0] not in ("--write", "--check"):
        print(__doc__)
        return 2
    mode, path = arguments
    text = serialize(document())
    if mode == "--write":
        with open(path, "w", encoding="utf-8") as output:
            output.write(text)
        print("wrote %d cases" % len(json.loads(text)["cases"]))
        return 0
    with open(path, encoding="utf-8") as stored:
        if stored.read() != text:
            print("trusted-time vectors differ from the model: FAIL")
            return 1
    print("trusted-time vectors match the model (%d cases): PASS" % len(json.loads(text)["cases"]))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
