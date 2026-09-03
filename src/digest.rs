/// BLAKE3 derive-key domain for canonical plan frames.
pub const DIGEST_PLAN_CONTEXT: &str = "vinary-tree/schedlib-interop/plan/v1";

/// BLAKE3 derive-key domain for canonical checkpoint frames.
pub const DIGEST_CHECKPOINT_CONTEXT: &str = "vinary-tree/schedlib-interop/checkpoint/v1";

/// Exact 256-bit domain-separated frame digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct FrameDigest([u8; 32]);

impl FrameDigest {
    /// Constructs a digest from its complete stable byte representation.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Borrows the complete stable byte representation.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Returns the complete stable byte representation.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Computes the domain-separated digest of one complete plan frame.
#[must_use]
pub fn digest_plan(bytes: &[u8]) -> FrameDigest {
    digest(DIGEST_PLAN_CONTEXT, b"SCHED-PLAN-V1!!!", bytes)
}

/// Computes the domain-separated digest of one complete checkpoint frame.
#[must_use]
pub fn digest_checkpoint(bytes: &[u8]) -> FrameDigest {
    digest(DIGEST_CHECKPOINT_CONTEXT, b"SCHED-CKPT-V1!!!", bytes)
}

pub(crate) fn digest_plan_controlled(
    bytes: &[u8],
    machine: &crate::Machine<'_>,
) -> Result<FrameDigest, crate::InteropError> {
    digest_controlled(DIGEST_PLAN_CONTEXT, b"SCHED-PLAN-V1!!!", bytes, machine)
}

pub(crate) fn digest_checkpoint_controlled(
    bytes: &[u8],
    machine: &crate::Machine<'_>,
) -> Result<FrameDigest, crate::InteropError> {
    digest_controlled(
        DIGEST_CHECKPOINT_CONTEXT,
        b"SCHED-CKPT-V1!!!",
        bytes,
        machine,
    )
}

fn digest(context: &str, schema: &[u8; 16], bytes: &[u8]) -> FrameDigest {
    let mut hasher = blake3::Hasher::new_derive_key(context);
    hasher.update(schema);
    hasher.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
    FrameDigest::from_bytes(*hasher.finalize().as_bytes())
}

fn digest_controlled(
    context: &str,
    schema: &[u8; 16],
    bytes: &[u8],
    machine: &crate::Machine<'_>,
) -> Result<FrameDigest, crate::InteropError> {
    let mut hasher = blake3::Hasher::new_derive_key(context);
    hasher.update(schema);
    hasher.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    for chunk in bytes.chunks(4_096) {
        machine.poll()?;
        hasher.update(chunk);
    }
    Ok(FrameDigest::from_bytes(*hasher.finalize().as_bytes()))
}
