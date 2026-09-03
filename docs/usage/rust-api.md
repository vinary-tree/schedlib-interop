# Rust API usage

## Encode and decode a bounded plan

This complete example uses stable `u64` task keys and the built-in
order-preserving key codec:

```rust
use schedlib::durable::PlanIdentity;
use schedlib_interop::{decode_plan, encode_plan, CodecLimits, U64KeyCodec};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let plan = PlanIdentity::new(
        1,
        vec![10_u64, 20],
        vec![(10, 20)],
        vec![(vec![1], vec![2]), (vec![3], vec![4])],
        vec![1, 1],
        2,
        String::from("example-v1"),
    )?;
    let limits = CodecLimits {
        max_bytes: 16 * 1024,
        max_tasks: 16,
        max_dependencies: 32,
        max_resources: 64,
        max_key_bytes: 256,
        max_profile_bytes: 256,
        max_events: 17,
    };
    let bytes = encode_plan(&plan, &U64KeyCodec, limits)?;
    let decoded = decode_plan(&bytes, &U64KeyCodec, limits)?;
    assert_eq!(decoded, plan);
    Ok(())
}
```

The same source is compiled and executed from
[`examples/canonical_roundtrip.rs`](../../examples/canonical_roundtrip.rs).

## Decode a recovery checkpoint

Use `decode_checkpoint_for` when a checkpoint will control recovery. It decodes
the embedded plan and confirms exact equality with the supplied active plan.
`decode_verified_checkpoint_for` adds an expected complete-frame digest but
retains the structural comparison.

## Define a key codec

Implement `CanonicalKeyCodec<K>` with a stable `KeyCodecId`, exact
`encoded_len`, append-only `encode_into`, and total rejection of invalid byte
strings in `decode`. The boundary verifies both equations for every key:

```math
decode(encode(k))=k
```

```math
encode(decode(b))=b.
```

Changing a key interpretation requires a new codec identity. Reusing an
identity for different bytes or semantics can make persisted plans ambiguous
and is forbidden.

## Apply cancellation and work limits

Use the controlled functions when input is externally supplied. A work limit
is checked before variable decoding or output allocation. An optional atomic
flag is sampled during bounded scan and digest chunks. Every error path returns
no `CodecReport`, so callers cannot observe a partial semantic value.
