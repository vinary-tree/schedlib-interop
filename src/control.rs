use std::sync::atomic::{AtomicBool, Ordering};

use crate::InteropError;

/// Complete caller-defined admission limits for one codec invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodecLimits {
    /// Maximum complete frame bytes.
    pub max_bytes: u64,
    /// Maximum plan tasks.
    pub max_tasks: u64,
    /// Maximum dependency edges.
    pub max_dependencies: u64,
    /// Maximum total read/write resource entries.
    pub max_resources: u64,
    /// Maximum total encoded key bytes.
    pub max_key_bytes: u64,
    /// Maximum semantic-profile bytes.
    pub max_profile_bytes: u64,
    /// Maximum checkpoint events.
    pub max_events: u64,
}

impl CodecLimits {
    /// Returns limits that admit every fixed-width representable count.
    #[must_use]
    pub const fn unbounded() -> Self {
        Self {
            max_bytes: u64::MAX,
            max_tasks: u64::MAX,
            max_dependencies: u64::MAX,
            max_resources: u64::MAX,
            max_key_bytes: u64::MAX,
            max_profile_bytes: u64::MAX,
            max_events: u64::MAX,
        }
    }
}

impl Default for CodecLimits {
    fn default() -> Self {
        Self::unbounded()
    }
}

/// Optional work and cancellation controls for one iterative codec operation.
#[derive(Debug, Clone, Copy)]
pub struct CodecControl<'a> {
    pub(crate) work_limit: u64,
    pub(crate) cancel_after_work: Option<u64>,
    pub(crate) cancellation: Option<&'a AtomicBool>,
}

impl CodecControl<'static> {
    /// Returns an invocation with `limit` logical work units available.
    #[must_use]
    pub const fn with_work_limit(limit: u64) -> Self {
        Self {
            work_limit: limit,
            cancel_after_work: None,
            cancellation: None,
        }
    }
}

impl<'a> CodecControl<'a> {
    /// Returns an invocation with no finite work limit or cancellation source.
    #[must_use]
    pub const fn unlimited() -> Self {
        Self {
            work_limit: u64::MAX,
            cancel_after_work: None,
            cancellation: None,
        }
    }

    /// Requests deterministic cancellation once admitted work reaches
    /// `threshold`.
    #[must_use]
    pub const fn with_cancel_after_work(mut self, threshold: u64) -> Self {
        self.cancel_after_work = Some(threshold);
        self
    }

    /// Samples `cancellation` throughout bounded scanning and before publish.
    #[must_use]
    pub const fn with_cancellation(mut self, cancellation: &'a AtomicBool) -> Self {
        self.cancellation = Some(cancellation);
        self
    }
}

/// Deterministic resource-accounting evidence for one completed invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodecMetrics {
    work: u64,
    cursor: u64,
    peak_heap_bytes: usize,
    published_bytes: u64,
    retained_reservations: u64,
    published: bool,
}

impl CodecMetrics {
    /// Returns the admitted logical work units.
    #[must_use]
    pub const fn work(self) -> u64 {
        self.work
    }

    /// Returns the final zero-based byte cursor.
    #[must_use]
    pub const fn cursor(self) -> u64 {
        self.cursor
    }

    /// Returns a conservative peak of codec-owned heap bytes.
    #[must_use]
    pub const fn peak_heap_bytes(self) -> usize {
        self.peak_heap_bytes
    }

    /// Returns the complete byte count made visible at publication.
    #[must_use]
    pub const fn published_bytes(self) -> u64 {
        self.published_bytes
    }

    /// Returns the number of exactly reserved collections retained in output.
    #[must_use]
    pub const fn retained_reservations(self) -> u64 {
        self.retained_reservations
    }

    /// Returns whether the complete result crossed the publication boundary.
    #[must_use]
    pub const fn published(self) -> bool {
        self.published
    }

    pub(crate) fn publish(&mut self, bytes: u64) {
        self.published_bytes = bytes;
        self.published = true;
    }
}

/// Complete value and deterministic metrics from a successful invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodecReport<T> {
    value: T,
    metrics: CodecMetrics,
}

impl<T> CodecReport<T> {
    pub(crate) const fn new(value: T, metrics: CodecMetrics) -> Self {
        Self { value, metrics }
    }

    /// Borrows the complete published value.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }

    /// Returns deterministic work and allocation evidence.
    #[must_use]
    pub const fn metrics(&self) -> CodecMetrics {
        self.metrics
    }

    /// Consumes the report and returns the complete published value.
    #[must_use]
    pub fn into_value(self) -> T {
        self.value
    }
}

pub(crate) struct Machine<'a> {
    control: CodecControl<'a>,
    work: u64,
}

impl<'a> Machine<'a> {
    pub(crate) fn new(control: CodecControl<'a>) -> Result<Self, InteropError> {
        let machine = Self { control, work: 0 };
        machine.poll()?;
        Ok(machine)
    }

    pub(crate) fn admit_work(&mut self, amount: u64) -> Result<(), InteropError> {
        let required = self
            .work
            .checked_add(amount)
            .ok_or(InteropError::ArithmeticOverflow)?;
        if required > self.control.work_limit {
            return Err(InteropError::WorkLimitExceeded {
                required,
                limit: self.control.work_limit,
            });
        }
        self.work = required;
        self.poll()
    }

    pub(crate) fn probe_work(&self, required: u64) -> Result<(), InteropError> {
        if required > self.control.work_limit {
            return Err(InteropError::WorkLimitExceeded {
                required,
                limit: self.control.work_limit,
            });
        }
        if self
            .control
            .cancel_after_work
            .is_some_and(|threshold| required >= threshold)
        {
            return Err(InteropError::Cancelled { work: required });
        }
        self.poll()
    }

    pub(crate) fn poll(&self) -> Result<(), InteropError> {
        let external = self
            .control
            .cancellation
            .is_some_and(|flag| flag.load(Ordering::Relaxed));
        let deterministic = self
            .control
            .cancel_after_work
            .is_some_and(|threshold| self.work >= threshold);
        if external || deterministic {
            Err(InteropError::Cancelled { work: self.work })
        } else {
            Ok(())
        }
    }

    pub(crate) fn metrics(
        &self,
        cursor: u64,
        peak_heap_bytes: usize,
        retained_reservations: u64,
    ) -> CodecMetrics {
        CodecMetrics {
            work: self.work,
            cursor,
            peak_heap_bytes,
            published_bytes: 0,
            retained_reservations,
            published: false,
        }
    }
}
