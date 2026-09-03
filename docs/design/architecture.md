# Architecture

## Responsibility rule

The dependency direction is intentional:

```text
schedlib semantic values -> schedlib-interop canonical bytes -> runtime artifacts
```

schedlib owns exact plan and checkpoint meaning. schedlib-interop borrows that
meaning through immutable views, validates portable bytes, and reconstructs
semantic objects through checked constructors. Runtime crates may store or
transport the bytes, but they do not redefine the grammar or acceptance rules.

## Admission components

The [admission-pipeline diagram](figures/admission-pipeline.svg) shows the full
path. Fixed-header admission runs before variable allocation. A scan pass then
checks exact boundaries and ordering without constructing a semantic result.
Only an admitted frame reserves output collections and enters the construction
pass. Exact active-plan comparison occurs before checkpoint publication.

## Dependency policy

The production dependency surface is deliberately small:

- schedlib provides semantic types and validation;
- BLAKE3 provides domain-separated digests; and
- the Rust standard library provides fixed-width byte operations and atomics.

Property testing is a development-only dependency. Filesystem APIs,
asynchronous runtimes, thread pools, global registries, and serialization
frameworks do not enter the crate.

## Concurrency

Every operation is pure over borrowed immutable inputs except for sampling an
optional atomic cancellation flag. There is no global sequence number, cache,
or mutable singleton. Independent invocations can therefore run concurrently;
equal inputs produce byte-identical outputs and equal typed errors.
