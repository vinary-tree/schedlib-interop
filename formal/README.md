# Formal contract

## Purpose

The formal layer fixes the interoperability semantics before codec
implementation. Its source boundary is schedlib commit
`b015bd1d494fb4f247983ada64b0402f56bea1fe`, recorded in
[`source.commit`](source.commit). The implementation may request additional
borrowed semantic views from schedlib, but it may not change the durable
protocol described by that commit.

## Evidence ladder

The forty-row [`invariants.tsv`](invariants.tsv) ledger maps every normative
claim to five independently reviewable forms:

1. a Rocq theorem or explicit non-applicability statement;
2. a configured TLA+ safety or liveness predicate;
3. an SMT control or explicit non-applicability statement;
4. a finite executable oracle and a causal mutant; and
5. an exact Rust refinement property frozen before implementation.

No one layer is treated as a substitute for the others. Rocq checks abstract
functional arguments. TLA+ explores phase ordering, cancellation, and
publication. SMT checks bounded arithmetic and discriminator controls. Python
enumerates independent finite domains and kills declared faults. Rust fixes the
concrete public behavior that production must satisfy.

## Acceptance states

Every row initially carries `required-before-implementation`. The verifier
rejects mixed states. After the complete Rust suite passes against production,
all rows change together to `accepted@COMMIT`, where `COMMIT` is a full
forty-hexadecimal implementation commit that resolves locally. Merely editing
formal artifacts cannot satisfy this transition.

## Resource discipline

All verification scripts use disk-backed directories below `target/`. The
top-level script re-executes itself in a systemd scope with a 4 GiB memory
ceiling, no swap, one CPU, and at most sixty-four tasks. TLC runs headlessly
with one worker and a 1 GiB Java heap. Cargo uses one build job.

## Run the suite

```sh
make verify-formal
```

Inspect captured logs below `target/verification/logs/` before removing build
evidence.
