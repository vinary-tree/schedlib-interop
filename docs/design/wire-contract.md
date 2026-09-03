# Version 1 wire contract

## Goals and trust boundary

The wire format preserves schedlib's exact structural identity across process,
host, and storage boundaries. Input bytes, declared counts, key codecs, and
digests are untrusted. An admitted object is returned only after the complete
input has passed structural, canonical, resource, codec, and identity checks.
No partial plan or checkpoint is observable on rejection or cancellation.

All multibyte integers use little-endian fixed-width representations. No
`usize`, pointer width, Rust enum layout, hash-map order, or allocator detail is
part of the wire grammar.

## Canonical plan frame

The fixed 112-byte header is:

| Offset | Width | Field |
|---:|---:|---|
| 0 | 8 | ASCII `SCHPLN`, then octets `0x00` and `0x01` |
| 8 | 16 | schema `SCHED-PLAN-V1!!!` |
| 24 | 2 | major version, value 1 |
| 26 | 2 | minor version, value 0 |
| 28 | 4 | reserved flags, value 0 |
| 32 | 32 | caller key-codec identity |
| 64 | 8 | schedlib plan schema |
| 72 | 4 | task count |
| 76 | 4 | dependency count |
| 80 | 8 | total read/write resource entries |
| 88 | 8 | total encoded key bytes |
| 96 | 8 | semantic-profile byte length |
| 104 | 8 | payload byte length |

The payload begins with the 64-bit plan budget followed by the exact UTF-8
semantic profile. Each task then contains a 64-bit key length, exact key bytes,
a 32-bit read count followed by 64-bit resource identifiers, a 32-bit write
count followed by identifiers, and a 64-bit cost. The final dependency section
contains sorted unique pairs of 32-bit dense task identifiers.

Task order is schedlib's canonical key order. Read and write sets are strictly
increasing. Dependencies are lexicographically increasing. The header totals,
per-task counts, payload length, and complete frame length must agree exactly.
Trailing bytes are rejected.

## Canonical checkpoint frame

The fixed 96-byte checkpoint header is:

| Offset | Width | Field |
|---:|---:|---|
| 0 | 8 | ASCII `SCHCKP`, then octets `0x00` and `0x01` |
| 8 | 16 | schema `SCHED-CKPT-V1!!!` |
| 24 | 2 | major version, value 1 |
| 26 | 2 | minor version, value 0 |
| 28 | 4 | reserved flags, value 0 |
| 32 | 8 | embedded canonical-plan byte length |
| 40 | 8 | ordered event count |
| 48 | 8 | published-prefix count |
| 56 | 8 | payload byte length |
| 64 | 32 | digest of the embedded canonical plan |

The payload is the complete canonical plan frame followed by one byte per
checkpoint event. Event discriminators are 1 for success, 2 for failure, 3 for
incomplete, 4 for cancellation, 5 for resource limitation, and 6 for
completion. Every other value is rejected.

The format does not duplicate ordinals, task identifiers, keys, or the resume
cursor. They are derived from the ordered event prefix and embedded exact plan.
Success events occupy the initial canonical task prefix. Failure or incomplete
may occur only once at the next task. Cancellation or resource limitation may
occur only once before an uncommitted task. Completion may occur only once
after every task succeeds. Every terminal event is last. The published count
is a prefix length no greater than the event count.

## Exact identity and digests

The embedded plan is decoded and compared structurally with the active
schedlib plan. A finite digest is never accepted as proof of plan equality.
The embedded plan digest is an early integrity check only.

Plan and checkpoint digests use distinct BLAKE3 derive-key contexts. Each
digest invocation includes its format schema, complete byte length, and exact
frame bytes. Consequently, equal bytes in different domains do not share a
digest invocation. Collision resistance remains a cryptographic assumption;
all semantic admission still uses exact bytes and exact decoded structure.

## Canonical key codecs

A key codec has an exact 256-bit identity and maps each caller key to one byte
string. At every encode and decode boundary, schedlib-interop checks:

1. decoding encoded bytes reconstructs the original key;
2. re-encoding a decoded key reproduces the same bytes;
3. decoded keys remain in strictly increasing schedlib order; and
4. no two task rows contain the same encoded key bytes.

These checks make accidental noncanonical or noninjective adapters fail closed.
The codec identity prevents interpreting the same bytes under a different key
domain.

## Admission machine

The decoder performs these phases iteratively:

1. reject a complete frame larger than the caller's byte limit;
2. parse the fixed header without allocation;
3. validate magic, schema, version, flags, codec identity, counts, and checked
   length arithmetic;
4. reject declared task, dependency, resource, key, profile, event, or work
   totals above caller limits;
5. scan variable fields without constructing a schedlib object, validating
   exact boundaries and canonical order;
6. reserve each output collection exactly once after admission;
7. decode keys and construct the canonical plan or checkpoint;
8. compare the complete decoded checkpoint plan with the active plan; and
9. publish one complete result at the return linearization point.

Cancellation is sampled before allocation, at bounded scanning intervals, and
immediately before publication. Rejection and cancellation return no partial
semantic object. Every loop cursor advances, native call-stack depth is
constant, work is linear in declared items and bytes, and retained heap is
linear in the decoded representation.
