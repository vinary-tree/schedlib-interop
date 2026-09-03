# schedlib-interop

`schedlib-interop` is the portable boundary for
[`schedlib`](https://github.com/vinary-tree/schedlib). It will encode exact
structural plan identities and committed-prefix checkpoints without moving
serialization, hashing, filesystem policy, or runtime storage into the
scheduling kernel.

## Current verification state

Production codec APIs are deliberately absent while the preimplementation
contract is reviewed. The contract fixes forty independently traceable
obligations spanning the wire grammar, checked admission, canonicality, exact
plan comparison, key-codec laws, checkpoint reconstruction, digest separation,
replay refinement, constant native stack, and linear resource use.

Run the complete formal gate with:

```sh
make verify-formal
```

The gate is valid only when the theorem checkers and bounded models pass and
the Rust suite fails solely because the reviewed production API is absent.
After implementation, the same properties must pass without changing their
names or weakening their assertions.

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
[the formal evidence guide](formal/README.md).
