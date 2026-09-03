# Scientific evidence

## Independent evidence ladder

The forty-row invariant ledger prevents any one verification technique from
standing in for all others:

| Layer | Question answered |
|---|---|
| Rocq kernel | Does the abstract functional obligation follow without admitted assumptions? |
| TLA+ | Do all bounded concurrent phase interleavings preserve safety and termination? |
| SMT | Are fixed-width arithmetic, discriminator, and admission counterexamples unsatisfiable? |
| Exhaustive oracle | Do independently implemented finite cases agree with every named property? |
| Causal mutation | Does each property fail when its specific fault is introduced? |
| Rust refinement | Does production behavior satisfy the frozen public contract? |

The layers use different representations so a shared implementation mistake is
less likely to make every check pass for the same wrong reason.

## Property-based testing

The production suite uses shrinking generators for canonical plans, arbitrary
byte strings, single-byte mutations, and checkpoint prefixes. Successful
decodes must re-encode byte-identically; arbitrary malformed bytes must never
panic or expose a partial value. The method follows the property-testing model
introduced by Claessen and Hughes in
[QuickCheck](https://doi.org/10.1145/351240.351266), while the frozen exhaustive
and mutation suites retain deterministic coverage of security boundaries.

The test dependency disables process-fork, timeout, and tempfile features.
This keeps the suite headless and prevents property tests from using
memory-backed temporary storage.

## Reproducibility

The accepted production refinement is commit
`269d8d137717c7c0a53e9418c5d52e8366967783`. All forty invariant-ledger rows
name that exact commit. The complete post-implementation formal transcript has
SHA-256
`4e6413ef0d344a661fac51338ee228f85d8cd2303f13bc17aa5ae8c7853656a8`.

Validation output is captured before inspection. Formal model metadata,
compiler targets, and logs live below `target/verification/`; Rust acceptance
logs live below `target/acceptance/`; documentation evidence lives below
`target/documentation/`. These generated artifacts are excluded from source
packages and can be removed after their hashes and verdicts are recorded.
