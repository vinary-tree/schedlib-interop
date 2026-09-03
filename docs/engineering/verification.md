# Engineering and verification

## Algorithms and bounds

Let $`n`$ be tasks, $`m`$ dependencies, $`r`$ total resource occurrences,
$`j`$ checkpoint events, and $`b`$ complete frame bytes. Encoding measures
fixed-width lengths before allocating, reserves the returned byte vector once,
and writes each semantic field once. Decoding uses one allocation-free scan and
one construction pass:

```math
T(n,m,r,j,b)=O(n+m+r+j+b)
```

```math
S(n,m,r,j,b)=O(n+m+r+j+b).
```

Key-codec callback work and heap owned inside a decoded key are caller costs;
the codec still invokes each key operation a constant number of times.

## Stack safety

All grammar state is held in flat byte cursors and heap-resident vectors. Task,
dependency, resource, key, profile, and event loops are iterative. Hashing is
fed in bounded chunks. The acceptance suite encodes and decodes 100,000 tasks
from a thread with a 64 KiB native stack.

## Validation precedence

The decoder follows this order:

1. enforce the complete byte limit;
2. parse and validate the fixed header without allocation;
3. use checked arithmetic for the exact frame end and logical work;
4. enforce declared count and work limits;
5. validate the variable grammar with an allocation-free forward scan;
6. allocate exact output capacities and construct the schedlib value;
7. compare an embedded checkpoint plan with the active plan; and
8. sample cancellation and publish one complete result.

Per-row resource and key counts may not exceed the remaining admitted header
totals. This prevents a dishonest inner count from inducing work omitted by the
admission calculation.

## Acceptance commands

Run all local gates with disk-backed evidence beneath `target/`:

```sh
make verify
```

The formal gate checks forty ledger rows, forty closed Rocq obligations, seven
TLA+ scenarios, forty-two SMT controls, 115,308 exhaustive oracle cases, forty
causal mutants, and the frozen Rust contract. The Rust gate checks formatting,
the Rust 1.85 minimum supported version, strict Clippy, debug and release
tests, examples, doctests, rustdoc, and packaging. The documentation gate
renders PlantUML headlessly and runs `vinary-doc-lint` in check-only mode.

Cargo normalizes the Git-pinned schedlib dependency to the declared registry
version while preparing a publishable archive. Therefore the release sequence
is strict: publish schedlib 0.1.0 first, then run the schedlib-interop package
gate and publish schedlib-interop. A missing registry dependency is a release
failure, never a skipped package check.
