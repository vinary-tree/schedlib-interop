# Threat model

## Trusted and untrusted inputs

schedlib semantic constructors and the selected key codec implementation are
trusted code. Frame bytes, every declared length or count, expected digests,
active-plan selection, cancellation timing, and storage provenance are
untrusted inputs.

## Threats and controls

| Threat | Control | Failure behavior |
|---|---|---|
| Oversized frame | Complete byte limit before hashing or allocation | `ByteLimitExceeded` |
| Integer wrap | Checked addition, multiplication, and width conversion | `ArithmeticOverflow` |
| Foreign key interpretation | Exact 256-bit codec identity in every plan header | `ForeignKeyCodec` |
| Noncanonical or colliding key codec | Decode/encode equality and strict decoded key order | `NonCanonicalKeyCodec` |
| Truncation or appended bytes | Exact declared payload and frame lengths | `LengthMismatch` |
| Hidden resource work | Inner counts bounded by remaining admitted totals | `NonCanonicalPlan` |
| Digest substitution | Complete embedded plan decoded and compared structurally | `ForeignPlan` |
| Digest domain confusion | Distinct immutable BLAKE3 derive-key contexts | `DigestMismatch` |
| Forged resume cursor | Cursor derived from the success event prefix | `NonCanonicalPlan` |
| Unknown event variant | Closed six-byte discriminator domain | `UnknownEventKind` |
| Cancellation race | Atomic sampling before allocation, during scans, and before publish | `Cancelled` |
| Corrupt in-memory checkpoint | schedlib structural validation before encoding | `NonCanonicalPlan` |

## Cryptographic scope

BLAKE3 authenticates nothing without a caller-owned trust mechanism. A runtime
may place the digest inside a signature, message authentication code, or trusted
manifest, but schedlib-interop does not own keys or trust anchors. Even a valid
digest cannot authorize checkpoint reuse under a structurally unequal active
plan.

## Denial-of-service scope

Applications should set finite `CodecLimits` and a finite work limit for
untrusted traffic. `CodecLimits::unbounded` is appropriate only when an outer
protocol has already established equivalent bounds. Key codec callbacks must
also avoid unbounded internal allocation or blocking because their internals
are outside this crate's control.
