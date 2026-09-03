use core::mem::size_of;

use schedlib::durable::{Checkpoint, CheckpointEventKind, PlanIdentity, PlanIdentityView};

use crate::{
    digest::{digest_checkpoint_controlled, digest_plan_controlled},
    CanonicalKeyCodec, CodecControl, CodecLimits, CodecMetrics, CodecReport, FrameDigest,
    InteropError, Machine,
};

/// Fixed canonical plan-frame header length.
pub const PLAN_HEADER_BYTES: usize = 112;
/// Fixed canonical checkpoint-frame header length.
pub const CHECKPOINT_HEADER_BYTES: usize = 96;
/// Canonical plan-frame magic.
pub const PLAN_MAGIC: [u8; 8] = *b"SCHPLN\0\x01";
/// Canonical checkpoint-frame magic.
pub const CHECKPOINT_MAGIC: [u8; 8] = *b"SCHCKP\0\x01";
/// Canonical plan schema identifier.
pub const PLAN_SCHEMA_ID: [u8; 16] = *b"SCHED-PLAN-V1!!!";
/// Canonical checkpoint schema identifier.
pub const CHECKPOINT_SCHEMA_ID: [u8; 16] = *b"SCHED-CKPT-V1!!!";
/// Supported plan major and minor version.
pub const PLAN_VERSION: (u16, u16) = (1, 0);
/// Supported checkpoint major and minor version.
pub const CHECKPOINT_VERSION: (u16, u16) = (1, 0);

#[derive(Debug, Clone, Copy)]
struct PlanMeasure {
    tasks: u32,
    dependencies: u32,
    resources: u64,
    key_bytes: u64,
    profile_bytes: u64,
    payload_bytes: u64,
    total_bytes: u64,
    maximum_key_bytes: u64,
    work: u64,
}

#[derive(Debug, Clone, Copy)]
struct PlanHeader {
    schema: u64,
    tasks: u32,
    dependencies: u32,
    resources: u64,
    key_bytes: u64,
    profile_bytes: u64,
    payload_bytes: u64,
}

#[derive(Debug, Clone, Copy)]
struct CheckpointHeader {
    plan_bytes: u64,
    events: u64,
    published: u64,
    payload_bytes: u64,
    plan_digest: FrameDigest,
}

struct Decoded<T> {
    value: T,
    metrics: CodecMetrics,
}

/// Encodes an exact plan identity into canonical version-one bytes.
///
/// # Errors
///
/// Returns a typed error when a count exceeds the format or caller limits,
/// arithmetic overflows, or the selected key codec violates its canonical
/// laws.
pub fn encode_plan<K, P, C>(
    plan: &PlanIdentity<K, P>,
    codec: &C,
    limits: CodecLimits,
) -> Result<Vec<u8>, InteropError>
where
    K: Clone + Ord,
    P: AsRef<str> + Clone + Eq,
    C: CanonicalKeyCodec<K>,
{
    encode_plan_with_control(plan, codec, limits, CodecControl::unlimited())
        .map(CodecReport::into_value)
}

/// Encodes a plan with explicit work and cancellation controls.
///
/// # Errors
///
/// Returns a typed admission, cancellation, arithmetic, structural, or key
/// codec error without publishing a partial frame.
pub fn encode_plan_with_control<K, P, C>(
    plan: &PlanIdentity<K, P>,
    codec: &C,
    limits: CodecLimits,
    control: CodecControl<'_>,
) -> Result<CodecReport<Vec<u8>>, InteropError>
where
    K: Clone + Ord,
    P: AsRef<str> + Clone + Eq,
    C: CanonicalKeyCodec<K>,
{
    let mut machine = Machine::new(control)?;
    let view = plan.view();
    let measure = measure_plan(view, codec, limits, &machine)?;
    machine.admit_work(measure.work)?;
    let (bytes, peak_heap_bytes) = build_plan(view, codec, measure, &machine)?;
    machine.poll()?;
    let mut metrics = machine.metrics(measure.total_bytes, peak_heap_bytes, 1);
    metrics.publish(measure.total_bytes);
    Ok(CodecReport::new(bytes, metrics))
}

/// Decodes one complete canonical plan frame.
///
/// # Errors
///
/// Returns a typed error for every malformed, noncanonical, foreign-codec, or
/// resource-exceeding input.
pub fn decode_plan<K, C>(
    bytes: &[u8],
    codec: &C,
    limits: CodecLimits,
) -> Result<PlanIdentity<K, String>, InteropError>
where
    K: Clone + Ord,
    C: CanonicalKeyCodec<K>,
{
    decode_plan_with_control(bytes, codec, limits, CodecControl::unlimited())
        .map(CodecReport::into_value)
}

/// Decodes a canonical plan with explicit work and cancellation controls.
///
/// # Errors
///
/// Returns a typed error without publishing a partial semantic object.
pub fn decode_plan_with_control<K, C>(
    bytes: &[u8],
    codec: &C,
    limits: CodecLimits,
    control: CodecControl<'_>,
) -> Result<CodecReport<PlanIdentity<K, String>>, InteropError>
where
    K: Clone + Ord,
    C: CanonicalKeyCodec<K>,
{
    let mut machine = Machine::new(control)?;
    let decoded = decode_plan_core(bytes, codec, limits, &mut machine, None, true)?;
    machine.poll()?;
    let mut metrics = decoded.metrics;
    metrics.publish(usize_to_u64(bytes.len())?);
    Ok(CodecReport::new(decoded.value, metrics))
}

/// Verifies a complete plan digest before decoding canonical semantics.
///
/// # Errors
///
/// Returns [`InteropError::DigestMismatch`] before construction when the
/// expected digest differs, or the ordinary typed decoding error otherwise.
pub fn decode_verified_plan<K, C>(
    bytes: &[u8],
    codec: &C,
    expected: FrameDigest,
    limits: CodecLimits,
) -> Result<PlanIdentity<K, String>, InteropError>
where
    K: Clone + Ord,
    C: CanonicalKeyCodec<K>,
{
    let mut machine = Machine::new(CodecControl::unlimited())?;
    let decoded = decode_plan_core(bytes, codec, limits, &mut machine, Some(expected), true)?;
    machine.poll()?;
    Ok(decoded.value)
}

/// Encodes a payload-free durable checkpoint into canonical version-one bytes.
///
/// # Errors
///
/// Returns a typed error when admission, canonical key validation, or checked
/// arithmetic fails.
pub fn encode_checkpoint<K, C>(
    checkpoint: &Checkpoint<K>,
    codec: &C,
    limits: CodecLimits,
) -> Result<Vec<u8>, InteropError>
where
    K: Clone + Ord,
    C: CanonicalKeyCodec<K>,
{
    encode_checkpoint_with_control(checkpoint, codec, limits, CodecControl::unlimited())
        .map(CodecReport::into_value)
}

/// Encodes a checkpoint with explicit work and cancellation controls.
///
/// # Errors
///
/// Returns a typed error without publishing a partial frame.
pub fn encode_checkpoint_with_control<K, C>(
    checkpoint: &Checkpoint<K>,
    codec: &C,
    limits: CodecLimits,
    control: CodecControl<'_>,
) -> Result<CodecReport<Vec<u8>>, InteropError>
where
    K: Clone + Ord,
    C: CanonicalKeyCodec<K>,
{
    let mut machine = Machine::new(control)?;
    checkpoint
        .validate()
        .map_err(|_| InteropError::NonCanonicalPlan)?;
    let view = checkpoint.view();
    let plan_measure = measure_plan(view.plan(), codec, limits, &machine)?;
    let events = usize_to_u64(view.event_count())?;
    enforce_count("events", events, limits.max_events)?;
    if events
        > usize_to_u64(view.plan().task_count())?
            .checked_add(1)
            .ok_or(InteropError::ArithmeticOverflow)?
    {
        return Err(InteropError::NonCanonicalPlan);
    }
    let payload_bytes = plan_measure
        .total_bytes
        .checked_add(events)
        .ok_or(InteropError::ArithmeticOverflow)?;
    let total_bytes = usize_to_u64(CHECKPOINT_HEADER_BYTES)?
        .checked_add(payload_bytes)
        .ok_or(InteropError::ArithmeticOverflow)?;
    enforce_bytes(total_bytes, limits.max_bytes)?;
    let checkpoint_work = total_bytes
        .checked_add(events)
        .and_then(|value| value.checked_add(plan_measure.work))
        .ok_or(InteropError::ArithmeticOverflow)?;
    machine.admit_work(checkpoint_work)?;

    let (plan_bytes, plan_peak) = build_plan(view.plan(), codec, plan_measure, &machine)?;
    machine.poll()?;
    let capacity = u64_to_usize(total_bytes)?;
    let mut output = Vec::with_capacity(capacity);
    output.extend_from_slice(&CHECKPOINT_MAGIC);
    output.extend_from_slice(&CHECKPOINT_SCHEMA_ID);
    put_u16(&mut output, CHECKPOINT_VERSION.0);
    put_u16(&mut output, CHECKPOINT_VERSION.1);
    put_u32(&mut output, 0);
    put_u64(&mut output, plan_measure.total_bytes);
    put_u64(&mut output, events);
    put_u64(&mut output, usize_to_u64(view.published_prefix())?);
    put_u64(&mut output, payload_bytes);
    output.extend_from_slice(digest_plan_controlled(&plan_bytes, &machine)?.as_bytes());
    output.extend_from_slice(&plan_bytes);
    for kind in view.event_kinds() {
        machine.poll()?;
        output.push(encode_event_kind(kind));
    }
    if output.len() != capacity {
        return Err(InteropError::LengthMismatch {
            declared: total_bytes,
            actual: usize_to_u64(output.len())?,
        });
    }
    machine.poll()?;
    let peak_heap_bytes = plan_peak
        .checked_add(u64_to_usize(total_bytes)?)
        .ok_or(InteropError::ArithmeticOverflow)?;
    let mut metrics = machine.metrics(total_bytes, peak_heap_bytes, 1);
    metrics.publish(total_bytes);
    Ok(CodecReport::new(output, metrics))
}

/// Decodes a checkpoint without comparing it to an active plan.
///
/// This operation validates the embedded plan and complete checkpoint event
/// language. Runtime recovery should prefer [`decode_checkpoint_for`].
///
/// # Errors
///
/// Returns a typed error without publishing a partial checkpoint.
pub fn decode_checkpoint_with_control<K, C>(
    bytes: &[u8],
    codec: &C,
    limits: CodecLimits,
    control: CodecControl<'_>,
) -> Result<CodecReport<Checkpoint<K>>, InteropError>
where
    K: Clone + Ord,
    C: CanonicalKeyCodec<K>,
{
    let mut machine = Machine::new(control)?;
    let decoded = decode_checkpoint_core(bytes, codec, limits, &mut machine, None)?;
    machine.poll()?;
    let mut metrics = decoded.metrics;
    metrics.publish(usize_to_u64(bytes.len())?);
    Ok(CodecReport::new(decoded.value, metrics))
}

/// Decodes a checkpoint and confirms exact equality with `active`.
///
/// # Errors
///
/// Returns [`InteropError::ForeignPlan`] for unequal structural identity or an
/// earlier typed frame error. A digest never substitutes for this comparison.
pub fn decode_checkpoint_for<K, P, C>(
    bytes: &[u8],
    active: &PlanIdentity<K, P>,
    codec: &C,
    limits: CodecLimits,
) -> Result<Checkpoint<K>, InteropError>
where
    K: Clone + Ord,
    P: AsRef<str> + Clone + Eq,
    C: CanonicalKeyCodec<K>,
{
    decode_checkpoint_for_digest(bytes, active, codec, limits, None)
}

/// Verifies a checkpoint digest, decodes it, and confirms exact active-plan
/// equality.
///
/// # Errors
///
/// Returns a digest, frame, canonicality, or foreign-plan error before making
/// a checkpoint visible.
pub fn decode_verified_checkpoint_for<K, P, C>(
    bytes: &[u8],
    active: &PlanIdentity<K, P>,
    codec: &C,
    expected: FrameDigest,
    limits: CodecLimits,
) -> Result<Checkpoint<K>, InteropError>
where
    K: Clone + Ord,
    P: AsRef<str> + Clone + Eq,
    C: CanonicalKeyCodec<K>,
{
    decode_checkpoint_for_digest(bytes, active, codec, limits, Some(expected))
}

fn decode_checkpoint_for_digest<K, P, C>(
    bytes: &[u8],
    active: &PlanIdentity<K, P>,
    codec: &C,
    limits: CodecLimits,
    expected: Option<FrameDigest>,
) -> Result<Checkpoint<K>, InteropError>
where
    K: Clone + Ord,
    P: AsRef<str> + Clone + Eq,
    C: CanonicalKeyCodec<K>,
{
    let mut machine = Machine::new(CodecControl::unlimited())?;
    let decoded = decode_checkpoint_core(bytes, codec, limits, &mut machine, expected)?;
    if decoded.value.view().plan() != active.view() {
        return Err(InteropError::ForeignPlan);
    }
    machine.poll()?;
    Ok(decoded.value)
}

fn measure_plan<K, C>(
    view: PlanIdentityView<'_, K>,
    codec: &C,
    limits: CodecLimits,
    machine: &Machine<'_>,
) -> Result<PlanMeasure, InteropError>
where
    K: Ord,
    C: CanonicalKeyCodec<K>,
{
    let task_count = usize_to_u64(view.task_count())?;
    let dependency_count = usize_to_u64(view.dependencies().len())?;
    enforce_count("tasks", task_count, limits.max_tasks)?;
    enforce_count("dependencies", dependency_count, limits.max_dependencies)?;
    let tasks = u32::try_from(task_count).map_err(|_| InteropError::ArithmeticOverflow)?;
    let dependencies =
        u32::try_from(dependency_count).map_err(|_| InteropError::ArithmeticOverflow)?;

    if view.effects().len() != view.task_count()
        || view.costs().len() != view.task_count()
        || !view.dependencies().windows(2).all(|pair| pair[0] < pair[1])
        || view.dependencies().iter().any(|(source, target)| {
            source.index() >= view.task_count() || target.index() >= view.task_count()
        })
    {
        return Err(InteropError::NonCanonicalPlan);
    }

    let profile_bytes = usize_to_u64(view.semantic_profile().len())?;
    enforce_count("profile bytes", profile_bytes, limits.max_profile_bytes)?;
    let fixed_task_bytes = task_count
        .checked_mul(24)
        .ok_or(InteropError::ArithmeticOverflow)?;
    let dependency_bytes = dependency_count
        .checked_mul(8)
        .ok_or(InteropError::ArithmeticOverflow)?;
    let minimum_total = usize_to_u64(PLAN_HEADER_BYTES)?
        .checked_add(8)
        .and_then(|value| value.checked_add(profile_bytes))
        .and_then(|value| value.checked_add(fixed_task_bytes))
        .and_then(|value| value.checked_add(dependency_bytes))
        .ok_or(InteropError::ArithmeticOverflow)?;
    let minimum_work = minimum_total
        .checked_add(task_count)
        .and_then(|value| value.checked_add(dependency_count))
        .ok_or(InteropError::ArithmeticOverflow)?;
    machine.probe_work(minimum_work)?;

    let mut key_bytes = 0_u64;
    let mut maximum_key_bytes = 0_u64;
    for key in view.keys() {
        let length = usize_to_u64(codec.encoded_len(key)?)?;
        key_bytes = key_bytes
            .checked_add(length)
            .ok_or(InteropError::ArithmeticOverflow)?;
        enforce_count("key bytes", key_bytes, limits.max_key_bytes)?;
        maximum_key_bytes = maximum_key_bytes.max(length);
        machine.probe_work(
            minimum_work
                .checked_add(key_bytes)
                .ok_or(InteropError::ArithmeticOverflow)?,
        )?;
    }
    enforce_count("key bytes", key_bytes, limits.max_key_bytes)?;

    let mut resources = 0_u64;
    for (reads, writes) in view.effects() {
        machine.poll()?;
        if !strictly_increasing(reads) || !strictly_increasing(writes) {
            return Err(InteropError::NonCanonicalPlan);
        }
        u32::try_from(reads.len()).map_err(|_| InteropError::ArithmeticOverflow)?;
        u32::try_from(writes.len()).map_err(|_| InteropError::ArithmeticOverflow)?;
        let read_count = usize_to_u64(reads.len())?;
        let write_count = usize_to_u64(writes.len())?;
        resources = resources
            .checked_add(read_count)
            .and_then(|value| value.checked_add(write_count))
            .ok_or(InteropError::ArithmeticOverflow)?;
        enforce_count("resources", resources, limits.max_resources)?;
    }
    enforce_count("resources", resources, limits.max_resources)?;

    let resource_bytes = resources
        .checked_mul(8)
        .ok_or(InteropError::ArithmeticOverflow)?;
    let payload_bytes = 8_u64
        .checked_add(profile_bytes)
        .and_then(|value| value.checked_add(fixed_task_bytes))
        .and_then(|value| value.checked_add(key_bytes))
        .and_then(|value| value.checked_add(resource_bytes))
        .and_then(|value| value.checked_add(dependency_bytes))
        .ok_or(InteropError::ArithmeticOverflow)?;
    let total_bytes = usize_to_u64(PLAN_HEADER_BYTES)?
        .checked_add(payload_bytes)
        .ok_or(InteropError::ArithmeticOverflow)?;
    enforce_bytes(total_bytes, limits.max_bytes)?;
    let work = total_bytes
        .checked_add(task_count)
        .and_then(|value| value.checked_add(dependency_count))
        .and_then(|value| value.checked_add(resources))
        .ok_or(InteropError::ArithmeticOverflow)?;
    machine.probe_work(work)?;
    Ok(PlanMeasure {
        tasks,
        dependencies,
        resources,
        key_bytes,
        profile_bytes,
        payload_bytes,
        total_bytes,
        maximum_key_bytes,
        work,
    })
}

fn build_plan<K, C>(
    view: PlanIdentityView<'_, K>,
    codec: &C,
    measure: PlanMeasure,
    machine: &Machine<'_>,
) -> Result<(Vec<u8>, usize), InteropError>
where
    K: Ord,
    C: CanonicalKeyCodec<K>,
{
    machine.poll()?;
    let capacity = u64_to_usize(measure.total_bytes)?;
    let scratch_capacity = u64_to_usize(measure.maximum_key_bytes)?;
    let mut output = Vec::with_capacity(capacity);
    output.extend_from_slice(&PLAN_MAGIC);
    output.extend_from_slice(&PLAN_SCHEMA_ID);
    put_u16(&mut output, PLAN_VERSION.0);
    put_u16(&mut output, PLAN_VERSION.1);
    put_u32(&mut output, 0);
    output.extend_from_slice(&codec.id().into_bytes());
    put_u64(&mut output, view.schema());
    put_u32(&mut output, measure.tasks);
    put_u32(&mut output, measure.dependencies);
    put_u64(&mut output, measure.resources);
    put_u64(&mut output, measure.key_bytes);
    put_u64(&mut output, measure.profile_bytes);
    put_u64(&mut output, measure.payload_bytes);
    put_u64(&mut output, view.budget());
    output.extend_from_slice(view.semantic_profile().as_bytes());

    let mut encoded = Vec::with_capacity(scratch_capacity);
    let mut reencoded = Vec::with_capacity(scratch_capacity);
    let mut effects = view.effects();
    for (index, key) in view.keys().iter().enumerate() {
        machine.poll()?;
        encoded.clear();
        codec.encode_into(key, &mut encoded)?;
        let expected = codec.encoded_len(key)?;
        if encoded.len() != expected {
            return Err(InteropError::NonCanonicalKeyCodec);
        }
        let decoded = codec.decode(&encoded)?;
        if &decoded != key {
            return Err(InteropError::NonCanonicalKeyCodec);
        }
        reencoded.clear();
        codec.encode_into(&decoded, &mut reencoded)?;
        if reencoded != encoded || codec.encoded_len(&decoded)? != encoded.len() {
            return Err(InteropError::NonCanonicalKeyCodec);
        }
        put_u64(&mut output, usize_to_u64(encoded.len())?);
        output.extend_from_slice(&encoded);
        let (reads, writes) = effects.next().ok_or(InteropError::NonCanonicalPlan)?;
        put_u32(
            &mut output,
            u32::try_from(reads.len()).map_err(|_| InteropError::ArithmeticOverflow)?,
        );
        put_resources(&mut output, reads, machine)?;
        put_u32(
            &mut output,
            u32::try_from(writes.len()).map_err(|_| InteropError::ArithmeticOverflow)?,
        );
        put_resources(&mut output, writes, machine)?;
        let cost = view
            .costs()
            .get(index)
            .copied()
            .ok_or(InteropError::NonCanonicalPlan)?;
        put_u64(&mut output, cost);
    }
    if effects.next().is_some() || view.costs().len() != view.task_count() {
        return Err(InteropError::NonCanonicalPlan);
    }
    for &(source, target) in view.dependencies() {
        machine.poll()?;
        put_u32(&mut output, source.get());
        put_u32(&mut output, target.get());
    }
    if output.len() != capacity {
        return Err(InteropError::LengthMismatch {
            declared: measure.total_bytes,
            actual: usize_to_u64(output.len())?,
        });
    }
    let scratch_bytes = measure
        .maximum_key_bytes
        .checked_mul(2)
        .ok_or(InteropError::ArithmeticOverflow)?;
    let peak = measure
        .total_bytes
        .checked_add(scratch_bytes)
        .ok_or(InteropError::ArithmeticOverflow)?;
    Ok((output, u64_to_usize(peak)?))
}

fn decode_plan_core<K, C>(
    bytes: &[u8],
    codec: &C,
    limits: CodecLimits,
    machine: &mut Machine<'_>,
    expected_digest: Option<FrameDigest>,
    admit_work: bool,
) -> Result<Decoded<PlanIdentity<K, String>>, InteropError>
where
    K: Clone + Ord,
    C: CanonicalKeyCodec<K>,
{
    let actual_bytes = usize_to_u64(bytes.len())?;
    enforce_bytes(actual_bytes, limits.max_bytes)?;
    let header = read_plan_header(bytes, codec)?;
    let work = validate_plan_header(header, actual_bytes, limits)?;
    if admit_work {
        machine.admit_work(work)?;
    }
    if let Some(expected) = expected_digest {
        let actual = digest_plan_controlled(bytes, machine)?;
        if actual != expected {
            return Err(InteropError::DigestMismatch { expected, actual });
        }
    }
    scan_plan(bytes, header, machine)?;
    construct_plan(bytes, header, codec, machine)
}

fn read_plan_header<K, C>(bytes: &[u8], codec: &C) -> Result<PlanHeader, InteropError>
where
    C: CanonicalKeyCodec<K>,
{
    require_minimum(bytes, PLAN_HEADER_BYTES)?;
    let mut reader = Reader::new(bytes);
    if reader.array::<8>()? != PLAN_MAGIC {
        return Err(InteropError::HeaderMismatch {
            field: "plan magic",
        });
    }
    if reader.array::<16>()? != PLAN_SCHEMA_ID {
        return Err(InteropError::HeaderMismatch {
            field: "plan schema",
        });
    }
    if reader.u16()? != PLAN_VERSION.0 || reader.u16()? != PLAN_VERSION.1 {
        return Err(InteropError::HeaderMismatch {
            field: "plan version",
        });
    }
    if reader.u32()? != 0 {
        return Err(InteropError::HeaderMismatch {
            field: "plan flags",
        });
    }
    let actual_codec = reader.array::<32>()?;
    let expected_codec = codec.id().into_bytes();
    if actual_codec != expected_codec {
        return Err(InteropError::ForeignKeyCodec {
            expected: expected_codec,
            actual: actual_codec,
        });
    }
    let header = PlanHeader {
        schema: reader.u64()?,
        tasks: reader.u32()?,
        dependencies: reader.u32()?,
        resources: reader.u64()?,
        key_bytes: reader.u64()?,
        profile_bytes: reader.u64()?,
        payload_bytes: reader.u64()?,
    };
    if reader.position() != PLAN_HEADER_BYTES {
        return Err(InteropError::ArithmeticOverflow);
    }
    Ok(header)
}

fn validate_plan_header(
    header: PlanHeader,
    actual_bytes: u64,
    limits: CodecLimits,
) -> Result<u64, InteropError> {
    let expected = usize_to_u64(PLAN_HEADER_BYTES)?
        .checked_add(header.payload_bytes)
        .ok_or(InteropError::ArithmeticOverflow)?;
    if expected != actual_bytes {
        return Err(InteropError::LengthMismatch {
            declared: expected,
            actual: actual_bytes,
        });
    }
    enforce_count("tasks", u64::from(header.tasks), limits.max_tasks)?;
    enforce_count(
        "dependencies",
        u64::from(header.dependencies),
        limits.max_dependencies,
    )?;
    enforce_count("resources", header.resources, limits.max_resources)?;
    enforce_count("key bytes", header.key_bytes, limits.max_key_bytes)?;
    enforce_count(
        "profile bytes",
        header.profile_bytes,
        limits.max_profile_bytes,
    )?;
    actual_bytes
        .checked_add(u64::from(header.tasks))
        .and_then(|value| value.checked_add(u64::from(header.dependencies)))
        .and_then(|value| value.checked_add(header.resources))
        .ok_or(InteropError::ArithmeticOverflow)
}

fn scan_plan(bytes: &[u8], header: PlanHeader, machine: &Machine<'_>) -> Result<(), InteropError> {
    let mut reader = Reader::new(&bytes[PLAN_HEADER_BYTES..]);
    let _budget = reader.u64()?;
    let profile = reader.take(u64_to_usize(header.profile_bytes)?)?;
    poll_slice(profile, machine)?;
    core::str::from_utf8(profile).map_err(|_| InteropError::InvalidUtf8)?;
    let mut key_bytes = 0_u64;
    let mut resources = 0_u64;
    for _ in 0..header.tasks {
        machine.poll()?;
        let key_length = reader.u64()?;
        key_bytes = key_bytes
            .checked_add(key_length)
            .ok_or(InteropError::ArithmeticOverflow)?;
        if key_bytes > header.key_bytes {
            return Err(InteropError::NonCanonicalPlan);
        }
        let key = reader.take(u64_to_usize(key_length)?)?;
        poll_slice(key, machine)?;
        let remaining = header
            .resources
            .checked_sub(resources)
            .ok_or(InteropError::NonCanonicalPlan)?;
        let read_count = scan_resource_set(&mut reader, machine, remaining)?;
        let remaining = remaining
            .checked_sub(read_count)
            .ok_or(InteropError::NonCanonicalPlan)?;
        let write_count = scan_resource_set(&mut reader, machine, remaining)?;
        resources = resources
            .checked_add(read_count)
            .and_then(|value| value.checked_add(write_count))
            .ok_or(InteropError::ArithmeticOverflow)?;
        let _cost = reader.u64()?;
    }
    let mut previous = None;
    for _ in 0..header.dependencies {
        machine.poll()?;
        let edge = (reader.u32()?, reader.u32()?);
        if edge.0 >= header.tasks
            || edge.1 >= header.tasks
            || previous.is_some_and(|prior| prior >= edge)
        {
            return Err(InteropError::NonCanonicalPlan);
        }
        previous = Some(edge);
    }
    if key_bytes != header.key_bytes
        || resources != header.resources
        || reader.position() != bytes.len() - PLAN_HEADER_BYTES
    {
        return Err(InteropError::NonCanonicalPlan);
    }
    Ok(())
}

fn construct_plan<K, C>(
    bytes: &[u8],
    header: PlanHeader,
    codec: &C,
    machine: &Machine<'_>,
) -> Result<Decoded<PlanIdentity<K, String>>, InteropError>
where
    K: Clone + Ord,
    C: CanonicalKeyCodec<K>,
{
    machine.poll()?;
    let tasks = usize::try_from(header.tasks).map_err(|_| InteropError::ArithmeticOverflow)?;
    let dependencies =
        usize::try_from(header.dependencies).map_err(|_| InteropError::ArithmeticOverflow)?;
    let mut reader = Reader::new(&bytes[PLAN_HEADER_BYTES..]);
    let budget = reader.u64()?;
    let profile = core::str::from_utf8(reader.take(u64_to_usize(header.profile_bytes)?)?)
        .map_err(|_| InteropError::InvalidUtf8)?
        .to_owned();
    let mut keys = Vec::with_capacity(tasks);
    let mut effects = Vec::with_capacity(tasks);
    let mut costs = Vec::with_capacity(tasks);
    let mut scratch = Vec::new();
    for _ in 0..tasks {
        machine.poll()?;
        let key_length = reader.u64()?;
        let encoded = reader.take(u64_to_usize(key_length)?)?;
        let key = codec.decode(encoded)?;
        if codec.encoded_len(&key)? != encoded.len()
            || keys.last().is_some_and(|previous| previous >= &key)
        {
            return Err(InteropError::NonCanonicalKeyCodec);
        }
        scratch.clear();
        codec.encode_into(&key, &mut scratch)?;
        if scratch != encoded {
            return Err(InteropError::NonCanonicalKeyCodec);
        }
        keys.push(key);
        let reads = read_resource_set(&mut reader, machine)?;
        let writes = read_resource_set(&mut reader, machine)?;
        effects.push((reads, writes));
        costs.push(reader.u64()?);
    }
    let mut dependency_keys = Vec::with_capacity(dependencies);
    for _ in 0..dependencies {
        machine.poll()?;
        let source =
            usize::try_from(reader.u32()?).map_err(|_| InteropError::ArithmeticOverflow)?;
        let target =
            usize::try_from(reader.u32()?).map_err(|_| InteropError::ArithmeticOverflow)?;
        let source = keys
            .get(source)
            .cloned()
            .ok_or(InteropError::NonCanonicalPlan)?;
        let target = keys
            .get(target)
            .cloned()
            .ok_or(InteropError::NonCanonicalPlan)?;
        dependency_keys.push((source, target));
    }
    let value = PlanIdentity::new(
        header.schema,
        keys,
        dependency_keys,
        effects,
        costs,
        budget,
        profile,
    )
    .map_err(|_| InteropError::NonCanonicalPlan)?;
    let task_width = size_of::<K>()
        .checked_add(size_of::<(Vec<u64>, Vec<u64>)>())
        .and_then(|value| value.checked_add(size_of::<u64>()))
        .ok_or(InteropError::ArithmeticOverflow)?;
    let task_bytes = u64::from(header.tasks)
        .checked_mul(usize_to_u64(task_width)?)
        .ok_or(InteropError::ArithmeticOverflow)?;
    let resource_bytes = header
        .resources
        .checked_mul(8)
        .ok_or(InteropError::ArithmeticOverflow)?;
    let dependency_bytes = u64::from(header.dependencies)
        .checked_mul(usize_to_u64(size_of::<(K, K)>())?)
        .ok_or(InteropError::ArithmeticOverflow)?;
    let representation = task_bytes
        .checked_add(resource_bytes)
        .and_then(|value| value.checked_add(dependency_bytes))
        .and_then(|value| value.checked_add(header.profile_bytes))
        .and_then(|value| value.checked_add(header.key_bytes))
        .ok_or(InteropError::ArithmeticOverflow)?;
    let metrics = machine.metrics(usize_to_u64(bytes.len())?, u64_to_usize(representation)?, 5);
    Ok(Decoded { value, metrics })
}

fn decode_checkpoint_core<K, C>(
    bytes: &[u8],
    codec: &C,
    limits: CodecLimits,
    machine: &mut Machine<'_>,
    expected_digest: Option<FrameDigest>,
) -> Result<Decoded<Checkpoint<K>>, InteropError>
where
    K: Clone + Ord,
    C: CanonicalKeyCodec<K>,
{
    let actual_bytes = usize_to_u64(bytes.len())?;
    enforce_bytes(actual_bytes, limits.max_bytes)?;
    let header = read_checkpoint_header(bytes)?;
    let expected_payload = header
        .plan_bytes
        .checked_add(header.events)
        .ok_or(InteropError::ArithmeticOverflow)?;
    let expected_total = usize_to_u64(CHECKPOINT_HEADER_BYTES)?
        .checked_add(header.payload_bytes)
        .ok_or(InteropError::ArithmeticOverflow)?;
    if expected_payload != header.payload_bytes || expected_total != actual_bytes {
        return Err(InteropError::LengthMismatch {
            declared: expected_total,
            actual: actual_bytes,
        });
    }
    enforce_count("events", header.events, limits.max_events)?;
    if header.published > header.events {
        return Err(InteropError::NonCanonicalPlan);
    }
    let plan_start = CHECKPOINT_HEADER_BYTES;
    let plan_end = plan_start
        .checked_add(u64_to_usize(header.plan_bytes)?)
        .ok_or(InteropError::ArithmeticOverflow)?;
    let plan_bytes = bytes
        .get(plan_start..plan_end)
        .ok_or(InteropError::LengthMismatch {
            declared: expected_total,
            actual: actual_bytes,
        })?;
    let plan_header = read_plan_header(plan_bytes, codec)?;
    let plan_work = validate_plan_header(plan_header, header.plan_bytes, limits)?;
    let maximum_events = u64::from(plan_header.tasks)
        .checked_add(1)
        .ok_or(InteropError::ArithmeticOverflow)?;
    if header.events > maximum_events {
        return Err(InteropError::NonCanonicalPlan);
    }
    let work = plan_work
        .checked_add(actual_bytes)
        .and_then(|value| value.checked_add(header.events))
        .ok_or(InteropError::ArithmeticOverflow)?;
    machine.admit_work(work)?;
    if let Some(expected) = expected_digest {
        let actual = digest_checkpoint_controlled(bytes, machine)?;
        if actual != expected {
            return Err(InteropError::DigestMismatch { expected, actual });
        }
    }
    let actual_plan_digest = digest_plan_controlled(plan_bytes, machine)?;
    if actual_plan_digest != header.plan_digest {
        return Err(InteropError::DigestMismatch {
            expected: header.plan_digest,
            actual: actual_plan_digest,
        });
    }
    let plan = decode_plan_core(plan_bytes, codec, limits, machine, None, false)?;
    let event_bytes = bytes.get(plan_end..).ok_or(InteropError::LengthMismatch {
        declared: expected_total,
        actual: actual_bytes,
    })?;
    if usize_to_u64(event_bytes.len())? != header.events {
        return Err(InteropError::LengthMismatch {
            declared: header.events,
            actual: usize_to_u64(event_bytes.len())?,
        });
    }
    machine.poll()?;
    let mut kinds = Vec::with_capacity(u64_to_usize(header.events)?);
    for (index, &value) in event_bytes.iter().enumerate() {
        machine.poll()?;
        kinds.push(decode_event_kind(value, usize_to_u64(index)?)?);
    }
    machine.poll()?;
    let checkpoint =
        Checkpoint::from_event_kinds(plan.value, kinds, u64_to_usize(header.published)?)
            .map_err(|_| InteropError::NonCanonicalPlan)?;
    let plan_heap = usize_to_u64(plan.metrics.peak_heap_bytes())?;
    let checkpoint_heap = header
        .events
        .checked_mul(65)
        .and_then(|value| value.checked_add(header.published.checked_mul(64)?))
        .and_then(|value| value.checked_add(plan_heap))
        .ok_or(InteropError::ArithmeticOverflow)?;
    let metrics = machine.metrics(actual_bytes, u64_to_usize(checkpoint_heap)?, 7);
    Ok(Decoded {
        value: checkpoint,
        metrics,
    })
}

fn read_checkpoint_header(bytes: &[u8]) -> Result<CheckpointHeader, InteropError> {
    require_minimum(bytes, CHECKPOINT_HEADER_BYTES)?;
    let mut reader = Reader::new(bytes);
    if reader.array::<8>()? != CHECKPOINT_MAGIC {
        return Err(InteropError::HeaderMismatch {
            field: "checkpoint magic",
        });
    }
    if reader.array::<16>()? != CHECKPOINT_SCHEMA_ID {
        return Err(InteropError::HeaderMismatch {
            field: "checkpoint schema",
        });
    }
    if reader.u16()? != CHECKPOINT_VERSION.0 || reader.u16()? != CHECKPOINT_VERSION.1 {
        return Err(InteropError::HeaderMismatch {
            field: "checkpoint version",
        });
    }
    if reader.u32()? != 0 {
        return Err(InteropError::HeaderMismatch {
            field: "checkpoint flags",
        });
    }
    let header = CheckpointHeader {
        plan_bytes: reader.u64()?,
        events: reader.u64()?,
        published: reader.u64()?,
        payload_bytes: reader.u64()?,
        plan_digest: FrameDigest::from_bytes(reader.array::<32>()?),
    };
    if reader.position() != CHECKPOINT_HEADER_BYTES {
        return Err(InteropError::ArithmeticOverflow);
    }
    Ok(header)
}

fn scan_resource_set(
    reader: &mut Reader<'_>,
    machine: &Machine<'_>,
    remaining: u64,
) -> Result<u64, InteropError> {
    let count = reader.u32()?;
    if u64::from(count) > remaining {
        return Err(InteropError::NonCanonicalPlan);
    }
    let mut previous = None;
    for _ in 0..count {
        machine.poll()?;
        let resource = reader.u64()?;
        if previous.is_some_and(|value| value >= resource) {
            return Err(InteropError::NonCanonicalPlan);
        }
        previous = Some(resource);
    }
    Ok(u64::from(count))
}

fn read_resource_set(
    reader: &mut Reader<'_>,
    machine: &Machine<'_>,
) -> Result<Vec<u64>, InteropError> {
    let count = usize::try_from(reader.u32()?).map_err(|_| InteropError::ArithmeticOverflow)?;
    let mut resources = Vec::with_capacity(count);
    for _ in 0..count {
        machine.poll()?;
        resources.push(reader.u64()?);
    }
    Ok(resources)
}

fn put_resources(
    output: &mut Vec<u8>,
    resources: &[u64],
    machine: &Machine<'_>,
) -> Result<(), InteropError> {
    for &resource in resources {
        machine.poll()?;
        put_u64(output, resource);
    }
    Ok(())
}

fn poll_slice(bytes: &[u8], machine: &Machine<'_>) -> Result<(), InteropError> {
    for _ in bytes.chunks(4_096) {
        machine.poll()?;
    }
    Ok(())
}

fn encode_event_kind(kind: CheckpointEventKind) -> u8 {
    match kind {
        CheckpointEventKind::Success => 1,
        CheckpointEventKind::Failure => 2,
        CheckpointEventKind::Incomplete => 3,
        CheckpointEventKind::Cancelled => 4,
        CheckpointEventKind::ResourceLimited => 5,
        CheckpointEventKind::Completed => 6,
    }
}

fn decode_event_kind(value: u8, index: u64) -> Result<CheckpointEventKind, InteropError> {
    match value {
        1 => Ok(CheckpointEventKind::Success),
        2 => Ok(CheckpointEventKind::Failure),
        3 => Ok(CheckpointEventKind::Incomplete),
        4 => Ok(CheckpointEventKind::Cancelled),
        5 => Ok(CheckpointEventKind::ResourceLimited),
        6 => Ok(CheckpointEventKind::Completed),
        _ => Err(InteropError::UnknownEventKind { value, index }),
    }
}

fn strictly_increasing(values: &[u64]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn enforce_bytes(actual: u64, limit: u64) -> Result<(), InteropError> {
    if actual > limit {
        Err(InteropError::ByteLimitExceeded { actual, limit })
    } else {
        Ok(())
    }
}

fn enforce_count(field: &'static str, actual: u64, limit: u64) -> Result<(), InteropError> {
    if actual > limit {
        Err(InteropError::CountLimitExceeded {
            field,
            actual,
            limit,
        })
    } else {
        Ok(())
    }
}

fn require_minimum(bytes: &[u8], minimum: usize) -> Result<(), InteropError> {
    if bytes.len() < minimum {
        Err(InteropError::LengthMismatch {
            declared: usize_to_u64(minimum)?,
            actual: usize_to_u64(bytes.len())?,
        })
    } else {
        Ok(())
    }
}

fn usize_to_u64(value: usize) -> Result<u64, InteropError> {
    u64::try_from(value).map_err(|_| InteropError::ArithmeticOverflow)
}

fn u64_to_usize(value: u64) -> Result<usize, InteropError> {
    usize::try_from(value).map_err(|_| InteropError::ArithmeticOverflow)
}

fn put_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    const fn position(&self) -> usize {
        self.position
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], InteropError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(InteropError::ArithmeticOverflow)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(InteropError::LengthMismatch {
                declared: usize_to_u64(end)?,
                actual: usize_to_u64(self.bytes.len())?,
            })?;
        self.position = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], InteropError> {
        self.take(N)?
            .try_into()
            .map_err(|_| InteropError::ArithmeticOverflow)
    }

    fn u16(&mut self) -> Result<u16, InteropError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, InteropError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, InteropError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
}
