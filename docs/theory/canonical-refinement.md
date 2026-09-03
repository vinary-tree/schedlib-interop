# Canonical representation and refinement

## Semantic source of truth

Let $`P`$ denote schedlib's exact structural plan and let $`E(P)`$ denote its
canonical byte encoding under one identified key codec. Canonicality requires
both directions below:

```math
D(E(P)) = P
```

```math
E(D(b)) = b \quad\text{for every admitted frame } b.
```

The first equation is semantic preservation. The second prevents multiple
accepted byte strings from representing the same plan. The decoder therefore
checks fixed headers, exact lengths, UTF-8, key round trips, strict key and
resource order, strict dependency order, event grammar, and all aggregate
counts before publication.

## Identity before digest

A 256-bit digest is finite, while the plan domain is not. Digest equality can
therefore be an integrity or lookup control but cannot prove semantic equality.
Checkpoint admission always decodes the complete embedded plan and compares
its seven structural fields with the active schedlib plan.

BLAKE3 is used in derive-key mode with separate immutable plan and checkpoint
contexts. The design follows the [BLAKE3 specification and rationale](https://github.com/BLAKE3-team/BLAKE3-specs),
but collision resistance remains a security assumption outside the exact
refinement proof.

## Iterative refinement machine

The codec is a deterministic cursor machine. Every transition advances a byte
or item cursor, rejects, or publishes. The finite measure is the sum of
unvisited frame bytes and unvisited declared items. No transition invokes
itself, so native call-stack use is constant in plan and checkpoint depth.

Temporal ordering is checked independently with the Temporal Logic of Actions
(TLA+). The underlying logic is described by Lamport in
[The Temporal Logic of Actions](https://doi.org/10.1145/177492.177726).
