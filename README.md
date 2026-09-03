# schedlib-interop

`schedlib-interop` is the portable boundary for
[`schedlib`](https://github.com/vinary-tree/schedlib). It encodes exact
structural plan identities and committed-prefix checkpoints without moving
serialization, hashing, filesystem policy, or runtime storage into the
scheduling kernel.

## Current verification state

The production codec implements the previously frozen forty-row contract. The
contract fixes forty independently traceable
obligations spanning the wire grammar, checked admission, canonicality, exact
plan comparison, key-codec laws, checkpoint reconstruction, digest separation,
replay refinement, constant native stack, and linear resource use.

Run the complete formal gate with:

```sh
make verify-formal
```

The theorem checkers, bounded models, executable oracles, causal mutants, and
production Rust properties must all pass without weakening their assertions.

## Architectural boundary

The crate owns:

- versioned canonical bytes for complete schedlib plan identity;
- versioned canonical bytes for checkpoint event kinds and receipt prefixes;
- caller-selected, domain-identified canonical key codecs;
- domain-separated BLAKE3 digests;
- bounded, cancellable, iterative admission and decoding; and
- conversion to validated schedlib semantic objects.

It does not own filesystem paths, atomic replacement, retention, locking,
network transport, or application payload serialization. Those policies
belong to runtime adapters.

See [the normative wire contract](docs/design/wire-contract.md) and
[the documentation index](docs/README.md).
