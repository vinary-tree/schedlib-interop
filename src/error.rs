use core::fmt;

use crate::digest::FrameDigest;

/// Failure returned by a caller-defined canonical key codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCodecError {
    /// The key or byte representation is outside the codec's admitted domain.
    Rejected,
}

impl fmt::Display for KeyCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("key codec rejected the value")
    }
}

impl std::error::Error for KeyCodecError {}

/// Typed, fail-closed interoperability error.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InteropError {
    /// A checked offset, length, count, or work calculation overflowed.
    ArithmeticOverflow,
    /// The declared complete frame length differs from the available bytes.
    LengthMismatch {
        /// Length required or declared by the frame.
        declared: u64,
        /// Number of bytes actually available.
        actual: u64,
    },
    /// The complete frame exceeds the caller's byte limit.
    ByteLimitExceeded {
        /// Complete frame length.
        actual: u64,
        /// Configured maximum frame length.
        limit: u64,
    },
    /// A declared collection count exceeds its caller limit.
    CountLimitExceeded {
        /// Stable name of the bounded count.
        field: &'static str,
        /// Declared count.
        actual: u64,
        /// Configured maximum count.
        limit: u64,
    },
    /// The admitted linear-work bound exceeds the caller's limit.
    WorkLimitExceeded {
        /// Required cumulative logical work.
        required: u64,
        /// Configured maximum logical work.
        limit: u64,
    },
    /// Cancellation was observed before result publication.
    Cancelled {
        /// Logical work admitted when cancellation was observed.
        work: u64,
    },
    /// A fixed header field did not match the versioned grammar.
    HeaderMismatch {
        /// Stable name of the mismatching field.
        field: &'static str,
    },
    /// The frame names a different key-codec identity.
    ForeignKeyCodec {
        /// Identity required by the selected codec.
        expected: [u8; 32],
        /// Identity declared by the frame.
        actual: [u8; 32],
    },
    /// A caller key codec rejected a key or byte sequence.
    KeyCodec(KeyCodecError),
    /// A key codec violated length, round-trip, ordering, or injectivity laws.
    NonCanonicalKeyCodec,
    /// Plan structure or canonical ordering is invalid.
    NonCanonicalPlan,
    /// The semantic profile is not exact UTF-8.
    InvalidUtf8,
    /// A verified frame does not match its expected digest.
    DigestMismatch {
        /// Digest supplied by the caller or checkpoint header.
        expected: FrameDigest,
        /// Digest computed over the complete domain-separated frame.
        actual: FrameDigest,
    },
    /// A checkpoint embeds a plan unequal to the active structural plan.
    ForeignPlan,
    /// A checkpoint event discriminator is outside the six-value domain.
    UnknownEventKind {
        /// Rejected discriminator byte.
        value: u8,
        /// Zero-based event position.
        index: u64,
    },
}

impl fmt::Display for InteropError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArithmeticOverflow => formatter.write_str("checked arithmetic overflow"),
            Self::LengthMismatch { declared, actual } => {
                write!(formatter, "frame length {actual} differs from {declared}")
            }
            Self::ByteLimitExceeded { actual, limit } => {
                write!(formatter, "frame length {actual} exceeds limit {limit}")
            }
            Self::CountLimitExceeded {
                field,
                actual,
                limit,
            } => write!(formatter, "{field} count {actual} exceeds limit {limit}"),
            Self::WorkLimitExceeded { required, limit } => {
                write!(formatter, "logical work {required} exceeds limit {limit}")
            }
            Self::Cancelled { work } => {
                write!(
                    formatter,
                    "codec cancelled after admitting {work} work units"
                )
            }
            Self::HeaderMismatch { field } => write!(formatter, "invalid {field} header field"),
            Self::ForeignKeyCodec { .. } => formatter.write_str("foreign key codec identity"),
            Self::KeyCodec(error) => error.fmt(formatter),
            Self::NonCanonicalKeyCodec => formatter.write_str("noncanonical key codec"),
            Self::NonCanonicalPlan => formatter.write_str("noncanonical plan"),
            Self::InvalidUtf8 => formatter.write_str("invalid semantic-profile UTF-8"),
            Self::DigestMismatch { .. } => formatter.write_str("digest mismatch"),
            Self::ForeignPlan => formatter.write_str("checkpoint belongs to a foreign plan"),
            Self::UnknownEventKind { value, index } => {
                write!(formatter, "unknown event kind {value} at position {index}")
            }
        }
    }
}

impl std::error::Error for InteropError {}

impl From<KeyCodecError> for InteropError {
    fn from(error: KeyCodecError) -> Self {
        Self::KeyCodec(error)
    }
}
