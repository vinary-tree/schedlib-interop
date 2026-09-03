//! Canonical, bounded interoperability for schedlib plans and checkpoints.
//!
//! The crate preserves exact structural scheduling identity in a versioned,
//! stack-safe byte grammar. It owns codecs and domain-separated digests, while
//! schedlib remains the semantic authority and runtime crates remain the
//! storage authority.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod control;
mod digest;
mod error;
mod key;
mod wire;

pub use control::{CodecControl, CodecLimits, CodecMetrics, CodecReport};
pub use digest::{
    digest_checkpoint, digest_plan, FrameDigest, DIGEST_CHECKPOINT_CONTEXT, DIGEST_PLAN_CONTEXT,
};
pub use error::{InteropError, KeyCodecError};
pub use key::{CanonicalKeyCodec, KeyCodecId, U64KeyCodec};
pub use wire::{
    decode_checkpoint_for, decode_checkpoint_with_control, decode_plan, decode_plan_with_control,
    decode_verified_checkpoint_for, decode_verified_plan, encode_checkpoint,
    encode_checkpoint_with_control, encode_plan, encode_plan_with_control, CHECKPOINT_HEADER_BYTES,
    CHECKPOINT_MAGIC, CHECKPOINT_SCHEMA_ID, CHECKPOINT_VERSION, PLAN_HEADER_BYTES, PLAN_MAGIC,
    PLAN_SCHEMA_ID, PLAN_VERSION,
};

use control::Machine;
