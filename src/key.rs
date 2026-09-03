use crate::KeyCodecError;

/// Exact 256-bit identity for one canonical key interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct KeyCodecId([u8; 32]);

impl KeyCodecId {
    /// Constructs an identity from its complete stable representation.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the complete stable representation.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical, domain-identified codec for caller task keys.
///
/// Implementations must be deterministic and injective. For every admitted
/// key `k`, decoding its encoded bytes must return `k`; re-encoding any
/// admitted decoded value must reproduce the identical byte string.
pub trait CanonicalKeyCodec<K> {
    /// Returns the immutable interpretation identity written into plan frames.
    fn id(&self) -> KeyCodecId;

    /// Returns the exact number of bytes that [`Self::encode_into`] appends.
    ///
    /// # Errors
    ///
    /// Returns [`KeyCodecError::Rejected`] when the key is outside the codec's
    /// admitted domain.
    fn encoded_len(&self, key: &K) -> Result<usize, KeyCodecError>;

    /// Appends the unique canonical representation of `key` to `output`.
    ///
    /// # Errors
    ///
    /// Returns [`KeyCodecError::Rejected`] when the key is outside the codec's
    /// admitted domain.
    fn encode_into(&self, key: &K, output: &mut Vec<u8>) -> Result<(), KeyCodecError>;

    /// Decodes one complete canonical key representation.
    ///
    /// # Errors
    ///
    /// Returns [`KeyCodecError::Rejected`] for every byte sequence outside the
    /// codec's admitted domain.
    fn decode(&self, bytes: &[u8]) -> Result<K, KeyCodecError>;
}

/// Canonical order-preserving big-endian codec for `u64` task keys.
#[derive(Debug, Clone, Copy, Default)]
pub struct U64KeyCodec;

impl CanonicalKeyCodec<u64> for U64KeyCodec {
    fn id(&self) -> KeyCodecId {
        KeyCodecId::new(*b"vinary/u64-big-endian/key/v1!!!!")
    }

    fn encoded_len(&self, _key: &u64) -> Result<usize, KeyCodecError> {
        Ok(8)
    }

    fn encode_into(&self, key: &u64, output: &mut Vec<u8>) -> Result<(), KeyCodecError> {
        output.extend_from_slice(&key.to_be_bytes());
        Ok(())
    }

    fn decode(&self, bytes: &[u8]) -> Result<u64, KeyCodecError> {
        let encoded: [u8; 8] = bytes.try_into().map_err(|_| KeyCodecError::Rejected)?;
        Ok(u64::from_be_bytes(encoded))
    }
}
