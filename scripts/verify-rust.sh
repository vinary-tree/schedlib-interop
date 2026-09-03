#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
evidence_directory="$repository_root/target/acceptance"
temporary_directory="$evidence_directory/tmp"
cargo_target_directory="$evidence_directory/cargo"
msrv_target_directory="$evidence_directory/cargo-msrv"
cargo_home_directory="$repository_root/target/cargo-home"
mkdir -p "$temporary_directory" "$cargo_target_directory" \
  "$msrv_target_directory" "$cargo_home_directory"

if [[ "${SCHEDLIB_INTEROP_RUST_SCOPED:-0}" != "1" ]]; then
  exec systemd-run --user --scope \
    -p MemoryMax=4G \
    -p MemorySwapMax=0 \
    -p CPUQuota=100% \
    -p TasksMax=64 \
    --setenv=SCHEDLIB_INTEROP_RUST_SCOPED=1 \
    --setenv=CARGO_BUILD_JOBS=1 \
    --setenv=CARGO_HOME="$cargo_home_directory" \
    --setenv=CARGO_TARGET_DIR="$cargo_target_directory" \
    --setenv=TMPDIR="$temporary_directory" \
    -- "$repository_root/scripts/verify-rust.sh"
fi

export CARGO_BUILD_JOBS=1
export CARGO_HOME="$cargo_home_directory"
export CARGO_TARGET_DIR="$cargo_target_directory"
export TMPDIR="$temporary_directory"

run_gate() {
  local name="$1"
  shift
  set +e
  "$@" 2>&1 | tee "$evidence_directory/$name.log"
  local status="${PIPESTATUS[0]}"
  set -e
  if [[ "$status" -ne 0 ]]; then
    return "$status"
  fi
}

run_gate cargo-fmt cargo fmt --all -- --check
run_gate cargo-check cargo check --locked --all-targets
run_gate cargo-check-msrv env CARGO_TARGET_DIR="$msrv_target_directory" \
  cargo +1.85.0 check --locked --all-targets
run_gate cargo-clippy cargo clippy --locked --all-targets -- -D warnings
run_gate cargo-test-debug cargo test --locked --all-targets
run_gate cargo-test-release cargo test --locked --all-targets --release
run_gate cargo-run-example cargo run --locked --release --example canonical_roundtrip
run_gate cargo-test-doc cargo test --locked --doc
run_gate cargo-doc env RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps
run_gate cargo-package cargo package --locked --allow-dirty --no-verify
