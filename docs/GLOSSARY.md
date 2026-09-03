# Glossary

Canonical frame
: The unique admitted byte representation of one semantic plan or checkpoint.

Checkpoint
: A payload-free schedlib recovery value containing exact plan identity, an
  ordered event-kind prefix, a publication-prefix length, and a derived resume
  cursor.

Codec identity
: The immutable 256-bit identifier that binds key bytes to one interpretation.

Dense task identifier
: A zero-based `u32` position assigned by schedlib after canonical key ordering.
  It is derived state, not an application key.

Domain separation
: Use of distinct BLAKE3 derive-key contexts so equal input bytes in the plan
  and checkpoint domains have different digest invocations.

Exact structural identity
: Equality of plan schema, keys, dependencies, effects, costs, budget, and
  semantic profile. A digest is never a substitute for this equality.

Publication
: The single return boundary at which a completely validated value becomes
  observable to the caller.

Retained reservation
: A capacity reservation belonging to a collection retained in a successful
  result. Scratch buffers are not retained reservations.

Semantic profile
: Exact UTF-8 text selected by the caller to distinguish execution semantics
  that otherwise share equal structural fields.

Work unit
: A deterministic count of admitted bytes and structural items. It is not
  elapsed time or a processor instruction count.
