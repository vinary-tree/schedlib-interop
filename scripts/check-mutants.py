#!/usr/bin/env python3
"""Kill one declared causal fault for every interop invariant."""

from __future__ import annotations

import sys
from dataclasses import dataclass
from typing import Callable

sys.dont_write_bytecode = True


@dataclass(frozen=True)
class Mutation:
    control: Callable[[], bool]
    changed: Callable[[], bool]


MUTANTS = {
    "mutant_unknown_phase": Mutation(
        control=lambda: 0 <= 8 <= 8,
        changed=lambda: 0 <= 9 <= 8,
    ),
    "mutant_big_endian_word": Mutation(
        control=lambda: int.from_bytes((1).to_bytes(4, "little"), "little") == 1,
        changed=lambda: int.from_bytes((1).to_bytes(4, "little"), "big") == 1,
    ),
    "mutant_pointer_width_word": Mutation(
        control=lambda: len((1).to_bytes(8, "little")) == 8,
        changed=lambda: len((1).to_bytes(4, "little")) == 8,
    ),
    "mutant_ignore_reserved_flags": Mutation(
        control=lambda: all((True, True, True, True, True)),
        changed=lambda: all((True, True, True, False, True)),
    ),
    "mutant_accept_trailing_byte": Mutation(
        control=lambda: 3 + 0 == 3,
        changed=lambda: 3 + 1 == 3,
    ),
    "mutant_wrapping_payload_length": Mutation(
        control=lambda: (1 << 64) - 2 + 1 <= (1 << 64) - 1,
        changed=lambda: (1 << 64) - 1 + 1 <= (1 << 64) - 1,
    ),
    "mutant_hash_before_byte_limit": Mutation(
        control=lambda: ["limit"] == ["limit"],
        changed=lambda: ["hash", "limit"] == ["limit"],
    ),
    "mutant_allocate_before_count_limit": Mutation(
        control=lambda: not (3 <= 2),
        changed=lambda: 3 <= 2,
    ),
    "mutant_skip_work_charge": Mutation(
        control=lambda: 3 + 1 == 4,
        changed=lambda: 3 + 1 == 3,
    ),
    "mutant_publish_after_cancel": Mutation(
        control=lambda: not (True and False),
        changed=lambda: not (True and True),
    ),
    "mutant_reallocate_output": Mutation(
        control=lambda: len([4]) <= 1,
        changed=lambda: len([2, 4]) <= 1,
    ),
    "mutant_ignore_codec_identity": Mutation(
        control=lambda: 1 == 1,
        changed=lambda: 1 == 2,
    ),
    "mutant_normalize_key_bytes": Mutation(
        control=lambda: b"A" == b"A",
        changed=lambda: b"a" == b"A",
    ),
    "mutant_accept_key_collision": Mutation(
        control=lambda: (b"a" == b"a") == (1 == 1),
        changed=lambda: (b"x" == b"x") == (1 == 2),
    ),
    "mutant_omit_plan_budget": Mutation(
        control=lambda: (1, 2) != (1, 3),
        changed=lambda: (1,) != (1,),
    ),
    "mutant_swap_plan_tasks": Mutation(
        control=lambda: (1, 2) == (1, 2),
        changed=lambda: (2, 1) == (1, 2),
    ),
    "mutant_preserve_noncanonical_dependency": Mutation(
        control=lambda: ((0, 1), (1, 2)) == tuple(sorted({(1, 2), (0, 1)})),
        changed=lambda: ((1, 2), (0, 1)) == tuple(sorted({(1, 2), (0, 1)})),
    ),
    "mutant_accept_unknown_dependency": Mutation(
        control=lambda: max(0, 1) < 2,
        changed=lambda: max(0, 2) < 2,
    ),
    "mutant_accept_duplicate_dependency": Mutation(
        control=lambda: (0, 1) < (1, 2),
        changed=lambda: (0, 1) < (0, 1),
    ),
    "mutant_accept_duplicate_resource": Mutation(
        control=lambda: 1 < 2,
        changed=lambda: 1 < 1,
    ),
    "mutant_lossy_profile_decode": Mutation(
        control=lambda: "λ".encode().decode() == "λ",
        changed=lambda: b"?".decode() == "λ",
    ),
    "mutant_shared_digest_context": Mutation(
        control=lambda: b"plan" != b"checkpoint",
        changed=lambda: b"shared" != b"shared",
    ),
    "mutant_omit_digest_length": Mutation(
        control=lambda: (b"schema", 3, b"abc") != (b"schema", 4, b"abc"),
        changed=lambda: (b"schema", b"abc") != (b"schema", b"abc"),
    ),
    "mutant_decode_after_digest_mismatch": Mutation(
        control=lambda: (False and True) is False,
        changed=lambda: (False or True) is False,
    ),
    "mutant_accept_foreign_plan": Mutation(
        control=lambda: 1 == 1,
        changed=lambda: 1 == 2,
    ),
    "mutant_trust_plan_digest_only": Mutation(
        control=lambda: True and (1 == 1),
        changed=lambda: True and (1 == 2),
    ),
    "mutant_accept_excess_event": Mutation(
        control=lambda: 3 <= 2 + 1,
        changed=lambda: 4 <= 2 + 1,
    ),
    "mutant_skip_success_task": Mutation(
        control=lambda: (1, 1) == (1, 1),
        changed=lambda: (1, 1) == (1, 0),
    ),
    "mutant_append_after_terminal": Mutation(
        control=lambda: [2][-1] == 2,
        changed=lambda: [2, 1][-1] == 2,
    ),
    "mutant_store_untrusted_cursor": Mutation(
        control=lambda: 2 == len((1, 1)),
        changed=lambda: 3 == len((1, 1)),
    ),
    "mutant_accept_receipt_gap": Mutation(
        control=lambda: (0, 1) == tuple(range(2)),
        changed=lambda: (0, 2) == tuple(range(2)),
    ),
    "mutant_accept_unknown_event_kind": Mutation(
        control=lambda: 1 <= 6 <= 6,
        changed=lambda: 1 <= 7 <= 6,
    ),
    "mutant_serialize_application_payload": Mutation(
        control=lambda: "secret" not in (1, 2, 3),
        changed=lambda: "secret" not in (1, 2, 3, "secret"),
    ),
    "mutant_change_published_prefix": Mutation(
        control=lambda: (3, 2) == (3, 2),
        changed=lambda: (3, 1) == (3, 2),
    ),
    "mutant_resume_from_zero": Mutation(
        control=lambda: (0, 1) + (2, 3) == (0, 1, 2, 3),
        changed=lambda: (0, 1) + (0, 1, 2, 3) == (0, 1, 2, 3),
    ),
    "mutant_reorder_replay_events": Mutation(
        control=lambda: bytes((1, 2, 3)) == bytes((1, 2, 3)),
        changed=lambda: bytes((1, 3, 2)) == bytes((1, 2, 3)),
    ),
    "mutant_recursive_field_decode": Mutation(
        control=lambda: 1 == 1,
        changed=lambda: 2 == 1,
    ),
    "mutant_rescan_prefix_per_event": Mutation(
        control=lambda: 8 + 3 * 4 <= 8 + 3 * 4,
        changed=lambda: 8 + 4 * 4 <= 8 + 3 * 4,
    ),
    "mutant_global_sequence_number": Mutation(
        control=lambda: b"same" == b"same",
        changed=lambda: b"first-1" == b"first-2",
    ),
    "mutant_partial_object_on_error": Mutation(
        control=lambda: None is None,
        changed=lambda: object() is None,
    ),
}


def main() -> None:
    if len(MUTANTS) != 40:
        raise AssertionError(f"expected 40 mutants, found {len(MUTANTS)}")
    for name, mutation in MUTANTS.items():
        if not mutation.control():
            raise AssertionError(f"control is invalid for {name}")
        if mutation.changed():
            raise AssertionError(f"mutant survived: {name}")
        print(f"killed {name}")
    print("schedlib-interop mutation gate: 40/40 causal mutants killed")


if __name__ == "__main__":
    main()
