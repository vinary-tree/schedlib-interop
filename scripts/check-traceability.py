#!/usr/bin/env python3
"""Validate exact bidirectional interop ledger traceability."""

from __future__ import annotations

import csv
import re
import subprocess
import sys
from pathlib import Path

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parents[1]
LEDGER = ROOT / "formal/invariants.tsv"
ROCQ = ROOT / "formal/coq/SchedlibInterop.v"
TLA = ROOT / "formal/tla/SchedlibInterop.tla"
SMT = ROOT / "formal/smt/schedlib-interop.smt2"
ORACLES = ROOT / "scripts/check-exhaustive.py"
MUTANTS = ROOT / "scripts/check-mutants.py"
CONTRACTS = ROOT / "tests/contracts.rs"
MANIFEST = ROOT / "Cargo.toml"
SOURCE_COMMIT = ROOT / "formal/source.commit"

COLUMNS = (
    "id",
    "kind",
    "statement",
    "rocq-obligation",
    "tla-obligation",
    "smt-obligation",
    "executable-oracle",
    "required-red-test",
    "causal-mutant",
    "acceptance-state",
)


def fail(message: str) -> None:
    print(f"ERROR: {message}", file=sys.stderr)
    raise SystemExit(1)


def unique(values: list[str], label: str) -> set[str]:
    if len(values) != len(set(values)):
        fail(f"duplicate {label}")
    return set(values)


def named_set(rows: list[dict[str, str]], column: str) -> set[str]:
    values: list[str] = []
    for row in rows:
        values.extend(
            value.strip()
            for value in row[column].split(";")
            if value.strip() != "not-applicable"
        )
    return set(values)


def require_named(text: str, names: set[str], label: str) -> None:
    missing = sorted(name for name in names if name not in text)
    if missing:
        fail(f"{label} is missing ledger names: {missing}")


def main() -> None:
    with LEDGER.open(encoding="utf-8", newline="") as source:
        reader = csv.DictReader(source, delimiter="\t")
        if tuple(reader.fieldnames or ()) != COLUMNS:
            fail(f"unexpected ledger columns: {reader.fieldnames}")
        rows = list(reader)
    if len(rows) != 40:
        fail(f"expected 40 invariant rows, found {len(rows)}")
    for index, row in enumerate(rows, start=2):
        if any(not row[column] for column in COLUMNS):
            fail(f"ledger row {index} contains an empty field")

    unique([row["id"] for row in rows], "invariant identifier")
    expected_oracles = unique(
        [row["executable-oracle"] for row in rows], "oracle identifier"
    )
    expected_tests = unique(
        [row["required-red-test"] for row in rows], "Rust property identifier"
    )
    expected_mutants = unique(
        [row["causal-mutant"] for row in rows], "mutant identifier"
    )

    states = {row["acceptance-state"] for row in rows}
    preimplementation = states == {"required-before-implementation"}
    accepted = all(
        re.fullmatch(r"accepted@[0-9a-f]{40}", state) for state in states
    )
    if not preimplementation and not accepted:
        fail("ledger must be uniformly preimplementation or commit-pinned accepted")
    if accepted:
        for state in states:
            commit = state.removeprefix("accepted@")
            result = subprocess.run(
                ["git", "cat-file", "-e", f"{commit}^{{commit}}"],
                cwd=ROOT,
                check=False,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            if result.returncode != 0:
                fail(f"accepted implementation commit is unavailable: {commit}")

    require_named(
        ROCQ.read_text(encoding="utf-8"),
        named_set(rows, "rocq-obligation"),
        "Rocq theory",
    )
    require_named(
        TLA.read_text(encoding="utf-8"),
        named_set(rows, "tla-obligation"),
        "TLA+ model",
    )
    require_named(
        SMT.read_text(encoding="utf-8"),
        named_set(rows, "smt-obligation"),
        "SMT model",
    )

    oracle_text = ORACLES.read_text(encoding="utf-8")
    actual_oracles = unique(
        re.findall(r"^def (oracle_[a-z0-9_]+)\(\) -> int:$", oracle_text, re.MULTILINE),
        "executable oracle",
    )
    mutant_text = MUTANTS.read_text(encoding="utf-8")
    actual_mutants = unique(
        re.findall(r'^    "(mutant_[a-z0-9_]+)": Mutation\($', mutant_text, re.MULTILINE),
        "executable mutant",
    )
    contract_text = CONTRACTS.read_text(encoding="utf-8")
    actual_tests = unique(
        re.findall(
            r"^fn ((?:prop|small_stack)_[a-z0-9_]+)\(\) \{$",
            contract_text,
            re.MULTILINE,
        ),
        "Rust property",
    )
    for label, expected, actual in (
        ("oracle", expected_oracles, actual_oracles),
        ("mutant", expected_mutants, actual_mutants),
        ("Rust property", expected_tests, actual_tests),
    ):
        if expected != actual:
            fail(
                f"{label} mismatch; missing={sorted(expected - actual)}, "
                f"extra={sorted(actual - expected)}"
            )
    for test in actual_tests:
        pattern = rf"#\[test\]\s+fn {re.escape(test)}\(\)"
        if len(re.findall(pattern, contract_text)) != 1:
            fail(f"Rust property must have exactly one #[test] attribute: {test}")

    source_commit = SOURCE_COMMIT.read_text(encoding="ascii").strip()
    if re.fullmatch(r"[0-9a-f]{40}", source_commit) is None:
        fail("formal/source.commit must contain one full Git commit")
    manifest = MANIFEST.read_text(encoding="utf-8")
    if f'rev = "{source_commit}"' not in manifest:
        fail("schedlib dependency revision differs from formal/source.commit")

    mode = "preimplementation" if preimplementation else "postimplementation"
    print(
        f"schedlib-interop traceability validated in {mode} mode: "
        "40 ledger rows, 40 Rocq names, 40 oracles, 40 mutants, "
        "and 40 Rust properties"
    )


if __name__ == "__main__":
    main()
