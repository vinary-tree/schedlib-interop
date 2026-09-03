#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
evidence_directory="$repository_root/target/verification"
log_directory="$evidence_directory/logs"
temporary_directory="$evidence_directory/tmp"
coq_directory="$evidence_directory/coq-integrated"
cargo_directory="$evidence_directory/cargo"

mkdir -p "$log_directory" "$temporary_directory" "$coq_directory" "$cargo_directory"

if [[ "${SCHEDLIB_INTEROP_FORMAL_SCOPED:-0}" != "1" ]]; then
  exec systemd-run --user --scope \
    -p MemoryMax=4G \
    -p MemorySwapMax=0 \
    -p CPUQuota=100% \
    -p TasksMax=64 \
    --setenv=SCHEDLIB_INTEROP_FORMAL_SCOPED=1 \
    --setenv=CARGO_BUILD_JOBS=1 \
    --setenv=CARGO_TARGET_DIR="$cargo_directory" \
    --setenv=TMPDIR="$temporary_directory" \
    --setenv=JAVA_TOOL_OPTIONS="-Xmx1024m -XX:+UseParallelGC -Djava.awt.headless=true -Djava.io.tmpdir=$temporary_directory" \
    -- "$repository_root/scripts/verify-formal.sh"
fi

export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR="$cargo_directory"
export TMPDIR="$temporary_directory"
export JAVA_TOOL_OPTIONS="${JAVA_TOOL_OPTIONS:--Xmx1024m -XX:+UseParallelGC -Djava.awt.headless=true -Djava.io.tmpdir=$temporary_directory}"

require_tool() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "required formal tool is unavailable: $1" >&2
    exit 1
  fi
}

for tool in cargo coqc coqchk diff python3 rg rustfmt tail tla2sany tlc z3; do
  require_tool "$tool"
done

python3 "$repository_root/scripts/check-traceability.py" \
  > "$log_directory/traceability.log" 2>&1
cat "$log_directory/traceability.log"

rustfmt --edition 2021 --check \
  "$repository_root/src/lib.rs" \
  "$repository_root/tests/contracts.rs" \
  > "$log_directory/rustfmt.log" 2>&1

cp "$repository_root/formal/coq/SchedlibInterop.v" "$coq_directory/SchedlibInterop.v"
cp "$repository_root/formal/coq/Assumptions.v" "$coq_directory/Assumptions.v"
(
  cd "$coq_directory"
  coqc -Q . SchedlibInterop SchedlibInterop.v
) > "$log_directory/coq.log" 2>&1
(
  cd "$coq_directory"
  coqc -Q . SchedlibInterop Assumptions.v
) > "$log_directory/coq-assumptions.log" 2>&1
if rg -n '^[[:space:]]*(Axiom|Conjecture|Admitted|admit|Abort)\b' \
    "$repository_root/formal/coq/SchedlibInterop.v" \
    "$repository_root/formal/coq/Assumptions.v" \
    > "$log_directory/coq-escapes.log"; then
  echo "proof escape found in Rocq source" >&2
  exit 1
fi
closed_count="$(rg -c '^Closed under the global context$' "$log_directory/coq-assumptions.log")"
if [[ "$closed_count" -ne 40 ]]; then
  echo "expected 40 closed Rocq assumption reports; observed $closed_count" >&2
  exit 1
fi
(
  cd "$coq_directory"
  coqchk -Q . SchedlibInterop SchedlibInterop.SchedlibInterop
) > "$log_directory/coq-kernel.log" 2>&1
rg -q '^Modules were successfully checked$' "$log_directory/coq-kernel.log"
echo "Rocq kernel accepted 40 closed obligations."

(
  cd "$repository_root/formal/tla"
  tla2sany SchedlibInterop.tla
) > "$log_directory/tla-syntax.log" 2>&1

for scenario in Valid HeaderReject LimitReject Cancel DigestReject ForeignPlan Malformed; do
  metadata_directory="$evidence_directory/tlc-$scenario"
  case "$metadata_directory" in
    "$repository_root"/target/verification/tlc-*)
      rm -rf "$metadata_directory"
      ;;
    *)
      echo "refusing to clean unexpected TLC path: $metadata_directory" >&2
      exit 1
      ;;
  esac
  mkdir -p "$metadata_directory"
  scenario_log="$log_directory/tlc-$scenario.log"
  (
    cd "$repository_root/formal/tla"
    tlc -workers 1 \
      -metadir "$metadata_directory" \
      -config "$scenario.cfg" \
      SchedlibInterop.tla
  ) > "$scenario_log" 2>&1
  rg -q '^Model checking completed\. No error has been found\.$' "$scenario_log"
  rg -q ' distinct states found, 0 states left on queue\.$' "$scenario_log"
done
echo "TLA+ completed seven exhaustive safety/liveness scenarios."

z3 -smt2 "$repository_root/formal/smt/schedlib-interop.smt2" \
  > "$log_directory/z3.log" 2>&1
diff -u "$repository_root/formal/smt/schedlib-interop.expected" \
  "$log_directory/z3.log" > "$log_directory/z3-diff.log"
verdict_count="$(rg -c '^(sat|unsat|unknown)$' "$log_directory/z3.log")"
unsat_count="$(rg -c '^unsat$' "$log_directory/z3.log")"
sat_count="$(rg -c '^sat$' "$log_directory/z3.log")"
if [[ "$verdict_count" -ne 42 || "$unsat_count" -ne 39 || "$sat_count" -ne 3 ]]; then
  echo "unexpected SMT verdict population" >&2
  exit 1
fi
if rg -q '^unknown$' "$log_directory/z3.log"; then
  echo "SMT solver returned unknown" >&2
  exit 1
fi
echo "SMT matched 39 unsatisfiable obligations and three satisfiable controls."

python3 "$repository_root/scripts/check-exhaustive.py" \
  > "$log_directory/exhaustive.log" 2>&1
tail -n 1 "$log_directory/exhaustive.log"
python3 "$repository_root/scripts/check-mutants.py" \
  > "$log_directory/mutants.log" 2>&1
tail -n 1 "$log_directory/mutants.log"

source_commit="$(<"$repository_root/formal/source.commit")"
if ! rg -Fq "#$source_commit" "$repository_root/Cargo.lock"; then
  echo "Cargo.lock does not resolve the exact formal schedlib source commit" >&2
  exit 1
fi

ledger="$repository_root/formal/invariants.tsv"
contract_log="$log_directory/required-red.log"
if rg -q $'\trequired-before-implementation$' "$ledger"; then
  expected_mode="red"
else
  expected_mode="green"
fi
set +e
if [[ "$expected_mode" == "red" ]]; then
  cargo test --locked --offline --test contracts --no-run \
    > "$contract_log" 2>&1
else
  cargo test --locked --offline --release --test contracts \
    > "$contract_log" 2>&1
fi
status="$?"
set -e
if [[ "$expected_mode" == "red" ]]; then
  if [[ "$status" -ne 101 ]]; then
    echo "required-red contract returned Cargo status $status instead of 101" >&2
    exit 1
  fi
  if rg -qi 'failed to download|network failure|could not resolve host|timed out while fetching' \
      "$contract_log"; then
    echo "required-red contract failed because of dependency transport" >&2
    exit 1
  fi
  if ! rg -Fq 'unresolved imports `schedlib_interop::decode_checkpoint_for`' \
      "$contract_log"; then
    echo "required-red contract did not fail at the reviewed missing API" >&2
    exit 1
  fi
  if ! rg -Fq 'due to 1 previous error' "$contract_log"; then
    echo "required-red contract has an unexpected secondary compile failure" >&2
    exit 1
  fi
  echo "Validated all 40 causal required-red interop properties."
elif [[ "$status" -ne 0 ]]; then
  echo "accepted interop properties failed with Cargo status $status" >&2
  exit 1
else
  echo "Validated all 40 causal postimplementation interop properties."
fi

placeholder_log="$log_directory/unfinished-markers.log"
paths=(
  "$repository_root/src"
  "$repository_root/tests/contracts.rs"
  "$repository_root/formal"
  "$repository_root/scripts/check-exhaustive.py"
  "$repository_root/scripts/check-mutants.py"
  "$repository_root/scripts/check-traceability.py"
)
if rg -n '\b(TODO|FIXME|HACK|XXX|placeholder|stub|workaround|unimplemented)\b' \
    "${paths[@]}" > "$placeholder_log"; then
  echo "unfinished marker found in normative interop artifacts" >&2
  exit 1
fi
echo "Unfinished-marker audit passed."
echo "schedlib-interop formal verification completed successfully."
