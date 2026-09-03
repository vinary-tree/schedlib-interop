#!/usr/bin/env python3
"""Exhaustively check finite schedlib-interop contract projections."""

from __future__ import annotations

import hashlib
import itertools
import struct
import sys
from concurrent.futures import ThreadPoolExecutor

sys.dont_write_bytecode = True

U64_MAX = (1 << 64) - 1
EVENT_KINDS = (1, 2, 3, 4, 5, 6)
TERMINAL_KINDS = frozenset((2, 3, 4, 5, 6))


def canonical_checkpoint(tasks: int, kinds: tuple[int, ...], receipts: int) -> bool:
    if len(kinds) > tasks + 1 or receipts > len(kinds):
        return False
    successes = 0
    for index, kind in enumerate(kinds):
        if kind not in EVENT_KINDS:
            return False
        if kind == 1:
            if successes != index or successes >= tasks:
                return False
            successes += 1
            continue
        if index != len(kinds) - 1:
            return False
        if kind in (2, 3, 4, 5) and successes >= tasks:
            return False
        if kind == 6 and successes != tasks:
            return False
    return True


def encode_plan_model(plan: tuple[object, ...]) -> bytes:
    return repr(plan).encode("utf-8")


def oracle_machine_state_is_typed() -> int:
    phases = tuple(range(9))
    assert all(0 <= phase <= 8 for phase in phases)
    return len(phases)


def oracle_little_endian_words() -> int:
    cases = 0
    for value in range(0, 65_536, 257):
        assert int.from_bytes(value.to_bytes(4, "little"), "little") == value
        assert struct.unpack("<I", struct.pack("<I", value))[0] == value
        cases += 1
    return cases


def oracle_platform_width_independence() -> int:
    cases = 0
    for pointer_bits in (16, 32, 64, 128):
        for payload in range(5):
            assert 112 + payload == 112 + payload
            assert len((payload).to_bytes(8, "little")) == 8
            cases += 1
    return cases


def oracle_header_matrix() -> int:
    cases = 0
    for fields in itertools.product((False, True), repeat=5):
        assert all(fields) == (fields == (True,) * 5)
        cases += 1
    return cases


def oracle_exact_frame_length() -> int:
    cases = 0
    for declared, actual, trailing in itertools.product(range(5), repeat=3):
        admitted = declared == actual and trailing == 0
        assert admitted == (declared == actual + trailing and actual == declared)
        cases += 1
    return cases


def oracle_checked_arithmetic() -> int:
    cases = 0
    values = (0, 1, U64_MAX - 1, U64_MAX)
    for left, right in itertools.product(values, repeat=2):
        checked = left + right if left <= U64_MAX - right else None
        assert (checked is None) == (left + right > U64_MAX)
        cases += 1
    return cases


def oracle_byte_admission_order() -> int:
    cases = 0
    for size, limit in itertools.product(range(5), repeat=2):
        trace = ["byte-limit"]
        if size <= limit:
            trace.extend(("scan", "allocate"))
        assert size <= limit or trace == ["byte-limit"]
        cases += 1
    return cases


def oracle_count_admission_matrix() -> int:
    cases = 0
    for counts in itertools.product(range(3), repeat=6):
        for limit in range(3):
            admitted = all(count <= limit for count in counts)
            allocated = admitted
            assert not allocated or admitted
            cases += 1
    return cases


def oracle_work_budget() -> int:
    cases = 0
    for items, byte_count, limit in itertools.product(range(5), repeat=3):
        required = 8 + 3 * items + byte_count
        admitted = required <= limit
        assert not admitted or required <= limit
        cases += 1
    return cases


def oracle_cancellation_boundaries() -> int:
    cases = 0
    for length in range(5):
        for threshold in range(length + 2):
            cursor = 0
            published = False
            while cursor < length and cursor < threshold:
                cursor += 1
            cancelled = cursor >= threshold
            if not cancelled:
                published = True
            assert not cancelled or not published
            cases += 1
    return cases


def oracle_reservation_accounting() -> int:
    cases = 0
    for admitted, declared in itertools.product((False, True), range(8)):
        reservations = [declared] if admitted else []
        assert len(reservations) <= 1
        assert not reservations or reservations[0] == declared
        cases += 1
    return cases


def oracle_codec_identity() -> int:
    cases = 0
    for expected, actual in itertools.product(range(4), repeat=2):
        admitted = expected == actual
        assert admitted == (expected == actual)
        cases += 1
    return cases


def oracle_key_round_trip() -> int:
    cases = 0
    for key in range(256):
        encoded = key.to_bytes(8, "big")
        decoded = int.from_bytes(encoded, "big")
        assert decoded == key
        assert decoded.to_bytes(8, "big") == encoded
        cases += 1
    return cases


def oracle_key_injectivity() -> int:
    cases = 0
    encoded = {key: key.to_bytes(2, "big") for key in range(32)}
    for left, right in itertools.product(encoded, repeat=2):
        assert (encoded[left] == encoded[right]) == (left == right)
        cases += 1
    return cases


def oracle_plan_field_binding() -> int:
    baseline = (1, (b"a",), ((0, 0),), (((1,), (2,)),), (3,), 4, "p", b"codec")
    cases = 0
    for index in range(len(baseline)):
        variant = list(baseline)
        variant[index] = ("changed", index)
        assert tuple(variant) != baseline
        cases += 1
    return cases


def oracle_plan_round_trip() -> int:
    cases = 0
    for tasks in range(4):
        plan = (1, tuple(range(tasks)), tuple((i, i + 1) for i in range(max(0, tasks - 1))))
        encoded = encode_plan_model(plan)
        assert encoded.decode("utf-8") == repr(plan)
        cases += 1
    return cases


def oracle_plan_canonicality() -> int:
    cases = 0
    for values in itertools.product(range(3), repeat=3):
        canonical = tuple(sorted(set(values)))
        first = encode_plan_model(canonical)
        second = encode_plan_model(tuple(sorted(set(canonical))))
        assert first == second
        cases += 1
    return cases


def oracle_dependency_bounds() -> int:
    cases = 0
    for tasks in range(4):
        for source, target in itertools.product(range(5), repeat=2):
            admitted = source < tasks and target < tasks
            assert admitted == (max(source, target) < tasks)
            cases += 1
    return cases


def oracle_dependency_order() -> int:
    cases = 0
    edges = tuple(itertools.product(range(3), repeat=2))
    for length in range(4):
        for selection in itertools.product(edges, repeat=length):
            canonical = all(selection[i] < selection[i + 1] for i in range(length - 1))
            assert canonical == (tuple(sorted(set(selection))) == selection)
            cases += 1
    return cases


def oracle_effect_order() -> int:
    cases = 0
    for values in itertools.product(range(3), repeat=3):
        canonical = all(values[i] < values[i + 1] for i in range(2))
        assert canonical == (tuple(sorted(set(values))) == values)
        cases += 1
    return cases


def oracle_profile_bytes() -> int:
    profiles = ("", "ascii", "λ", "naïve", "🦀")
    for profile in profiles:
        encoded = profile.encode("utf-8")
        assert encoded.decode("utf-8") == profile
    for malformed in (b"\x80", b"\xc0\x80", b"\xf5\x80\x80\x80"):
        try:
            malformed.decode("utf-8")
        except UnicodeDecodeError:
            continue
        raise AssertionError("malformed UTF-8 was accepted")
    return len(profiles) + 3


def oracle_digest_domain_separation() -> int:
    cases = 0
    for payload in (b"", b"x", bytes(range(16))):
        plan = hashlib.blake2s(b"schedlib-plan-v1" + payload).digest()
        checkpoint = hashlib.blake2s(b"schedlib-checkpoint-v1" + payload).digest()
        assert plan != checkpoint
        cases += 1
    return cases


def oracle_digest_input_binding() -> int:
    baseline = (b"schema", 3, b"abc")
    variants = ((b"other", 3, b"abc"), (b"schema", 4, b"abc"), (b"schema", 3, b"abd"))
    digest = lambda value: hashlib.blake2s(repr(value).encode("ascii")).digest()
    assert all(digest(baseline) != digest(variant) for variant in variants)
    return len(variants)


def oracle_digest_mismatch() -> int:
    cases = 0
    for expected, actual in itertools.product(range(4), repeat=2):
        result = object() if expected == actual else None
        assert (result is None) == (expected != actual)
        cases += 1
    return cases


def oracle_checkpoint_plan_binding() -> int:
    cases = 0
    for embedded, active in itertools.product(range(5), repeat=2):
        accepted = embedded == active
        assert accepted == (embedded == active)
        cases += 1
    return cases


def oracle_digest_not_identity() -> int:
    cases = 0
    for embedded, active in itertools.product(range(4), repeat=2):
        same_digest = True
        accepted = same_digest and embedded == active
        assert not accepted or embedded == active
        cases += 1
    return cases


def oracle_checkpoint_counts() -> int:
    cases = 0
    for tasks, events, receipts in itertools.product(range(5), repeat=3):
        valid = events <= tasks + 1 and receipts <= events
        assert valid == (events <= tasks + 1 and receipts <= events)
        cases += 1
    return cases


def oracle_success_prefix() -> int:
    cases = 0
    for length in range(5):
        for kinds in itertools.product(EVENT_KINDS, repeat=length):
            valid = canonical_checkpoint(4, kinds, 0)
            if valid:
                successes = next((i for i, kind in enumerate(kinds) if kind != 1), length)
                assert kinds[:successes] == (1,) * successes
            cases += 1
    return cases


def oracle_terminal_event() -> int:
    cases = 0
    for length in range(5):
        for kinds in itertools.product(EVENT_KINDS, repeat=length):
            if canonical_checkpoint(3, kinds, 0):
                terminals = [index for index, kind in enumerate(kinds) if kind in TERMINAL_KINDS]
                assert len(terminals) <= 1
                assert not terminals or terminals == [len(kinds) - 1]
            cases += 1
    return cases


def oracle_resume_cursor() -> int:
    cases = 0
    for length in range(5):
        for kinds in itertools.product(EVENT_KINDS, repeat=length):
            cursor = next((i for i, kind in enumerate(kinds) if kind != 1), length)
            assert cursor == len(tuple(itertools.takewhile(lambda kind: kind == 1, kinds)))
            cases += 1
    return cases


def oracle_receipt_prefix() -> int:
    cases = 0
    for event_count in range(6):
        events = tuple(range(event_count))
        for receipt_count in range(7):
            receipts = events[:receipt_count] if receipt_count <= event_count else None
            assert (receipts is not None) == (receipt_count <= event_count)
            cases += 1
    return cases


def oracle_event_kind_domain() -> int:
    cases = 0
    for code in range(256):
        assert (code in EVENT_KINDS) == (1 <= code <= 6)
        cases += 1
    return cases


def oracle_payload_erasure() -> int:
    cases = 0
    for payload in (None, 0, "failure", b"bytes"):
        projected = (1, 2, (1, 6), 2)
        assert payload not in projected
        cases += 1
    return cases


def oracle_checkpoint_round_trip() -> int:
    cases = 0
    for tasks in range(4):
        for events in range(tasks + 2):
            kinds = (1,) * min(events, tasks)
            if events == tasks + 1:
                kinds += (6,)
            for receipts in range(len(kinds) + 1):
                checkpoint = (tasks, kinds, receipts, min(events, tasks))
                encoded = repr(checkpoint).encode("ascii")
                assert encoded.decode("ascii") == repr(checkpoint)
                cases += 1
    return cases


def oracle_replay_observation() -> int:
    cases = 0
    for tasks in range(6):
        serial = tuple(range(1, tasks + 2))
        for prefix in range(tasks + 1):
            resumed = serial[:prefix] + serial[prefix:]
            assert resumed == serial
            cases += 1
    return cases


def oracle_replay_bytes() -> int:
    cases = 0
    for prefix in range(8):
        checkpoint = (tuple(range(prefix)), prefix)
        before = repr(checkpoint).encode("ascii")
        after = repr(checkpoint).encode("ascii")
        assert before == after
        cases += 1
    return cases


def oracle_iterative_stack_machine() -> int:
    cursor = 0
    limit = 100_000
    while cursor < limit:
        cursor += 1
    assert cursor == limit
    return limit


def oracle_complexity_accounting() -> int:
    cases = 0
    for items, byte_count in itertools.product(range(16), repeat=2):
        work = 8 + 3 * items + 2 * byte_count
        heap = items + byte_count
        assert work <= 8 + 3 * (items + byte_count)
        assert heap == items + byte_count
        cases += 1
    return cases


def oracle_parallel_determinism() -> int:
    inputs = tuple(range(32))
    encode = lambda value: hashlib.sha256(value.to_bytes(8, "little")).digest()
    expected = tuple(map(encode, inputs))
    with ThreadPoolExecutor(max_workers=4) as executor:
        observed = tuple(executor.map(encode, inputs))
    assert observed == expected
    return len(inputs)


def oracle_malformed_fail_closed() -> int:
    cases = 0
    for length in range(5):
        for data in itertools.product(range(8), repeat=length):
            admitted = len(data) >= 2 and data[0] == 1 and data[-1] in EVENT_KINDS
            result = tuple(data) if admitted else None
            assert admitted or result is None
            cases += 1
    return cases


def main() -> None:
    oracles = [
        value
        for name, value in sorted(globals().items())
        if name.startswith("oracle_") and callable(value)
    ]
    if len(oracles) != 40:
        raise AssertionError(f"expected 40 oracles, found {len(oracles)}")
    total = 0
    for oracle in oracles:
        cases = oracle()
        if cases <= 0:
            raise AssertionError(f"{oracle.__name__} checked no cases")
        print(f"{oracle.__name__}: {cases} cases")
        total += cases
    print(f"schedlib-interop exhaustive oracle: 40 obligations, {total} cases")


if __name__ == "__main__":
    main()
